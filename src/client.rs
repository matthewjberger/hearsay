use crate::{
    Error, Message, Result, Route,
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
        mpsc::{self, Receiver, Sender, error::TryRecvError},
    },
};

const RECONNECTION_INTERVAL: Duration = Duration::from_secs(2);
const MAX_RECONNECTION_BACKOFF: Duration = Duration::from_secs(30);
const INBOUND_QUEUE_CAPACITY: usize = 1024;
const OUTBOUND_QUEUE_CAPACITY: usize = 1024;

type ReceiverSlot = Arc<Mutex<Option<Receiver<Message>>>>;

#[derive(Debug, Clone)]
pub struct ClientSettings {
    /// Number of additional connection attempts after the first failure before
    /// [`connect`] gives up. `Some(0)` makes a single attempt; `Some(n)` retries
    /// up to `n` more times; `None` retries forever, so `connect` will not return
    /// until a broker is reachable.
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

#[derive(Clone)]
pub struct Client {
    state: Arc<RwLock<ClientState>>,
    settings: ClientSettings,
}

struct ClientState {
    id: String,
    outbound: Option<Sender<Vec<u8>>>,
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
            outbound: None,
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

pub(crate) async fn assign_client_id(client: &Client, id: &str) {
    client.state.write().await.id = id.to_string();
}

pub async fn is_connected(client: &Client) -> bool {
    let receiver_slot = {
        let state = client.state.read().await;
        if state.outbound.is_none() {
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

/// Connects to a broker. With `autoreconnect` enabled the client also
/// re-establishes the connection if it later drops and re-sends its
/// subscriptions, but it does not replay messages published to those topics
/// while it was disconnected; any such messages are lost.
pub async fn connect(client: &Client, address: &str) -> Result<()> {
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
    topic: impl AsRef<str>,
    payload: &impl Serialize,
    route: Route,
) -> Result<()> {
    let payload_json = serde_json::to_string(payload)?;
    publish_json(client, topic, &payload_json, route).await
}

pub async fn publish_json(
    client: &Client,
    topic: impl AsRef<str>,
    payload: &str,
    route: Route,
) -> Result<()> {
    let publish_event = PeerEvent::PublishText {
        id: client.state.read().await.id.clone(),
        topic: topic.as_ref().to_string(),
        payload: payload.to_string(),
        local_only: matches!(route, Route::Local),
    };
    send_event(&client.state, &publish_event).await
}

pub async fn publish_bytes(
    client: &Client,
    topic: impl AsRef<str>,
    payload: &[u8],
    route: Route,
) -> Result<()> {
    let publish_event = PeerEvent::PublishBinary {
        id: client.state.read().await.id.clone(),
        topic: topic.as_ref().to_string(),
        payload: payload.to_vec(),
        local_only: matches!(route, Route::Local),
    };
    send_event(&client.state, &publish_event).await
}

pub(crate) async fn forward_text(
    client: &Client,
    topic: &str,
    payload: &str,
    visited: Vec<String>,
    sequence: u64,
) -> Result<()> {
    let forward_event = PeerEvent::ForwardText {
        id: client.state.read().await.id.clone(),
        topic: topic.to_string(),
        payload: payload.to_string(),
        local_only: false,
        visited,
        sequence,
    };
    send_event(&client.state, &forward_event).await
}

pub(crate) async fn forward_bytes(
    client: &Client,
    topic: &str,
    payload: &[u8],
    visited: Vec<String>,
    sequence: u64,
) -> Result<()> {
    let forward_event = PeerEvent::ForwardBinary {
        id: client.state.read().await.id.clone(),
        topic: topic.to_string(),
        payload: payload.to_vec(),
        local_only: false,
        visited,
        sequence,
    };
    send_event(&client.state, &forward_event).await
}

pub async fn subscribe(client: &Client, topics: &[impl AsRef<str>]) -> Result<()> {
    for topic in topics {
        let topic = topic.as_ref();
        let event = {
            let mut state = client.state.write().await;
            if state.outbound.is_none() {
                state.pending_subscriptions.insert(topic.to_string());
                None
            } else {
                Some(PeerEvent::Subscribe {
                    id: state.id.clone(),
                    topic: topic.to_string(),
                })
            }
        };
        let Some(event) = event else {
            continue;
        };
        if let Err(error) = send_event(&client.state, &event).await {
            client
                .state
                .write()
                .await
                .pending_subscriptions
                .insert(topic.to_string());
            return Err(error);
        }
        client
            .state
            .write()
            .await
            .subscriptions
            .insert(topic.to_string());
    }
    Ok(())
}

pub async fn unsubscribe(client: &Client, topics: &[impl AsRef<str>]) -> Result<()> {
    for topic in topics {
        let topic = topic.as_ref();
        let event = {
            let mut state = client.state.write().await;
            state.pending_subscriptions.remove(topic);
            state.subscriptions.remove(topic);
            if state.outbound.is_none() {
                None
            } else {
                Some(PeerEvent::Unsubscribe {
                    id: state.id.clone(),
                    topic: topic.to_string(),
                })
            }
        };
        if let Some(event) = event {
            send_event(&client.state, &event).await?;
        }
    }
    Ok(())
}

/// Result of a non-blocking [`try_next_message`]: a message is available, the
/// queue is currently empty but the client is still connected, or the client
/// has disconnected and no further messages will arrive until reconnection.
#[derive(Debug)]
pub enum Reception {
    Message(Message),
    Empty,
    Disconnected,
}

pub async fn try_next_message(client: &Client) -> Reception {
    let receiver_slot = client.state.read().await.receiver.clone();
    let mut receiver_guard = receiver_slot.lock().await;
    let Some(receiver) = receiver_guard.as_mut() else {
        return Reception::Disconnected;
    };
    match receiver.try_recv() {
        Ok(message) => Reception::Message(message),
        Err(TryRecvError::Disconnected) => {
            *receiver_guard = None;
            drop(receiver_guard);
            client.state.write().await.outbound = None;
            Reception::Disconnected
        }
        Err(TryRecvError::Empty) => Reception::Empty,
    }
}

pub async fn next_message(client: &Client) -> Option<Message> {
    let receiver_slot = client.state.read().await.receiver.clone();
    let mut receiver_guard = receiver_slot.lock().await;
    let receiver = receiver_guard.as_mut()?;
    let received = receiver.recv().await;
    match received {
        Some(message) => Some(message),
        None => {
            *receiver_guard = None;
            drop(receiver_guard);
            client.state.write().await.outbound = None;
            None
        }
    }
}

pub async fn open_bridge(
    client: &Client,
    source_address: &str,
    target_address: &str,
) -> Result<()> {
    open_bridge_acked(client, source_address, target_address, false).await
}

pub(crate) async fn open_bridge_acked(
    client: &Client,
    source_address: &str,
    target_address: &str,
    ack: bool,
) -> Result<()> {
    let bridge_event = PeerEvent::OpenBridge {
        id: client.state.read().await.id.clone(),
        source_address: source_address.to_string(),
        target_address: target_address.to_string(),
        ack,
    };
    send_event(&client.state, &bridge_event).await
}

pub async fn close_bridge(client: &Client, target_address: &str) -> Result<()> {
    let close_event = PeerEvent::CloseBridge {
        id: String::new(),
        target_address: target_address.to_string(),
        ack: false,
    };
    send_event(&client.state, &close_event).await
}

pub(crate) async fn notify_close_bridge(client: &Client, id: &str) -> Result<()> {
    let close_event = PeerEvent::CloseBridge {
        id: id.to_string(),
        target_address: String::new(),
        ack: true,
    };
    send_event(&client.state, &close_event).await
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
    let (message_sender, message_receiver) = mpsc::channel(INBOUND_QUEUE_CAPACITY);
    let (outbound_sender, outbound_receiver) = mpsc::channel(OUTBOUND_QUEUE_CAPACITY);
    let (id, read_timeout, receiver_slot) = {
        let mut guard = state.write().await;
        guard.outbound = Some(outbound_sender);
        (guard.id.clone(), guard.read_timeout, guard.receiver.clone())
    };
    tokio::spawn(writer_task(write_half, outbound_receiver));
    tokio::spawn(read_task(read_half, message_sender, read_timeout));
    *receiver_slot.lock().await = Some(message_receiver);
    let hello_event = PeerEvent::Hello { id };
    send_event(state, &hello_event).await?;
    resubscribe(state).await?;
    Ok(())
}

async fn reconnection_task(
    state: Weak<RwLock<ClientState>>,
    max_connection_attempts: Option<u16>,
    timeout_per_attempt: Duration,
) {
    let mut backoff = RECONNECTION_INTERVAL;
    loop {
        tokio::time::sleep(backoff).await;
        let Some(state_handle) = state.upgrade() else {
            break;
        };
        let (connected, address) = {
            let guard = state_handle.read().await;
            (guard.outbound.is_some(), guard.broker_address.clone())
        };
        if connected {
            backoff = RECONNECTION_INTERVAL;
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
        backoff = (backoff * 2).min(MAX_RECONNECTION_BACKOFF);
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
        if let Some(max_attempts) = max_connection_attempts
            && attempt >= max_attempts
        {
            return Err(Error::MaxConnectionAttempts);
        }
        attempt += 1;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

pub(crate) async fn resolve_addresses(address: &str) -> Result<Vec<SocketAddr>> {
    let mut addresses: Vec<SocketAddr> = tokio::net::lookup_host(address).await?.collect();
    addresses.sort_by_key(|resolved| !resolved.is_ipv4());
    if addresses.is_empty() {
        return Err(Error::AddressResolution);
    }
    Ok(addresses)
}

async fn create_tcp_connection(address: &str) -> Result<TcpStream> {
    let addresses = resolve_addresses(address).await?;
    let mut last_error: crate::Error = Error::AddressResolution;
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
    message_sender: Sender<Message>,
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
        if message_sender.send(message).await.is_err() {
            break;
        }
    }
}

async fn writer_task(mut write_half: OwnedWriteHalf, mut outbound: Receiver<Vec<u8>>) {
    while let Some(frame) = outbound.recv().await {
        if write_half.write_all(&frame).await.is_err() {
            break;
        }
    }
}

async fn send_event(state: &Arc<RwLock<ClientState>>, event: &PeerEvent) -> Result<()> {
    let frame = frame_payload(&serialize_payload(event)?);
    let outbound = state.read().await.outbound.clone();
    let Some(outbound) = outbound else {
        return Err(Error::NotConnected);
    };
    if outbound.send(frame).await.is_err() {
        state.write().await.outbound = None;
        return Err(Error::NotConnected);
    }
    Ok(())
}

async fn resubscribe(state: &Arc<RwLock<ClientState>>) -> Result<()> {
    let topics: HashSet<String> = {
        let guard = state.read().await;
        guard
            .subscriptions
            .iter()
            .cloned()
            .chain(guard.pending_subscriptions.iter().cloned())
            .collect()
    };
    for topic in topics {
        let subscribe_event = PeerEvent::Subscribe {
            id: state.read().await.id.clone(),
            topic: topic.clone(),
        };
        send_event(state, &subscribe_event).await?;
        let mut guard = state.write().await;
        guard.subscriptions.insert(topic.clone());
        guard.pending_subscriptions.remove(&topic);
    }
    Ok(())
}
