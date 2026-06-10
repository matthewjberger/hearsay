<h1 align="center">hearsay 👂🗣️</h1>

<p align="center">
  <a href="https://github.com/matthewjberger/hearsay"><img alt="github" src="https://img.shields.io/badge/github-matthewjberger/hearsay-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20"></a>
  <a href="https://crates.io/crates/hearsay"><img alt="crates.io" src="https://img.shields.io/crates/v/hearsay.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20"></a>
  <a href="https://github.com/matthewjberger/hearsay/blob/main/LICENSE-MIT"><img alt="license" src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=for-the-badge&labelColor=555555" height="20"></a>
</p>

<p align="center"><strong>Let your Rust apps talk to each other.</strong></p>

<p align="center">
  <code>cargo add hearsay</code>
</p>

hearsay is topic-based pub/sub for Rust, built on [tokio](https://tokio.rs). One process hosts the broker with a single call; any number of others connect and exchange JSON or binary messages over TCP. The broker is a value your program holds, not a separate server to run.

- **Typed contracts**: message schemas are strongly typed enums via [enum2contract](https://github.com/matthewjberger/enum2contract), re-exported so contracts need no extra dependency
- **Bridging**: brokers connect to each other
- **WebSockets**: browser and WASM apps join the same bus
- **Spawning**: the broker can own, supervise, and restart its client processes
- **Batching**: high-frequency traffic coalesces into single binary frames

## Quick Start

One program hosts the broker:

```rust
let broker = hearsay::start_broker("127.0.0.1:9612").await?;
```

Any other program connects to it as a client:

```rust
let mut client = hearsay::create_client("listener", hearsay::ClientSettings::default());
hearsay::connect(&mut client, "127.0.0.1:9612").await?;
hearsay::subscribe(&mut client, &["greetings"]).await?;

hearsay::publish(&client, "greetings", &"hello".to_string(), hearsay::Route::Global).await?;

if let Some(message) = hearsay::next_message(&mut client).await {
    println!("{}", message.payload);
}
```

The bus runs for as long as the `Broker` is held; `stop_broker` (or dropping it) shuts everything down.

## Features

| Feature | Adds |
|---------|------|
| `websockets` | The broker accepts WebSocket peers, so browser and WASM apps participate directly |
| `spawn` | The broker owns, supervises, and restarts client app processes |

Both are additive; the default build is the TCP broker, client, contracts, batching, and lifecycle runner.

## WebSockets

```toml
hearsay = { version = "0.1", features = ["websockets"] }
```

```rust
let broker = hearsay::start_broker("127.0.0.1:9612").await?;
hearsay::start_websocket_listener(&broker, "127.0.0.1:9613").await?;
```

WebSocket peers speak the same wire protocol as TCP peers and share the broker's subscription table. On `wasm32` targets the crate compiles down to the protocol types (`PeerEvent`, `Message`), so a browser client depends on hearsay itself for the wire format and encodes them with [postcard](https://docs.rs/postcard) directly.

## Spawn

```toml
hearsay = { version = "0.1", features = ["spawn"] }
```

With the `spawn` feature, the broker owns its client app processes: they are killed when the broker is dropped, restarted according to a per-app policy, and each child receives the broker address in the `HEARSAY_BROKER` environment variable.

```rust
let broker = hearsay::start_broker("127.0.0.1:9612").await?;
hearsay::spawn_app(&broker, hearsay::App {
    name: "worker".to_string(),
    path: "target/debug/worker".to_string(),
    restart_policy: hearsay::RestartPolicy::OnFailure,
    ..Default::default()
}).await?;

for line in hearsay::drain_output(&broker).await {
    println!("[{}] {}", line.app_name, line.line);
}
```

## Batching

High-frequency traffic coalesces into single binary messages, paying the per-message overhead once per flush instead of once per item:

```rust
let mut batch = hearsay::create_batch("scene/updates", hearsay::Route::Global, 64, Duration::from_millis(50));
hearsay::push_to_batch(&client, &mut batch, update).await?;  // flushes on size or interval
hearsay::flush_batch(&client, &mut batch).await?;            // or flush manually

// subscriber side
let updates: Vec<EntityUpdate> = hearsay::read_batch(&message)?;
```

## Example

`examples/pingpong.rs` exchanges a command and an event between two services using an enum contract. Run each role as its own process in separate terminals:

```
cargo run --example pingpong -- broker
cargo run --example pingpong -- responder
cargo run --example pingpong -- requester
```

Run all three in a single process:

```
cargo run --example pingpong
```

Or have the broker spawn and supervise the other roles as child processes:

```
cargo run --example pingpong --features spawn -- host
```

## Demo

The [demo/](demo) directory is a complete multi-process application built on hearsay: a process-per-window desktop shell using Bevy and egui. The first window hosts the broker and connects to it as a client; "New Window" spawns another process of the same executable, supervised by the broker, and all windows coordinate layouts over pub/sub topics.

```
just run-demo
```

## Documentation

Architecture documentation lives in [docs/](docs/ARCHITECTURE.md), covering the broker, client, wire protocol and contracts, batching, lifecycles, process spawning, and WebSockets.

## License

Dual-licensed under MIT ([LICENSE-MIT](LICENSE-MIT)) or Apache 2.0 ([LICENSE-APACHE](LICENSE-APACHE)).
