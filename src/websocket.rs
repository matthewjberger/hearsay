use crate::{
    Result,
    broker::{Broker, BrokerEvent, PeerWriter, WebSocketSink, forward_peer_event, resolve_address},
    contract::PeerEvent,
};
use futures_util::StreamExt;
use std::time::Duration;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc::Sender, watch},
};
use tokio_tungstenite::tungstenite;

pub async fn start_websocket_listener(broker: &Broker, address: &str) -> Result<()> {
    let listener = TcpListener::bind(resolve_address(address).await?).await?;
    tokio::spawn(websocket_accept_loop(
        listener,
        broker.sender.clone(),
        broker.shutdown_sender.subscribe(),
    ));
    Ok(())
}

async fn websocket_accept_loop(
    listener: TcpListener,
    event_sender: Sender<BrokerEvent>,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, _address)) => {
                    tokio::spawn(websocket_connection_task(event_sender.clone(), stream));
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            },
            _ = shutdown.changed() => break,
        }
    }
}

async fn websocket_connection_task(event_sender: Sender<BrokerEvent>, stream: TcpStream) {
    let Ok(websocket) = tokio_tungstenite::accept_async(stream).await else {
        return;
    };
    let (sink, mut messages) = websocket.split();
    let mut writer: Option<PeerWriter> = Some(Box::new(WebSocketSink(sink)));
    let mut shutdown_signal = None;
    while let Some(Ok(message)) = messages.next().await {
        let event = match message {
            tungstenite::Message::Binary(data) => match postcard::from_bytes::<PeerEvent>(&data) {
                Ok(event) => event,
                Err(_) => continue,
            },
            tungstenite::Message::Close(_) => break,
            _ => continue,
        };
        if !forward_peer_event(event, &event_sender, &mut writer, &mut shutdown_signal).await {
            break;
        }
    }
    drop(shutdown_signal);
}
