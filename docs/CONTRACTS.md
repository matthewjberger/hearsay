# Contracts and the Wire Protocol

This document covers the wire protocol (framing and the protocol enums) and the contract pattern applications use to define their own strongly typed messages. Source lives in `src/contract.rs` and `src/wire.rs`.

## 1. Framing

All protocol values are serialized with [postcard](https://docs.rs/postcard). On TCP, each frame is a 4-byte big-endian `u32` length followed by the postcard bytes (`frame_payload` / `read_frame` in `src/wire.rs`). On WebSocket, each binary frame carries the postcard bytes directly with no length prefix, since WebSocket frames are self-delimiting.

Frames larger than 64 MiB are rejected on both encode and decode, so a corrupt length prefix cannot trigger an unbounded allocation. A rejected frame kills the connection, which the reconnection machinery then handles.

## 2. Protocol enums

Two types cross the wire:

**`PeerEvent`** (client to broker) is public because it is the protocol that non-Rust and WASM clients implement directly:

```rust
pub enum PeerEvent {
    Hello { id: String },
    Subscribe { id: String, topic: String },
    Unsubscribe { id: String, topic: String },
    PublishText { id: String, topic: String, payload: String, local_only: bool },
    PublishBinary { id: String, topic: String, payload: Vec<u8>, local_only: bool },
    OpenBridge { id: String, source_address: String, target_address: String, ack: bool },
    CloseBridge { target_address: String, ack: bool },
}
```

`Hello` registers the peer for delivery; publishes are routed regardless, but a peer that has not sent `Hello` has no writer registered and receives nothing. `id` is the peer's identity for the subscription table and for bridge cycle prevention.

**`Message`** (broker to client) is what subscribers receive:

```rust
pub struct Message {
    pub topic: String,
    pub payload: String,        // JSON text publishes
    pub bytes: Option<Vec<u8>>, // binary publishes
}
```

`Route` (`Global`, `Local`) is a client-side concept; only the `local_only` flag derived from it crosses the wire. See [CLIENT.md](CLIENT.md) section 3.

Clients that implement the protocol directly (WebSocket and WASM peers) encode `PeerEvent`s and decode `Message`s with postcard themselves: `postcard::to_allocvec(&event)` and `postcard::from_bytes::<Message>(&bytes)`. The protocol types and `read_batch` compile on `wasm32`, so a browser crate depends on hearsay itself for the wire format instead of mirroring it.

## 3. The contract pattern

Applications define their messages as enums with the [enum2contract](https://github.com/matthewjberger/enum2contract) derive. hearsay re-exports the crate, so importing `hearsay::enum2contract` is all an application needs; no direct enum2contract dependency is required. Each variant declares its topic, and the macro generates a payload struct, a topic constructor, and a message constructor per variant:

```rust
use hearsay::enum2contract::{self, EnumContract};
use serde::{Deserialize, Serialize};

#[derive(Debug, EnumContract, Serialize, Deserialize, Default)]
pub enum PingPongContract {
    #[topic("pingpong/command-{channel}")]
    Command { command: PingPongCommand },

    #[topic("pingpong/event-{channel}")]
    Event { event: PingPongEvent },

    #[topic("")]
    #[default]
    #[serde(other)]
    Empty,
}
```

Generated per variant:

- `CommandPayload { command: PingPongCommand }` with `to_json()` and `from_json()`.
- `PingPongContract::command_topic(channel: &str) -> String`, substituting each `{placeholder}`.
- `PingPongContract::command(channel: &str) -> (String, CommandPayload)`, returning the topic and a default payload ready to mutate.

The conventions:

- A `Command { command: XCommand }` variant for action requests and an `Event { event: XEvent }` variant for things that happened, with the inner types as plain enums.
- An `Empty` variant with `#[topic("")]`, `#[default]`, and `#[serde(other)]` on both the contract and the inner enums, so unknown variants deserialize to a harmless catch-all instead of failing when peers run different versions.

## 4. Publishing and receiving with a contract

Publish with the generated constructors:

```rust
let (topic, mut payload) = PingPongContract::command("broadcast");
payload.command = PingPongCommand::Ping;
hearsay::publish(&client, &topic, &payload, hearsay::Route::Global).await?;
```

Receive by matching the topic and deserializing the payload struct:

```rust
match message.topic.as_str() {
    topic if PingPongContract::command_topic("broadcast") == topic => {
        if let Ok(payload) = CommandPayload::from_json(&message.payload) {
            // handle payload.command
        }
    }
    _ => {}
}
```

## 5. The broker's own contract

`BrokerContract` in `src/contract.rs` is the one contract the library defines, covering broker introspection and announcements: `RequestSubscriptions`/`ReportSubscriptions`, `RequestPeers`/`ReportPeers`, `RequestBridges`/`ReportBridges`, `PeerConnected`, and `BridgeCreated`. Its generated payload types (`ReportPeersPayload`, `BridgeCreatedPayload`, and so on) are re-exported so applications can subscribe to and decode these topics. Application contracts are deliberately out of scope for the library; see [ARCHITECTURE.md](ARCHITECTURE.md) section 3.
