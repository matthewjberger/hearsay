#[cfg(not(target_arch = "wasm32"))]
use crate::{Client, Route, publish_bytes, wire::serialize_payload};
use crate::{Message, Result};
#[cfg(not(target_arch = "wasm32"))]
use serde::Serialize;
use serde::de::DeserializeOwned;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

#[cfg(not(target_arch = "wasm32"))]
pub struct Batch<T> {
    pub topic: String,
    pub route: Route,
    pub items: Vec<T>,
    pub max_items: usize,
    pub flush_interval: Duration,
    pub last_flush: Instant,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn create_batch<T>(
    topic: &str,
    route: Route,
    max_items: usize,
    flush_interval: Duration,
) -> Batch<T> {
    Batch {
        topic: topic.to_string(),
        route,
        items: Vec::new(),
        max_items,
        flush_interval,
        last_flush: Instant::now(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn push_to_batch<T: Serialize>(
    client: &Client,
    batch: &mut Batch<T>,
    item: T,
) -> Result<()> {
    batch.items.push(item);
    if batch.items.len() >= batch.max_items || batch.last_flush.elapsed() >= batch.flush_interval {
        flush_batch(client, batch).await?;
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn flush_batch<T: Serialize>(client: &Client, batch: &mut Batch<T>) -> Result<()> {
    if batch.items.is_empty() {
        return Ok(());
    }
    let payload = serialize_payload(&batch.items)?;
    publish_bytes(client, &batch.topic, &payload, batch.route).await?;
    batch.items.clear();
    batch.last_flush = Instant::now();
    Ok(())
}

pub fn read_batch<T: DeserializeOwned>(message: &Message) -> Result<Vec<T>> {
    let Some(bytes) = message.bytes.as_ref() else {
        return Err("message has no binary payload".into());
    };
    Ok(postcard::from_bytes(bytes)?)
}
