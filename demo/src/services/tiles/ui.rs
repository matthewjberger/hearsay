use crate::prelude::*;
use egui_tiles::Tiles;

#[derive(Resource)]
pub struct VisualTree {
    pub tree: egui_tiles::Tree<Pane>,
    pub tree_behavior: TreeBehavior,
    pub draft_message: Message,
    pub api_visible: bool,
    pub editing_project_name: bool,
    pub project_name_edit_buffer: String,
    pub editing_layout_name: bool,
    pub layout_name_edit_buffer: String,
    pub layout_name: String,
    pub layout_is_modified: bool,
}

impl Default for VisualTree {
    fn default() -> Self {
        Self {
            tree: reset_layout(),
            tree_behavior: TreeBehavior::default(),
            draft_message: Message::default(),
            api_visible: false,
            editing_project_name: false,
            project_name_edit_buffer: String::new(),
            editing_layout_name: false,
            layout_name_edit_buffer: String::new(),
            layout_name: "Default Layout".to_string(),
            layout_is_modified: false,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct Pane {
    pub widget_kind: UiWidgetKind,

    #[serde(skip)]
    pub widget: UiWidget,
}

impl<'de> Deserialize<'de> for Pane {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WidgetPane {
            widget_kind: UiWidgetKind,
        }
        let pane = WidgetPane::deserialize(deserializer)?;
        Ok(Pane {
            widget_kind: pane.widget_kind,
            widget: UiWidget::from(&pane.widget_kind),
        })
    }
}

pub fn reset_layout() -> egui_tiles::Tree<Pane> {
    let mut tiles = Tiles::default();
    let root = tiles.insert_tab_tile(vec![]);
    let mut tree = egui_tiles::Tree::new("tree", root, tiles);

    if let Some(root) = tree.root() {
        let pane = tree.tiles.insert_pane(Pane {
            widget_kind: UiWidgetKind::Empty,
            widget: UiWidget::Empty,
        });
        if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(tabs))) =
            tree.tiles.get_mut(root)
        {
            tabs.add_child(pane);
            tabs.set_active(pane);
        }
    }

    tree
}

pub fn collect_widget_ids(tree: &egui_tiles::Tree<Pane>) -> Vec<String> {
    tree.tiles
        .iter()
        .filter_map(|(_, tile)| {
            if let egui_tiles::Tile::Pane(pane) = tile {
                get_widget_id(&pane.widget).map(|widget_id| widget_id.to_string())
            } else {
                None
            }
        })
        .collect()
}

#[derive(Serialize, Deserialize, Clone)]
pub struct WindowTree {
    pub tree: egui_tiles::Tree<Pane>,
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
    pub tree: egui_tiles::Tree<Pane>,
    pub layout_name: Option<String>,
}

#[derive(Resource)]
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

#[derive(Resource, Default)]
pub struct LayoutState {
    pub is_loaded: bool,
    pub layout_name: String,
    pub is_modified: bool,
}

#[derive(Event, Debug)]
pub enum TileTreeCommand {
    InsertPane {
        parent: egui_tiles::TileId,
        widget_kind: UiWidgetKind,
    },
    ReinitializeWidgets,
}

pub fn apply_theme_and_widget_context(
    mut contexts: Query<&mut EguiContext, With<bevy::window::PrimaryWindow>>,
    theme_state: Res<ThemeState>,
    status: Res<BrokerConnectionStatus>,
) {
    let Ok(mut egui_context) = contexts.get_single_mut() else {
        return;
    };
    let context = egui_context.get_mut();
    context.set_visuals(get_active_theme_visuals(&theme_state));
    context.data_mut(|data| {
        data.insert_temp(
            egui::Id::new("widget_context"),
            WidgetContext {
                is_connected: status.connected,
            },
        );
    });
}

pub fn deliver_dialog_results(
    mut messages: EventReader<Message>,
    mut visual_tree: ResMut<VisualTree>,
) {
    for message in messages.read() {
        if !matches!(
            message,
            Message::Filesystem {
                message: FileSystemMessage::Result(_),
            } | Message::Modal {
                message: ModalServiceMessage::ModalResult { .. },
            }
        ) {
            continue;
        }
        for (_tile_id, tile) in visual_tree.tree.tiles.iter_mut() {
            if let egui_tiles::Tile::Pane(pane) = tile {
                pane.widget.receive_message(message);
            }
        }
    }
}

pub fn sync_modification_flags(
    mut visual_tree: ResMut<VisualTree>,
    mut project_ui: ProjectUiState,
) {
    if visual_tree.tree_behavior.layout_modified {
        visual_tree.tree_behavior.layout_modified = false;
        visual_tree.layout_is_modified = true;
        project_ui.layout_state.is_modified = true;
        project_ui.layout_state.is_loaded = true;
    }
    if visual_tree.tree_behavior.project_modified {
        visual_tree.tree_behavior.project_modified = false;
        project_ui.project_state.is_modified = true;
    }
}

pub fn draw_shell_ui(
    mut contexts: Query<&mut EguiContext, With<bevy::window::PrimaryWindow>>,
    mut visual_tree: ResMut<VisualTree>,
    mut shell: ShellState,
    mut project_ui: ProjectUiState,
    mut message_bus_events: EventWriter<MessageBusEvent>,
) {
    let Ok(mut egui_context) = contexts.get_single_mut() else {
        return;
    };
    let context = egui_context.get_mut();

    egui::TopBottomPanel::top("top_panel").show(context, |ui| {
        egui::menu::bar(ui, |ui| {
            if shell.role.is_primary() {
                display_project_section(
                    ui,
                    &mut shell,
                    &mut project_ui,
                    &mut visual_tree,
                    &mut message_bus_events,
                );
            } else {
                ui.label("Window");
            }

            ui.separator();

            display_layout_section(
                ui,
                &mut project_ui,
                &mut visual_tree,
                &mut message_bus_events,
            );

            ui.separator();

            display_view_menu(ui, &mut project_ui, &mut visual_tree);

            ui.separator();

            if ui.button("New Window").clicked() {
                if shell.role.is_primary() {
                    message_bus_events.send(MessageBusEvent::RouteMessage(Message::Broker {
                        message: BrokerServiceMessage::SpawnWindow,
                    }));
                } else {
                    let (topic, payload) = ShellContract::request_spawn();
                    if let Ok(payload_json) = payload.to_json() {
                        message_bus_events.send(MessageBusEvent::RouteMessage(Message::Broker {
                            message: BrokerServiceMessage::Publish {
                                topic,
                                message: payload_json,
                            },
                        }));
                    }
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).weak());

                ui.separator();

                if let Some(fps) = ui
                    .ctx()
                    .data(|data| data.get_temp::<f32>(egui::Id::new("fps_counter")))
                {
                    ui.label(egui::RichText::new(format!("{:3} FPS", fps as u32)).weak());
                    ui.separator();
                }

                if shell.status.connected {
                    ui.label(
                        egui::RichText::new(&shell.status.address)
                            .color(ui.visuals().hyperlink_color)
                            .strong(),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("disconnected")
                            .color(ui.visuals().error_fg_color)
                            .strong(),
                    );
                }

                ui.separator();

                let role_text = if shell.role.is_primary() {
                    "Primary"
                } else {
                    "Window"
                };
                ui.label(egui::RichText::new(role_text).strong());
            });
        });
    });

    if visual_tree.api_visible {
        let mut api_visible = visual_tree.api_visible;
        egui::Window::new("Api")
            .collapsible(true)
            .resizable(true)
            .title_bar(true)
            .open(&mut api_visible)
            .show(context, |ui| {
                if ui.button("Send Message").clicked() {
                    message_bus_events.send(MessageBusEvent::RouteMessage(
                        visual_tree.draft_message.clone(),
                    ));
                }
                ui.separator();
                visual_tree.draft_message.ui_mut(ui);
            });
        visual_tree.api_visible = api_visible;
    }

    egui::CentralPanel::default().show(context, |ui| {
        let VisualTree {
            tree,
            tree_behavior,
            ..
        } = &mut *visual_tree;
        tree.ui(tree_behavior, ui);
    });
}

pub fn flush_widget_outputs(
    mut visual_tree: ResMut<VisualTree>,
    mut project_ui: ProjectUiState,
    mut message_bus_events: EventWriter<MessageBusEvent>,
    mut tile_commands: EventWriter<TileTreeCommand>,
) {
    for (_tile_id, tile) in visual_tree.tree.tiles.iter_mut() {
        if let egui_tiles::Tile::Pane(pane) = tile {
            for message in pane.widget.drain_messages() {
                message_bus_events.send(MessageBusEvent::RouteMessage(message));
            }
        }
    }

    for widget_id in visual_tree.tree_behavior.removed_widget_ids.drain(..) {
        message_bus_events.send(MessageBusEvent::RouteMessage(Message::Broker {
            message: BrokerServiceMessage::WidgetRemoved { widget_id },
        }));
    }

    if let Some((parent, widget_kind)) = visual_tree.tree_behavior.add_child_to.take() {
        tile_commands.send(TileTreeCommand::InsertPane {
            parent,
            widget_kind,
        });
    }

    if visual_tree.tree_behavior.reset_layout_requested {
        visual_tree.tree_behavior.reset_layout_requested = false;
        visual_tree.tree = reset_layout();
        visual_tree.layout_is_modified = true;
        project_ui.layout_state.is_modified = true;
        project_ui.project_state.is_modified = true;
        tile_commands.send(TileTreeCommand::ReinitializeWidgets);
    }
}

fn display_project_section(
    ui: &mut egui::Ui,
    shell: &mut ShellState,
    project_ui: &mut ProjectUiState,
    visual_tree: &mut VisualTree,
    message_bus_events: &mut EventWriter<MessageBusEvent>,
) {
    if visual_tree.editing_project_name {
        ui.horizontal(|ui| {
            let text_edit = ui.text_edit_singleline(&mut visual_tree.project_name_edit_buffer);

            if ui.button("❌").clicked() {
                visual_tree.editing_project_name = false;
                visual_tree.project_name_edit_buffer.clear();
            }

            if ui.button("✅").clicked()
                || text_edit.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter))
            {
                if !visual_tree.project_name_edit_buffer.is_empty() {
                    project_ui.project_state.project_name =
                        visual_tree.project_name_edit_buffer.clone();
                    project_ui.project_state.is_modified = true;
                }
                visual_tree.editing_project_name = false;
                visual_tree.project_name_edit_buffer.clear();
            }
        });
        return;
    }

    egui::menu::menu_button(ui, "Project", |ui| {
        ui.style_mut().spacing.item_spacing = egui::vec2(8.0, 8.0);

        if ui.button("New Project").clicked() {
            message_bus_events.send(MessageBusEvent::RouteMessage(Message::Project {
                message: ProjectMessage::CloseProject,
            }));
            ui.close_menu();
        }

        if ui.button("Load Project").clicked() {
            message_bus_events.send(MessageBusEvent::RouteMessage(Message::Filesystem {
                message: FileSystemMessage::Command(FileSystemCommand::PickFile {
                    tag: TAG_PROJECT.to_string(),
                    filter_name: "project".to_string(),
                    extensions: vec!["project.json".to_string()],
                }),
            }));
            ui.close_menu();
        }

        if ui.button("Save As Project").clicked() {
            begin_project_save(
                shell,
                project_ui,
                visual_tree,
                message_bus_events,
                CollectionDestination::Dialog,
            );
            ui.close_menu();
        }

        let save_enabled = project_ui.project_state.is_modified
            && project_ui.project_state.project_file_path.is_some();
        let save_response = ui.add_enabled(save_enabled, egui::Button::new("Save Project"));
        if save_response.clicked() {
            if let Some(path) = project_ui.project_state.project_file_path.clone() {
                begin_project_save(
                    shell,
                    project_ui,
                    visual_tree,
                    message_bus_events,
                    CollectionDestination::Path(path),
                );
            }
            ui.close_menu();
        }

        ui.separator();

        let current_project_is_startup = if let Some(current_path) =
            &project_ui.project_state.project_file_path
            && let Some(startup_path) = &project_ui.user_settings.default_project_path
        {
            current_path == startup_path
        } else {
            false
        };

        let set_startup_enabled =
            !current_project_is_startup && project_ui.project_state.project_file_path.is_some();
        let set_startup_response = ui.add_enabled(
            set_startup_enabled,
            egui::Button::new("Set as Startup Project"),
        );
        if set_startup_response.clicked() {
            if let Some(path) = project_ui.project_state.project_file_path.clone() {
                project_ui.user_settings.default_project_path = Some(path.clone());
                project_ui.user_settings.add_recent_project(path);
                if let Err(error) = project_ui.user_settings.save() {
                    bevy::log::error!("Failed to save settings: {error}");
                }
            }
            ui.close_menu();
        }

        let unset_startup_response = ui.add_enabled(
            project_ui.user_settings.default_project_path.is_some(),
            egui::Button::new("Unset Startup Project"),
        );
        if unset_startup_response.clicked() {
            project_ui.user_settings.default_project_path = None;
            if let Err(error) = project_ui.user_settings.save() {
                bevy::log::error!("Failed to save settings: {error}");
            }
            ui.close_menu();
        }

        ui.separator();

        ui.label(
            egui::RichText::new("Recent Projects")
                .weak()
                .small()
                .strong(),
        );
        ui.add_space(4.0);

        if project_ui.user_settings.recent_projects.is_empty() {
            ui.label(
                egui::RichText::new("  No recent projects")
                    .weak()
                    .small()
                    .italics(),
            );
            ui.add_space(4.0);
        }

        let recent_projects = project_ui.user_settings.recent_projects.clone();
        for project_path in recent_projects {
            let project_name = UserSettings::get_recent_project_name(&project_path);
            let is_startup_project = project_ui.user_settings.default_project_path.as_deref()
                == Some(project_path.as_str());

            let display_name = if is_startup_project {
                format!("⭐ {project_name}")
            } else {
                project_name
            };

            let button = ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(display_name).color(ui.visuals().hyperlink_color),
                    )
                    .min_size(egui::vec2(250.0, 0.0)),
                )
                .on_hover_text(&project_path);

            if button.clicked() {
                match fs::read(&project_path) {
                    Ok(bytes) => {
                        message_bus_events.send(MessageBusEvent::RouteMessage(
                            Message::Filesystem {
                                message: FileSystemMessage::Result(FileSystemResult::Success(
                                    FileSystemSuccess::File {
                                        path: project_path.clone(),
                                        bytes,
                                        tag: TAG_PROJECT.to_string(),
                                    },
                                )),
                            },
                        ));
                    }
                    Err(error) => {
                        bevy::log::error!("Failed to read recent project file: {error}");
                    }
                }
                ui.close_menu();
            }
        }

        if !project_ui.user_settings.recent_projects.is_empty() {
            ui.add_space(4.0);
            if ui
                .button(
                    egui::RichText::new("Clear Recent Projects")
                        .color(ui.visuals().error_fg_color)
                        .small(),
                )
                .clicked()
            {
                project_ui.user_settings.clear_recent_projects();
                if let Err(error) = project_ui.user_settings.save() {
                    bevy::log::error!("Failed to save settings: {error}");
                }
                ui.close_menu();
            }
        }
    });

    let project_text = if project_ui.project_state.project_name.is_empty() {
        "Untitled Project"
    } else {
        &project_ui.project_state.project_name
    };

    let project_display = if project_ui.project_state.is_modified {
        format!("{project_text} *")
    } else {
        project_text.to_string()
    };

    ui.label(project_display);

    if ui
        .small_button("✏")
        .on_hover_text("Edit project name")
        .clicked()
    {
        visual_tree.editing_project_name = true;
        visual_tree.project_name_edit_buffer = project_ui.project_state.project_name.clone();
        if visual_tree.project_name_edit_buffer.is_empty() {
            visual_tree.project_name_edit_buffer = "Untitled Project".to_string();
        }
    }
}

fn display_layout_section(
    ui: &mut egui::Ui,
    project_ui: &mut ProjectUiState,
    visual_tree: &mut VisualTree,
    message_bus_events: &mut EventWriter<MessageBusEvent>,
) {
    if visual_tree.editing_layout_name {
        ui.horizontal(|ui| {
            let text_edit = ui.text_edit_singleline(&mut visual_tree.layout_name_edit_buffer);

            if ui.button("❌").clicked() {
                visual_tree.editing_layout_name = false;
                visual_tree.layout_name_edit_buffer.clear();
            }

            if ui.button("✅").clicked()
                || text_edit.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter))
            {
                if !visual_tree.layout_name_edit_buffer.is_empty() {
                    visual_tree.layout_name = visual_tree.layout_name_edit_buffer.clone();
                    visual_tree.layout_is_modified = true;
                    project_ui.layout_state.layout_name = visual_tree.layout_name.clone();
                    project_ui.layout_state.is_modified = true;
                    project_ui.project_state.is_modified = true;
                }
                visual_tree.editing_layout_name = false;
                visual_tree.layout_name_edit_buffer.clear();
            }
        });
        return;
    }

    egui::menu::menu_button(ui, "Layout", |ui| {
        ui.style_mut().spacing.item_spacing = egui::vec2(8.0, 8.0);

        if ui.button("Save Layout").clicked() {
            let layout_name = if visual_tree.layout_name.is_empty() {
                "Untitled Layout".to_string()
            } else {
                visual_tree.layout_name.clone()
            };

            let save_file = LayoutSaveFile {
                version: env!("CARGO_PKG_VERSION").to_string(),
                tree: visual_tree.tree.clone(),
                layout_name: Some(layout_name),
            };

            match serde_json::to_string_pretty(&save_file) {
                Ok(json) => {
                    message_bus_events.send(MessageBusEvent::RouteMessage(Message::Filesystem {
                        message: FileSystemMessage::Command(FileSystemCommand::SaveFile {
                            tag: TAG_LAYOUT_SAVE.to_string(),
                            bytes: json.into_bytes(),
                            filter_name: "layout".to_string(),
                            extensions: vec!["layout.json".to_string()],
                        }),
                    }));
                }
                Err(error) => {
                    bevy::log::error!("Failed to serialize layout: {error}");
                }
            }
            ui.close_menu();
        }

        if ui.button("Load Layout").clicked() {
            message_bus_events.send(MessageBusEvent::RouteMessage(Message::Filesystem {
                message: FileSystemMessage::Command(FileSystemCommand::PickFile {
                    tag: TAG_LAYOUT.to_string(),
                    filter_name: "layout".to_string(),
                    extensions: vec!["layout.json".to_string()],
                }),
            }));
            ui.close_menu();
        }

        if ui.button("Reset Layout").clicked() {
            remove_tree_widgets(&visual_tree.tree, message_bus_events);
            visual_tree.tree = reset_layout();
            visual_tree.layout_name = "Default Layout".to_string();
            visual_tree.layout_is_modified = false;
            project_ui.layout_state.layout_name = "Default Layout".to_string();
            project_ui.layout_state.is_loaded = true;
            project_ui.layout_state.is_modified = false;
            project_ui.project_state.is_modified = true;
            ui.close_menu();
        }
    });

    let layout_name = if visual_tree.layout_name.is_empty() {
        "Default Layout"
    } else {
        &visual_tree.layout_name
    };

    let layout_display = if visual_tree.layout_is_modified {
        format!("{layout_name} *")
    } else {
        layout_name.to_string()
    };

    ui.label(layout_display);

    if ui
        .small_button("✏")
        .on_hover_text("Edit layout name")
        .clicked()
    {
        visual_tree.editing_layout_name = true;
        visual_tree.layout_name_edit_buffer = visual_tree.layout_name.clone();
        if visual_tree.layout_name_edit_buffer.is_empty() {
            visual_tree.layout_name_edit_buffer = "Untitled Layout".to_string();
        }
    }
}

fn display_view_menu(
    ui: &mut egui::Ui,
    project_ui: &mut ProjectUiState,
    visual_tree: &mut VisualTree,
) {
    project_ui.theme_state.preview_index = None;

    egui::menu::menu_button(ui, "View", |ui| {
        ui.style_mut().spacing.item_spacing = egui::vec2(8.0, 8.0);

        let selected_index = project_ui.theme_state.selected_index;
        let presets = project_ui.theme_state.presets.clone();
        for (preset_index, preset) in presets.iter().enumerate() {
            let is_selected = preset_index == selected_index;
            let response = ui.selectable_label(is_selected, preset.name);
            if response.hovered() {
                project_ui.theme_state.preview_index = Some(preset_index);
            }
            if response.clicked() {
                project_ui.theme_state.selected_index = preset_index;
                project_ui.user_settings.theme_name = Some(preset.name.to_string());
                if let Err(error) = project_ui.user_settings.save() {
                    bevy::log::error!("Failed to save theme setting: {error}");
                }
                ui.close_menu();
            }
        }

        ui.separator();

        let api_text = if visual_tree.api_visible {
            "Hide Api Window"
        } else {
            "Show Api Window"
        };
        if ui.button(api_text).clicked() {
            visual_tree.api_visible = !visual_tree.api_visible;
            ui.close_menu();
        }
    });
}

pub fn deliver_topic_messages(
    mut topic_events: EventReader<TopicEvent>,
    registry: Res<SubscriptionRegistry>,
    mut visual_tree: ResMut<VisualTree>,
) {
    for event in topic_events.read() {
        let Some(subscribers) = registry.topic_subscribers.get(&event.topic) else {
            continue;
        };
        let message = Message::Topic {
            topic: event.topic.clone(),
            payload: event.payload.clone(),
            bytes: event.bytes.clone(),
        };
        for (_tile_id, tile) in visual_tree.tree.tiles.iter_mut() {
            if let egui_tiles::Tile::Pane(pane) = tile
                && let Some(widget_id) = get_widget_id(&pane.widget)
                && subscribers.iter().any(|subscriber| subscriber == widget_id)
            {
                pane.widget.receive_message(&message);
            }
        }
    }
}

pub fn process_tile_tree_commands(
    mut commands: EventReader<TileTreeCommand>,
    mut visual_tree: ResMut<VisualTree>,
    mut project_ui: ProjectUiState,
) {
    for command in commands.read() {
        match command {
            TileTreeCommand::InsertPane {
                parent,
                widget_kind,
            } => {
                let mut placeholder_to_remove = None;
                if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(tabs))) =
                    visual_tree.tree.tiles.get(*parent)
                {
                    let children = &tabs.children;
                    if children.len() == 1
                        && let Some(only_child_id) = children.first()
                        && let Some(egui_tiles::Tile::Pane(existing_pane)) =
                            visual_tree.tree.tiles.get(*only_child_id)
                        && matches!(existing_pane.widget_kind, UiWidgetKind::Empty)
                    {
                        placeholder_to_remove = Some(*only_child_id);
                    }
                }

                if let Some(tile_id_to_remove) = placeholder_to_remove {
                    visual_tree.tree.tiles.remove(tile_id_to_remove);
                }

                let pane = visual_tree.tree.tiles.insert_pane(Pane {
                    widget_kind: *widget_kind,
                    widget: UiWidget::from(widget_kind),
                });
                if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(tabs))) =
                    visual_tree.tree.tiles.get_mut(*parent)
                {
                    tabs.add_child(pane);
                    tabs.set_active(pane);
                    visual_tree.layout_is_modified = true;
                    project_ui.layout_state.is_modified = true;
                    project_ui.project_state.is_modified = true;
                }
            }
            TileTreeCommand::ReinitializeWidgets => {
                visual_tree.tree_behavior = TreeBehavior::default();
            }
        }
    }
}

pub fn handle_tiletree_messages(
    mut tile_messages: EventReader<TileTreeMessage>,
    mut message_bus_events: EventWriter<MessageBusEvent>,
    mut visual_tree: ResMut<VisualTree>,
    mut project_ui: ProjectUiState,
) {
    for message in tile_messages.read() {
        match message {
            TileTreeMessage::ProcessFileResult(result) => {
                let FileSystemResult::Success(FileSystemSuccess::File { path, bytes, tag }) =
                    result
                else {
                    continue;
                };
                match tag.as_str() {
                    TAG_PROJECT => {
                        let Ok(json) = std::str::from_utf8(bytes) else {
                            continue;
                        };
                        match serde_json::from_str::<ProjectSaveFile>(json) {
                            Ok(save_file) => {
                                let trees = save_file
                                    .windows
                                    .iter()
                                    .map(|window_tree| window_tree.tree.clone())
                                    .collect();
                                let layout_names = save_file
                                    .windows
                                    .iter()
                                    .map(|window_tree| window_tree.layout_name.clone())
                                    .collect::<Vec<_>>();
                                message_bus_events.send(MessageBusEvent::RouteMessage(
                                    Message::Project {
                                        message: ProjectMessage::ProjectLoaded {
                                            trees,
                                            project_name: save_file.project_name,
                                            layout_names,
                                            path: path.clone(),
                                        },
                                    },
                                ));
                            }
                            Err(error) => {
                                bevy::log::error!("Failed to parse project file: {error}");
                            }
                        }
                    }
                    TAG_PROJECT_SAVE => {
                        message_bus_events.send(MessageBusEvent::RouteMessage(Message::Project {
                            message: ProjectMessage::ProjectSaved { path: path.clone() },
                        }));
                    }
                    TAG_LAYOUT => {
                        let Ok(json) = std::str::from_utf8(bytes) else {
                            continue;
                        };
                        match serde_json::from_str::<LayoutSaveFile>(json) {
                            Ok(save_file) => {
                                message_bus_events.send(MessageBusEvent::RouteMessage(
                                    Message::Project {
                                        message: ProjectMessage::LayoutLoaded {
                                            tree: Box::new(save_file.tree),
                                            layout_name: save_file.layout_name,
                                        },
                                    },
                                ));
                            }
                            Err(error) => {
                                bevy::log::error!("Failed to parse layout file: {error}");
                            }
                        }
                    }
                    TAG_LAYOUT_SAVE => {
                        visual_tree.layout_is_modified = false;
                        project_ui.layout_state.is_modified = false;
                        message_bus_events.send(MessageBusEvent::RouteMessage(Message::Notify {
                            message: NotificationServiceMessage::Show {
                                text: format!("Layout saved\n{path}"),
                                kind: NotificationKind::Success,
                                duration_in_seconds: 3.0,
                            },
                        }));
                    }
                    _ => {}
                }
            }
            TileTreeMessage::Empty => {}
        }
    }
}

pub fn update_window_title(
    project_state: Res<ProjectState>,
    role: Res<WindowRole>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
) {
    let Ok(mut window) = windows.get_single_mut() else {
        return;
    };

    let window_title = if role.is_primary() {
        if let Some(project_path) = &project_state.project_file_path {
            if project_state.is_modified {
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

    if window.title != window_title {
        window.title = window_title;
    }
}
