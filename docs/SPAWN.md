# Spawn

With the `spawn` feature (`spawn = ["tokio/process"]`), the broker owns and supervises client app processes directly. The design intent: the broker is programmatically responsible for all of its client apps, and the apps go down when the broker does. Source lives in `src/spawn.rs`.

There is no spawn contract and there are no spawn topics. Spawning is first-class: control is function calls on the `Broker`, not messages over the bus. A program that wants remote control of spawning defines its own contract and calls these functions from its `receive_message` handler.

## 1. Data types

```rust
pub struct App {
    pub name: String,                                    // unique key
    pub path: String,                                    // executable path or name (PATH-resolved)
    pub args: String,                                    // whitespace-split
    pub environment_variables: HashMap<String, String>,
    pub restart_policy: RestartPolicy,
}

pub enum RestartPolicy { Never, OnFailure, Always }      // default Never

pub enum AppStatus { NotFound, Running, Stopped, ExitedSuccessfully, ExitedWithError(String) }

pub struct OutputLine {
    pub app_name: String,
    pub stream: OutputStream,                            // Stdout or Stderr
    pub line: String,
    pub timestamp_ms: u64,                               // unix millis
}
```

Apps are keyed by `name`. There is no separate id layer.

## 2. Functions

All take `&Broker`:

- `spawn_app(&broker, app)` launches the process. Errors if an app with that name is currently running; a previously exited or stopped name is replaced.
- `stop_app(&broker, name)` kills the process and marks it `Stopped`. Stopped apps are never restarted by supervision.
- `restart_app(&broker, name)` kills (if running) and relaunches from the stored descriptor.
- `app_status(&broker, name)` and `app_statuses(&broker)` poll child exit state on demand.
- `drain_output(&broker)` returns every `OutputLine` captured since the last drain, across all apps and both streams.

## 3. Launching

Every child is spawned with:

- `kill_on_drop(true)`, the lifetime guarantee (section 5).
- Piped stdout and stderr, each consumed by a reader task that pushes `OutputLine`s into the spawner's channel. Reader tasks exit on stream EOF (the child died) or when the channel closes (the broker was dropped).
- The app's `environment_variables`, plus `HEARSAY_BROKER` set to the broker's listen address, so spawned apps never hard-code where to connect:

```rust
let broker_address = std::env::var("HEARSAY_BROKER")?;
```

- `CREATE_NO_WINDOW` on Windows, so console children do not flash windows.

`args` is split on whitespace; arguments containing spaces are not expressible.

## 4. Supervision

A single supervision task ticks every 500ms. For each app with a live child it polls `try_wait`; on exit it records the status and applies the policy:

| Exit | `Never` | `OnFailure` | `Always` |
|---|---|---|---|
| success | record | record | relaunch |
| failure | record | relaunch | relaunch |

Relaunches go through the same launch path (env injection, output capture, `kill_on_drop`). The tick interval acts as the natural restart spacing. Manual `stop_app` sets `Stopped` and clears the child, which supervision skips, so stopped means stopped.

The supervision task holds only a `Weak` reference to the spawner state and exits once the broker is gone.

## 5. Lifetime: apps die with the broker

The spawner state, including every `Child` handle, lives inside the `Broker` struct. Dropping the `Broker` drops the state, every child was spawned with `kill_on_drop(true)`, so every managed process is killed by the runtime. The supervision task fails its next `Weak` upgrade and exits; the output reader tasks see EOF and exit. Nothing requires explicit shutdown calls.

The one caveat is inherent to `kill_on_drop`: it runs when destructors run. `std::process::exit` and an aborting panic skip destructors, in which case children are orphaned until they notice the dead broker connection themselves.

## 6. The host pattern

The intended shape is a single host process that brings up the broker and its constellation:

```rust
let broker = hearsay::start_broker("127.0.0.1:9612").await?;
hearsay::spawn_app(&broker, hearsay::App {
    name: "worker".to_string(),
    path: "target/debug/worker".to_string(),
    restart_policy: hearsay::RestartPolicy::OnFailure,
    ..Default::default()
}).await?;
```

`examples/pingpong.rs` demonstrates it end to end with `cargo run --example pingpong --features spawn -- host`: the host starts the broker, spawns the responder and requester roles as child processes of its own executable, streams their stdout, and exits once the requester completes, killing the responder on the way out.
