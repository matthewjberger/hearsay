# Client

The client connects to a broker, publishes, subscribes, and receives messages. Source lives in `src/client.rs`. The public API is free functions over the `Client` struct; the struct itself only carries state.

## 1. State

```rust
pub struct Client {
    state: Arc<RwLock<ClientState>>,
    settings: ClientSettings,
}
```

`ClientState` holds the peer id, the TCP write half, the receiver slot, the subscription cache, pending subscriptions, the configured read timeout, the broker address, and the reconnection-task flag. The receiver slot is `Arc<Mutex<Option<UnboundedReceiver<Message>>>>`, kept separate from the rest of the state so that waiting for a message never blocks the state lock (see [ARCHITECTURE.md](ARCHITECTURE.md) section 5).

`create_client(name, settings)` assigns the peer id as `{name}_{uuid_v4}`, so multiple instances of the same program get distinct identities on the broker. `assign_client_id` overrides the id (the broker uses this for bridge cycle prevention).

`ClientSettings`:

- `max_connection_attempts: Option<u16>`. `Some(n)` retries up to n times with a 1-second pause between attempts; `None` means exactly one attempt (used by bridges).
- `autoreconnect: bool`. Spawns the background reconnection task on first connect.
- `timeout_per_attempt: Duration`. Per-attempt TCP connect timeout.
- `read_timeout: Option<Duration>`. If set, the read task treats a silent connection as dead after this long without a frame.

## 2. Connecting and reconnecting

`connect(&mut client, address)` records the address in state, runs `establish_connection`, and (if `autoreconnect` is set and no task exists yet) spawns the reconnection task. Exactly one reconnection task ever exists per client, and it holds only a `Weak` reference to the state, so it exits when the `Client` is dropped.

`establish_connection`:

1. `connect_with_retries` dials the address using standard host resolution, attempting IPv4 addresses first to match the broker's bind preference (so `localhost` reaches a broker that bound `127.0.0.1` even when the resolver lists `::1` first, or when a firewall black-holes one family). TCP keepalive (1s probe, 1s interval) is configured on the stream.
2. The stream is split. Under the state lock: the write half is installed, the read task is spawned, `PeerEvent::Hello { id }` is sent, and `resubscribe` replays the union of cached and pending subscriptions (a pending topic is only removed from the pending set after its subscribe frame is written successfully, so partial failures cannot lose topics).
3. After the state lock is released, the new receiver is installed into the receiver slot. Messages the read task produced in the meantime are buffered in the channel, so nothing is lost to the ordering.

The reconnection task ticks every 2 seconds; when the client is disconnected and an address is known, it re-runs `establish_connection`. Reconnecting to a different address is just calling `connect` again: the recorded address changes and the existing task follows it.

Disconnection is detected in two places: `notify_broker` clears the writer when a TCP write fails (so publish-only clients and bridges notice promptly), and `next_message`/`try_next_message` clear the writer and receiver when the receive channel closes (the read task exited because the socket died).

## 3. Publishing

- `publish(client, topic, payload, route)` serializes any `impl Serialize` to JSON and delegates to `publish_json`.
- `publish_json(client, topic, payload, route)` sends `PeerEvent::PublishText` with the JSON string.
- `publish_bytes(client, topic, payload, route)` sends `PeerEvent::PublishBinary` with raw bytes; subscribers receive it in `Message.bytes`.

The `Route` controls the scope of the publish:

- `Route::Global` (default): delivered to local subscribers and forwarded across bridges.
- `Route::Local`: delivered to local subscribers only; sets `local_only` on the wire so the broker skips bridges.

All three functions take `&Client`; mutation happens behind the state lock. A write failure clears the writer and surfaces as an error.

## 4. Subscriptions

`subscribe(client, topics)` sends `PeerEvent::Subscribe` per topic and caches it. While disconnected, topics go to the pending set instead and are applied on the next successful connect. If a subscribe write fails mid-call, the topic is moved to pending and the error is returned, so the subscription still applies after reconnect.

`unsubscribe(client, topics)` removes the topic from both the pending set and the cache and notifies the broker when connected, so an offline unsubscribe does not resurrect on reconnect.

`subscriptions(client)` returns the current cache.

## 5. Receiving

- `next_message(&mut client)` awaits the receive channel. `None` means the connection is gone (and the writer/receiver have been cleared so reconnection can proceed). The await holds only the receiver-slot mutex and is cancel-safe, so wrapping it in `tokio::time::timeout` (as `run_lifecycle` does) cannot lose messages or the receiver.
- `try_next_message(&mut client)` is the non-blocking variant using `try_recv`.

There is no message filtering in the client: every message for every subscribed topic comes through the same stream, and receivers match on `message.topic` (see [CONTRACTS.md](CONTRACTS.md) for the idiomatic pattern).

## 6. Bridge requests

`open_bridge(client, source_address, target_address, ack)` and `close_bridge(client, target_address, ack)` send the corresponding `PeerEvent`s. Applications use the `ack: false` forms; the `ack: true` forms are the broker-to-broker acknowledgement halves of the handshake described in [BROKER.md](BROKER.md) section 6.
