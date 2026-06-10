# WebSockets

With the `websockets` feature (`websockets = ["dep:tokio-tungstenite", "dep:futures-util"]`), the broker accepts WebSocket peers alongside TCP peers. This is the path for browser and WASM apps, which cannot open raw TCP sockets. Source lives in `src/websocket.rs`, with the transport split in `src/broker.rs` (`PeerWriter`).

WebSocket support is integrated into the broker rather than provided as a separate gateway process: a WebSocket peer is a first-class peer in the same subscription table as TCP peers, with no translation protocol and no polling relay in between.

## 1. Starting the listener

```rust
let broker = hearsay::start_broker("127.0.0.1:9612").await?;
hearsay::start_websocket_listener(&broker, "127.0.0.1:9613").await?;
```

`start_websocket_listener` binds its own address, resolved the same way as the TCP listener (standard host resolution, IPv4 preferred, so `localhost` binds loopback and `0.0.0.0` is the explicit all-interfaces opt-in), and spawns an accept loop that feeds the same broker event channel as the TCP listener. Each accepted connection goes through the tungstenite handshake and then `websocket_connection_task`, which mirrors the TCP connection task: it decodes incoming events, forwards them via the shared `forward_peer_event`, and parks the sink half as the peer's writer (`PeerWriter::WebSocket`).

## 2. Wire format

The protocol is identical to TCP except for framing (see [CONTRACTS.md](CONTRACTS.md)):

- Client to broker: each **binary** WebSocket frame is one postcard-encoded `PeerEvent`. No length prefix; WebSocket frames are self-delimiting.
- Broker to client: each binary frame is one postcard-encoded `Message`.
- Text frames and pings are ignored; a close frame ends the session. Undecodable binary frames are skipped rather than killing the connection.
- tungstenite's default 64 MiB message cap matches the library's own frame cap.

A session looks like:

1. Send `PeerEvent::Hello { id }` (binary, postcard).
2. Send `PeerEvent::Subscribe { id, topic }` per topic.
3. Send `PeerEvent::PublishText { .. }` / `PeerEvent::PublishBinary { .. }` to publish.
4. Read binary frames and decode each as `Message`.

Events are encoded with `postcard::to_allocvec(&event)` and incoming frames decoded with `postcard::from_bytes::<Message>(&bytes)`.

`tests/websocket.rs` is the reference implementation of a WebSocket peer (using tokio-tungstenite as the client) and verifies both directions interoperate with a TCP peer through one broker.

## 3. Disconnects and lifetimes

A WebSocket peer gets the same writer task, shutdown oneshot, and generation-checked disconnect handling as a TCP peer ([BROKER.md](BROKER.md) section 4). When the socket dies, the peer is removed from the subscription table; reconnecting with the same id replaces the old session safely.

## 4. WASM clients

A browser client implements the four-step session above over the platform WebSocket API, and it depends on hearsay itself for the wire format: on `wasm32` targets the crate compiles down to the protocol surface (`PeerEvent`, `Message`, `BrokerContract` and its payloads, and `read_batch`), with all the tokio-backed machinery and its dependencies compiled out. There is no type mirroring and no gateway process: the WASM client serializes `PeerEvent`s with postcard, sends them as binary frames, and deserializes incoming frames as `Message`. CI checks the wasm32 build on every push.
