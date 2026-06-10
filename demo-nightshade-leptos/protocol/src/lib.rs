use serde::{Deserialize, Serialize};

/// Envelope field carrying the serialized message in every `postMessage`.
pub const MESSAGE_KEY: &str = "message";
/// Envelope field carrying the transferred `OffscreenCanvas` (on `Init` only).
pub const CANVAS_KEY: &str = "canvas";

/// Lifecycle phase of a forwarded touch contact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

/// Page to worker. Pixel quantities are physical surface pixels (CSS pixels
/// times the device pixel ratio), origin at the canvas top-left.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    /// Sent once with the `OffscreenCanvas` in the transfer list.
    Init {
        width: f32,
        height: f32,
    },
    Resize {
        width: f32,
        height: f32,
    },
    /// Absolute cursor position in physical pixels. Drives the engine camera.
    PointerMove {
        x: f32,
        y: f32,
    },
    /// A mouse button changed. `button` is 0 left, 1 middle, 2 right.
    PointerButton {
        button: u8,
        pressed: bool,
    },
    /// Wheel delta in raw pixels (the worker converts to scroll lines).
    Wheel {
        delta: f32,
    },
    /// A touch contact in physical pixels. Drives the engine touch controller:
    /// one finger orbits, two fingers pan, a pinch zooms. `id` is the pointer id.
    Touch {
        id: u64,
        phase: TouchPhase,
        x: f32,
        y: f32,
    },
    /// A keyboard event. `code` is the DOM `KeyboardEvent.code`, `text` the
    /// produced character if any.
    Key {
        code: String,
        pressed: bool,
        text: Option<String>,
    },
    /// A click without drag: GPU-pick and select the entity at this position.
    Pick {
        x: f32,
        y: f32,
    },
    /// Example game message. Replace with your own as the game grows.
    SpawnCube,
}

/// The selected entity, reported after a pick resolves.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectedEntity {
    pub id: u32,
    pub name: String,
}

/// Worker to page.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WorkerMessage {
    /// The renderer is up and the render loop is running.
    Ready { adapter: String },
    /// Streamed twice a second for the HUD.
    Stats { fps: f32, entity_count: u32 },
    /// The pick result: the entity under the click, or `None` for background.
    Selected { detail: Option<SelectedEntity> },
    /// Example game message. Replace with your own as the game grows.
    CubeCount { count: u32 },
}
