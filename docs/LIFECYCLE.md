# Lifecycle

The `Lifecycle` trait and `run_lifecycle` runner are the harness for long-running services: programs that connect to a broker, react to messages, and do periodic work on a fixed cadence. Source lives in `src/lifecycle.rs`.

## 1. The trait

```rust
pub trait Lifecycle: Send {
    fn initialize(&mut self, _client: &mut Client) -> impl Future<Output = ()> + Send;
    fn update(&mut self, _client: &mut Client) -> impl Future<Output = ()> + Send;
    fn receive_message(
        &mut self,
        _message: &Message,
        _client: &mut Client,
    ) -> impl Future<Output = Option<Interrupt>> + Send;
}
```

All three methods have default no-op bodies (`std::future::ready`), so implementors only write the hooks they need, as plain `async fn`s:

```rust
struct Responder;

impl Lifecycle for Responder {
    async fn initialize(&mut self, client: &mut Client) {
        let _ = subscribe(client, &[&MyContract::command_topic("all")]).await;
    }

    async fn receive_message(&mut self, message: &Message, client: &mut Client) -> Option<Interrupt> {
        // match message.topic, deserialize, publish responses
        None
    }
}
```

The trait declares `impl Future + Send` return types rather than `async fn` so that `run_lifecycle` futures are spawnable with `tokio::spawn` even though the lifecycle type is generic. The `Send` supertrait exists for the same reason.

- `initialize` runs once, after the client has connected. Subscribe here.
- `receive_message` runs for each message received before the current update deadline. Returning `Some(Interrupt::UpdateImmediately)` ends the receive window early and jumps straight to `update`; returning `Some(Interrupt::Stop)` exits `run_lifecycle` entirely with `Ok(())`.
- `update` runs once per interval, after the receive window closes.

## 2. The runner

```rust
pub struct LifecycleSettings {
    pub name: String,           // client name (a uuid suffix is appended)
    pub broker_address: String,
    pub update_interval: Duration,
}

pub async fn run_lifecycle(lifecycle: impl Lifecycle, settings: LifecycleSettings) -> Result<()>
```

`run_lifecycle` creates a client with default settings (which includes autoreconnect), connects, calls `initialize`, and then loops forever:

```text
loop {
    deadline = now + update_interval
    receive messages until deadline {
        receive_message(message, client)
        break early on Interrupt::UpdateImmediately
    }
    update(client)
}
```

The receive window is implemented with `tokio::time::timeout` around `next_message`, recomputing the remaining duration after each message. `next_message` is cancel-safe (see [CLIENT.md](CLIENT.md) section 5), so a deadline firing mid-wait loses nothing. A disconnected client also ends the window; the client's reconnection task restores the connection and resubscribes in the background while the loop keeps cycling.

`run_lifecycle` returns in two cases: with an error if the initial connect fails, and with `Ok(())` when `receive_message` returns `Interrupt::Stop` (a graceful shutdown the service decides for itself, typically in response to a quit command on one of its topics). Otherwise it runs forever. Run one lifecycle per task: spawn additional ones with `tokio::spawn(run_lifecycle(...))`, as `examples/pingpong.rs` does with its responder.

## 3. Choosing the interval

`update_interval` is the service's tick. Messages are processed as they arrive within the window, so the interval bounds update latency, not message latency. Short intervals (tens of milliseconds) suit control loops; long intervals suit services that are purely reactive, where `update` is just a heartbeat.
