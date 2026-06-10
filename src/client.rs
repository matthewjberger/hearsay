use crate::{
    Message, Result, Route,
    contract::PeerEvent,
    wire::{frame_payload, read_frame, serialize_payload},
};
use serde::Serialize;
use socket2::TcpKeepalive;
use std::{
    collections::HashSet,
    net::SocketAddr,
    sync::{Arc, Weak},
    time::Duration,
};
use tokio::{
    io::AsyncWriteExt,
    net::{
        TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
    sync::{
        Mutex, RwLock,
        mpsc::{self, UnboundedReceiver, UnboundedSender, error::TryRecvError},
    },
};

const RECONNECTION_INTERVAL: Duration = Duration::from_secs(2);

type ReceiverSlot = Arc<Mutex<Option<UnboundedReceiver<Message>>>>;

#[derive(Debug, Clone)]
pub struct ClientSettings {
    pub max_connection_attempts: Option<u16>,
    pub autoreconnect: bool,
    pub timeout_per_attempt: Duration,
    pub read_timeout: Option<Duration>,
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            max_connection_attempts: Some(100),
            autoreconnect: true,
            timeout_per_attempt: Duration::from_secs(2),
            read_timeout: None,
        }
    }
}

pub struct Client {
    state: Arc<RwLock<ClientState>>,
    settings: ClientSettings,
}

struct ClientState {
    id: String,
    writer: Option<OwnedWriteHalf>,
    receiver: ReceiverSlot,
    subscriptions: HashSet<String>,
    pending_subscriptions: HashSet<String>,
    read_timeout: Option<Duration>,
    broker_address: Option<String>,
    reconnection_task_spawned: bool,
}

pub fn create_client(name: &str, settings: ClientSettings) -> Client {
    let id = format!("{name}_{}", uuid::Uuid::new_v4());
    let read_timeout = settings.read_timeout;
    Client {
        state: Arc::new(RwLock::new(ClientState {
            id,
            writer: None,
            receiver: Arc::new(Mutex::new(None)),
            subscriptions: HashSet::new(),
            pending_subscriptions: HashSet::new(),
            read_timeout,
            broker_address: None,
            reconnection_task_spawned: false,
        })),
        settings,
    }
}

pub async fn client_id(client: &Client) -> String {
    client.state.read().await.id.clone()
}

pub async fn assign_client_id(client: &mut Client, id: &str) {
    client.state.write().await.id = id.to_string();
}

pub async fn is_connected(client: &Client) -> bool {
    let receiver_slot = {
        let state = client.state.read().await;
        if state.writer.is_none() {
            return false;
        }
        state.receiver.clone()
    };
    let receiver_guard = receiver_slot.lock().await;
    receiver_guard
        .as_ref()
        .is_some_and(|receiver| !receiver.is_closed())
}

pub async fn subscriptions(client: &Client) -> HashSet<String> {
    client.state.read().await.subscriptions.clone()
}

pub async fn connect(client: &mut Client, address: &str) -> Result<()> {
    client.state.write().await.broker_address = Some(address.to_string());
    establish_connection(
        &client.state,
        address,
        client.settings.max_connection_attempts,
        client.settings.timeout_per_attempt,
    )
    .await?;
    if client.settings.autoreconnect {
        let mut state = client.state.write().await;
        if !state.reconnection_task_spawned {
            state.reconnection_task_spawned = true;
            tokio::spawn(reconnection_task(
                Arc::downgrade(&client.state),
                client.settings.max_connection_attempts,
                client.settings.timeout_per_attempt,
            ));
        }
    }
    Ok(())
}

pub async fn publish(
    client: &Client,
    topic: &str,
    payload: &impl Serialize,
    route: Route,
) -> Result<()> {
    let payload_json = serde_json::to_string(payload)?;
    publish_json(client, topic, &payload_json, route).await
}

pub async fn publish_json(client: &Client, topic: &str, payload: &str, route: Route) -> Result<()> {
    let mut state = client.state.write().await;
    let publish_event = PeerEvent::PublishText {
        id: state.id.clone(),
        topic: topic.to_string(),
        payload: payload.to_string(),
        local_only: matches!(route, Route::Local),
    };
    notify_broker(&mut state, &publish_event).await
}

pub async fn publish_bytes(
    client: &Client,
    topic: &str,
    payload: &[u8],
    route: Route,
) -> Result<()> {
    let mut state = client.state.write().await;
    let publish_event = PeerEvent::PublishBinary {
        id: state.id.clone(),
        topic: topic.to_string(),
        payload: payload.to_vec(),
        local_only: matches!(route, Route::Local),
    };
    notify_broker(&mut state, &publish_event).await
}

pub async fn subscribe(client: &mut Client, topics: &[&str]) -> Result<()> {
    let mut state = client.state.write().await;
    for topic in topics {
        if state.writer.is_none() {
            state.pending_subscriptions.insert((*topic).to_string());
            continue;
        }
        if let Err(error) = create_subscription(&mut state, topic).await {
            state.pending_subscriptions.insert((*topic).to_string());
            return Err(error);
        }
    }
    Ok(())
}

pub async fn unsubscribe(client: &mut Client, topics: &[&str]) -> Result<()> {
    let mut state = client.state.write().await;
    for topic in topics {
        state.pending_subscriptions.remove(*topic);
        state.subscriptions.remove(*topic);
        if state.writer.is_none() {
            continue;
        }
        let unsubscribe_event = PeerEvent::Unsubscribe {
            id: state.id.clone(),
            topic: (*topic).to_string(),
        };
        notify_broker(&mut state, &unsubscribe_event).await?;
    }
    Ok(())
}

pub async fn try_next_message(client: &mut Client) -> Option<Message> {
    let receiver_slot = client.state.read().await.receiver.clone();
    let mut receiver_guard = receiver_slot.lock().await;
    let receiver = receiver_guard.as_mut()?;
    match receiver.try_recv() {
        Ok(message) => Some(message),
        Err(TryRecvError::Disconnected) => {
            *receiver_guard = None;
            drop(receiver_guard);
            client.state.write().await.writer = None;
            None
        }
        Err(TryRecvError::Empty) => None,
    }
}

pub async fn next_message(client: &mut Client) -> Option<Message> {
    let receiver_slot = client.state.read().await.receiver.clone();
    let mut receiver_guard = receiver_slot.lock().await;
    let receiver = receiver_guard.as_mut()?;
    let received = receiver.recv().await;
    match received {
        Some(message) => Some(message),
        None => {
            *receiver_guard = None;
            drop(receiver_guard);
            client.state.write().await.writer = None;
            None
        }
    }
}

pub async fn open_bridge(
    client: &Client,
    source_address: &str,
    target_address: &str,
    ack: bool,
) -> Result<()> {
    let mut state = client.state.write().await;
    let bridge_event = PeerEvent::OpenBridge {
        id: state.id.clone(),
        source_address: source_address.to_string(),
        target_address: target_address.to_string(),
        ack,
    };
    notify_broker(&mut state, &bridge_event).await
}

pub async fn close_bridge(client: &Client, target_address: &str, ack: bool) -> Result<()> {
    let mut state = client.state.write().await;
    let close_event = PeerEvent::CloseBridge {
        target_address: target_address.to_string(),
        ack,
    };
    notify_broker(&mut state, &close_event).await
}

async fn establish_connection(
    state: &Arc<RwLock<ClientState>>,
    address: &str,
    max_connection_attempts: Option<u16>,
    timeout_per_attempt: Duration,
) -> Result<()> {
    let stream =
        connect_with_retries(address, max_connection_attempts, timeout_per_attempt).await?;
    let (read_half, write_half) = stream.into_split();
    let (message_sender, message_receiver) = mpsc::unbounded_channel();
    let receiver_slot = {
        let mut guard = state.write().await;
        guard.writer = Some(write_half);
        tokio::spawn(read_task(read_half, message_sender, guard.read_timeout));
        let hello_event = PeerEvent::Hello {
            id: guard.id.clone(),
        };
        notify_broker(&mut guard, &hello_event).await?;
        resubscribe(&mut guard).await?;
        guard.receiver.clone()
    };
    *receiver_slot.lock().await = Some(message_receiver);
    Ok(())
}

async fn reconnection_task(
    state: Weak<RwLock<ClientState>>,
    max_connection_attempts: Option<u16>,
    timeout_per_attempt: Duration,
) {
    loop {
        tokio::time::sleep(RECONNECTION_INTERVAL).await;
        let Some(state_handle) = state.upgrade() else {
            break;
        };
        let (connected, address) = {
            let guard = state_handle.read().await;
            (guard.writer.is_some(), guard.broker_address.clone())
        };
        if connected {
            continue;
        }
        let Some(address) = address else {
            continue;
        };
        let _ = establish_connection(
            &state_handle,
            &address,
            max_connection_attempts,
            timeout_per_attempt,
        )
        .await;
    }
}

async fn connect_with_retries(
    address: &str,
    max_connection_attempts: Option<u16>,
    timeout_per_attempt: Duration,
) -> Result<TcpStream> {
    let mut attempt: u16 = 0;
    loop {
        if let Ok(Ok(stream)) =
            tokio::time::timeout(timeout_per_attempt, create_tcp_connection(address)).await
        {
            return Ok(stream);
        }
        match max_connection_attempts {
            Some(max_attempts) if attempt >= max_attempts => {
                return Err("maximum connection attempts reached".into());
            }
            Some(_) => {}
            None => return Err("connection attempt failed".into()),
        }
        attempt += 1;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

pub(crate) async fn resolve_addresses(address: &str) -> Result<Vec<SocketAddr>> {
    let mut addresses: Vec<SocketAddr> = tokio::net::lookup_host(address).await?.collect();
    addresses.sort_by_key(|resolved| !resolved.is_ipv4());
    if addresses.is_empty() {
        return Err("could not resolve address".into());
    }
    Ok(addresses)
}

async fn create_tcp_connection(address: &str) -> Result<TcpStream> {
    let addresses = resolve_addresses(address).await?;
    let mut last_error: crate::Error = "could not resolve address".into();
    for socket_address in addresses {
        match TcpStream::connect(socket_address).await {
            Ok(stream) => {
                configure_keepalive(&stream)?;
                return Ok(stream);
            }
            Err(error) => last_error = error.into(),
        }
    }
    Err(last_error)
}

fn configure_keepalive(stream: &TcpStream) -> Result<()> {
    let socket_reference = socket2::SockRef::from(stream);
    let keepalive = TcpKeepalive::new()
        .with_time(Duration::from_secs(1))
        .with_interval(Duration::from_secs(1));
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let keepalive = keepalive.with_retries(3);
    socket_reference.set_tcp_keepalive(&keepalive)?;
    Ok(())
}

async fn read_task(
    mut read_half: OwnedReadHalf,
    message_sender: UnboundedSender<Message>,
    read_timeout: Option<Duration>,
) {
    loop {
        let message = match read_timeout {
            Some(timeout_duration) => {
                match tokio::time::timeout(timeout_duration, read_frame::<Message>(&mut read_half))
                    .await
                {
                    Ok(Ok(message)) => message,
                    _ => break,
                }
            }
            None => match read_frame::<Message>(&mut read_half).await {
                Ok(message) => message,
                Err(_) => break,
            },
        };
        if message_sender.send(message).is_err() {
            break;
        }
    }
}

async fn notify_broker(state: &mut ClientState, event: &PeerEvent) -> Result<()> {
    let frame = frame_payload(&serialize_payload(event)?);
    let Some(writer) = state.writer.as_mut() else {
        return Ok(());
    };
    if let Err(error) = writer.write_all(&frame).await {
        state.writer = None;
        return Err(error.into());
    }
    Ok(())
}

async fn resubscribe(state: &mut ClientState) -> Result<()> {
    let mut topics: HashSet<String> = state.subscriptions.iter().cloned().collect();
    topics.extend(state.pending_subscriptions.iter().cloned());
    for topic in topics {
        create_subscription(state, &topic).await?;
        state.pending_subscriptions.remove(&topic);
    }
    Ok(())
}

async fn create_subscription(state: &mut ClientState, topic: &str) -> Result<()> {
    let subscribe_event = PeerEvent::Subscribe {
        id: state.id.clone(),
        topic: topic.to_string(),
    };
    notify_broker(state, &subscribe_event).await?;
    state.subscriptions.insert(topic.to_string());
    Ok(())
}
