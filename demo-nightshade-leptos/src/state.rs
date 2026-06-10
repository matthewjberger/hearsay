use std::collections::VecDeque;

use leptos::prelude::*;
use protocol::SelectedEntity;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastKind {
    pub fn class(&self) -> &'static str {
        match self {
            ToastKind::Info => "toast info",
            ToastKind::Success => "toast success",
            ToastKind::Warning => "toast warning",
            ToastKind::Error => "toast error",
        }
    }
}

#[derive(Clone)]
pub struct Toast {
    pub id: u32,
    pub text: String,
    pub kind: ToastKind,
}

#[derive(Clone)]
pub struct ModalRequest {
    pub id: u32,
    pub panel_id: u32,
    pub title: String,
    pub body: String,
    pub confirm_text: String,
    pub cancel_text: String,
}

/// One template panel: the per-widget state the bevy demo kept in each
/// `TemplateWidget`, as signals.
#[derive(Clone, Copy)]
pub struct PanelState {
    pub id: u32,
    pub subscribed: RwSignal<bool>,
    pub received_text: RwSignal<Vec<String>>,
    pub received_binary_count: RwSignal<u32>,
    pub last_binary_length: RwSignal<u32>,
    pub outgoing_text: RwSignal<String>,
    pub picked_file: RwSignal<Option<String>>,
    pub picked_folder: RwSignal<Option<String>>,
    pub saved_file: RwSignal<Option<String>>,
    pub last_modal_result: RwSignal<Option<bool>>,
    pub modal_open: RwSignal<bool>,
}

impl PanelState {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            subscribed: RwSignal::new(true),
            received_text: RwSignal::new(Vec::new()),
            received_binary_count: RwSignal::new(0),
            last_binary_length: RwSignal::new(0),
            outgoing_text: RwSignal::new(String::new()),
            picked_file: RwSignal::new(None),
            picked_folder: RwSignal::new(None),
            saved_file: RwSignal::new(None),
            last_modal_result: RwSignal::new(None),
            modal_open: RwSignal::new(false),
        }
    }
}

/// A window's worth of layout for the project save file: which panels it
/// shows and the layout's name.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct WindowLayout {
    pub layout_name: String,
    pub panels: Vec<String>,
}

pub struct ReportedLayout {
    pub window_id: String,
    pub layout: WindowLayout,
}

pub enum SaveDestination {
    Recents,
    Download,
}

pub struct Collection {
    pub collected: Vec<ReportedLayout>,
    pub destination: SaveDestination,
}

/// All page state, grouped as signals. `Copy`, so it threads into every
/// component and closure without cloning.
#[derive(Clone, Copy)]
pub struct DemoState {
    pub is_primary: bool,
    pub shell_id: StoredValue<String>,
    pub ready: RwSignal<bool>,
    pub adapter: RwSignal<String>,
    pub fps: RwSignal<f32>,
    pub entity_count: RwSignal<u32>,
    pub cube_count: RwSignal<u32>,
    pub selected: RwSignal<Option<SelectedEntity>>,
    pub grabbing: RwSignal<bool>,
    pub hearsay_connected: RwSignal<bool>,
    pub hearsay_client_id: RwSignal<String>,
    pub panels: RwSignal<Vec<PanelState>>,
    pub next_panel_id: RwSignal<u32>,
    pub next_toast_id: RwSignal<u32>,
    pub next_modal_id: RwSignal<u32>,
    pub toasts: RwSignal<Vec<Toast>>,
    pub modals: RwSignal<Vec<ModalRequest>>,
    pub project_name: RwSignal<String>,
    pub project_modified: RwSignal<bool>,
    pub layout_name: RwSignal<String>,
    pub layout_modified: RwSignal<bool>,
    pub recents: RwSignal<Vec<String>>,
    pub startup_project: RwSignal<Option<String>>,
    pub api_visible: RwSignal<bool>,
    pub theme_index: RwSignal<usize>,
    pub known_windows: RwSignal<Vec<String>>,
    pub pending_assignments: StoredValue<VecDeque<WindowLayout>>,
    pub collection: StoredValue<Option<Collection>>,
    pub startup_loaded: StoredValue<bool>,
}

impl DemoState {
    pub fn new(is_primary: bool, shell_id: String) -> Self {
        let state = Self {
            is_primary,
            shell_id: StoredValue::new(shell_id),
            ready: RwSignal::new(false),
            adapter: RwSignal::new(String::new()),
            fps: RwSignal::new(0.0),
            entity_count: RwSignal::new(0),
            cube_count: RwSignal::new(0),
            selected: RwSignal::new(None),
            grabbing: RwSignal::new(false),
            hearsay_connected: RwSignal::new(false),
            hearsay_client_id: RwSignal::new(String::new()),
            panels: RwSignal::new(Vec::new()),
            next_panel_id: RwSignal::new(0),
            next_toast_id: RwSignal::new(0),
            next_modal_id: RwSignal::new(0),
            toasts: RwSignal::new(Vec::new()),
            modals: RwSignal::new(Vec::new()),
            project_name: RwSignal::new("Untitled Project".to_string()),
            project_modified: RwSignal::new(false),
            layout_name: RwSignal::new("Default Layout".to_string()),
            layout_modified: RwSignal::new(false),
            recents: RwSignal::new(Vec::new()),
            startup_project: RwSignal::new(None),
            api_visible: RwSignal::new(false),
            theme_index: RwSignal::new(0),
            known_windows: RwSignal::new(Vec::new()),
            pending_assignments: StoredValue::new(VecDeque::new()),
            collection: StoredValue::new(None),
            startup_loaded: StoredValue::new(false),
        };
        state.add_panel();
        state
    }

    pub fn add_panel(&self) -> PanelState {
        let id = self.next_panel_id.get_untracked();
        self.next_panel_id.set(id + 1);
        let panel = PanelState::new(id);
        self.panels.update(|panels| panels.push(panel));
        panel
    }

    pub fn remove_panel(&self, id: u32) {
        self.panels.update(|panels| {
            panels.retain(|panel| panel.id != id);
        });
        self.layout_modified.set(true);
        self.project_modified.set(true);
    }

    pub fn push_toast(&self, text: &str, kind: ToastKind, duration_milliseconds: i32) {
        let id = self.next_toast_id.get_untracked();
        self.next_toast_id.set(id + 1);
        self.toasts.update(|toasts| {
            toasts.push(Toast {
                id,
                text: text.to_string(),
                kind,
            });
        });
        let toasts = self.toasts;
        crate::shell::set_page_timeout(
            move || {
                toasts.update(|toasts| toasts.retain(|toast| toast.id != id));
            },
            duration_milliseconds,
        );
    }

    pub fn show_modal(
        &self,
        panel_id: u32,
        title: &str,
        body: &str,
        confirm_text: &str,
        cancel_text: &str,
    ) {
        let id = self.next_modal_id.get_untracked();
        self.next_modal_id.set(id + 1);
        self.modals.update(|modals| {
            modals.push(ModalRequest {
                id,
                panel_id,
                title: title.to_string(),
                body: body.to_string(),
                confirm_text: confirm_text.to_string(),
                cancel_text: cancel_text.to_string(),
            });
        });
    }

    pub fn current_layout(&self) -> WindowLayout {
        WindowLayout {
            layout_name: self.layout_name.get_untracked(),
            panels: self
                .panels
                .get_untracked()
                .iter()
                .map(|_| "Template".to_string())
                .collect(),
        }
    }

    pub fn apply_layout(&self, layout: &WindowLayout) {
        self.panels.set(Vec::new());
        for _ in &layout.panels {
            self.add_panel();
        }
        self.layout_name.set(layout.layout_name.clone());
        self.layout_modified.set(false);
    }
}
