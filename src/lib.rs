//! Topic-based pub/sub messaging over TCP, built on tokio.

mod batch;
#[cfg(not(target_arch = "wasm32"))]
mod bridge;
#[cfg(not(target_arch = "wasm32"))]
mod broker;
#[cfg(not(target_arch = "wasm32"))]
mod client;
mod contract;
#[cfg(not(target_arch = "wasm32"))]
mod lifecycle;
#[cfg(all(feature = "spawn", not(target_arch = "wasm32")))]
mod spawn;
#[cfg(all(feature = "websockets", not(target_arch = "wasm32")))]
mod websocket;
#[cfg(not(target_arch = "wasm32"))]
mod wire;

pub use enum2contract;

pub use self::{batch::*, contract::*};

#[cfg(not(target_arch = "wasm32"))]
pub use self::{broker::*, client::*, lifecycle::*};

#[cfg(all(feature = "spawn", not(target_arch = "wasm32")))]
pub use self::spawn::*;

#[cfg(all(feature = "websockets", not(target_arch = "wasm32")))]
pub use self::websocket::*;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;
