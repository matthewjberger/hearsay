use crate::api_panel::{ApiPanel, build_api_panel, template_json};
use crate::broker::{
    BrokerConnectionStatus, BrokerLink, RuntimeEvent, SubscriptionRegistry, WindowRole,
    process_broker_service_message, start_broker_runtime,
};
use crate::chrome::{
    Chrome, LAYOUT_MENU_LOAD, LAYOUT_MENU_RESET, LAYOUT_MENU_SAVE, PROJECT_MENU_CLEAR_RECENT,
    PROJECT_MENU_LOAD, PROJECT_MENU_NEW, PROJECT_MENU_RECENT_BASE, PROJECT_MENU_SAVE,
    PROJECT_MENU_SAVE_AS, PROJECT_MENU_SET_STARTUP, PROJECT_MENU_UNSET_STARTUP,
    VIEW_MENU_TOGGLE_API, build_chrome, open_menu_at_button, rebuild_project_menu,
};
use crate::filesystem::FilesystemService;
use crate::messages::Message;
use crate::messages::*;
use crate::modal_service::ModalService;
use crate::settings::UserSettings;
use crate::shell::*;
use crate::themes;
use crate::tiles::{TilesState, WIDGET_KINDS, build_tile_area};
use nightshade::prelude::*;
use std::collections::VecDeque;

pub struct ProjectState {
    pub project_name: String,
    pub is_modified: bool,
    pub project_file_path: Option<String>,
}

impl Default for ProjectState {
    fn default() -> Self {
        Self {
            project_name: "Untitled Project".to_string(),
            is_modified: false,
            project_file_path: None,
        }
    }
}

pub struct LayoutState {
    pub is_loaded: bool,
    pub layout_name: String,
    pub is_modified: bool,
}

impl Default for LayoutState {
    fn default() -> Self {
        Self {
            is_loaded: false,
            layout_name: "Default Layout".to_string(),
            is_modified: false,
        }
    }
}

pub struct Core {
    pub role: WindowRole,
    pub settings: UserSettings,
    pub link: BrokerLink,
    pub registry: SubscriptionRegistry,
    pub status: BrokerConnectionStatus,
    pub filesystem: FilesystemService,
    pub window_registry: WindowRegistry,
    pub collection: ProjectCollection,
    pub shell_initialized: bool,
    pub bus: VecDeque<Message>,
    pub project: ProjectState,
    pub layout: LayoutState,
    pub modals: ModalService,
}

pub struct UiState {
    pub chrome: Chrome,
    pub api: ApiPanel,
    pub tiles: TilesState,
    pub palette: Entity,
    pub rendered_fps: u32,
    pub rendered_project_label: String,
    pub rendered_layout_label: String,
}

pub struct Demo {
    core: Core,
    ui: Option<UiState>,
}

impl Demo {
    pub fn new() -> Self {
        let role = WindowRole::detect();
        let settings = UserSettings::load();
        let link = start_broker_runtime(role.clone());
        Self {
            core: Core {
                role,
                settings,
                link,
                registry: SubscriptionRegistry::default(),
                status: BrokerConnectionStatus::default(),
                filesystem: FilesystemService::default(),
                window_registry: WindowRegistry::default(),
                collection: ProjectCollection::default(),
                shell_initialized: false,
                bus: VecDeque::new(),
                project: ProjectState::default(),
                layout: LayoutState::default(),
                modals: ModalService::default(),
            },
            ui: None,
        }
    }
}

impl State for Demo {
    fn initialize(&mut self, world: &mut World) {
        world.resources.window.title = "Hearsay Demo".to_string();
        world.resources.retained_ui.enabled = true;
        world.resources.render_settings.vsync_enabled = false;
        world.resources.render_settings.render_world_to_swapchain = false;
        world.resources.render_settings.clear_color = [0.0, 0.0, 0.0, 1.0];
        world.resources.retained_ui.background_color = None;

        let camera = spawn_ortho_camera(world, vec2(0.0, 0.0));
        world.resources.active_camera = Some(camera);

        themes::install_themes(world, self.core.settings.theme_name.as_deref());

        let is_primary = self.core.role.is_primary();
        let mut tree = UiTreeBuilder::new(world);
        let root = tree
            .add_node()
            .boundary(Rl(vec2(0.0, 0.0)), Rl(vec2(100.0, 100.0)))
            .fg(ThemeColor::Background)
            .entity();

        let mut chrome = None;
        let mut tiles = None;
        let mut api = None;
        let mut palette = Entity::default();
        tree.in_parent(root, |tree| {
            chrome = Some(build_chrome(tree, is_primary));
            tiles = Some(build_tile_area(tree));
            api = Some(build_api_panel(tree));
            palette = tree.add_command_palette(8);
            for kind in WIDGET_KINDS {
                ui_command_palette_register(tree.world_mut(), palette, kind, "", "Widgets");
            }
        });
        tree.finish();

        let mut chrome = chrome.expect("chrome built");
        let mut tiles = tiles.expect("tiles built");
        if is_primary {
            rebuild_project_menu(world, &mut chrome, &self.core.settings);
        }
        tiles.refresh_snapshot(world);

        self.ui = Some(UiState {
            chrome,
            api: api.expect("api built"),
            tiles,
            palette,
            rendered_fps: u32::MAX,
            rendered_project_label: String::new(),
            rendered_layout_label: String::new(),
        });

        if is_primary {
            load_startup_project(&mut self.core);
        }
    }

    fn run_systems(&mut self, world: &mut World) {
        let Some(ui) = self.ui.as_mut() else {
            return;
        };
        frame(world, &mut self.core, ui);
    }
}

fn load_startup_project(core: &mut Core) {
    let argument_path = std::env::args().nth(1);
    let settings_path = core.settings.default_project_path.clone();
    let Some(project_path) = argument_path.or(settings_path) else {
        return;
    };

    if !std::path::Path::new(&project_path).exists() {
        notify(
            core,
            &format!("Startup project file not found: {project_path}"),
            NotificationKind::Warning,
            5.0,
        );
        return;
    }

    match std::fs::read_to_string(&project_path) {
        Ok(json) => match serde_json::from_str::<ProjectSaveFile>(&json) {
            Ok(save_file) => {
                let trees: Vec<TileLayout> = save_file
                    .windows
                    .iter()
                    .map(|window_tree| window_tree.layout.clone())
                    .collect();
                let layout_names: Vec<String> = save_file
                    .windows
                    .iter()
                    .map(|window_tree| window_tree.layout_name.clone())
                    .collect();
                core.bus.push_back(Message::Project {
                    message: ProjectMessage::ProjectLoaded {
                        trees,
                        project_name: save_file.project_name,
                        layout_names,
                        path: project_path,
                    },
                });
            }
            Err(error) => {
                notify(
                    core,
                    &format!("Failed to parse startup project file: {error}"),
                    NotificationKind::Error,
                    5.0,
                );
            }
        },
        Err(error) => {
            notify(
                core,
                &format!("Failed to read startup project file: {error}"),
                NotificationKind::Error,
                5.0,
            );
        }
    }
}

fn notify(core: &mut Core, text: &str, kind: NotificationKind, duration: f64) {
    core.bus.push_back(Message::Notify {
        message: NotificationServiceMessage::Show {
            text: text.to_string(),
            kind,
            duration_in_seconds: duration,
        },
    });
}

fn publish_payload(
    bus: &mut VecDeque<Message>,
    topic: &str,
    payload_json: Result<String, serde_json::Error>,
) {
    let Ok(payload_json) = payload_json else {
        return;
    };
    bus.push_back(Message::Broker {
        message: BrokerServiceMessage::Publish {
            topic: topic.to_string(),
            message: payload_json,
        },
    });
}

fn frame(world: &mut World, core: &mut Core, ui: &mut UiState) {
    let theme_background = world
        .resources
        .retained_ui
        .theme_state
        .active_theme()
        .background_color;
    world.resources.retained_ui.background_color = Some(theme_background);

    for event in core.link.drain_events() {
        match event {
            RuntimeEvent::Connected { client_id, address } => {
                core.status.connected = true;
                core.status.address = address;
                core.status.client_id = client_id;
                core.bus
                    .push_back(Message::ConnectionStatus { connected: true });
            }
            RuntimeEvent::Disconnected => {
                core.status.connected = false;
                core.bus
                    .push_back(Message::ConnectionStatus { connected: false });
                if !core.role.is_primary() {
                    world.resources.window.should_exit = true;
                }
            }
            RuntimeEvent::Failed { reason } => {
                core.status.connected = false;
                notify(
                    core,
                    &format!("Broker runtime failed: {reason}"),
                    NotificationKind::Error,
                    8.0,
                );
                if !core.role.is_primary() {
                    world.resources.window.should_exit = true;
                }
            }
            RuntimeEvent::Inbound {
                topic,
                payload,
                bytes,
            } => {
                core.bus.push_back(Message::Topic {
                    topic,
                    payload,
                    bytes,
                });
            }
        }
    }

    initialize_shell_session(core);

    for result in core.filesystem.poll_results() {
        core.bus.push_back(Message::Filesystem {
            message: FileSystemMessage::Result(result),
        });
    }

    let events: Vec<UiEvent> = ui_events(world).to_vec();
    for event in &events {
        handle_ui_event(world, core, ui, event);
    }

    let delta_time = world.resources.window.timing.delta_time;
    tick_project_collection(world, core, ui, delta_time);

    process_bus(world, core, ui);

    let connected = core.status.connected;
    for index in 0..ui.tiles.widgets.len() {
        ui.tiles.widgets[index].update(world, connected);
    }
    let mut outgoing = Vec::new();
    for widget in &mut ui.tiles.widgets {
        outgoing.extend(widget.drain_messages());
    }
    core.bus.extend(outgoing);
    process_bus(world, core, ui);

    ui.tiles.update_rect(world);

    if ui.tiles.detect_layout_change(world) {
        core.layout.is_modified = true;
        core.layout.is_loaded = true;
        core.project.is_modified = true;
    }

    refresh_chrome(world, core, ui);
}

fn initialize_shell_session(core: &mut Core) {
    if !core.status.connected {
        core.shell_initialized = false;
        return;
    }
    if core.shell_initialized {
        return;
    }
    core.shell_initialized = true;

    let window_id = core.status.client_id.clone();
    let topics = if core.role.is_primary() {
        vec![
            ShellContract::announce_topic(),
            ShellContract::report_tree_topic(),
            ShellContract::request_spawn_topic(),
        ]
    } else {
        vec![
            ShellContract::assign_layout_topic(&window_id),
            ShellContract::close_topic(&window_id),
            ShellContract::request_trees_topic(),
        ]
    };
    core.bus.push_back(Message::Broker {
        message: BrokerServiceMessage::Subscribe {
            topics,
            widget_id: SHELL_SERVICE_ID.to_string(),
        },
    });

    if !core.role.is_primary() {
        let (topic, mut payload) = ShellContract::announce();
        payload.window_id = window_id;
        publish_payload(&mut core.bus, &topic, payload.to_json());
    }
}

fn process_bus(world: &mut World, core: &mut Core, ui: &mut UiState) {
    let mut safety = 0;
    while let Some(message) = core.bus.pop_front() {
        route_message(world, core, ui, &message);
        safety += 1;
        if safety > 4096 {
            break;
        }
    }
}

fn deliver_to_widgets(ui: &mut UiState, message: &Message) {
    for widget in &mut ui.tiles.widgets {
        widget.receive_message(message);
    }
}

fn route_message(world: &mut World, core: &mut Core, ui: &mut UiState, message: &Message) {
    match message {
        Message::Broker { message } => {
            process_broker_service_message(message, &core.link, &mut core.registry);
        }
        Message::Filesystem { message } => match message {
            FileSystemMessage::Command(command) => {
                core.filesystem.execute(command);
            }
            FileSystemMessage::Result(result) => {
                deliver_to_widgets(
                    ui,
                    &Message::Filesystem {
                        message: FileSystemMessage::Result(result.clone()),
                    },
                );
                core.bus.push_back(Message::Tiles {
                    message: TileTreeMessage::ProcessFileResult(result.clone()),
                });
            }
            FileSystemMessage::Empty => {}
        },
        Message::Modal { message } => match message {
            ModalServiceMessage::ShowConfirm {
                id,
                title,
                body,
                confirm_text,
                cancel_text,
            } => {
                core.modals.show_confirm(
                    world,
                    id,
                    title,
                    body,
                    confirm_text.as_deref(),
                    cancel_text.as_deref(),
                );
            }
            ModalServiceMessage::CloseModal(id) => {
                core.modals.close_modal(world, id);
            }
            ModalServiceMessage::ModalResult { .. } => {
                deliver_to_widgets(
                    ui,
                    &Message::Modal {
                        message: message.clone(),
                    },
                );
            }
            ModalServiceMessage::Empty => {}
        },
        Message::Notify { message } => {
            if let NotificationServiceMessage::Show {
                text,
                kind,
                duration_in_seconds,
            } = message
            {
                let severity = match kind {
                    NotificationKind::Info | NotificationKind::Empty => ToastSeverity::Info,
                    NotificationKind::Warning => ToastSeverity::Warning,
                    NotificationKind::Error => ToastSeverity::Error,
                    NotificationKind::Success => ToastSeverity::Success,
                };
                let duration = if *duration_in_seconds < 0.01 {
                    3600.0
                } else {
                    *duration_in_seconds as f32
                };
                ui_show_toast(world, text, severity, duration);
            }
        }
        Message::Project { message } => {
            handle_project_message(world, core, ui, message);
        }
        Message::Tiles { message } => {
            if let TileTreeMessage::ProcessFileResult(result) = message {
                handle_file_result(core, ui, world, result);
            }
        }
        Message::Topic {
            topic,
            payload,
            bytes,
        } => {
            handle_topic(world, core, ui, topic, payload, bytes.clone());
        }
        Message::ConnectionStatus { .. } => {
            deliver_to_widgets(ui, message);
        }
        Message::Empty => {}
    }
}

fn handle_topic(
    world: &mut World,
    core: &mut Core,
    ui: &mut UiState,
    topic: &str,
    payload: &str,
    bytes: Option<Vec<u8>>,
) {
    let window_id = core.status.client_id.clone();
    if core.role.is_primary() {
        if topic == ShellContract::announce_topic() {
            if let Ok(announce) = AnnouncePayload::from_json(payload) {
                if !core
                    .window_registry
                    .known_windows
                    .contains(&announce.window_id)
                {
                    core.window_registry
                        .known_windows
                        .push(announce.window_id.clone());
                }
                if let Some((layout_name, tree_json)) =
                    core.window_registry.pending_layouts.pop_front()
                {
                    let (assign_topic, mut assignment) =
                        ShellContract::assign_layout(&announce.window_id);
                    assignment.layout_name = layout_name;
                    assignment.tree_json = tree_json;
                    publish_payload(&mut core.bus, &assign_topic, assignment.to_json());
                }
            }
        } else if topic == ShellContract::report_tree_topic() {
            if let Ok(report) = ReportTreePayload::from_json(payload)
                && let Some(collection) = core.collection.active.as_mut()
            {
                collection.collected.push(ReportedTree {
                    window_id: report.window_id,
                    layout_name: report.layout_name,
                    tree_json: report.tree_json,
                });
            }
        } else if topic == ShellContract::request_spawn_topic() {
            core.bus.push_back(Message::Broker {
                message: BrokerServiceMessage::SpawnWindow,
            });
        }
    } else if topic == ShellContract::assign_layout_topic(&window_id) {
        if let Ok(assignment) = AssignLayoutPayload::from_json(payload) {
            match serde_json::from_str::<TileLayout>(&assignment.tree_json) {
                Ok(layout) => {
                    ui.tiles.apply_layout(world, &layout, &mut core.bus);
                    core.layout.layout_name = assignment.layout_name;
                    core.layout.is_loaded = true;
                    core.layout.is_modified = false;
                }
                Err(error) => {
                    notify(
                        core,
                        &format!("Failed to parse assigned layout: {error}"),
                        NotificationKind::Error,
                        5.0,
                    );
                }
            }
        }
    } else if topic == ShellContract::close_topic(&window_id) {
        world.resources.window.should_exit = true;
    } else if topic == ShellContract::request_trees_topic() {
        let Some(layout) = ui.tiles.current_layout(world) else {
            return;
        };
        let Ok(tree_json) = serde_json::to_string(&layout) else {
            return;
        };
        let (report_topic, mut payload_value) = ShellContract::report_tree();
        payload_value.window_id = window_id;
        payload_value.layout_name = core.layout.layout_name.clone();
        payload_value.tree_json = tree_json;
        publish_payload(&mut core.bus, &report_topic, payload_value.to_json());
    }

    if let Some(subscribers) = core.registry.topic_subscribers.get(topic) {
        let delivery = Message::Topic {
            topic: topic.to_string(),
            payload: payload.to_string(),
            bytes,
        };
        for widget in &mut ui.tiles.widgets {
            if subscribers
                .iter()
                .any(|subscriber| subscriber == widget.rpc.widget_id())
            {
                widget.receive_message(&delivery);
            }
        }
    }
}

fn handle_ui_event(world: &mut World, core: &mut Core, ui: &mut UiState, event: &UiEvent) {
    match event {
        UiEvent::ButtonClicked(entity) => {
            let entity = *entity;
            if entity == ui.chrome.project_button {
                open_menu_at_button(world, ui.chrome.project_button, ui.chrome.project_menu);
            } else if entity == ui.chrome.layout_button {
                open_menu_at_button(world, ui.chrome.layout_button, ui.chrome.layout_menu);
            } else if entity == ui.chrome.view_button {
                open_menu_at_button(world, ui.chrome.view_button, ui.chrome.view_menu);
            } else if entity == ui.chrome.project_edit_button {
                let initial = if core.project.project_name.is_empty() {
                    "Untitled Project".to_string()
                } else {
                    core.project.project_name.clone()
                };
                ui.chrome.set_project_editing(world, true, &initial);
            } else if entity == ui.chrome.project_edit_ok {
                commit_project_name(world, core, ui);
            } else if entity == ui.chrome.project_edit_cancel {
                ui.chrome.set_project_editing(world, false, "");
            } else if entity == ui.chrome.layout_edit_button {
                let initial = if core.layout.layout_name.is_empty() {
                    "Untitled Layout".to_string()
                } else {
                    core.layout.layout_name.clone()
                };
                ui.chrome.set_layout_editing(world, true, &initial);
            } else if entity == ui.chrome.layout_edit_ok {
                commit_layout_name(world, core, ui);
            } else if entity == ui.chrome.layout_edit_cancel {
                ui.chrome.set_layout_editing(world, false, "");
            } else if entity == ui.chrome.new_window_button {
                if core.role.is_primary() {
                    core.bus.push_back(Message::Broker {
                        message: BrokerServiceMessage::SpawnWindow,
                    });
                } else {
                    let (topic, payload) = ShellContract::request_spawn();
                    publish_payload(&mut core.bus, &topic, payload.to_json());
                }
            } else if entity == ui.chrome.add_panel_button {
                ui_show_command_palette(world, ui.palette);
            } else if entity == ui.api.send_button {
                send_api_message(world, core, ui);
            } else {
                for widget in &mut ui.tiles.widgets {
                    if widget.handle_ui_event(event) {
                        break;
                    }
                }
            }
        }
        UiEvent::TextInputSubmitted { entity, .. } => {
            if *entity == ui.chrome.project_edit_input {
                commit_project_name(world, core, ui);
            } else if *entity == ui.chrome.layout_edit_input {
                commit_layout_name(world, core, ui);
            }
        }
        UiEvent::TextInputChanged { .. } => {
            for widget in &mut ui.tiles.widgets {
                if widget.handle_ui_event(event) {
                    break;
                }
            }
        }
        UiEvent::ContextMenuItemClicked { entity, tag, .. } => {
            if *entity == ui.chrome.project_menu {
                handle_project_menu_action(world, core, ui, *tag);
            } else if *entity == ui.chrome.layout_menu {
                handle_layout_menu_action(world, core, ui, *tag);
            } else if *entity == ui.chrome.view_menu && *tag == VIEW_MENU_TOGGLE_API {
                ui.api.visible = !ui.api.visible;
                ui_set_visible(world, ui.api.panel, ui.api.visible);
            }
        }
        UiEvent::DropdownChanged {
            entity,
            selected_index,
        } => {
            if *entity == ui.chrome.theme_dropdown {
                let theme_name = world
                    .resources
                    .retained_ui
                    .theme_state
                    .current_theme
                    .name
                    .clone();
                core.settings.theme_name = Some(theme_name);
                if let Err(error) = core.settings.save() {
                    notify(
                        core,
                        &format!("Failed to save theme setting: {error}"),
                        NotificationKind::Error,
                        5.0,
                    );
                }
            } else if *entity == ui.api.dropdown {
                ui_text_area_set_value(world, ui.api.text_area, &template_json(*selected_index));
            }
        }
        UiEvent::CommandPaletteExecuted {
            entity,
            command_index,
        } if *entity == ui.palette && WIDGET_KINDS.get(*command_index).is_some() => {
            ui.tiles.add_template_pane(world);
            core.layout.is_modified = true;
            core.layout.is_loaded = true;
            core.project.is_modified = true;
            ui.tiles.refresh_snapshot(world);
        }
        UiEvent::ModalClosed { entity, confirmed } => {
            if let Some(id) = core.modals.handle_modal_closed(world, *entity, *confirmed) {
                core.bus.push_back(Message::Modal {
                    message: ModalServiceMessage::ModalResult {
                        id,
                        confirmed: *confirmed,
                    },
                });
            }
        }
        UiEvent::TileTabClosed {
            container, pane_id, ..
        } if *container == ui.tiles.container => {
            ui.tiles.handle_tab_closed(world, *pane_id, &mut core.bus);
            core.layout.is_modified = true;
            core.layout.is_loaded = true;
            core.project.is_modified = true;
            ui.tiles.refresh_snapshot(world);
        }
        _ => {}
    }
}

fn commit_project_name(world: &mut World, core: &mut Core, ui: &mut UiState) {
    let text = world
        .ui
        .get_ui_text_input(ui.chrome.project_edit_input)
        .map(|data| data.text.clone())
        .unwrap_or_default();
    if !text.is_empty() {
        core.project.project_name = text;
        core.project.is_modified = true;
    }
    ui.chrome.set_project_editing(world, false, "");
}

fn commit_layout_name(world: &mut World, core: &mut Core, ui: &mut UiState) {
    let text = world
        .ui
        .get_ui_text_input(ui.chrome.layout_edit_input)
        .map(|data| data.text.clone())
        .unwrap_or_default();
    if !text.is_empty() {
        core.layout.layout_name = text;
        core.layout.is_modified = true;
        core.project.is_modified = true;
    }
    ui.chrome.set_layout_editing(world, false, "");
}

fn send_api_message(world: &mut World, core: &mut Core, ui: &mut UiState) {
    let text = world
        .ui
        .get_ui_text_area(ui.api.text_area)
        .map(|data| data.text.clone())
        .unwrap_or_default();
    match serde_json::from_str::<Message>(&text) {
        Ok(message) => {
            core.bus.push_back(message);
            ui_set_text(world, ui.api.status_label, "Message sent");
        }
        Err(error) => {
            ui_set_text(world, ui.api.status_label, &format!("Parse error: {error}"));
        }
    }
}

fn handle_project_menu_action(world: &mut World, core: &mut Core, ui: &mut UiState, tag: u32) {
    match tag {
        PROJECT_MENU_NEW => {
            core.bus.push_back(Message::Project {
                message: ProjectMessage::CloseProject,
            });
        }
        PROJECT_MENU_LOAD => {
            core.bus.push_back(Message::Filesystem {
                message: FileSystemMessage::Command(FileSystemCommand::PickFile {
                    tag: TAG_PROJECT.to_string(),
                    filter_name: "project".to_string(),
                    extensions: vec!["project.json".to_string()],
                }),
            });
        }
        PROJECT_MENU_SAVE_AS => {
            begin_project_save(world, core, ui, CollectionDestination::Dialog);
        }
        PROJECT_MENU_SAVE => {
            if let Some(path) = core.project.project_file_path.clone() {
                begin_project_save(world, core, ui, CollectionDestination::Path(path));
            } else {
                begin_project_save(world, core, ui, CollectionDestination::Dialog);
            }
        }
        PROJECT_MENU_SET_STARTUP => {
            if let Some(path) = core.project.project_file_path.clone() {
                core.settings.default_project_path = Some(path.clone());
                core.settings.add_recent_project(path);
                if let Err(error) = core.settings.save() {
                    notify(
                        core,
                        &format!("Failed to save settings: {error}"),
                        NotificationKind::Error,
                        5.0,
                    );
                }
            } else {
                notify(
                    core,
                    "Save the project first to set it as the startup project",
                    NotificationKind::Warning,
                    4.0,
                );
            }
        }
        PROJECT_MENU_UNSET_STARTUP => {
            core.settings.default_project_path = None;
            if let Err(error) = core.settings.save() {
                notify(
                    core,
                    &format!("Failed to save settings: {error}"),
                    NotificationKind::Error,
                    5.0,
                );
            }
        }
        PROJECT_MENU_CLEAR_RECENT => {
            core.settings.clear_recent_projects();
            if let Err(error) = core.settings.save() {
                notify(
                    core,
                    &format!("Failed to save settings: {error}"),
                    NotificationKind::Error,
                    5.0,
                );
            }
        }
        recent if recent >= PROJECT_MENU_RECENT_BASE => {
            let index = (recent - PROJECT_MENU_RECENT_BASE) as usize;
            let Some(path) = ui.chrome.menu_recent_paths.get(index).cloned() else {
                return;
            };
            match std::fs::read(&path) {
                Ok(bytes) => {
                    core.bus.push_back(Message::Filesystem {
                        message: FileSystemMessage::Result(FileSystemResult::Success(
                            FileSystemSuccess::File {
                                path,
                                bytes,
                                tag: TAG_PROJECT.to_string(),
                            },
                        )),
                    });
                }
                Err(error) => {
                    notify(
                        core,
                        &format!("Failed to read recent project file: {error}"),
                        NotificationKind::Error,
                        5.0,
                    );
                }
            }
        }
        _ => {}
    }
}

fn handle_layout_menu_action(world: &mut World, core: &mut Core, ui: &mut UiState, tag: u32) {
    match tag {
        LAYOUT_MENU_SAVE => {
            let layout_name = if core.layout.layout_name.is_empty() {
                "Untitled Layout".to_string()
            } else {
                core.layout.layout_name.clone()
            };
            let Some(layout) = ui.tiles.current_layout(world) else {
                return;
            };
            let save_file = LayoutSaveFile {
                version: env!("CARGO_PKG_VERSION").to_string(),
                layout,
                layout_name: Some(layout_name),
            };
            match serde_json::to_string_pretty(&save_file) {
                Ok(json) => {
                    core.bus.push_back(Message::Filesystem {
                        message: FileSystemMessage::Command(FileSystemCommand::SaveFile {
                            tag: TAG_LAYOUT_SAVE.to_string(),
                            bytes: json.into_bytes(),
                            filter_name: "layout".to_string(),
                            extensions: vec!["layout.json".to_string()],
                        }),
                    });
                }
                Err(error) => {
                    notify(
                        core,
                        &format!("Failed to serialize layout: {error}"),
                        NotificationKind::Error,
                        5.0,
                    );
                }
            }
        }
        LAYOUT_MENU_LOAD => {
            core.bus.push_back(Message::Filesystem {
                message: FileSystemMessage::Command(FileSystemCommand::PickFile {
                    tag: TAG_LAYOUT.to_string(),
                    filter_name: "layout".to_string(),
                    extensions: vec!["layout.json".to_string()],
                }),
            });
        }
        LAYOUT_MENU_RESET => {
            ui.tiles.reset(world, &mut core.bus);
            core.layout.layout_name = "Default Layout".to_string();
            core.layout.is_loaded = true;
            core.layout.is_modified = false;
            core.project.is_modified = true;
        }
        _ => {}
    }
}

fn handle_file_result(
    core: &mut Core,
    ui: &mut UiState,
    world: &mut World,
    result: &FileSystemResult,
) {
    let _ = world;
    let _ = ui;
    let FileSystemResult::Success(FileSystemSuccess::File { path, bytes, tag }) = result else {
        return;
    };
    match tag.as_str() {
        TAG_PROJECT => {
            let Ok(json) = std::str::from_utf8(bytes) else {
                return;
            };
            match serde_json::from_str::<ProjectSaveFile>(json) {
                Ok(save_file) => {
                    let trees = save_file
                        .windows
                        .iter()
                        .map(|window_tree| window_tree.layout.clone())
                        .collect();
                    let layout_names = save_file
                        .windows
                        .iter()
                        .map(|window_tree| window_tree.layout_name.clone())
                        .collect::<Vec<_>>();
                    core.bus.push_back(Message::Project {
                        message: ProjectMessage::ProjectLoaded {
                            trees,
                            project_name: save_file.project_name,
                            layout_names,
                            path: path.clone(),
                        },
                    });
                }
                Err(error) => {
                    notify(
                        core,
                        &format!("Failed to parse project file: {error}"),
                        NotificationKind::Error,
                        5.0,
                    );
                }
            }
        }
        TAG_PROJECT_SAVE => {
            core.bus.push_back(Message::Project {
                message: ProjectMessage::ProjectSaved { path: path.clone() },
            });
        }
        TAG_LAYOUT => {
            let Ok(json) = std::str::from_utf8(bytes) else {
                return;
            };
            match serde_json::from_str::<LayoutSaveFile>(json) {
                Ok(save_file) => {
                    core.bus.push_back(Message::Project {
                        message: ProjectMessage::LayoutLoaded {
                            layout: Box::new(save_file.layout),
                            layout_name: save_file.layout_name,
                        },
                    });
                }
                Err(error) => {
                    notify(
                        core,
                        &format!("Failed to parse layout file: {error}"),
                        NotificationKind::Error,
                        5.0,
                    );
                }
            }
        }
        TAG_LAYOUT_SAVE => {
            core.layout.is_modified = false;
            notify(
                core,
                &format!("Layout saved\n{path}"),
                NotificationKind::Success,
                3.0,
            );
        }
        _ => {}
    }
}

fn close_child_windows(core: &mut Core) {
    let window_ids: Vec<String> = core.window_registry.known_windows.drain(..).collect();
    for window_id in window_ids {
        let (topic, payload) = ShellContract::close(&window_id);
        publish_payload(&mut core.bus, &topic, payload.to_json());
    }
    core.window_registry.pending_layouts.clear();
}

fn handle_project_message(
    world: &mut World,
    core: &mut Core,
    ui: &mut UiState,
    message: &ProjectMessage,
) {
    match message {
        ProjectMessage::ProjectLoaded {
            trees,
            project_name,
            layout_names,
            path,
        } => {
            if !core.role.is_primary() {
                return;
            }
            let Some(first_tree) = trees.first() else {
                return;
            };

            close_child_windows(core);
            ui.tiles.apply_layout(world, first_tree, &mut core.bus);

            core.layout.layout_name = layout_names
                .first()
                .cloned()
                .unwrap_or_else(|| "Default Layout".to_string());
            core.layout.is_loaded = true;
            core.layout.is_modified = false;

            for (window_index, tree) in trees.iter().enumerate().skip(1) {
                let Ok(tree_json) = serde_json::to_string(tree) else {
                    continue;
                };
                let layout_name = layout_names
                    .get(window_index)
                    .cloned()
                    .unwrap_or_else(|| "Default Layout".to_string());
                core.window_registry
                    .pending_layouts
                    .push_back((layout_name, tree_json));
                core.bus.push_back(Message::Broker {
                    message: BrokerServiceMessage::SpawnWindow,
                });
            }

            let derived_project_name = if let Some(name) = project_name
                && !name.trim().is_empty()
            {
                name.clone()
            } else {
                std::path::PathBuf::from(path)
                    .file_stem()
                    .and_then(|file_stem| file_stem.to_str())
                    .unwrap_or("Untitled Project")
                    .to_string()
            };
            core.project.project_name = derived_project_name;
            core.project.project_file_path = Some(path.clone());
            core.project.is_modified = false;

            core.settings.add_recent_project(path.clone());
            if let Err(error) = core.settings.save() {
                notify(
                    core,
                    &format!("Failed to save recent projects: {error}"),
                    NotificationKind::Error,
                    5.0,
                );
            }

            notify(
                core,
                &format!("Opened project: {}", core.project.project_name),
                NotificationKind::Success,
                3.0,
            );
        }
        ProjectMessage::ProjectSaved { path } => {
            core.project.is_modified = false;
            core.project.project_file_path = Some(path.clone());
            core.layout.is_modified = false;

            core.settings.add_recent_project(path.clone());
            if let Err(error) = core.settings.save() {
                notify(
                    core,
                    &format!("Failed to save recent projects: {error}"),
                    NotificationKind::Error,
                    5.0,
                );
            }

            notify(
                core,
                &format!("Project saved\n{path}"),
                NotificationKind::Success,
                3.0,
            );
        }
        ProjectMessage::LayoutLoaded {
            layout,
            layout_name,
        } => {
            ui.tiles.apply_layout(world, layout, &mut core.bus);
            if let Some(layout_name) = layout_name {
                core.layout.layout_name = layout_name.clone();
            }
            core.layout.is_loaded = true;
            core.layout.is_modified = false;
        }
        ProjectMessage::CloseProject => {
            if !core.role.is_primary() {
                return;
            }
            close_child_windows(core);
            ui.tiles.reset(world, &mut core.bus);
            core.layout.layout_name = "Default Layout".to_string();
            core.layout.is_loaded = false;
            core.layout.is_modified = false;
            core.project = ProjectState::default();

            notify(core, "Project closed", NotificationKind::Info, 3.0);
        }
        ProjectMessage::Empty => {}
    }
}

fn begin_project_save(
    world: &mut World,
    core: &mut Core,
    ui: &mut UiState,
    destination: CollectionDestination,
) {
    if core.window_registry.known_windows.is_empty() {
        let Some(layout) = ui.tiles.current_layout(world) else {
            return;
        };
        let save_file = ProjectSaveFile {
            version: env!("CARGO_PKG_VERSION").to_string(),
            windows: vec![WindowTree {
                layout,
                layout_name: core.layout.layout_name.clone(),
            }],
            project_name: Some(core.project.project_name.clone()),
        };
        finalize_project_save(core, save_file, destination);
    } else {
        let (topic, payload) = ShellContract::request_trees();
        publish_payload(&mut core.bus, &topic, payload.to_json());
        core.collection.active = Some(CollectionState {
            remaining_seconds: 1.0,
            collected: Vec::new(),
            destination,
        });
    }
}

fn finalize_project_save(
    core: &mut Core,
    save_file: ProjectSaveFile,
    destination: CollectionDestination,
) {
    let json = match serde_json::to_string_pretty(&save_file) {
        Ok(json) => json,
        Err(error) => {
            notify(
                core,
                &format!("Failed to serialize project: {error}"),
                NotificationKind::Error,
                5.0,
            );
            return;
        }
    };
    match destination {
        CollectionDestination::Path(path) => match std::fs::write(&path, json) {
            Ok(_) => {
                core.project.is_modified = false;
                core.layout.is_modified = false;
                notify(
                    core,
                    &format!("Project saved\n{path}"),
                    NotificationKind::Success,
                    3.0,
                );
            }
            Err(error) => {
                notify(
                    core,
                    &format!("Failed to save project: {error}"),
                    NotificationKind::Error,
                    5.0,
                );
            }
        },
        CollectionDestination::Dialog => {
            core.bus.push_back(Message::Filesystem {
                message: FileSystemMessage::Command(FileSystemCommand::SaveFile {
                    tag: TAG_PROJECT_SAVE.to_string(),
                    bytes: json.into_bytes(),
                    filter_name: "project".to_string(),
                    extensions: vec!["project.json".to_string()],
                }),
            });
        }
    }
}

fn tick_project_collection(world: &mut World, core: &mut Core, ui: &mut UiState, delta_time: f32) {
    let (complete, timed_out) = match core.collection.active.as_mut() {
        Some(collection) => {
            collection.remaining_seconds -= delta_time;
            let all_reported =
                collection.collected.len() >= core.window_registry.known_windows.len();
            let timed_out = collection.remaining_seconds <= 0.0 && !all_reported;
            (all_reported || timed_out, timed_out)
        }
        None => (false, false),
    };
    if !complete {
        return;
    }
    let Some(collection) = core.collection.active.take() else {
        return;
    };

    if timed_out {
        core.window_registry.known_windows.retain(|window_id| {
            collection
                .collected
                .iter()
                .any(|reported| &reported.window_id == window_id)
        });
    }

    let Some(own_layout) = ui.tiles.current_layout(world) else {
        return;
    };
    let mut windows = vec![WindowTree {
        layout: own_layout,
        layout_name: core.layout.layout_name.clone(),
    }];
    for window_id in &core.window_registry.known_windows {
        let Some(reported) = collection
            .collected
            .iter()
            .find(|reported| &reported.window_id == window_id)
        else {
            continue;
        };
        let Ok(layout) = serde_json::from_str::<TileLayout>(&reported.tree_json) else {
            continue;
        };
        windows.push(WindowTree {
            layout,
            layout_name: reported.layout_name.clone(),
        });
    }

    let save_file = ProjectSaveFile {
        version: env!("CARGO_PKG_VERSION").to_string(),
        windows,
        project_name: Some(core.project.project_name.clone()),
    };
    finalize_project_save(core, save_file, collection.destination);
}

fn refresh_chrome(world: &mut World, core: &mut Core, ui: &mut UiState) {
    let fps = world.resources.window.timing.frames_per_second as u32;
    if fps != ui.rendered_fps {
        ui.rendered_fps = fps;
        ui_set_text(world, ui.chrome.fps_label, &format!("{fps} FPS"));
    }

    if ui.chrome.rendered_connected != Some(core.status.connected) {
        ui.chrome.rendered_connected = Some(core.status.connected);
        let theme = world
            .resources
            .retained_ui
            .theme_state
            .active_theme()
            .clone();
        let (text, color) = if core.status.connected {
            (core.status.address.clone(), theme.accent_color)
        } else {
            ("disconnected".to_string(), theme.error_color)
        };
        ui_set_text(world, ui.chrome.address_label, &text);
        if let Some(node_color) = world.ui.get_ui_node_color_mut(ui.chrome.address_label) {
            node_color.colors[UiBase::INDEX] = Some(color);
        }
    }

    let project_text = if core.project.project_name.is_empty() {
        "Untitled Project"
    } else {
        &core.project.project_name
    };
    let project_display = if core.project.is_modified {
        format!("{project_text} *")
    } else {
        project_text.to_string()
    };
    if project_display != ui.rendered_project_label {
        ui.rendered_project_label = project_display.clone();
        ui_set_text(world, ui.chrome.project_label, &project_display);
    }

    let layout_name = if core.layout.layout_name.is_empty() {
        "Default Layout"
    } else {
        &core.layout.layout_name
    };
    let layout_display = if core.layout.is_modified {
        format!("{layout_name} *")
    } else {
        layout_name.to_string()
    };
    if layout_display != ui.rendered_layout_label {
        ui.rendered_layout_label = layout_display.clone();
        ui_set_text(world, ui.chrome.layout_label, &layout_display);
    }

    if core.role.is_primary() {
        let signature = (
            core.settings.recent_projects.clone(),
            core.settings.default_project_path.clone(),
        );
        if signature != ui.chrome.menu_signature {
            rebuild_project_menu(world, &mut ui.chrome, &core.settings);
        }
    }

    let window_title = if core.role.is_primary() {
        if let Some(project_path) = &core.project.project_file_path {
            if core.project.is_modified {
                format!("Hearsay Demo - {project_path} (*)")
            } else {
                format!("Hearsay Demo - {project_path}")
            }
        } else {
            "Hearsay Demo".to_string()
        }
    } else {
        "Hearsay Demo - Window".to_string()
    };
    if world.resources.window.title != window_title {
        world.resources.window.title = window_title;
    }
}
