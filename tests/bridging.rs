use hearsay::{
    BridgeCreatedPayload, BrokerContract, ClientSettings, Route, connect, create_client,
    next_message, open_bridge, publish, start_broker, subscribe,
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
async fn bridged_brokers_deliver_messages() -> hearsay::Result<()> {
    let first_broker_address = "127.0.0.1:9934";
    let second_broker_address = "127.0.0.1:9935";
    let _first_broker = start_broker(first_broker_address).await?;
    let _second_broker = start_broker(second_broker_address).await?;

    let topic = "test/bridged";
    let mut first_client = create_client("first", test_settings());
    connect(&mut first_client, first_broker_address).await?;
    subscribe(&mut first_client, &[topic]).await?;

    let mut bridge_requester = create_client("bridge_requester", test_settings());
    connect(&mut bridge_requester, second_broker_address).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    open_bridge(
        &bridge_requester,
        second_broker_address,
        first_broker_address,
        false,
    )
    .await?;
    tokio::time::sleep(Duration::from_secs(2)).await;

    let mut second_client = create_client("second", test_settings());
    connect(&mut second_client, second_broker_address).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let payload = TestPayload {
        name: "Matthew".to_string(),
        age: 30,
    };
    publish(&second_client, topic, &payload, Route::Global).await?;

    let message = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let Some(message) = next_message(&mut first_client).await else {
                continue;
            };
            if message.topic == topic {
                return message;
            }
        }
    })
    .await?;
    let received: TestPayload = serde_json::from_str(&message.payload)?;
    assert_eq!(received, payload);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn local_route_does_not_cross_bridges() -> hearsay::Result<()> {
    let first_broker_address = "127.0.0.1:9953";
    let second_broker_address = "127.0.0.1:9954";
    let _first_broker = start_broker(first_broker_address).await?;
    let _second_broker = start_broker(second_broker_address).await?;

    let topic = "test/local";
    let mut remote_subscriber = create_client("remote", test_settings());
    connect(&mut remote_subscriber, first_broker_address).await?;
    subscribe(&mut remote_subscriber, &[topic]).await?;

    let mut local_subscriber = create_client("local", test_settings());
    connect(&mut local_subscriber, second_broker_address).await?;
    subscribe(&mut local_subscriber, &[topic]).await?;

    let mut bridge_requester = create_client("bridge_requester", test_settings());
    connect(&mut bridge_requester, second_broker_address).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    open_bridge(
        &bridge_requester,
        second_broker_address,
        first_broker_address,
        false,
    )
    .await?;
    tokio::time::sleep(Duration::from_secs(2)).await;

    let payload = TestPayload {
        name: "Matthew".to_string(),
        age: 30,
    };
    publish(&bridge_requester, topic, &payload, Route::Local).await?;

    let local_message = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let Some(message) = next_message(&mut local_subscriber).await else {
                continue;
            };
            if message.topic == topic {
                return message;
            }
        }
    })
    .await?;
    let received: TestPayload = serde_json::from_str(&local_message.payload)?;
    assert_eq!(received, payload);

    let remote_message = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let Some(message) = next_message(&mut remote_subscriber).await else {
                continue;
            };
            if message.topic == topic {
                return message;
            }
        }
    })
    .await;
    assert!(remote_message.is_err());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn bidirectional_bridge_delivers_each_message_once() -> hearsay::Result<()> {
    let first_broker_address = "127.0.0.1:9955";
    let second_broker_address = "127.0.0.1:9956";
    let _first_broker = start_broker(first_broker_address).await?;
    let _second_broker = start_broker(second_broker_address).await?;

    let topic = "test/once";
    let mut first_client = create_client("first", test_settings());
    connect(&mut first_client, first_broker_address).await?;
    subscribe(&mut first_client, &[topic]).await?;

    let mut bridge_requester = create_client("bridge_requester", test_settings());
    connect(&mut bridge_requester, second_broker_address).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    open_bridge(
        &bridge_requester,
        second_broker_address,
        first_broker_address,
        false,
    )
    .await?;
    tokio::time::sleep(Duration::from_secs(2)).await;

    let mut second_client = create_client("second", test_settings());
    connect(&mut second_client, second_broker_address).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let payload = TestPayload {
        name: "Matthew".to_string(),
        age: 30,
    };
    publish(&second_client, topic, &payload, Route::Global).await?;

    let mut deliveries = 0;
    let collect = async {
        loop {
            let Some(message) = next_message(&mut first_client).await else {
                continue;
            };
            if message.topic == topic {
                deliveries += 1;
            }
        }
    };
    let _ = tokio::time::timeout(Duration::from_secs(2), collect).await;
    assert_eq!(
        deliveries, 1,
        "expected exactly one delivery, saw {deliveries}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn bridge_creation_is_announced() -> hearsay::Result<()> {
    let first_broker_address = "127.0.0.1:9936";
    let second_broker_address = "127.0.0.1:9937";
    let _first_broker = start_broker(first_broker_address).await?;
    let _second_broker = start_broker(second_broker_address).await?;

    let mut first_client = create_client("first", test_settings());
    connect(&mut first_client, first_broker_address).await?;
    subscribe(
        &mut first_client,
        &[&BrokerContract::bridge_created_topic()],
    )
    .await?;

    let mut bridge_requester = create_client("bridge_requester", test_settings());
    connect(&mut bridge_requester, second_broker_address).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    open_bridge(
        &bridge_requester,
        second_broker_address,
        first_broker_address,
        false,
    )
    .await?;

    let message = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let Some(message) = next_message(&mut first_client).await else {
                continue;
            };
            if message.topic == BrokerContract::bridge_created_topic() {
                return message;
            }
        }
    })
    .await?;
    assert!(BridgeCreatedPayload::from_json(&message.payload).is_ok());
    Ok(())
}
