use crate::prelude::*;
use bevy::ecs::system::SystemParam;
use enum2contract::EnumContract;
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

#[derive(Resource, Default)]
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
    pub timer: Timer,
    pub collected: Vec<ReportedTree>,
    pub destination: CollectionDestination,
}

#[derive(Resource, Default)]
pub struct ProjectCollection {
    pub active: Option<CollectionState>,
}

#[derive(SystemParam)]
pub struct ShellState<'w> {
    pub role: Res<'w, WindowRole>,
    pub status: Res<'w, BrokerConnectionStatus>,
    pub registry: ResMut<'w, WindowRegistry>,
    pub collection: ResMut<'w, ProjectCollection>,
}

#[derive(SystemParam)]
pub struct ProjectUiState<'w> {
    pub project_state: ResMut<'w, ProjectState>,
    pub layout_state: ResMut<'w, LayoutState>,
    pub theme_state: ResMut<'w, ThemeState>,
    pub user_settings: ResMut<'w, UserSettings>,
}

pub struct ShellPlugin;

impl Plugin for ShellPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WindowRegistry>()
            .init_resource::<ProjectCollection>()
            .add_systems(
                Update,
                (
                    initialize_shell_session,
                    handle_shell_topics,
                    tick_project_collection,
                    handle_project_messages,
                ),
            );
    }
}

fn initialize_shell_session(
    shell: ShellState,
    mut message_bus_events: EventWriter<MessageBusEvent>,
    mut initialized: Local<bool>,
) {
    if !shell.status.connected {
        *initialized = false;
        return;
    }
    if *initialized {
        return;
    }
    *initialized = true;

    let window_id = shell.status.client_id.clone();
    let topics = if shell.role.is_primary() {
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
    message_bus_events.send(MessageBusEvent::RouteMessage(Message::Broker {
        message: BrokerServiceMessage::Subscribe {
            topics,
            widget_id: SHELL_SERVICE_ID.to_string(),
        },
    }));

    if !shell.role.is_primary() {
        let (topic, mut payload) = ShellContract::announce();
        payload.window_id = window_id;
        publish_payload(&mut message_bus_events, &topic, payload.to_json());
    }
}

fn publish_payload(
    message_bus_events: &mut EventWriter<MessageBusEvent>,
    topic: &str,
    payload_json: Result<String, serde_json::Error>,
) {
    let Ok(payload_json) = payload_json else {
        return;
    };
    message_bus_events.send(MessageBusEvent::RouteMessage(Message::Broker {
        message: BrokerServiceMessage::Publish {
            topic: topic.to_string(),
            message: payload_json,
        },
    }));
}

fn handle_shell_topics(
    mut topic_events: EventReader<TopicEvent>,
    mut shell: ShellState,
    mut visual_tree: ResMut<VisualTree>,
    mut project_ui: ProjectUiState,
    mut tile_commands: EventWriter<TileTreeCommand>,
    mut message_bus_events: EventWriter<MessageBusEvent>,
    mut exit_events: EventWriter<AppExit>,
) {
    let window_id = shell.status.client_id.clone();
    for event in topic_events.read() {
        if shell.role.is_primary() {
            handle_primary_topic(event, &mut shell, &mut message_bus_events);
        } else {
            handle_child_topic(
                event,
                &window_id,
                &mut visual_tree,
                &mut project_ui,
                &mut tile_commands,
                &mut message_bus_events,
                &mut exit_events,
            );
        }
    }
}

fn handle_primary_topic(
    event: &TopicEvent,
    shell: &mut ShellState,
    message_bus_events: &mut EventWriter<MessageBusEvent>,
) {
    if event.topic == ShellContract::announce_topic() {
        let Ok(payload) = AnnouncePayload::from_json(&event.payload) else {
            return;
        };
        if !shell.registry.known_windows.contains(&payload.window_id) {
            shell.registry.known_windows.push(payload.window_id.clone());
        }
        if let Some((layout_name, tree_json)) = shell.registry.pending_layouts.pop_front() {
            let (topic, mut assignment) = ShellContract::assign_layout(&payload.window_id);
            assignment.layout_name = layout_name;
            assignment.tree_json = tree_json;
            publish_payload(message_bus_events, &topic, assignment.to_json());
        }
    } else if event.topic == ShellContract::report_tree_topic() {
        if let Ok(payload) = ReportTreePayload::from_json(&event.payload)
            && let Some(collection) = shell.collection.active.as_mut()
        {
            collection.collected.push(ReportedTree {
                window_id: payload.window_id,
                layout_name: payload.layout_name,
                tree_json: payload.tree_json,
            });
        }
    } else if event.topic == ShellContract::request_spawn_topic() {
        message_bus_events.send(MessageBusEvent::RouteMessage(Message::Broker {
            message: BrokerServiceMessage::SpawnWindow,
        }));
    }
}

fn handle_child_topic(
    event: &TopicEvent,
    window_id: &str,
    visual_tree: &mut VisualTree,
    project_ui: &mut ProjectUiState,
    tile_commands: &mut EventWriter<TileTreeCommand>,
    message_bus_events: &mut EventWriter<MessageBusEvent>,
    exit_events: &mut EventWriter<AppExit>,
) {
    if event.topic == ShellContract::assign_layout_topic(window_id) {
        let Ok(payload) = AssignLayoutPayload::from_json(&event.payload) else {
            return;
        };
        let Ok(tree) = serde_json::from_str::<egui_tiles::Tree<Pane>>(&payload.tree_json) else {
            bevy::log::error!("Failed to parse assigned layout tree");
            return;
        };
        remove_tree_widgets(&visual_tree.tree, message_bus_events);
        visual_tree.tree = tree;
        visual_tree.layout_name = payload.layout_name.clone();
        visual_tree.layout_is_modified = false;
        project_ui.layout_state.layout_name = payload.layout_name;
        project_ui.layout_state.is_loaded = true;
        project_ui.layout_state.is_modified = false;
        tile_commands.send(TileTreeCommand::ReinitializeWidgets);
    } else if event.topic == ShellContract::close_topic(window_id) {
        exit_events.send(AppExit::Success);
    } else if event.topic == ShellContract::request_trees_topic() {
        let Ok(tree_json) = serde_json::to_string(&visual_tree.tree) else {
            return;
        };
        let (topic, mut payload) = ShellContract::report_tree();
        payload.window_id = window_id.to_string();
        payload.layout_name = visual_tree.layout_name.clone();
        payload.tree_json = tree_json;
        publish_payload(message_bus_events, &topic, payload.to_json());
    }
}

pub fn remove_tree_widgets(
    tree: &egui_tiles::Tree<Pane>,
    message_bus_events: &mut EventWriter<MessageBusEvent>,
) {
    for widget_id in collect_widget_ids(tree) {
        message_bus_events.send(MessageBusEvent::RouteMessage(Message::Broker {
            message: BrokerServiceMessage::WidgetRemoved { widget_id },
        }));
    }
}

pub fn begin_project_save(
    shell: &mut ShellState,
    project_ui: &mut ProjectUiState,
    visual_tree: &mut VisualTree,
    message_bus_events: &mut EventWriter<MessageBusEvent>,
    destination: CollectionDestination,
) {
    if shell.registry.known_windows.is_empty() {
        let save_file = ProjectSaveFile {
            version: env!("CARGO_PKG_VERSION").to_string(),
            windows: vec![WindowTree {
                tree: visual_tree.tree.clone(),
                layout_name: visual_tree.layout_name.clone(),
            }],
            project_name: Some(project_ui.project_state.project_name.clone()),
        };
        finalize_project_save(
            save_file,
            destination,
            project_ui,
            visual_tree,
            message_bus_events,
        );
    } else {
        let (topic, payload) = ShellContract::request_trees();
        publish_payload(message_bus_events, &topic, payload.to_json());
        shell.collection.active = Some(CollectionState {
            timer: Timer::from_seconds(1.0, TimerMode::Once),
            collected: Vec::new(),
            destination,
        });
    }
}

fn finalize_project_save(
    save_file: ProjectSaveFile,
    destination: CollectionDestination,
    project_ui: &mut ProjectUiState,
    visual_tree: &mut VisualTree,
    message_bus_events: &mut EventWriter<MessageBusEvent>,
) {
    let json = match serde_json::to_string_pretty(&save_file) {
        Ok(json) => json,
        Err(error) => {
            bevy::log::error!("Failed to serialize project: {error}");
            return;
        }
    };
    match destination {
        CollectionDestination::Path(path) => match fs::write(&path, json) {
            Ok(_) => {
                project_ui.project_state.is_modified = false;
                project_ui.layout_state.is_modified = false;
                visual_tree.layout_is_modified = false;
                message_bus_events.send(MessageBusEvent::RouteMessage(Message::Notify {
                    message: NotificationServiceMessage::Show {
                        text: format!("Project saved\n{path}"),
                        kind: NotificationKind::Success,
                        duration_in_seconds: 3.0,
                    },
                }));
            }
            Err(error) => {
                message_bus_events.send(MessageBusEvent::RouteMessage(Message::Notify {
                    message: NotificationServiceMessage::Show {
                        text: format!("Failed to save project: {error}"),
                        kind: NotificationKind::Error,
                        duration_in_seconds: 5.0,
                    },
                }));
            }
        },
        CollectionDestination::Dialog => {
            message_bus_events.send(MessageBusEvent::RouteMessage(Message::Filesystem {
                message: FileSystemMessage::Command(FileSystemCommand::SaveFile {
                    tag: TAG_PROJECT_SAVE.to_string(),
                    bytes: json.into_bytes(),
                    filter_name: "project".to_string(),
                    extensions: vec!["project.json".to_string()],
                }),
            }));
        }
    }
}

fn tick_project_collection(
    time: Res<Time>,
    mut shell: ShellState,
    mut visual_tree: ResMut<VisualTree>,
    mut project_ui: ProjectUiState,
    mut message_bus_events: EventWriter<MessageBusEvent>,
) {
    let (complete, timed_out) = match shell.collection.active.as_mut() {
        Some(collection) => {
            collection.timer.tick(time.delta());
            let all_reported = collection.collected.len() >= shell.registry.known_windows.len();
            let timed_out = collection.timer.finished() && !all_reported;
            (all_reported || timed_out, timed_out)
        }
        None => (false, false),
    };
    if !complete {
        return;
    }
    let Some(collection) = shell.collection.active.take() else {
        return;
    };

    if timed_out {
        shell.registry.known_windows.retain(|window_id| {
            collection
                .collected
                .iter()
                .any(|reported| &reported.window_id == window_id)
        });
    }

    let mut windows = vec![WindowTree {
        tree: visual_tree.tree.clone(),
        layout_name: visual_tree.layout_name.clone(),
    }];
    for window_id in &shell.registry.known_windows {
        let Some(reported) = collection
            .collected
            .iter()
            .find(|reported| &reported.window_id == window_id)
        else {
            continue;
        };
        let Ok(tree) = serde_json::from_str::<egui_tiles::Tree<Pane>>(&reported.tree_json) else {
            continue;
        };
        windows.push(WindowTree {
            tree,
            layout_name: reported.layout_name.clone(),
        });
    }

    let save_file = ProjectSaveFile {
        version: env!("CARGO_PKG_VERSION").to_string(),
        windows,
        project_name: Some(project_ui.project_state.project_name.clone()),
    };
    finalize_project_save(
        save_file,
        collection.destination,
        &mut project_ui,
        &mut visual_tree,
        &mut message_bus_events,
    );
}

fn handle_project_messages(
    mut project_messages: EventReader<ProjectMessage>,
    mut shell: ShellState,
    mut visual_tree: ResMut<VisualTree>,
    mut project_ui: ProjectUiState,
    mut tile_commands: EventWriter<TileTreeCommand>,
    mut message_bus_events: EventWriter<MessageBusEvent>,
) {
    for project_message in project_messages.read() {
        match project_message {
            ProjectMessage::ProjectLoaded {
                trees,
                project_name,
                layout_names,
                path,
            } => {
                if !shell.role.is_primary() {
                    continue;
                }
                let Some(first_tree) = trees.first() else {
                    continue;
                };

                close_child_windows(&mut shell, &mut message_bus_events);
                remove_tree_widgets(&visual_tree.tree, &mut message_bus_events);

                visual_tree.tree = first_tree.clone();
                visual_tree.layout_name = layout_names
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Default Layout".to_string());
                visual_tree.layout_is_modified = false;
                project_ui.layout_state.layout_name = visual_tree.layout_name.clone();
                project_ui.layout_state.is_loaded = true;
                project_ui.layout_state.is_modified = false;
                tile_commands.send(TileTreeCommand::ReinitializeWidgets);

                for (window_index, tree) in trees.iter().enumerate().skip(1) {
                    let Ok(tree_json) = serde_json::to_string(tree) else {
                        continue;
                    };
                    let layout_name = layout_names
                        .get(window_index)
                        .cloned()
                        .unwrap_or_else(|| "Default Layout".to_string());
                    shell
                        .registry
                        .pending_layouts
                        .push_back((layout_name, tree_json));
                    message_bus_events.send(MessageBusEvent::RouteMessage(Message::Broker {
                        message: BrokerServiceMessage::SpawnWindow,
                    }));
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
                project_ui.project_state.project_name = derived_project_name;
                project_ui.project_state.project_file_path = Some(path.clone());
                project_ui.project_state.is_modified = false;

                project_ui.user_settings.add_recent_project(path.clone());
                if let Err(error) = project_ui.user_settings.save() {
                    bevy::log::error!("Failed to save recent projects: {error}");
                }

                message_bus_events.send(MessageBusEvent::RouteMessage(Message::Notify {
                    message: NotificationServiceMessage::Show {
                        text: format!("Opened project: {}", project_ui.project_state.project_name),
                        kind: NotificationKind::Success,
                        duration_in_seconds: 3.0,
                    },
                }));
            }
            ProjectMessage::ProjectSaved { path } => {
                project_ui.project_state.is_modified = false;
                project_ui.project_state.project_file_path = Some(path.clone());
                project_ui.layout_state.is_modified = false;
                visual_tree.layout_is_modified = false;

                project_ui.user_settings.add_recent_project(path.clone());
                if let Err(error) = project_ui.user_settings.save() {
                    bevy::log::error!("Failed to save recent projects: {error}");
                }

                message_bus_events.send(MessageBusEvent::RouteMessage(Message::Notify {
                    message: NotificationServiceMessage::Show {
                        text: format!("Project saved\n{path}"),
                        kind: NotificationKind::Success,
                        duration_in_seconds: 3.0,
                    },
                }));
            }
            ProjectMessage::LayoutLoaded { tree, layout_name } => {
                remove_tree_widgets(&visual_tree.tree, &mut message_bus_events);
                visual_tree.tree = (**tree).clone();
                if let Some(layout_name) = layout_name {
                    visual_tree.layout_name = layout_name.clone();
                }
                visual_tree.layout_is_modified = false;
                project_ui.layout_state.layout_name = visual_tree.layout_name.clone();
                project_ui.layout_state.is_loaded = true;
                project_ui.layout_state.is_modified = false;
                tile_commands.send(TileTreeCommand::ReinitializeWidgets);
            }
            ProjectMessage::CloseProject => {
                if !shell.role.is_primary() {
                    continue;
                }
                close_child_windows(&mut shell, &mut message_bus_events);
                remove_tree_widgets(&visual_tree.tree, &mut message_bus_events);
                visual_tree.tree = reset_layout();
                visual_tree.layout_name = "Default Layout".to_string();
                visual_tree.layout_is_modified = false;
                tile_commands.send(TileTreeCommand::ReinitializeWidgets);

                *project_ui.project_state = ProjectState::default();
                project_ui.layout_state.is_loaded = false;
                project_ui.layout_state.is_modified = false;
                project_ui.layout_state.layout_name = String::new();

                message_bus_events.send(MessageBusEvent::RouteMessage(Message::Notify {
                    message: NotificationServiceMessage::Show {
                        text: "Project closed".to_string(),
                        kind: NotificationKind::Info,
                        duration_in_seconds: 3.0,
                    },
                }));
            }
            ProjectMessage::Empty => {}
        }
    }
}

fn close_child_windows(
    shell: &mut ShellState,
    message_bus_events: &mut EventWriter<MessageBusEvent>,
) {
    let window_ids: Vec<String> = shell.registry.known_windows.drain(..).collect();
    for window_id in window_ids {
        let (topic, payload) = ShellContract::close(&window_id);
        publish_payload(message_bus_events, &topic, payload.to_json());
    }
    shell.registry.pending_layouts.clear();
}
