# Architecture

This document describes the overall structure of hearsay: the module layout, the design principles the codebase follows, the ownership and lifetime model, and the locking discipline. Subsystem details live in the sibling documents: [BROKER.md](BROKER.md), [CLIENT.md](CLIENT.md), [CONTRACTS.md](CONTRACTS.md), [BATCHING.md](BATCHING.md), [LIFECYCLE.md](LIFECYCLE.md), [SPAWN.md](SPAWN.md), and [WEBSOCKETS.md](WEBSOCKETS.md).

All file paths are relative to the repository root.

## 1. What hearsay is

hearsay lets Rust programs talk to each other. One process hosts a broker; any number of processes connect to it as clients over TCP (or WebSocket, with the `websockets` feature) and exchange messages through topic-based pub/sub. Topics are exact-match strings. Payloads are JSON text or raw bytes. Brokers can be bridged so that messages flow between machines. With the `spawn` feature, the broker also owns and supervises the client app processes themselves.

## 2. Module layout

- `src/lib.rs` declares the modules, re-exports the public surface, and defines `Error` (a boxed error) and `Result`.
- `src/contract.rs` holds all protocol data types: `Message`, `Route`, `PeerEvent`, and the `BrokerContract` introspection contract. Types only, no behavior.
- `src/wire.rs` holds the framing functions: postcard serialization with a 64 MiB cap, and the 4-byte big-endian length prefix used on TCP.
- `src/client.rs` is the client: connection management, reconnection, subscriptions, publishing, and message receipt.
- `src/batch.rs` is the batching primitive: size- and interval-triggered coalescing of high-frequency items into single binary messages.
- `src/broker.rs` is the broker: the listener, the per-connection tasks, the single-owner event loop, peer and subscription state, and bridge management.
- `src/bridge.rs` is a thin internal wrapper: a `Bridge` is a `Client` connected to a remote broker, plus the identity and address the broker needs to manage it.
- `src/lifecycle.rs` is the service runner: the `Lifecycle` trait and `run_lifecycle`.
- `src/spawn.rs` (feature `spawn`) is broker-owned process management.
- `src/websocket.rs` (feature `websockets`) is the WebSocket listener that lets browser and WASM peers join the broker.

The dependency direction is strictly upward: `wire` and `contract` at the bottom; `client` and `batch` built on those; `bridge` reusing `client`; `broker` on top of all of them; `lifecycle` and `spawn` depending only on `client` and `broker` respectively. There are no cycles.

On `wasm32` targets the crate compiles down to its protocol surface: `Message`, `Route`, `PeerEvent`, `BrokerContract` and its payloads, and `read_batch`. Everything tokio-backed (broker, client, bridge, lifecycle, spawn, websocket listener, wire framing, the sending half of batching) is compiled out, and tokio, socket2, and uuid are target-gated dependencies that never appear in a wasm build. This is what lets a browser client depend on hearsay for the exact wire types instead of mirroring them; see [WEBSOCKETS.md](WEBSOCKETS.md) section 4. CI checks the wasm32 build on every push.

## 3. Design principles

**Data-oriented, no object orientation.** State lives in plain structs (`Broker`, `Client`, `ClientSettings`, `BrokerState`, `ClientState`, `Bridge`, `Spawner`). Every operation is a free function taking the struct it operates on: `connect(&mut client, address)`, `publish(&client, topic, payload, route)`, `spawn_app(&broker, app)`. There are no inherent methods, no client trait hierarchy, and the only trait in the crate is `Lifecycle`, which exists because user code must be invoked by the runner.

**Private by default.** Internal types (`BrokerEvent`, `PeerWriter`, `ClientState`, `SpawnerState`, `Bridge`) are module-private or `pub(crate)`. The public surface is the set of free functions plus the protocol data types. `PeerEvent` is public because it is the wire protocol that non-Rust and WASM clients must speak.

**Single-owner state for the broker.** All broker state is plain locals owned by one event-loop task (`broker_loop`). Connection tasks communicate with it over an unbounded channel. There is no shared-state locking in the broker at all.

**No contracts in the library.** The broker's own introspection topics (`BrokerContract`) are the only topics the library defines. Application message contracts belong to applications, defined with [enum2contract](https://github.com/matthewjberger/enum2contract); see [CONTRACTS.md](CONTRACTS.md).

**No logging.** The library is silent. Failures surface through `Result` returns or through state (a client that observes a dead connection reports `is_connected` as false).

## 4. Ownership and lifetime model

Every background task is tied to the value that owns its state, so dropping the value tears the machinery down:

- The **client reconnection task** holds a `Weak` reference to the client state. When the `Client` is dropped, the next tick fails to upgrade and the task exits. At most one reconnection task exists per client, guarded by a flag in `ClientState`.
- The **spawn supervision task** holds a `Weak` reference to the spawner state inside the `Broker`. Dropping the `Broker` drops the state, which drops every `tokio::process::Child`; each child was spawned with `kill_on_drop(true)`, so all managed processes are killed, and the supervision task exits on its next tick.
- The **broker loop and accept loops** are tied to the `Broker` value through a watch channel. `stop_broker(&broker)` signals shutdown explicitly, and dropping the `Broker` has the same effect (the watch sender drops, which the loops observe). Either way the listeners stop accepting, the event loop exits, peer writer channels close, and every connected client sees its connection die. `broker_is_running` reports whether the loop is still alive. Anything that should outlive its `Broker` handle must keep the handle.
- **Per-connection reader and writer tasks** exit when their TCP stream dies or their channel closes. Peer cleanup in the broker is generation-checked so a stale disconnect from an old connection never tears down a newer one (see [BROKER.md](BROKER.md)).

## 5. Locking discipline

The broker has no locks. The client has exactly two, and they are never held at the same time:

- `Client.state: Arc<RwLock<ClientState>>` guards identity, the writer half, subscriptions, and reconnection bookkeeping. It is held only across short critical sections (including the actual TCP write).
- The receiver lives in its own slot, `ClientState.receiver: Arc<Mutex<Option<UnboundedReceiver<Message>>>>`. `next_message` takes the state lock briefly (to clone the slot handle), releases it, and only then locks the receiver slot for the potentially long `recv().await`. Reconnection installs a new receiver through the same slot, also without holding the state lock.

Because every code path acquires at most one of the two locks at a time, lock-order deadlocks are impossible by construction. Holding the receiver mutex across `recv().await` is intentional and cancel-safe: if the caller's timeout cancels the future, the guard drops and the receiver stays in the slot.

## 6. Features

- `spawn = ["tokio/process"]` adds broker-owned process management. No new third-party dependencies.
- `websockets = ["dep:tokio-tungstenite", "dep:futures-util"]` adds the WebSocket listener.

Both are additive: the default build is the TCP broker, client, contracts, and lifecycle runner only. CI checks the crate with no features, with all targets, and with all features, so the default surface can never silently depend on an optional one.

## 7. Errors

`hearsay::Error` is `Box<dyn std::error::Error + Send + Sync>` and `hearsay::Result<T>` wraps it. Functions that can fail return `Result`; functions that report state (`is_connected`, `app_status`, `next_message`) return the state directly. Inside the broker loop, per-event errors are swallowed so one malformed event cannot stop the bus.
