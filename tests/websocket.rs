#![cfg(feature = "websockets")]

use futures_util::{SinkExt, StreamExt};
use hearsay::{
    ClientSettings, PeerEvent, Route, connect, create_client, next_message, publish, start_broker,
    start_websocket_listener, subscribe,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct TestPayload {
    name: String,
    age: u8,
}

type WebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn send_event(websocket: &mut WebSocket, event: &PeerEvent) -> hearsay::Result<()> {
    let bytes = postcard::to_allocvec(event)?;
    websocket
        .send(tungstenite::Message::Binary(bytes.into()))
        .await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn websocket_and_tcp_peers_interoperate() -> hearsay::Result<()> {
    let broker_address = "127.0.0.1:9939";
    let websocket_address = "127.0.0.1:9940";
    let broker = start_broker(broker_address).await?;
    start_websocket_listener(&broker, websocket_address).await?;

    let mut tcp_client = create_client(
        "tcp_peer",
        ClientSettings {
            autoreconnect: false,
            ..Default::default()
        },
    );
    connect(&mut tcp_client, broker_address).await?;
    subscribe(&mut tcp_client, &["from/websocket"]).await?;

    let (mut websocket, _response) =
        tokio_tungstenite::connect_async(format!("ws://{websocket_address}")).await?;
    let websocket_id = "websocket_peer".to_string();
    send_event(
        &mut websocket,
        &PeerEvent::Hello {
            id: websocket_id.clone(),
        },
    )
    .await?;
    send_event(
        &mut websocket,
        &PeerEvent::Subscribe {
            id: websocket_id.clone(),
            topic: "from/tcp".to_string(),
        },
    )
    .await?;
    tokio::time::sleep(Duration::from_millis(250)).await;

    let payload = TestPayload {
        name: "Matthew".to_string(),
        age: 30,
    };
    send_event(
        &mut websocket,
        &PeerEvent::PublishText {
            id: websocket_id.clone(),
            topic: "from/websocket".to_string(),
            payload: serde_json::to_string(&payload)?,
            local_only: false,
        },
    )
    .await?;

    let message = tokio::time::timeout(Duration::from_secs(5), next_message(&mut tcp_client))
        .await?
        .expect("expected a message from the websocket peer");
    assert_eq!(message.topic, "from/websocket");
    let received: TestPayload = serde_json::from_str(&message.payload)?;
    assert_eq!(received, payload);

    publish(&tcp_client, "from/tcp", &payload, Route::Global).await?;

    let message = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let frame = websocket
                .next()
                .await
                .expect("websocket closed unexpectedly")
                .expect("websocket read failed");
            if let tungstenite::Message::Binary(data) = frame {
                return postcard::from_bytes::<hearsay::Message>(&data)
                    .expect("failed to decode broker message");
            }
        }
    })
    .await?;
    assert_eq!(message.topic, "from/tcp");
    let received: TestPayload = serde_json::from_str(&message.payload)?;
    assert_eq!(received, payload);
    Ok(())
}
