# Broker

The broker is the message router: it accepts peer connections, maintains the subscription table, delivers publishes to subscribers, answers introspection requests, and manages bridges to other brokers. Source lives in `src/broker.rs` with the bridge wrapper in `src/bridge.rs`.

## 1. Startup

`start_broker(address)` resolves the address with standard host resolution (`resolve_address`, preferring an IPv4 result), so `localhost` binds the loopback interface and hostnames work. Listening on all interfaces requires the explicit `0.0.0.0:port`. The protocol carries no authentication, so the bind address is the security boundary: local tools should bind loopback, and `0.0.0.0` belongs on trusted networks only (any peer that can reach the port can subscribe to every topic, publish, and request bridges).

The listener itself is built with socket2 (`create_listener`): `SO_REUSEADDR`, keepalive (30s initial probe, 1s interval, 3 retries where the platform supports it), and a backlog of 1024. The configured socket is converted to a nonblocking std listener and then into a tokio `TcpListener`.

Two tasks are spawned: `broker_loop` (the event loop) and `accept_loop` (which spawns a `connection_task` per accepted stream). `start_broker` returns a `Broker` holding the event-channel sender and a shutdown watch sender; `broker_is_running` reports whether the loop is still alive by checking the event sender. With the `spawn` feature, the `Broker` also carries the process spawner (see [SPAWN.md](SPAWN.md)).

The broker stops in two ways, with identical effect: `stop_broker(&broker)` sends the shutdown signal explicitly, and dropping the `Broker` drops the watch sender, which the loops observe the same way. Both accept loops (TCP and WebSocket) and the event loop exit, peer writer channels close, sockets shut, and connected clients observe the disconnect. Hold the `Broker` for as long as the bus should run.

## 2. Connection tasks

`connection_task` owns the read half of a peer's TCP stream and parks the write half in an `Option<PeerWriter>`. It reads length-prefixed postcard `PeerEvent` frames and hands each to `forward_peer_event`, which maps them onto the internal `BrokerEvent` channel:

- `Hello { id }` takes the parked writer, creates a oneshot shutdown channel, keeps the sender, and forwards `BrokerEvent::Hello { id, writer, shutdown }`. A second `Hello` on the same socket is ignored (the writer was already taken).
- Every other event forwards as `BrokerEvent::Peer(event)`.

When the read side dies, the task drops its shutdown sender. That resolves the oneshot held by the peer's writer task, which exits and reports the disconnect.

`PeerWriter` is the transport abstraction: `Tcp(OwnedWriteHalf)` or, with the `websockets` feature, `WebSocket(SplitSink<...>)`. `write_to_peer` adds the transport framing: the TCP variant prepends the 4-byte length prefix; the WebSocket variant sends the payload as one binary frame. Everything upstream of the writer deals in unframed postcard bytes.

## 3. The event loop

`broker_loop` owns all broker state as one plain struct, `BrokerState`:

- `peers: HashMap<String, UnboundedSender<Vec<u8>>>` maps peer id to that peer's writer-task channel.
- `peer_generations` and `generation_counter` implement reconnect-race protection (section 4).
- `subscriptions: HashMap<String, Vec<String>>` maps topic to subscriber ids. Matching is exact string equality; there are no wildcards.
- `bridges: Vec<Bridge>`.
- `disconnect_sender`, cloned into every writer task.

The loop is a `tokio::select!` over four sources: the broker event channel, the disconnect channel, a 2-second bridge-maintenance interval that reconnects any bridge whose connection has died (section 6), and the shutdown watch channel (section 1).

## 4. Peers, generations, and disconnects

`establish_peer` removes any existing peer with the same id, increments the generation counter, records the new generation, creates the peer's message channel, and spawns `connection_writer_task` with the writer, the shutdown receiver, and the `(name, generation)` pair.

The writer task loops over `select!`: messages from the channel are written to the transport (exiting on write failure or channel close), and the shutdown oneshot resolving means the read side died. Either way the task ends by sending `(name, generation)` on the disconnect channel.

`remove_disconnected_peer` only acts if the reported generation matches the current one for that name. This is what makes same-id reconnects safe: when a client reconnects, `establish_peer` bumps the generation and replaces the channel; the old writer task's eventual disconnect report carries the old generation and is ignored, so the new connection's subscriptions survive.

## 5. Delivery

A `PublishText` event is handled in three steps:

1. `answer_introspection_requests` checks the topic against the `BrokerContract` request topics and publishes the matching report (section 7).
2. The message is forwarded over every bridge except the one whose id matches the publisher (cycle prevention, section 6), unless the publish was `local_only` (`Route::Local`).
3. `deliver_to_subscribers` serializes the `Message` once with postcard and sends the bytes to every subscribed peer's writer channel. Peers that are not in the table (already disconnected) are skipped silently.

`PublishBinary` follows the same path with `Message.bytes` populated instead of `Message.payload`.

`Subscribe` and `Unsubscribe` only mutate the local subscription table. Subscriptions are deliberately not forwarded over bridges: cross-broker delivery is publish-driven (every non-local publish crosses every bridge), so forwarding subscriptions would only cause the remote broker to stream messages into a bridge client that never reads them.

## 6. Bridges

A `Bridge` (`src/bridge.rs`) is a `Client` connected to the remote broker's address, plus its peer id and the `target_address`. Bridge clients use `autoreconnect: false` and a single connection attempt; the broker loop owns their reconnection.

The handshake is symmetric. A client asks its local broker to bridge by sending `OpenBridge { source_address, target_address, ack: false }`:

1. The local broker prunes any existing bridge to the same target (sending a removal acknowledgement over the old bridge if it was alive), creates a bridge client, and connects it to `target_address`.
2. Because `ack` is false, it then sends `OpenBridge { source_address: target, target_address: source, ack: true }` over the new bridge client, asking the remote broker to create the reverse bridge.
3. The remote broker creates its reverse bridge with the requesting peer's id as an override id, connects it back, and publishes `BrokerContract::BridgeCreated`.

The override id is the cycle prevention: messages arriving at a broker from a bridge peer carry that peer's id as publisher, and the matching reverse bridge has the same id, so the broker never republishes a message back across the bridge it arrived on.

`CloseBridge { ack: false }` acknowledges removal to the remote side and drops the local bridge; the `ack: true` form is the acknowledgement itself.

Bridge health is checked on the maintenance tick using read-side liveness (`is_connected`): a bridge counts as dead when its writer is gone or its receiver channel has closed, which happens as soon as the bridge's read task observes the dead socket. Dead bridges are reconnected in place with a single attempt per tick.

## 7. Introspection topics

The broker answers three request topics, publishing the report globally (across bridges) as `BROKER_ID` (`"broker"`):

| Request topic | Report topic | Payload |
|---|---|---|
| `hearsay/subscriptions/request` | `hearsay/subscriptions/report` | the full topic-to-subscribers table |
| `hearsay/peers/request` | `hearsay/peers/report` | connected peer ids |
| `hearsay/bridges/request` | `hearsay/bridges/report` | bridge ids |

It also announces `hearsay/peers/connected` (`PeerConnected { id }`) whenever a peer connects, and `hearsay/bridges/created` (`BridgeCreated`) when an acknowledged bridge comes up. All five are defined by `BrokerContract` in `src/contract.rs`.
