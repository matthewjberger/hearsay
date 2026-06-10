use criterion::{Criterion, criterion_group, criterion_main};
use hearsay::{
    Broker, Client, ClientSettings, Route, connect, create_client, next_message, publish,
    start_broker, subscribe,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::runtime::Runtime;

#[derive(Debug, Serialize, Deserialize)]
struct BenchPayload {
    sequence: u64,
    body: String,
}

fn bench_settings() -> ClientSettings {
    ClientSettings {
        autoreconnect: false,
        ..Default::default()
    }
}

async fn connected_pair(broker_address: &str, topic: &str) -> (Broker, Client, Client) {
    let broker = start_broker(broker_address).await.unwrap();
    let mut publisher = create_client("bench_publisher", bench_settings());
    connect(&mut publisher, broker_address).await.unwrap();
    let mut subscriber = create_client("bench_subscriber", bench_settings());
    connect(&mut subscriber, broker_address).await.unwrap();
    subscribe(&mut subscriber, &[topic]).await.unwrap();
    tokio::time::sleep(Duration::from_millis(250)).await;
    (broker, publisher, subscriber)
}

fn round_trip(criterion: &mut Criterion) {
    let runtime = Runtime::new().unwrap();
    let topic = "bench/round-trip";
    let (_broker, publisher, subscriber) =
        runtime.block_on(connected_pair("127.0.0.1:9950", topic));
    let subscriber = tokio::sync::Mutex::new(subscriber);
    let payload = BenchPayload {
        sequence: 1,
        body: "0123456789".repeat(10),
    };

    criterion.bench_function("round_trip", |bencher| {
        bencher.to_async(&runtime).iter(|| async {
            publish(&publisher, topic, &payload, Route::Global)
                .await
                .unwrap();
            next_message(&mut *subscriber.lock().await).await.unwrap()
        });
    });
}

fn publish_only(criterion: &mut Criterion) {
    let runtime = Runtime::new().unwrap();
    let broker_address = "127.0.0.1:9951";
    let (_broker, publisher) = runtime.block_on(async {
        let broker = start_broker(broker_address).await.unwrap();
        let mut publisher = create_client("bench_publisher", bench_settings());
        connect(&mut publisher, broker_address).await.unwrap();
        (broker, publisher)
    });
    let payload = BenchPayload {
        sequence: 1,
        body: "0123456789".repeat(10),
    };

    criterion.bench_function("publish_only", |bencher| {
        bencher.to_async(&runtime).iter(|| async {
            publish(&publisher, "bench/publish-only", &payload, Route::Global)
                .await
                .unwrap()
        });
    });
}

criterion_group!(benches, round_trip, publish_only);
criterion_main!(benches);
