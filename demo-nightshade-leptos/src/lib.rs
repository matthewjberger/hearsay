//! The Leptos UI on the main thread.
//!
//! ## Architecture
//!
//! - `src/app.rs` composes the components and forwards keyboard input.
//! - `src/bridge.rs` spawns the worker and converts `WorkerMessage`s into
//!   signal writes, and `ClientMessage`s into `postMessage` envelopes.
//! - `src/hearsay_link.rs` is the hearsay peer: a WebSocket session against
//!   the broker speaking postcard-encoded `PeerEvent`/`Message` frames.
//! - `src/shell.rs` is the multi-window shell: role detection, the shell
//!   contract topics, and project/layout persistence.
//! - `src/state.rs` is all page state, grouped as `Copy` signals.
//! - `src/themes.rs` applies the demo palettes as CSS variables.
//! - `src/components/` holds the components: the viewport canvas, the top
//!   bar, the renderer HUD, the template panels, toasts, modals, and the Api
//!   composer.
//!
//! Add a new feature by extending the `protocol` messages, handling them in
//! `bridge.rs` (page side) and `worker/src/lib.rs` (worker side), and
//! building the UI in a new file under `src/components/`.

mod app;
mod bridge;
mod components;
mod hearsay_link;
mod shell;
mod state;
mod themes;

pub use app::App;
