use enum2contract::EnumContract;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const SHELL_SERVICE_ID: &str = "shell-service";

#[derive(Debug, EnumContract, Serialize, Deserialize)]
pub enum ShellContract {
    #[topic("shell/window/announce")]
    Announce { window_id: String },

    #[topic("shell/window/assign-{window}")]
    AssignLayout {
        layout_name: String,
        tree_json: String,
    },

    #[topic("shell/window/close-{window}")]
    Close,

    #[topic("shell/window/request-trees")]
    RequestTrees,

    #[topic("shell/window/report-tree")]
    ReportTree {
        window_id: String,
        layout_name: String,
        tree_json: String,
    },

    #[topic("shell/window/request-spawn")]
    RequestSpawn,
}

#[derive(Default)]
pub struct WindowRegistry {
    pub known_windows: Vec<String>,
    pub pending_layouts: VecDeque<(String, String)>,
}

pub enum CollectionDestination {
    Path(String),
    Dialog,
}

pub struct ReportedTree {
    pub window_id: String,
    pub layout_name: String,
    pub tree_json: String,
}

pub struct CollectionState {
    pub remaining_seconds: f32,
    pub collected: Vec<ReportedTree>,
    pub destination: CollectionDestination,
}

#[derive(Default)]
pub struct ProjectCollection {
    pub active: Option<CollectionState>,
}
