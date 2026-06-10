//! The page side of the multi-window shell: role detection, the shell
//! contract topics, project and layout persistence, and the collection flow
//! that gathers every window's layout into one project save file.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::{JsCast, JsValue};

use crate::hearsay_link::{BridgeSlot, HearsaySlot, publish_text};
use crate::state::{
    Collection, DemoState, ReportedLayout, SaveDestination, ToastKind, WindowLayout,
};
use crate::themes::local_storage;

pub const SPAWN_TOPIC: &str = "shell/leptos/request-spawn";
pub const ANNOUNCE_TOPIC: &str = "shell/leptos/announce";
pub const REQUEST_LAYOUTS_TOPIC: &str = "shell/leptos/request-layouts";
pub const REPORT_LAYOUT_TOPIC: &str = "shell/leptos/report-layout";
const COLLECTION_TIMEOUT_MILLISECONDS: i32 = 1000;
const RECENTS_KEY: &str = "hearsay-demo-leptos-recents";
const STARTUP_KEY: &str = "hearsay-demo-leptos-startup";
const PROJECT_PREFIX: &str = "hearsay-demo-leptos-project-";

pub fn assign_topic(window_id: &str) -> String {
    format!("shell/leptos/assign-{window_id}")
}

pub fn close_topic(shell_id: &str) -> String {
    format!("shell/leptos/close-{shell_id}")
}

#[derive(Serialize, Deserialize)]
struct AnnouncePayload {
    window_id: String,
}

#[derive(Serialize, Deserialize)]
struct ReportPayload {
    window_id: String,
    layout: WindowLayout,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectSaveFile {
    pub version: String,
    pub project_name: Option<String>,
    pub windows: Vec<WindowLayout>,
}

impl Default for ProjectSaveFile {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            project_name: None,
            windows: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct LayoutSaveFile {
    pub version: String,
    pub layout: WindowLayout,
}

/// Reads the role and shell id the desktop shell passed through the page
/// URL. A page served by `trunk serve` has neither and acts as a primary.
pub fn detect_shell() -> (bool, String) {
    let search = web_sys::window()
        .and_then(|window| window.location().search().ok())
        .unwrap_or_default();
    let mut is_primary = true;
    let mut shell_id = format!(
        "page-{:08x}",
        (js_sys::Math::random() * u32::MAX as f64) as u32
    );
    for pair in search.trim_start_matches('?').split('&') {
        if let Some(role) = pair.strip_prefix("role=") {
            is_primary = role != "child";
        }
        if let Some(shell) = pair.strip_prefix("shell=") {
            shell_id = shell.to_string();
        }
    }
    (is_primary, shell_id)
}

pub fn set_page_timeout(callback: impl FnOnce() + 'static, milliseconds: i32) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let closure = Closure::once_into_js(callback);
    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
        closure.unchecked_ref(),
        milliseconds,
    );
}

pub fn shell_topics(state: DemoState) -> Vec<String> {
    let shell_id = state.shell_id.get_value();
    if state.is_primary {
        vec![ANNOUNCE_TOPIC.to_string(), REPORT_LAYOUT_TOPIC.to_string()]
    } else {
        vec![assign_topic(&shell_id), REQUEST_LAYOUTS_TOPIC.to_string()]
    }
}

/// Runs once per websocket session after `Hello` and the subscriptions: a
/// child announces itself, a primary loads the startup project on the first
/// connection.
pub fn on_connected(state: DemoState, link: HearsaySlot) {
    if !state.is_primary {
        let payload = AnnouncePayload {
            window_id: state.shell_id.get_value(),
        };
        if let Ok(json) = serde_json::to_string(&payload) {
            publish_text(link, ANNOUNCE_TOPIC, &json);
        }
        return;
    }
    if state.startup_loaded.get_value() {
        return;
    }
    state.startup_loaded.set_value(true);
    if let Some(name) = state.startup_project.get_untracked()
        && let Some(json) = stored_project(&name)
    {
        load_project_json(state, link, &json);
    }
}

/// Handles one shell-contract topic. Returns `true` when the topic was a
/// shell topic.
pub fn handle_shell_topic(state: DemoState, link: HearsaySlot, message: &hearsay::Message) -> bool {
    let shell_id = state.shell_id.get_value();
    if state.is_primary && message.topic == ANNOUNCE_TOPIC {
        if let Ok(payload) = serde_json::from_str::<AnnouncePayload>(&message.payload) {
            state.known_windows.update(|windows| {
                if !windows.contains(&payload.window_id) {
                    windows.push(payload.window_id.clone());
                }
            });
            let assignment = state
                .pending_assignments
                .try_update_value(|pending| pending.pop_front())
                .flatten();
            if let Some(layout) = assignment
                && let Ok(json) = serde_json::to_string(&layout)
            {
                publish_text(link, &assign_topic(&payload.window_id), &json);
            }
        }
        return true;
    }
    if state.is_primary && message.topic == REPORT_LAYOUT_TOPIC {
        if let Ok(payload) = serde_json::from_str::<ReportPayload>(&message.payload) {
            let complete = state
                .collection
                .try_update_value(|collection| {
                    let Some(collection) = collection.as_mut() else {
                        return false;
                    };
                    collection.collected.push(ReportedLayout {
                        window_id: payload.window_id,
                        layout: payload.layout,
                    });
                    collection.collected.len() >= state.known_windows.get_untracked().len()
                })
                .unwrap_or(false);
            if complete {
                finalize_collection(state, link);
            }
        }
        return true;
    }
    if !state.is_primary && message.topic == assign_topic(&shell_id) {
        if let Ok(layout) = serde_json::from_str::<WindowLayout>(&message.payload) {
            state.apply_layout(&layout);
        }
        return true;
    }
    if !state.is_primary && message.topic == REQUEST_LAYOUTS_TOPIC {
        let payload = ReportPayload {
            window_id: shell_id,
            layout: state.current_layout(),
        };
        if let Ok(json) = serde_json::to_string(&payload) {
            publish_text(link, REPORT_LAYOUT_TOPIC, &json);
        }
        return true;
    }
    false
}

pub fn request_spawn_window(link: HearsaySlot) {
    publish_text(link, SPAWN_TOPIC, "{}");
}

pub fn close_child_windows(state: DemoState, link: HearsaySlot) {
    let windows = state.known_windows.get_untracked();
    state.known_windows.set(Vec::new());
    state
        .pending_assignments
        .update_value(|pending| pending.clear());
    for window_id in windows {
        publish_text(link, &close_topic(&window_id), "{}");
    }
}

/// Starts a project save: alone it saves immediately, with child windows it
/// requests their layouts and finishes when all report or the timeout fires.
pub fn begin_project_save(state: DemoState, link: HearsaySlot, destination: SaveDestination) {
    if state.known_windows.get_untracked().is_empty() {
        let save_file = ProjectSaveFile {
            version: env!("CARGO_PKG_VERSION").to_string(),
            project_name: Some(state.project_name.get_untracked()),
            windows: vec![state.current_layout()],
        };
        finalize_project_save(state, &save_file, destination);
        return;
    }
    state.collection.set_value(Some(Collection {
        collected: Vec::new(),
        destination,
    }));
    publish_text(link, REQUEST_LAYOUTS_TOPIC, "{}");
    set_page_timeout(
        move || {
            if state.collection.with_value(Option::is_some) {
                finalize_collection(state, link);
            }
        },
        COLLECTION_TIMEOUT_MILLISECONDS,
    );
}

fn finalize_collection(state: DemoState, _link: HearsaySlot) {
    let Some(collection) = state.collection.try_update_value(Option::take).flatten() else {
        return;
    };

    state.known_windows.update(|windows| {
        windows.retain(|window_id| {
            collection
                .collected
                .iter()
                .any(|reported| &reported.window_id == window_id)
        });
    });

    let mut windows = vec![state.current_layout()];
    for window_id in state.known_windows.get_untracked() {
        if let Some(reported) = collection
            .collected
            .iter()
            .find(|reported| reported.window_id == window_id)
        {
            windows.push(reported.layout.clone());
        }
    }
    let save_file = ProjectSaveFile {
        version: env!("CARGO_PKG_VERSION").to_string(),
        project_name: Some(state.project_name.get_untracked()),
        windows,
    };
    finalize_project_save(state, &save_file, collection.destination);
}

fn finalize_project_save(
    state: DemoState,
    save_file: &ProjectSaveFile,
    destination: SaveDestination,
) {
    let Ok(json) = serde_json::to_string_pretty(save_file) else {
        state.push_toast("Failed to serialize project", ToastKind::Error, 5000);
        return;
    };
    let name = state.project_name.get_untracked();
    store_recent_project(state, &name, &json);
    if matches!(destination, SaveDestination::Download) {
        download_file(&format!("{name}.project.json"), &json);
    }
    state.project_modified.set(false);
    state.layout_modified.set(false);
    state.push_toast(&format!("Project saved: {name}"), ToastKind::Success, 3000);
}

/// Applies a parsed project file: the first window's layout lands here, each
/// extra window spawns a new shell that gets its layout when it announces.
pub fn load_project_json(state: DemoState, link: HearsaySlot, json: &str) {
    let Ok(save_file) = serde_json::from_str::<ProjectSaveFile>(json) else {
        state.push_toast("Failed to parse project file", ToastKind::Error, 5000);
        return;
    };
    let Some(first) = save_file.windows.first() else {
        return;
    };
    close_child_windows(state, link);
    state.apply_layout(first);

    for layout in save_file.windows.iter().skip(1) {
        state
            .pending_assignments
            .update_value(|pending| pending.push_back(layout.clone()));
        request_spawn_window(link);
    }

    let name = save_file
        .project_name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Untitled Project".to_string());
    state.project_name.set(name.clone());
    state.project_modified.set(false);
    store_recent_project(state, &name, json);
    state.push_toast(&format!("Opened project: {name}"), ToastKind::Success, 3000);
}

pub fn close_project(state: DemoState, link: HearsaySlot) {
    close_child_windows(state, link);
    state.apply_layout(&WindowLayout {
        layout_name: "Default Layout".to_string(),
        panels: vec!["Template".to_string()],
    });
    state.project_name.set("Untitled Project".to_string());
    state.project_modified.set(false);
    state.push_toast("Project closed", ToastKind::Info, 3000);
}

pub fn download_file(file_name: &str, contents: &str) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let array = js_sys::Array::new();
    array.push(&JsValue::from_str(contents));
    let Ok(blob) = web_sys::Blob::new_with_str_sequence(&array) else {
        return;
    };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else {
        return;
    };
    if let Ok(element) = document.create_element("a")
        && let Ok(anchor) = element.dyn_into::<web_sys::HtmlAnchorElement>()
    {
        anchor.set_href(&url);
        anchor.set_download(file_name);
        anchor.click();
    }
    let _ = web_sys::Url::revoke_object_url(&url);
}

pub fn load_recents(state: DemoState) {
    let Some(storage) = local_storage() else {
        return;
    };
    if let Ok(Some(json)) = storage.get_item(RECENTS_KEY)
        && let Ok(names) = serde_json::from_str::<Vec<String>>(&json)
    {
        state.recents.set(names);
    }
    if let Ok(Some(name)) = storage.get_item(STARTUP_KEY) {
        state.startup_project.set(Some(name));
    }
}

fn persist_recents(state: DemoState) {
    let Some(storage) = local_storage() else {
        return;
    };
    if let Ok(json) = serde_json::to_string(&state.recents.get_untracked()) {
        let _ = storage.set_item(RECENTS_KEY, &json);
    }
    match state.startup_project.get_untracked() {
        Some(name) => {
            let _ = storage.set_item(STARTUP_KEY, &name);
        }
        None => {
            let _ = storage.remove_item(STARTUP_KEY);
        }
    }
}

fn store_recent_project(state: DemoState, name: &str, json: &str) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(&format!("{PROJECT_PREFIX}{name}"), json);
    }
    state.recents.update(|recents| {
        recents.retain(|recent| recent != name);
        recents.insert(0, name.to_string());
        const MAX_RECENT_PROJECTS: usize = 10;
        if recents.len() > MAX_RECENT_PROJECTS {
            recents.truncate(MAX_RECENT_PROJECTS);
        }
    });
    persist_recents(state);
}

pub fn stored_project(name: &str) -> Option<String> {
    local_storage()?
        .get_item(&format!("{PROJECT_PREFIX}{name}"))
        .ok()
        .flatten()
}

pub fn set_startup_project(state: DemoState, name: Option<String>) {
    state.startup_project.set(name);
    persist_recents(state);
}

pub fn clear_recent_projects(state: DemoState) {
    state.recents.update(|recents| {
        let startup = state.startup_project.get_untracked();
        recents.retain(|recent| Some(recent) == startup.as_ref());
    });
    persist_recents(state);
}

/// Routes a non-shell broker message to the panels and the engine: text and
/// binary land in every subscribed panel, the spawn topic spawns a cube.
pub fn handle_panel_topics(state: DemoState, bridge: BridgeSlot, message: &hearsay::Message) {
    use crate::hearsay_link::{BINARY_TOPIC, SPAWN_TOPIC as CUBE_TOPIC, TEXT_TOPIC};
    const MAX_RECEIVED_MESSAGES: usize = 25;
    match message.topic.as_str() {
        TEXT_TOPIC => {
            for panel in state.panels.get_untracked() {
                if !panel.subscribed.get_untracked() {
                    continue;
                }
                panel.received_text.update(|received| {
                    received.push(message.payload.clone());
                    if received.len() > MAX_RECEIVED_MESSAGES {
                        let excess = received.len() - MAX_RECEIVED_MESSAGES;
                        received.drain(0..excess);
                    }
                });
            }
        }
        BINARY_TOPIC => {
            if let Some(bytes) = &message.bytes {
                for panel in state.panels.get_untracked() {
                    if !panel.subscribed.get_untracked() {
                        continue;
                    }
                    panel
                        .received_binary_count
                        .update(|count| *count = count.saturating_add(1));
                    panel.last_binary_length.set(bytes.len() as u32);
                }
            }
        }
        CUBE_TOPIC => {
            if let Some(bridge) = bridge.get_value() {
                crate::bridge::send(&bridge, &protocol::ClientMessage::SpawnCube);
            }
        }
        _ => {}
    }
}
