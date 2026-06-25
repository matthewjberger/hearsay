use crate::{
    BrokerContract, Message, Result,
    bridge::{Bridge, BridgeCommand, ForwardPayload, connect_bridge, spawn_bridge},
    contract::PeerEvent,
    open_bridge,
    wire::{frame_payload, read_frame, serialize_payload},
};
#[cfg(feature = "websockets")]
use futures_util::SinkExt;
use socket2::{Domain, Socket, TcpKeepalive, Type};
use std::{collections::HashMap, net::SocketAddr, time::Duration};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream, tcp::OwnedWriteHalf},
    sync::{
        mpsc::{self, Receiver, Sender, UnboundedSender},
        oneshot, watch,
    },
};

const BROKER_ID: &str = "broker";
const BROKER_EVENT_CAPACITY: usize = 1024;
const PEER_QUEUE_CAPACITY: usize = 1024;

pub struct Broker {
    pub(crate) sender: Sender<BrokerEvent>,
    pub(crate) shutdown_sender: watch::Sender<bool>,
    #[cfg(feature = "spawn")]
    pub(crate) spawner: crate::spawn::Spawner,
}

pub(crate) enum PeerWriter {
    Tcp(OwnedWriteHalf),
    #[cfg(feature = "websockets")]
    WebSocket(
        futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<TcpStream>,
            tokio_tungstenite::tungstenite::Message,
        >,
    ),
}

async fn write_to_peer(writer: &mut PeerWriter, payload: Vec<u8>) -> Result<()> {
    match writer {
        PeerWriter::Tcp(write_half) => Ok(write_half.write_all(&frame_payload(&payload)).await?),
        #[cfg(feature = "websockets")]
        PeerWriter::WebSocket(sink) => Ok(sink
            .send(tokio_tungstenite::tungstenite::Message::Binary(
                payload.into(),
            ))
            .await?),
    }
}

pub async fn start_broker(address: &str) -> Result<Broker> {
    let listener = create_listener(resolve_address(address).await?)?;
    let (event_sender, event_receiver) = mpsc::channel(BROKER_EVENT_CAPACITY);
    let (shutdown_sender, shutdown_receiver) = watch::channel(false);
    let instance_id = uuid::Uuid::new_v4().to_string();
    tokio::spawn(broker_loop(
        event_receiver,
        shutdown_receiver.clone(),
        instance_id,
    ));
    tokio::spawn(accept_loop(
        listener,
        event_sender.clone(),
        shutdown_receiver,
    ));
    Ok(Broker {
        sender: event_sender,
        shutdown_sender,
        #[cfg(feature = "spawn")]
        spawner: crate::spawn::create_spawner(address),
    })
}

pub fn broker_is_running(broker: &Broker) -> bool {
    !broker.sender.is_closed()
}

pub fn stop_broker(broker: &Broker) {
    let _ = broker.shutdown_sender.send(true);
}

pub(crate) enum BrokerEvent {
    Hello {
        id: String,
        writer: PeerWriter,
        shutdown: oneshot::Receiver<()>,
    },
    Peer(PeerEvent),
}

struct BrokerState {
    instance_id: String,
    peers: HashMap<String, Sender<Vec<u8>>>,
    peer_generations: HashMap<String, u64>,
    generation_counter: u64,
    subscriptions: HashMap<String, Vec<String>>,
    bridges: Vec<Bridge>,
    disconnect_sender: UnboundedSender<(String, u64)>,
}

pub(crate) async fn resolve_address(address: &str) -> Result<SocketAddr> {
    Ok(crate::client::resolve_addresses(address).await?[0])
}

fn create_listener(socket_address: SocketAddr) -> Result<TcpListener> {
    let socket = Socket::new(Domain::for_address(socket_address), Type::STREAM, None)?;
    socket.set_reuse_address(true)?;
    socket.bind(&socket_address.into())?;
    socket.set_keepalive(true)?;
    let keepalive = TcpKeepalive::new()
        .with_time(Duration::from_secs(30))
        .with_interval(Duration::from_secs(1));
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let keepalive = keepalive.with_retries(3);
    socket.set_tcp_keepalive(&keepalive)?;
    socket.listen(1024)?;
    let std_listener: std::net::TcpListener = socket.into();
    std_listener.set_nonblocking(true)?;
    Ok(TcpListener::from_std(std_listener)?)
}

async fn accept_loop(
    listener: TcpListener,
    event_sender: Sender<BrokerEvent>,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, _address)) => {
                    tokio::spawn(connection_task(event_sender.clone(), stream));
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            },
            _ = shutdown.changed() => break,
        }
    }
}

async fn connection_task(event_sender: Sender<BrokerEvent>, stream: TcpStream) {
    let (mut read_half, write_half) = stream.into_split();
    let mut writer = Some(PeerWriter::Tcp(write_half));
    let mut shutdown_signal = None;
    loop {
        let Ok(event) = read_frame::<PeerEvent>(&mut read_half).await else {
            break;
        };
        if !forward_peer_event(event, &event_sender, &mut writer, &mut shutdown_signal).await {
            break;
        }
    }
    drop(shutdown_signal);
}

pub(crate) async fn forward_peer_event(
    event: PeerEvent,
    event_sender: &Sender<BrokerEvent>,
    writer: &mut Option<PeerWriter>,
    shutdown_signal: &mut Option<oneshot::Sender<()>>,
) -> bool {
    let forwarded = match event {
        PeerEvent::Hello { id } => match writer.take() {
            Some(peer_writer) => {
                let (shutdown_sender, shutdown_receiver) = oneshot::channel();
                *shutdown_signal = Some(shutdown_sender);
                event_sender
                    .send(BrokerEvent::Hello {
                        id,
                        writer: peer_writer,
                        shutdown: shutdown_receiver,
                    })
                    .await
            }
            None => Ok(()),
        },
        event => event_sender.send(BrokerEvent::Peer(event)).await,
    };
    forwarded.is_ok()
}

async fn connection_writer_task(
    mut messages: Receiver<Vec<u8>>,
    mut writer: PeerWriter,
    mut shutdown: oneshot::Receiver<()>,
    disconnect_sender: UnboundedSender<(String, u64)>,
    name: String,
    generation: u64,
) {
    loop {
        tokio::select! {
            message = messages.recv() => match message {
                Some(payload) => {
                    if write_to_peer(&mut writer, payload).await.is_err() {
                        break;
                    }
                }
                None => break,
            },
            _ = &mut shutdown => break,
        }
    }
    let _ = disconnect_sender.send((name, generation));
}

async fn broker_loop(
    mut events: Receiver<BrokerEvent>,
    mut shutdown: watch::Receiver<bool>,
    instance_id: String,
) {
    let (disconnect_sender, mut disconnect_receiver) = mpsc::unbounded_channel();
    let mut state = BrokerState {
        instance_id,
        peers: HashMap::new(),
        peer_generations: HashMap::new(),
        generation_counter: 0,
        subscriptions: HashMap::new(),
        bridges: Vec::new(),
        disconnect_sender,
    };
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Some(event) => {
                    let _ = handle_broker_event(&mut state, event).await;
                }
                None => break,
            },
            disconnect = disconnect_receiver.recv() => {
                if let Some((name, generation)) = disconnect {
                    remove_disconnected_peer(&mut state, &name, generation);
                }
            },
            _ = shutdown.changed() => break,
        }
    }
}

async fn handle_broker_event(state: &mut BrokerState, event: BrokerEvent) -> Result<()> {
    match event {
        BrokerEvent::Hello {
            id,
            writer,
            shutdown,
        } => {
            establish_peer(state, &id, writer, shutdown);
            let (topic, mut payload) = BrokerContract::peer_connected();
            payload.id.clone_from(&id);
            let visited = origin_visited(state);
            publish_everywhere(state, &id, &topic, payload.to_json()?, false, &visited)?;
        }
        BrokerEvent::Peer(PeerEvent::Hello { .. }) => {}
        BrokerEvent::Peer(PeerEvent::Subscribe { id, topic }) => {
            subscribe_to_topic(&mut state.subscriptions, id, topic);
        }
        BrokerEvent::Peer(PeerEvent::Unsubscribe { id, topic }) => {
            unsubscribe_from_topic(&mut state.subscriptions, &id, &topic);
        }
        BrokerEvent::Peer(PeerEvent::PublishText {
            id,
            topic,
            payload,
            local_only,
        }) => {
            answer_introspection_requests(state, &topic)?;
            let visited = origin_visited(state);
            publish_everywhere(state, &id, &topic, payload, local_only, &visited)?;
        }
        BrokerEvent::Peer(PeerEvent::PublishBinary {
            id,
            topic,
            payload,
            local_only,
        }) => {
            let visited = origin_visited(state);
            publish_bytes_everywhere(state, &id, &topic, payload, local_only, &visited)?;
        }
        BrokerEvent::Peer(PeerEvent::ForwardText {
            id,
            topic,
            payload,
            local_only,
            visited,
        }) => {
            if visited.iter().any(|seen| seen == &state.instance_id) {
                return Ok(());
            }
            answer_introspection_requests(state, &topic)?;
            let visited = extend_visited(visited, &state.instance_id);
            publish_everywhere(state, &id, &topic, payload, local_only, &visited)?;
        }
        BrokerEvent::Peer(PeerEvent::ForwardBinary {
            id,
            topic,
            payload,
            local_only,
            visited,
        }) => {
            if visited.iter().any(|seen| seen == &state.instance_id) {
                return Ok(());
            }
            let visited = extend_visited(visited, &state.instance_id);
            publish_bytes_everywhere(state, &id, &topic, payload, local_only, &visited)?;
        }
        BrokerEvent::Peer(PeerEvent::OpenBridge {
            id,
            source_address,
            target_address,
            ack,
        }) => {
            open_broker_bridge(state, id, target_address, source_address, ack).await?;
        }
        BrokerEvent::Peer(PeerEvent::CloseBridge {
            target_address,
            ack,
        }) => {
            drop_bridge(state, &target_address, !ack);
        }
    }
    Ok(())
}

fn origin_visited(state: &BrokerState) -> Vec<String> {
    vec![state.instance_id.clone()]
}

fn extend_visited(mut visited: Vec<String>, instance_id: &str) -> Vec<String> {
    if !visited.iter().any(|entry| entry == instance_id) {
        visited.push(instance_id.to_string());
    }
    visited
}

fn establish_peer(
    state: &mut BrokerState,
    id: &str,
    writer: PeerWriter,
    shutdown: oneshot::Receiver<()>,
) {
    state.peers.remove(id);
    state.generation_counter += 1;
    let generation = state.generation_counter;
    state.peer_generations.insert(id.to_string(), generation);
    let (message_sender, message_receiver) = mpsc::channel(PEER_QUEUE_CAPACITY);
    state.peers.insert(id.to_string(), message_sender);
    tokio::spawn(connection_writer_task(
        message_receiver,
        writer,
        shutdown,
        state.disconnect_sender.clone(),
        id.to_string(),
        generation,
    ));
}

fn remove_disconnected_peer(state: &mut BrokerState, name: &str, generation: u64) {
    if state.peer_generations.get(name).copied() != Some(generation) {
        return;
    }
    state.peers.remove(name);
    state.peer_generations.remove(name);
    for subscribers in state.subscriptions.values_mut() {
        subscribers.retain(|subscriber| subscriber != name);
    }
}

fn subscribe_to_topic(subscriptions: &mut HashMap<String, Vec<String>>, id: String, topic: String) {
    let subscribers = subscriptions.entry(topic).or_default();
    if !subscribers.contains(&id) {
        subscribers.push(id);
    }
}

fn unsubscribe_from_topic(subscriptions: &mut HashMap<String, Vec<String>>, id: &str, topic: &str) {
    if let Some(subscribers) = subscriptions.get_mut(topic) {
        subscribers.retain(|subscriber| subscriber != id);
    }
}

fn answer_introspection_requests(state: &BrokerState, topic: &str) -> Result<()> {
    let visited = origin_visited(state);
    if topic == BrokerContract::request_subscriptions_topic() {
        let (report_topic, mut payload) = BrokerContract::report_subscriptions();
        payload.subscriptions = state
            .subscriptions
            .iter()
            .map(|(subscription_topic, subscribers)| {
                (subscription_topic.clone(), subscribers.clone())
            })
            .collect();
        publish_everywhere(
            state,
            BROKER_ID,
            &report_topic,
            payload.to_json()?,
            false,
            &visited,
        )?;
    } else if topic == BrokerContract::request_peers_topic() {
        let (report_topic, mut payload) = BrokerContract::report_peers();
        payload.peers = state.peers.keys().cloned().collect();
        publish_everywhere(
            state,
            BROKER_ID,
            &report_topic,
            payload.to_json()?,
            false,
            &visited,
        )?;
    } else if topic == BrokerContract::request_bridges_topic() {
        let (report_topic, mut payload) = BrokerContract::report_bridges();
        payload.bridges = state
            .bridges
            .iter()
            .map(|bridge| bridge.id.clone())
            .collect();
        publish_everywhere(
            state,
            BROKER_ID,
            &report_topic,
            payload.to_json()?,
            false,
            &visited,
        )?;
    }
    Ok(())
}

fn publish_everywhere(
    state: &BrokerState,
    publisher_id: &str,
    topic: &str,
    payload: String,
    local_only: bool,
    visited: &[String],
) -> Result<()> {
    if !local_only {
        for bridge in &state.bridges {
            if bridge.id != publisher_id {
                let _ = bridge.commands.try_send(BridgeCommand::Forward {
                    topic: topic.to_string(),
                    payload: ForwardPayload::Text(payload.clone()),
                    visited: visited.to_vec(),
                });
            }
        }
    }
    let message = Message {
        topic: topic.to_string(),
        payload,
        bytes: None,
    };
    deliver_to_subscribers(state, topic, &message)
}

fn publish_bytes_everywhere(
    state: &BrokerState,
    publisher_id: &str,
    topic: &str,
    payload: Vec<u8>,
    local_only: bool,
    visited: &[String],
) -> Result<()> {
    if !local_only {
        for bridge in &state.bridges {
            if bridge.id != publisher_id {
                let _ = bridge.commands.try_send(BridgeCommand::Forward {
                    topic: topic.to_string(),
                    payload: ForwardPayload::Binary(payload.clone()),
                    visited: visited.to_vec(),
                });
            }
        }
    }
    let message = Message {
        topic: topic.to_string(),
        payload: String::new(),
        bytes: Some(payload),
    };
    deliver_to_subscribers(state, topic, &message)
}

fn deliver_to_subscribers(state: &BrokerState, topic: &str, message: &Message) -> Result<()> {
    let Some(subscribers) = state.subscriptions.get(topic) else {
        return Ok(());
    };
    let payload_bytes = serialize_payload(message)?;
    for subscriber in subscribers {
        if let Some(peer) = state.peers.get(subscriber) {
            let _ = peer.try_send(payload_bytes.clone());
        }
    }
    Ok(())
}

async fn open_broker_bridge(
    state: &mut BrokerState,
    id: String,
    target_address: String,
    source_address: String,
    ack: bool,
) -> Result<()> {
    drop_bridge(state, &target_address, !ack);
    let override_id = if ack { Some(id.clone()) } else { None };
    let Some((client, bridge_id)) = connect_bridge(override_id, &target_address).await else {
        return Ok(());
    };
    if ack {
        let (topic, mut payload) = BrokerContract::bridge_created();
        payload.id.clone_from(&bridge_id);
        payload.source_address = source_address;
        payload.target_address = target_address.clone();
        let visited = origin_visited(state);
        publish_everywhere(state, &id, &topic, payload.to_json()?, false, &visited)?;
    } else {
        open_bridge(&client, &target_address, &source_address, true).await?;
    }
    let commands = spawn_bridge(client, target_address.clone());
    state.bridges.push(Bridge {
        id: bridge_id,
        target_address,
        commands,
    });
    Ok(())
}

fn drop_bridge(state: &mut BrokerState, target_address: &str, notify_remote: bool) {
    state.bridges.retain(|bridge| {
        if bridge.target_address == target_address {
            let command = if notify_remote {
                BridgeCommand::CloseAndNotify
            } else {
                BridgeCommand::CloseLocal
            };
            let _ = bridge.commands.try_send(command);
            false
        } else {
            true
        }
    });
}
