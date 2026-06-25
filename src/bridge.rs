use crate::{
    Client, ClientSettings, assign_client_id, client_id, connect, create_client, forward_bytes,
    forward_text, is_connected, notify_close_bridge,
};
use std::time::Duration;
use tokio::sync::mpsc::{Receiver, Sender, channel};

const BRIDGE_QUEUE_CAPACITY: usize = 1024;
const BRIDGE_RECONNECT_INTERVAL: Duration = Duration::from_secs(2);

pub(crate) enum ForwardPayload {
    Text(String),
    Binary(Vec<u8>),
}

pub(crate) enum BridgeCommand {
    Forward {
        topic: String,
        payload: ForwardPayload,
        visited: Vec<String>,
        sequence: u64,
    },
    CloseAndNotify,
    CloseLocal,
}

pub(crate) struct Bridge {
    pub(crate) id: String,
    pub(crate) target_address: String,
    pub(crate) commands: Sender<BridgeCommand>,
}

pub(crate) async fn connect_bridge(
    override_id: Option<String>,
    target_address: &str,
) -> Option<(Client, String)> {
    let settings = ClientSettings {
        autoreconnect: false,
        max_connection_attempts: Some(0),
        ..Default::default()
    };
    let mut client = create_client("bridge", settings);
    if let Some(id) = override_id {
        assign_client_id(&mut client, &id).await;
    }
    let id = client_id(&client).await;
    if connect(&mut client, target_address).await.is_err() {
        return None;
    }
    Some((client, id))
}

pub(crate) fn spawn_bridge(
    client: Client,
    id: String,
    target_address: String,
) -> Sender<BridgeCommand> {
    let (sender, receiver) = channel(BRIDGE_QUEUE_CAPACITY);
    tokio::spawn(bridge_task(client, receiver, id, target_address));
    sender
}

async fn bridge_task(
    mut client: Client,
    mut commands: Receiver<BridgeCommand>,
    id: String,
    target_address: String,
) {
    let mut reconnect = tokio::time::interval(BRIDGE_RECONNECT_INTERVAL);
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(BridgeCommand::Forward { topic, payload, visited, sequence }) => {
                    forward(&client, &topic, payload, visited, sequence).await;
                }
                Some(BridgeCommand::CloseAndNotify) => {
                    let _ = notify_close_bridge(&client, &id).await;
                    break;
                }
                Some(BridgeCommand::CloseLocal) | None => break,
            },
            _ = reconnect.tick() => {
                if !is_connected(&client).await {
                    let _ = connect(&mut client, &target_address).await;
                }
            }
        }
    }
}

async fn forward(
    client: &Client,
    topic: &str,
    payload: ForwardPayload,
    visited: Vec<String>,
    sequence: u64,
) {
    match payload {
        ForwardPayload::Text(text) => {
            let _ = forward_text(client, topic, &text, visited, sequence).await;
        }
        ForwardPayload::Binary(bytes) => {
            let _ = forward_bytes(client, topic, &bytes, visited, sequence).await;
        }
    }
}
