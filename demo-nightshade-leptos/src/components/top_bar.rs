use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{JsFuture, spawn_local};

use crate::hearsay_link::HearsaySlot;
use crate::shell;
use crate::state::{DemoState, SaveDestination, ToastKind, WindowLayout};
use crate::themes::{THEMES, apply_theme, save_theme};

fn read_picked_file(input: NodeRef<leptos::html::Input>, handle: impl Fn(String) + 'static) {
    let Some(input) = input.get_untracked() else {
        return;
    };
    let Some(file) = input.files().and_then(|files| files.get(0)) else {
        return;
    };
    input.set_value("");
    spawn_local(async move {
        if let Ok(text) = JsFuture::from(file.text()).await
            && let Some(text) = text.as_string()
        {
            handle(text);
        }
    });
}

/// The top bar: project and layout menus with inline name editing, the Add
/// Panel and New Window actions, the theme picker, and the status readouts.
#[component]
pub fn TopBar(link: HearsaySlot, state: DemoState) -> impl IntoView {
    let project_menu_open = RwSignal::new(false);
    let layout_menu_open = RwSignal::new(false);
    let project_file_input = NodeRef::<leptos::html::Input>::new();
    let layout_file_input = NodeRef::<leptos::html::Input>::new();

    let on_new_project = move |_| {
        project_menu_open.set(false);
        shell::close_project(state, link);
    };
    let on_load_project = move |_| {
        project_menu_open.set(false);
        if let Some(input) = project_file_input.get_untracked() {
            input.click();
        }
    };
    let on_project_file = move |_| {
        read_picked_file(project_file_input, move |json| {
            shell::load_project_json(state, link, &json);
        });
    };
    let on_save_project = move |_| {
        project_menu_open.set(false);
        shell::begin_project_save(state, link, SaveDestination::Recents);
    };
    let on_save_project_as = move |_| {
        project_menu_open.set(false);
        shell::begin_project_save(state, link, SaveDestination::Download);
    };
    let on_set_startup = move |_| {
        project_menu_open.set(false);
        shell::set_startup_project(state, Some(state.project_name.get_untracked()));
    };
    let on_unset_startup = move |_| {
        project_menu_open.set(false);
        shell::set_startup_project(state, None);
    };
    let on_clear_recents = move |_| {
        project_menu_open.set(false);
        shell::clear_recent_projects(state);
    };

    let on_save_layout = move |_| {
        layout_menu_open.set(false);
        let layout = state.current_layout();
        let save_file = shell::LayoutSaveFile {
            version: env!("CARGO_PKG_VERSION").to_string(),
            layout,
        };
        if let Ok(json) = serde_json::to_string_pretty(&save_file) {
            let name = state.layout_name.get_untracked();
            shell::download_file(&format!("{name}.layout.json"), &json);
            state.layout_modified.set(false);
            state.push_toast(&format!("Layout saved: {name}"), ToastKind::Success, 3000);
        }
    };
    let on_load_layout = move |_| {
        layout_menu_open.set(false);
        if let Some(input) = layout_file_input.get_untracked() {
            input.click();
        }
    };
    let on_layout_file = move |_| {
        read_picked_file(layout_file_input, move |json| {
            match serde_json::from_str::<shell::LayoutSaveFile>(&json) {
                Ok(save_file) => {
                    state.apply_layout(&save_file.layout);
                    state.project_modified.set(true);
                }
                Err(_) => {
                    state.push_toast("Failed to parse layout file", ToastKind::Error, 5000);
                }
            }
        });
    };
    let on_reset_layout = move |_| {
        layout_menu_open.set(false);
        state.apply_layout(&WindowLayout {
            layout_name: "Default Layout".to_string(),
            panels: vec!["Template".to_string()],
        });
        state.project_modified.set(true);
    };

    let on_new_window = move |_| {
        shell::request_spawn_window(link);
    };
    let on_toggle_api = move |_| {
        state.api_visible.update(|visible| *visible = !*visible);
    };

    let on_project_name = move |event: leptos::ev::Event| {
        let value = event_target_value(&event);
        if !value.is_empty() {
            state.project_name.set(value);
            state.project_modified.set(true);
        }
    };
    let on_layout_name = move |event: leptos::ev::Event| {
        let value = event_target_value(&event);
        if !value.is_empty() {
            state.layout_name.set(value);
            state.layout_modified.set(true);
            state.project_modified.set(true);
        }
    };
    let on_theme = move |event: leptos::ev::Event| {
        let Some(select) = event
            .target()
            .and_then(|target| target.dyn_into::<web_sys::HtmlSelectElement>().ok())
        else {
            return;
        };
        let index = select.selected_index().max(0) as usize;
        state.theme_index.set(index);
        apply_theme(index);
        save_theme(index);
    };

    let status_class = move || {
        if state.hearsay_connected.get() {
            "bar-status connected"
        } else {
            "bar-status disconnected"
        }
    };

    view! {
        <div class="top-bar">
            <Show when=move || state.is_primary fallback=|| view! { <span class="bar-label">"Window"</span> }>
                <div class="bar-menu">
                    <button
                        class="bar-button"
                        on:click=move |_| project_menu_open.update(|open| *open = !*open)
                    >
                        "Project"
                    </button>
                    <Show when=move || project_menu_open.get() fallback=|| ()>
                        <div class="bar-menu-popup">
                            <button on:click=on_new_project>"New Project"</button>
                            <button on:click=on_load_project>"Load Project..."</button>
                            <div class="bar-menu-divider"></div>
                            <button on:click=on_save_project>"Save Project"</button>
                            <button on:click=on_save_project_as>"Save As Project..."</button>
                            <div class="bar-menu-divider"></div>
                            <button on:click=on_set_startup>"Set as Startup Project"</button>
                            <button on:click=on_unset_startup>"Unset Startup Project"</button>
                            <div class="bar-menu-divider"></div>
                            <For
                                each=move || state.recents.get()
                                key=|name| name.clone()
                                children=move |name| {
                                    let load_name = name.clone();
                                    let display = move || {
                                        if state.startup_project.get().as_deref()
                                            == Some(load_name.as_str())
                                        {
                                            format!("Open: {load_name} (startup)")
                                        } else {
                                            format!("Open: {load_name}")
                                        }
                                    };
                                    let open_name = name.clone();
                                    view! {
                                        <button on:click=move |_| {
                                            project_menu_open.set(false);
                                            match shell::stored_project(&open_name) {
                                                Some(json) => {
                                                    shell::load_project_json(state, link, &json)
                                                }
                                                None => state.push_toast(
                                                    "Stored project is missing",
                                                    ToastKind::Error,
                                                    4000,
                                                ),
                                            }
                                        }>{display}</button>
                                    }
                                }
                            />
                            <Show when=move || !state.recents.get().is_empty() fallback=|| ()>
                                <div class="bar-menu-divider"></div>
                                <button on:click=on_clear_recents>"Clear Recent Projects"</button>
                            </Show>
                        </div>
                    </Show>
                </div>
                <input
                    class="bar-input"
                    type="text"
                    prop:value=move || state.project_name.get()
                    on:change=on_project_name
                />
                <span class="bar-modified">
                    {move || if state.project_modified.get() { "*" } else { "" }}
                </span>
            </Show>

            <div class="bar-menu">
                <button
                    class="bar-button"
                    on:click=move |_| layout_menu_open.update(|open| *open = !*open)
                >
                    "Layout"
                </button>
                <Show when=move || layout_menu_open.get() fallback=|| ()>
                    <div class="bar-menu-popup">
                        <button on:click=on_save_layout>"Save Layout..."</button>
                        <button on:click=on_load_layout>"Load Layout..."</button>
                        <div class="bar-menu-divider"></div>
                        <button on:click=on_reset_layout>"Reset Layout"</button>
                    </div>
                </Show>
            </div>
            <input
                class="bar-input"
                type="text"
                prop:value=move || state.layout_name.get()
                on:change=on_layout_name
            />
            <span class="bar-modified">
                {move || if state.layout_modified.get() { "*" } else { "" }}
            </span>

            <button class="bar-button" on:click=on_new_window>"New Window"</button>
            <button class="bar-button" on:click=on_toggle_api>"Api"</button>
            <select class="bar-select" on:change=on_theme>
                {THEMES
                    .iter()
                    .enumerate()
                    .map(|(index, theme)| {
                        view! {
                            <option
                                value=index.to_string()
                                selected=move || state.theme_index.get() == index
                            >
                                {theme.name}
                            </option>
                        }
                    })
                    .collect_view()}
            </select>

            <div class="bar-spacer"></div>

            <span class="bar-label">{move || format!("{:.0} FPS", state.fps.get())}</span>
            <span class=status_class>
                {move || {
                    if state.hearsay_connected.get() {
                        "127.0.0.1:9613".to_string()
                    } else {
                        "disconnected".to_string()
                    }
                }}
            </span>
            <span class="bar-label">{if state.is_primary { "Primary" } else { "Window" }}</span>

            <input
                node_ref=project_file_input
                type="file"
                accept=".json"
                class="hidden-input"
                on:change=on_project_file
            />
            <input
                node_ref=layout_file_input
                type="file"
                accept=".json"
                class="hidden-input"
                on:change=on_layout_file
            />
        </div>
    }
}
