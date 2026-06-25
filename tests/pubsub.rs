use hearsay::{
    ClientSettings, Route, broker_is_running, connect, create_client, is_connected, next_message,
    publish, publish_bytes, start_broker, stop_broker, subscribe, unsubscribe,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct TestPayload {
    name: String,
    age: u8,
}

fn test_settings() -> ClientSettings {
    ClientSettings {
        autoreconnect: false,
        ..Default::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn round_trip() -> hearsay::Result<()> {
    let broker_address = "127.0.0.1:9931";
    let _broker = start_broker(broker_address).await?;

    let subscriber = create_client("subscriber", test_settings());
    connect(&subscriber, broker_address).await?;

    let publisher = create_client("publisher", test_settings());
    connect(&publisher, broker_address).await?;

    let topic = "test/data";
    subscribe(&subscriber, &[topic]).await?;
    tokio::time::sleep(Duration::from_millis(250)).await;

    let payload = TestPayload {
        name: "Matthew".to_string(),
        age: 30,
    };
    publish(&publisher, topic, &payload, Route::Global).await?;

    let message = tokio::time::timeout(Duration::from_secs(5), next_message(&subscriber))
        .await?
        .expect("expected a message");
    assert_eq!(message.topic, topic);
    let received: TestPayload = serde_json::from_str(message.text().unwrap())?;
    assert_eq!(received, payload);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn binary_round_trip() -> hearsay::Result<()> {
    let broker_address = "127.0.0.1:9932";
    let _broker = start_broker(broker_address).await?;

    let subscriber = create_client("subscriber", test_settings());
    connect(&subscriber, broker_address).await?;

    let publisher = create_client("publisher", test_settings());
    connect(&publisher, broker_address).await?;

    let topic = "test/binary";
    subscribe(&subscriber, &[topic]).await?;
    tokio::time::sleep(Duration::from_millis(250)).await;

    let payload = vec![1_u8, 2, 3, 4, 5];
    publish_bytes(&publisher, topic, &payload, Route::Global).await?;

    let message = tokio::time::timeout(Duration::from_secs(5), next_message(&subscriber))
        .await?
        .expect("expected a message");
    assert_eq!(message.topic, topic);
    assert_eq!(message.bytes(), Some(payload.as_slice()));
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn stopped_broker_shuts_down_and_disconnects_peers() -> hearsay::Result<()> {
    let broker_address = "127.0.0.1:9948";
    let broker = start_broker(broker_address).await?;

    let subscriber = create_client("subscriber", test_settings());
    connect(&subscriber, broker_address).await?;
    subscribe(&subscriber, &["test/shutdown"]).await?;
    tokio::time::sleep(Duration::from_millis(250)).await;

    assert!(broker_is_running(&broker));
    stop_broker(&broker);
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!broker_is_running(&broker));

    let message = tokio::time::timeout(Duration::from_secs(5), next_message(&subscriber)).await?;
    assert!(message.is_none());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn localhost_binds_loopback() -> hearsay::Result<()> {
    let _broker = start_broker("localhost:9949").await?;

    let subscriber = create_client("subscriber", test_settings());
    connect(&subscriber, "localhost:9949").await?;
    subscribe(&subscriber, &["test/localhost"]).await?;

    let publisher = create_client("publisher", test_settings());
    connect(&publisher, "127.0.0.1:9949").await?;
    tokio::time::sleep(Duration::from_millis(250)).await;

    let payload = TestPayload {
        name: "Matthew".to_string(),
        age: 30,
    };
    publish(&publisher, "test/localhost", &payload, Route::Global).await?;

    let message = tokio::time::timeout(Duration::from_secs(5), next_message(&subscriber))
        .await?
        .expect("expected a message");
    assert_eq!(message.topic, "test/localhost");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn offline_unsubscribe_does_not_resurrect_on_reconnect() -> hearsay::Result<()> {
    let broker_address = "127.0.0.1:9952";
    let topic = "test/resurrect";
    let broker = start_broker(broker_address).await?;

    let subscriber = create_client("subscriber", ClientSettings::default());
    connect(&subscriber, broker_address).await?;
    subscribe(&subscriber, &[topic]).await?;
    tokio::time::sleep(Duration::from_millis(250)).await;

    stop_broker(&broker);
    drop(broker);
    while next_message(&subscriber).await.is_some() {}
    assert!(!is_connected(&subscriber).await);

    unsubscribe(&subscriber, &[topic]).await?;

    let _broker = start_broker(broker_address).await?;
    while !is_connected(&subscriber).await {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    tokio::time::sleep(Duration::from_millis(250)).await;

    let publisher = create_client("publisher", test_settings());
    connect(&publisher, broker_address).await?;
    tokio::time::sleep(Duration::from_millis(250)).await;

    let payload = TestPayload {
        name: "Matthew".to_string(),
        age: 30,
    };
    publish(&publisher, topic, &payload, Route::Global).await?;

    let message = tokio::time::timeout(Duration::from_secs(2), next_message(&subscriber)).await;
    assert!(message.is_err());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn pending_subscriptions_apply_on_connect() -> hearsay::Result<()> {
    let broker_address = "127.0.0.1:9933";
    let _broker = start_broker(broker_address).await?;

    let topic = "test/pending";
    let subscriber = create_client("subscriber", test_settings());
    subscribe(&subscriber, &[topic]).await?;
    connect(&subscriber, broker_address).await?;

    let publisher = create_client("publisher", test_settings());
    connect(&publisher, broker_address).await?;
    tokio::time::sleep(Duration::from_millis(250)).await;

    let payload = TestPayload {
        name: "Matthew".to_string(),
        age: 30,
    };
    publish(&publisher, topic, &payload, Route::Global).await?;

    let message = tokio::time::timeout(Duration::from_secs(5), next_message(&subscriber))
        .await?
        .expect("expected a message");
    assert_eq!(message.topic, topic);
    Ok(())
}
