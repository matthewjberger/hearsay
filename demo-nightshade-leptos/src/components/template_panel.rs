use leptos::prelude::*;
use serde::Serialize;

use crate::hearsay_link::{self, BINARY_TOPIC, HearsaySlot, SPAWN_TOPIC, TEXT_TOPIC};
use crate::shell::download_file;
use crate::state::{DemoState, PanelState, ToastKind};

#[derive(Serialize)]
struct TemplateNote {
    text: String,
}

/// One template panel: the bevy demo's `TemplateWidget` in browser idiom.
/// Subscriptions, typed/raw/binary publishing, the received log,
/// notifications, file actions, and a confirm modal.
#[component]
pub fn TemplatePanel(link: HearsaySlot, state: DemoState, panel: PanelState) -> impl IntoView {
    let file_input = NodeRef::<leptos::html::Input>::new();
    let folder_input = NodeRef::<leptos::html::Input>::new();

    let on_close = move |_| {
        state.remove_panel(panel.id);
    };
    let on_toggle_subscription = move |_| {
        panel
            .subscribed
            .update(|subscribed| *subscribed = !*subscribed);
    };
    let on_input = move |event: leptos::ev::Event| {
        panel.outgoing_text.set(event_target_value(&event));
    };
    let on_publish_typed = move |_| {
        let note = TemplateNote {
            text: panel.outgoing_text.get_untracked(),
        };
        if let Ok(payload) = serde_json::to_string(&note) {
            hearsay_link::publish_text(link, TEXT_TOPIC, &payload);
        }
    };
    let on_publish_raw = move |_| {
        let payload = format!("{{\"raw\":\"{}\"}}", panel.outgoing_text.get_untracked());
        hearsay_link::publish_text(link, TEXT_TOPIC, &payload);
    };
    let on_publish_binary = move |_| {
        let bytes = panel.outgoing_text.get_untracked().into_bytes();
        hearsay_link::publish_binary(link, BINARY_TOPIC, bytes);
    };
    let on_spawn_via_broker = move |_| {
        hearsay_link::publish_text(link, SPAWN_TOPIC, "{}");
    };
    let on_clear = move |_| {
        panel.received_text.set(Vec::new());
        panel.received_binary_count.set(0);
        panel.last_binary_length.set(0);
    };

    let on_notify_info = move |_| state.push_toast("An informational toast", ToastKind::Info, 3000);
    let on_notify_success = move |_| state.push_toast("A success toast", ToastKind::Success, 3000);
    let on_notify_warning = move |_| state.push_toast("A warning toast", ToastKind::Warning, 3000);
    let on_notify_error = move |_| state.push_toast("An error toast", ToastKind::Error, 3000);

    let on_pick_file = move |_| {
        if let Some(input) = file_input.get_untracked() {
            input.click();
        }
    };
    let on_file_picked = move |_| {
        let Some(input) = file_input.get_untracked() else {
            return;
        };
        if let Some(file) = input.files().and_then(|files| files.get(0)) {
            panel.picked_file.set(Some(format!(
                "{} ({} bytes)",
                file.name(),
                file.size() as u64
            )));
        }
        input.set_value("");
    };
    let on_pick_folder = move |_| {
        if let Some(input) = folder_input.get_untracked() {
            let _ = input.set_attribute("webkitdirectory", "");
            input.click();
        }
    };
    let on_folder_picked = move |_| {
        let Some(input) = folder_input.get_untracked() else {
            return;
        };
        if let Some(files) = input.files()
            && files.length() > 0
        {
            panel
                .picked_folder
                .set(Some(format!("{} files selected", files.length())));
        }
        input.set_value("");
    };
    let on_save_file = move |_| {
        let contents = panel.outgoing_text.get_untracked();
        let file_name = format!("panel-{}.txt", panel.id);
        download_file(&file_name, &contents);
        panel.saved_file.set(Some(file_name));
    };

    let on_show_modal = move |_| {
        if panel.modal_open.get_untracked() {
            return;
        }
        panel.modal_open.set(true);
        state.show_modal(
            panel.id,
            "Template Modal",
            "Confirm the template action?",
            "Yes, proceed",
            "No, cancel",
        );
    };

    let connection_class = move || {
        if state.hearsay_connected.get() {
            "panel-status connected"
        } else {
            "panel-status disconnected"
        }
    };

    view! {
        <div class="template-panel">
            <div class="panel-header">
                <span class="panel-title">"Template"</span>
                <span class="panel-id">{format!("panel-{}", panel.id)}</span>
                <button class="panel-close" on:click=on_close>"X"</button>
            </div>
            <div class=connection_class>
                {move || {
                    if state.hearsay_connected.get() {
                        "Connected to broker"
                    } else {
                        "Disconnected from broker"
                    }
                }}
            </div>

            <div class="panel-section">"Subscriptions"</div>
            <div class="panel-dim">
                {move || {
                    if panel.subscribed.get() {
                        format!("{TEXT_TOPIC}, {BINARY_TOPIC}")
                    } else {
                        "No active subscriptions".to_string()
                    }
                }}
            </div>
            <button class="hud-button" on:click=on_toggle_subscription>
                {move || if panel.subscribed.get() { "Unsubscribe" } else { "Subscribe" }}
            </button>

            <div class="panel-section">"Publish"</div>
            <input
                class="hearsay-input"
                type="text"
                placeholder="Type a message..."
                prop:value=move || panel.outgoing_text.get()
                on:input=on_input
            />
            <div class="hearsay-button-row">
                <button
                    class="hud-button"
                    on:click=on_publish_typed
                    disabled=move || !state.hearsay_connected.get()
                >
                    "Typed"
                </button>
                <button
                    class="hud-button"
                    on:click=on_publish_raw
                    disabled=move || !state.hearsay_connected.get()
                >
                    "Raw"
                </button>
                <button
                    class="hud-button"
                    on:click=on_publish_binary
                    disabled=move || !state.hearsay_connected.get()
                >
                    "Binary"
                </button>
            </div>
            <button
                class="hud-button"
                on:click=on_spawn_via_broker
                disabled=move || !state.hearsay_connected.get()
            >
                "Spawn Cube via Broker"
            </button>

            <div class="panel-section">"Received"</div>
            <div class="panel-dim">
                {move || {
                    format!(
                        "Binary messages: {} (last {} bytes)",
                        panel.received_binary_count.get(),
                        panel.last_binary_length.get(),
                    )
                }}
            </div>
            <div class="hearsay-received">
                <Show
                    when=move || !panel.received_text.get().is_empty()
                    fallback=|| view! { <div class="hearsay-empty">"No text messages received"</div> }
                >
                    <For
                        each=move || {
                            panel
                                .received_text
                                .get()
                                .into_iter()
                                .enumerate()
                                .rev()
                                .collect::<Vec<_>>()
                        }
                        key=|(index, payload)| (*index, payload.clone())
                        children=|(index, payload)| {
                            view! {
                                <div class="hearsay-received-row">
                                    <span class="hearsay-received-index">{format!("[{index}]")}</span>
                                    <span>{payload}</span>
                                </div>
                            }
                        }
                    />
                </Show>
            </div>
            <button class="hud-button" on:click=on_clear>"Clear Received"</button>

            <div class="panel-section">"Notifications"</div>
            <div class="hearsay-button-row">
                <button class="hud-button" on:click=on_notify_info>"Info"</button>
                <button class="hud-button" on:click=on_notify_success>"Success"</button>
                <button class="hud-button" on:click=on_notify_warning>"Warning"</button>
                <button class="hud-button" on:click=on_notify_error>"Error"</button>
            </div>

            <div class="panel-section">"Files"</div>
            <div class="hearsay-button-row">
                <button class="hud-button" on:click=on_pick_file>"Pick File"</button>
                <button class="hud-button" on:click=on_pick_folder>"Pick Folder"</button>
                <button class="hud-button" on:click=on_save_file>"Save File"</button>
            </div>
            <Show when=move || panel.picked_file.get().is_some() fallback=|| ()>
                <div class="panel-dim">
                    {move || format!("Picked file: {}", panel.picked_file.get().unwrap_or_default())}
                </div>
            </Show>
            <Show when=move || panel.picked_folder.get().is_some() fallback=|| ()>
                <div class="panel-dim">
                    {move || {
                        format!("Picked folder: {}", panel.picked_folder.get().unwrap_or_default())
                    }}
                </div>
            </Show>
            <Show when=move || panel.saved_file.get().is_some() fallback=|| ()>
                <div class="panel-dim">
                    {move || format!("Saved file: {}", panel.saved_file.get().unwrap_or_default())}
                </div>
            </Show>

            <div class="panel-section">"Modal"</div>
            <button
                class="hud-button"
                on:click=on_show_modal
                disabled=move || panel.modal_open.get()
            >
                "Show Modal"
            </button>
            <div class="panel-dim">
                {move || match panel.last_modal_result.get() {
                    Some(true) => "Last result: confirmed".to_string(),
                    Some(false) => "Last result: cancelled".to_string(),
                    None => "No modal result yet".to_string(),
                }}
            </div>

            <input
                node_ref=file_input
                type="file"
                class="hidden-input"
                on:change=on_file_picked
            />
            <input
                node_ref=folder_input
                type="file"
                class="hidden-input"
                on:change=on_folder_picked
            />
        </div>
    }
}
