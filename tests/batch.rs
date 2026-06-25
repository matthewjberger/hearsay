use hearsay::{
    ClientSettings, Route, connect, create_batch, create_client, flush_batch, next_message,
    push_to_batch, read_batch, start_broker, subscribe,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
struct EntityUpdate {
    entity: u64,
    position: (f32, f32, f32),
}

fn test_settings() -> ClientSettings {
    ClientSettings {
        autoreconnect: false,
        ..Default::default()
    }
}

fn sample_updates(count: u64) -> Vec<EntityUpdate> {
    (0..count)
        .map(|index| EntityUpdate {
            entity: index,
            position: (index as f32, 0.0, 0.0),
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn batch_flushes_at_max_items() -> hearsay::Result<()> {
    let broker_address = "127.0.0.1:9945";
    let _broker = start_broker(broker_address).await?;
    let subscriber = create_client("subscriber", test_settings());
    connect(&subscriber, broker_address).await?;
    subscribe(&subscriber, &["updates/size"]).await?;
    let publisher = create_client("publisher", test_settings());
    connect(&publisher, broker_address).await?;
    tokio::time::sleep(Duration::from_millis(250)).await;

    let mut batch = create_batch("updates/size", Route::Global, 3, Duration::from_secs(60));
    for update in sample_updates(3) {
        push_to_batch(&publisher, &mut batch, update).await?;
    }
    assert!(batch.items.is_empty());

    let message = tokio::time::timeout(Duration::from_secs(5), next_message(&subscriber))
        .await?
        .expect("expected a batch message");
    assert_eq!(message.topic, "updates/size");
    let updates: Vec<EntityUpdate> = read_batch(&message)?;
    assert_eq!(updates, sample_updates(3));
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn batch_flushes_after_interval() -> hearsay::Result<()> {
    let broker_address = "127.0.0.1:9946";
    let _broker = start_broker(broker_address).await?;
    let subscriber = create_client("subscriber", test_settings());
    connect(&subscriber, broker_address).await?;
    subscribe(&subscriber, &["updates/interval"]).await?;
    let publisher = create_client("publisher", test_settings());
    connect(&publisher, broker_address).await?;
    tokio::time::sleep(Duration::from_millis(250)).await;

    let mut batch = create_batch(
        "updates/interval",
        Route::Global,
        100,
        Duration::from_millis(100),
    );
    push_to_batch(&publisher, &mut batch, sample_updates(1)[0].clone()).await?;
    assert_eq!(batch.items.len(), 1);

    tokio::time::sleep(Duration::from_millis(150)).await;
    push_to_batch(&publisher, &mut batch, sample_updates(2)[1].clone()).await?;
    assert!(batch.items.is_empty());

    let message = tokio::time::timeout(Duration::from_secs(5), next_message(&subscriber))
        .await?
        .expect("expected a batch message");
    let updates: Vec<EntityUpdate> = read_batch(&message)?;
    assert_eq!(updates.len(), 2);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn manual_flush_sends_pending_items() -> hearsay::Result<()> {
    let broker_address = "127.0.0.1:9947";
    let _broker = start_broker(broker_address).await?;
    let subscriber = create_client("subscriber", test_settings());
    connect(&subscriber, broker_address).await?;
    subscribe(&subscriber, &["updates/manual"]).await?;
    let publisher = create_client("publisher", test_settings());
    connect(&publisher, broker_address).await?;
    tokio::time::sleep(Duration::from_millis(250)).await;

    let mut batch = create_batch(
        "updates/manual",
        Route::Global,
        100,
        Duration::from_secs(60),
    );
    for update in sample_updates(2) {
        push_to_batch(&publisher, &mut batch, update).await?;
    }
    assert_eq!(batch.items.len(), 2);
    flush_batch(&publisher, &mut batch).await?;
    assert!(batch.items.is_empty());

    let message = tokio::time::timeout(Duration::from_secs(5), next_message(&subscriber))
        .await?
        .expect("expected a batch message");
    let updates: Vec<EntityUpdate> = read_batch(&message)?;
    assert_eq!(updates, sample_updates(2));
    Ok(())
}
