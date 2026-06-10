use nightshade::prelude::TileLayout;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    ConnectionStatus {
        connected: bool,
    },

    Topic {
        topic: String,
        payload: String,
        #[serde(default)]
        bytes: Option<Vec<u8>>,
    },

    Broker {
        message: BrokerServiceMessage,
    },

    Filesystem {
        message: FileSystemMessage,
    },

    Modal {
        message: ModalServiceMessage,
    },

    Notify {
        message: NotificationServiceMessage,
    },

    Project {
        message: ProjectMessage,
    },

    Tiles {
        message: TileTreeMessage,
    },

    #[default]
    #[serde(other)]
    Empty,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub enum ProjectMessage {
    ProjectLoaded {
        trees: Vec<TileLayout>,
        project_name: Option<String>,
        layout_names: Vec<String>,
        path: String,
    },
    ProjectSaved {
        path: String,
    },
    LayoutLoaded {
        layout: Box<TileLayout>,
        layout_name: Option<String>,
    },
    CloseProject,
    #[default]
    #[serde(other)]
    Empty,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub enum BrokerServiceMessage {
    Publish {
        topic: String,
        message: String,
    },
    PublishBytes {
        topic: String,
        bytes: Vec<u8>,
    },
    Subscribe {
        topics: Vec<String>,
        widget_id: String,
    },
    Unsubscribe {
        topics: Vec<String>,
        widget_id: String,
    },
    WidgetRemoved {
        widget_id: String,
    },
    SpawnWindow,
    #[default]
    #[serde(other)]
    Empty,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum FileSystemMessage {
    Command(FileSystemCommand),
    Result(FileSystemResult),

    #[default]
    Empty,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum FileSystemCommand {
    #[default]
    None,

    PickFile {
        tag: String,
        filter_name: String,
        extensions: Vec<String>,
    },

    PickFolder {
        tag: String,
    },

    SaveFile {
        tag: String,
        bytes: Vec<u8>,
        filter_name: String,
        extensions: Vec<String>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum FileSystemResult {
    Success(FileSystemSuccess),
    Error(FilesystemError),
}

impl Default for FileSystemResult {
    fn default() -> Self {
        Self::Success(FileSystemSuccess::Empty)
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum FileSystemSuccess {
    #[default]
    Empty,

    File {
        path: String,
        bytes: Vec<u8>,
        tag: String,
    },

    Folder {
        path: String,
        tag: String,
    },
}

#[derive(Default, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum FilesystemError {
    #[default]
    None,
    ReadError(String),
    WriteError(String),
    NoFileSelected,
}

pub const TAG_PROJECT: &str = "project";
pub const TAG_PROJECT_SAVE: &str = "project_save";
pub const TAG_LAYOUT: &str = "layout";
pub const TAG_LAYOUT_SAVE: &str = "layout_save";

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub enum ModalServiceMessage {
    ShowConfirm {
        id: String,
        title: String,
        body: String,
        confirm_text: Option<String>,
        cancel_text: Option<String>,
    },
    CloseModal(String),
    ModalResult {
        id: String,
        confirmed: bool,
    },
    #[default]
    #[serde(other)]
    Empty,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ModalResult {
    Confirmed,
    #[default]
    Cancelled,
}

impl From<bool> for ModalResult {
    fn from(value: bool) -> Self {
        if value {
            ModalResult::Confirmed
        } else {
            ModalResult::Cancelled
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub enum NotificationServiceMessage {
    Show {
        text: String,
        kind: NotificationKind,
        duration_in_seconds: f64,
    },
    #[default]
    #[serde(other)]
    Empty,
}

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum NotificationKind {
    Info,
    Warning,
    Error,
    Success,
    #[default]
    #[serde(other)]
    Empty,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub enum TileTreeMessage {
    ProcessFileResult(FileSystemResult),

    #[default]
    #[serde(other)]
    Empty,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct WindowTree {
    pub layout: TileLayout,
    pub layout_name: String,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectSaveFile {
    pub version: String,
    pub windows: Vec<WindowTree>,
    pub project_name: Option<String>,
}

impl Default for ProjectSaveFile {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            windows: Vec::new(),
            project_name: None,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct LayoutSaveFile {
    pub version: String,
    pub layout: TileLayout,
    pub layout_name: Option<String>,
}
