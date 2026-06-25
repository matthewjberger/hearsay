use hearsay::{
    BridgeCreatedPayload, BrokerContract, ClientSettings, ReportBridgesPayload, Route,
    close_bridge, connect, create_client, next_message, open_bridge, publish, start_broker,
    subscribe,
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

async fn count_bridges(broker_address: &str) -> usize {
    let inspector = create_client("inspector", test_settings());
    connect(&inspector, broker_address).await.unwrap();
    subscribe(&inspector, &[&BrokerContract::report_bridges_topic()])
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    let (request_topic, request_payload) = BrokerContract::request_bridges();
    publish(&inspector, &request_topic, &request_payload, Route::Local)
        .await
        .unwrap();
    let message = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let Some(message) = next_message(&inspector).await else {
                continue;
            };
            if message.topic == BrokerContract::report_bridges_topic() {
                return message;
            }
        }
    })
    .await
    .unwrap();
    ReportBridgesPayload::from_json(message.text().unwrap())
        .unwrap()
        .bridges
        .len()
}

#[tokio::test(flavor = "multi_thread")]
async fn bridged_brokers_deliver_messages() -> hearsay::Result<()> {
    let first_broker_address = "127.0.0.1:9934";
    let second_broker_address = "127.0.0.1:9935";
    let _first_broker = start_broker(first_broker_address).await?;
    let _second_broker = start_broker(second_broker_address).await?;

    let topic = "test/bridged";
    let first_client = create_client("first", test_settings());
    connect(&first_client, first_broker_address).await?;
    subscribe(&first_client, &[topic]).await?;

    let bridge_requester = create_client("bridge_requester", test_settings());
    connect(&bridge_requester, second_broker_address).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    open_bridge(
        &bridge_requester,
        second_broker_address,
        first_broker_address,
    )
    .await?;
    tokio::time::sleep(Duration::from_secs(2)).await;

    let second_client = create_client("second", test_settings());
    connect(&second_client, second_broker_address).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let payload = TestPayload {
        name: "Matthew".to_string(),
        age: 30,
    };
    publish(&second_client, topic, &payload, Route::Global).await?;

    let message = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let Some(message) = next_message(&first_client).await else {
                continue;
            };
            if message.topic == topic {
                return message;
            }
        }
    })
    .await?;
    let received: TestPayload = serde_json::from_str(message.text().unwrap())?;
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
    let remote_subscriber = create_client("remote", test_settings());
    connect(&remote_subscriber, first_broker_address).await?;
    subscribe(&remote_subscriber, &[topic]).await?;

    let local_subscriber = create_client("local", test_settings());
    connect(&local_subscriber, second_broker_address).await?;
    subscribe(&local_subscriber, &[topic]).await?;

    let bridge_requester = create_client("bridge_requester", test_settings());
    connect(&bridge_requester, second_broker_address).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    open_bridge(
        &bridge_requester,
        second_broker_address,
        first_broker_address,
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
            let Some(message) = next_message(&local_subscriber).await else {
                continue;
            };
            if message.topic == topic {
                return message;
            }
        }
    })
    .await?;
    let received: TestPayload = serde_json::from_str(local_message.text().unwrap())?;
    assert_eq!(received, payload);

    let remote_message = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let Some(message) = next_message(&remote_subscriber).await else {
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
    let first_client = create_client("first", test_settings());
    connect(&first_client, first_broker_address).await?;
    subscribe(&first_client, &[topic]).await?;

    let bridge_requester = create_client("bridge_requester", test_settings());
    connect(&bridge_requester, second_broker_address).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    open_bridge(
        &bridge_requester,
        second_broker_address,
        first_broker_address,
    )
    .await?;
    tokio::time::sleep(Duration::from_secs(2)).await;

    let second_client = create_client("second", test_settings());
    connect(&second_client, second_broker_address).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let payload = TestPayload {
        name: "Matthew".to_string(),
        age: 30,
    };
    publish(&second_client, topic, &payload, Route::Global).await?;

    let mut deliveries = 0;
    let collect = async {
        loop {
            let Some(message) = next_message(&first_client).await else {
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
async fn closing_a_bridge_tears_down_both_directions() -> hearsay::Result<()> {
    let first_broker_address = "127.0.0.1:9957";
    let second_broker_address = "127.0.0.1:9958";
    let _first_broker = start_broker(first_broker_address).await?;
    let _second_broker = start_broker(second_broker_address).await?;

    let bridge_requester = create_client("bridge_requester", test_settings());
    connect(&bridge_requester, second_broker_address).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    open_bridge(
        &bridge_requester,
        second_broker_address,
        first_broker_address,
    )
    .await?;
    tokio::time::sleep(Duration::from_secs(2)).await;

    assert_eq!(count_bridges(first_broker_address).await, 1);
    assert_eq!(count_bridges(second_broker_address).await, 1);

    close_bridge(&bridge_requester, first_broker_address).await?;
    tokio::time::sleep(Duration::from_secs(1)).await;

    assert_eq!(count_bridges(second_broker_address).await, 0);
    assert_eq!(count_bridges(first_broker_address).await, 0);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn mesh_paths_deliver_each_message_once() -> hearsay::Result<()> {
    let a_address = "127.0.0.1:9960";
    let b_address = "127.0.0.1:9961";
    let c_address = "127.0.0.1:9962";
    let _a_broker = start_broker(a_address).await?;
    let _b_broker = start_broker(b_address).await?;
    let _c_broker = start_broker(c_address).await?;

    let topic = "test/mesh";
    let consumer = create_client("consumer", test_settings());
    connect(&consumer, c_address).await?;
    subscribe(&consumer, &[topic]).await?;

    let opener_ab = create_client("opener_ab", test_settings());
    connect(&opener_ab, a_address).await?;
    let opener_bc = create_client("opener_bc", test_settings());
    connect(&opener_bc, b_address).await?;
    let opener_ac = create_client("opener_ac", test_settings());
    connect(&opener_ac, a_address).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    open_bridge(&opener_ab, a_address, b_address).await?;
    open_bridge(&opener_bc, b_address, c_address).await?;
    open_bridge(&opener_ac, a_address, c_address).await?;
    tokio::time::sleep(Duration::from_secs(3)).await;

    let publisher = create_client("publisher", test_settings());
    connect(&publisher, a_address).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let payload = TestPayload {
        name: "Matthew".to_string(),
        age: 30,
    };
    publish(&publisher, topic, &payload, Route::Global).await?;

    let mut deliveries = 0;
    let collect = async {
        loop {
            let Some(message) = next_message(&consumer).await else {
                continue;
            };
            if message.topic == topic {
                deliveries += 1;
            }
        }
    };
    let _ = tokio::time::timeout(Duration::from_secs(3), collect).await;
    assert_eq!(
        deliveries, 1,
        "expected exactly one delivery across redundant mesh paths, saw {deliveries}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn bridge_creation_is_announced() -> hearsay::Result<()> {
    let first_broker_address = "127.0.0.1:9936";
    let second_broker_address = "127.0.0.1:9937";
    let _first_broker = start_broker(first_broker_address).await?;
    let _second_broker = start_broker(second_broker_address).await?;

    let first_client = create_client("first", test_settings());
    connect(&first_client, first_broker_address).await?;
    subscribe(&first_client, &[&BrokerContract::bridge_created_topic()]).await?;

    let bridge_requester = create_client("bridge_requester", test_settings());
    connect(&bridge_requester, second_broker_address).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    open_bridge(
        &bridge_requester,
        second_broker_address,
        first_broker_address,
    )
    .await?;

    let message = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let Some(message) = next_message(&first_client).await else {
                continue;
            };
            if message.topic == BrokerContract::bridge_created_topic() {
                return message;
            }
        }
    })
    .await?;
    assert!(BridgeCreatedPayload::from_json(message.text().unwrap()).is_ok());
    Ok(())
}
