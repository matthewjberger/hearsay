# Batching

Batching is a core primitive (no feature gate) for high-frequency traffic: it coalesces many small items into one binary message so that the per-message fixed costs are paid once per flush instead of once per item. Source lives in `src/batch.rs`.

## 1. Why it exists

The cost of a message is dominated by fixed overhead, not payload size: framing, the syscall, the broker's event-loop dispatch, per-subscriber fan-out, and channel wakeups. The benchmarks put a full publish-to-receive round trip at roughly 24µs and the publish path alone at roughly 4µs, nearly independent of payload size. Sending a thousand per-frame updates individually pays that fixed cost a thousand times, and each subscriber receives a thousand deliveries. One batch pays it once, and each subscriber receives one delivery.

The practical pattern is two traffic classes: interactive events (small, latency-sensitive) go immediately as ordinary contract messages; bulk state (entity transforms, telemetry, output streams) goes through a `Batch`.

## 2. Data and functions

```rust
pub struct Batch<T> {
    pub topic: String,
    pub route: Route,
    pub items: Vec<T>,
    pub max_items: usize,
    pub flush_interval: Duration,
    pub last_flush: Instant,
}
```

All fields are public; the struct is plain data owned by the caller.

- `create_batch(topic, route, max_items, flush_interval) -> Batch<T>`
- `push_to_batch(client, &mut batch, item)` appends, then flushes if `items.len() >= max_items` or `flush_interval` has elapsed since the last flush.
- `flush_batch(client, &mut batch)` publishes unconditionally (a no-op when empty).
- `read_batch::<T>(&message) -> Result<Vec<T>>` decodes a received batch on the subscriber side.

The interval check happens on push, so a batch with no traffic holds its items until the next push or a manual flush. Callers with a periodic loop (the `Lifecycle` `update` hook is the natural place) call `flush_batch` once per tick to bound worst-case latency.

A useful property of the elapsed-time trigger: the first item pushed after an idle period flushes immediately, because the interval has already elapsed. Bursts therefore start with low latency and then coalesce.

## 3. Wire format

A flush serializes `Vec<T>` with postcard and publishes it via `publish_bytes`, so subscribers receive a `Message` with `bytes` populated and `payload` empty. `read_batch` is the inverse. The 64 MiB frame cap bounds the worst-case batch; a batch that would exceed it fails to serialize and is kept (see below) rather than sent.

## 4. Failure semantics

`flush_batch` only clears `items` after the publish succeeds. If the connection is down, the error propagates, the items stay in the batch, and the next push or flush retries, so a reconnect does not silently drop a batch. The trade-off is that items accumulate while disconnected; callers that prefer dropping stale state over replaying it can `items.clear()` themselves, since the field is public.

## 5. Coalescing

Because the buffer is just `Vec<T>` on a public field, senders can rewrite it before flushing, deduplicating to last-writer-wins per key, dropping superseded updates, or sorting for delta encoding. The library deliberately does not impose a coalescing policy; it owns the flush triggers and the wire format, nothing else.
