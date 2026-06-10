use crate::messages::Message;
use crate::messages::*;
use crate::rpc::WidgetRpc;
use nightshade::prelude::*;
use serde::Serialize;

const TEXT_TOPIC: &str = "template/text";
const BINARY_TOPIC: &str = "template/binary";
const PICK_FILE_TAG: &str = "template-file";
const PICK_FOLDER_TAG: &str = "template-folder";
const SAVE_FILE_TAG: &str = "template-save";
const MAX_RECEIVED_MESSAGES: usize = 25;
const ROW_HEIGHT: f32 = 30.0;
const LABEL_HEIGHT: f32 = 20.0;

#[derive(Serialize)]
struct TemplateNote {
    text: String,
}

pub struct TemplateWidget {
    pub rpc: WidgetRpc,
    pub pane_id: TileId,
    pub content_entity: Entity,
    subscribed: bool,
    auto_subscribe_pending: bool,
    outgoing_text: String,
    received_text: Vec<String>,
    received_binary_count: usize,
    last_binary_length: usize,
    picked_file: Option<(String, usize)>,
    picked_folder: Option<String>,
    saved_file: Option<String>,
    last_modal_result: Option<ModalResult>,
    rendered_connected: Option<bool>,
    subscriptions_dirty: bool,
    received_dirty: bool,
    files_dirty: bool,
    modal_dirty: bool,
    connection_label: Entity,
    topic_labels: Vec<Entity>,
    no_subscriptions_label: Entity,
    subscription_button: Entity,
    message_input: Entity,
    publish_typed_button: Entity,
    publish_raw_button: Entity,
    publish_binary_button: Entity,
    binary_label: Entity,
    received_rows: Vec<Entity>,
    no_received_label: Entity,
    clear_received_button: Entity,
    notify_info_button: Entity,
    notify_success_button: Entity,
    notify_warning_button: Entity,
    notify_error_button: Entity,
    pick_file_button: Entity,
    pick_folder_button: Entity,
    save_file_button: Entity,
    picked_file_label: Entity,
    picked_folder_label: Entity,
    saved_file_label: Entity,
    show_modal_button: Entity,
    modal_result_label: Entity,
}

fn set_flow_fraction(world: &mut World, entity: Entity, fraction: f32, height: f32) {
    if let Some(node) = world.ui.get_ui_layout_node_mut(entity) {
        node.flow_child_size = Some(Rl(vec2(fraction, 0.0)) + Ab(vec2(-6.0, height)));
    }
}

fn add_row(tree: &mut UiTreeBuilder, height: f32) -> Entity {
    tree.add_node()
        .size(100.pct(), height.px())
        .flow_horizontal()
        .padding(0.0)
        .gap(6.0)
        .align_cross(FlowAlignment::Center)
        .entity()
}

fn add_label(tree: &mut UiTreeBuilder, text: &str, size: f32, role: ThemeColor) -> Entity {
    tree.add_node()
        .size(100.pct(), LABEL_HEIGHT.px())
        .with_text(text, size)
        .text_left()
        .fg(role)
        .entity()
}

impl TemplateWidget {
    pub fn build(world: &mut World, pane_id: TileId, content_entity: Entity) -> Self {
        let rpc = WidgetRpc::default();
        let widget_id_line = format!("Widget id: {}", rpc.widget_id());

        let mut tree = UiTreeBuilder::from_parent(world, content_entity);
        let scroll = tree.add_scroll_area_fill(8.0, 6.0);
        let scroll_content = widget::<UiScrollAreaData>(tree.world_mut(), scroll)
            .map(|data| data.content_entity)
            .unwrap_or(content_entity);
        tree.push_parent(scroll_content);

        tree.add_node()
            .size(100.pct(), (26.0).px())
            .with_text("Template Widget", 19.0)
            .text_left()
            .fg(ThemeColor::Text)
            .entity();
        add_label(&mut tree, &widget_id_line, 12.0, ThemeColor::TextDisabled);
        let connection_label = add_label(
            &mut tree,
            "Disconnected from broker",
            13.0,
            ThemeColor::Error,
        );
        tree.add_separator();

        add_label(&mut tree, "Subscriptions", 14.0, ThemeColor::Text);
        let mut topic_labels = Vec::new();
        for _ in 0..2 {
            let topic_label = add_label(&mut tree, "", 13.0, ThemeColor::TextAccent);
            ui_set_visible(tree.world_mut(), topic_label, false);
            topic_labels.push(topic_label);
        }
        let no_subscriptions_label = add_label(
            &mut tree,
            "No active subscriptions",
            13.0,
            ThemeColor::TextDisabled,
        );
        let subscription_button = tree.add_button("Subscribe");
        tree.add_separator();

        add_label(&mut tree, "Publish", 14.0, ThemeColor::Text);
        let message_row = add_row(&mut tree, ROW_HEIGHT);
        let mut message_input = Entity::default();
        tree.in_parent(message_row, |tree| {
            let prompt = tree
                .add_node()
                .with_text("Message:", 13.0)
                .text_left()
                .fg(ThemeColor::TextDisabled)
                .entity();
            if let Some(node) = tree.world_mut().ui.get_ui_layout_node_mut(prompt) {
                node.flow_child_size = Some(Ab(vec2(64.0, LABEL_HEIGHT)).into());
            }
            message_input = tree.add_text_input("Type a message...");
            let world = tree.world_mut();
            if let Some(node) = world.ui.get_ui_layout_node_mut(message_input) {
                node.flow_child_size = Some(Rl(vec2(100.0, 0.0)) + Ab(vec2(-76.0, 28.0)));
            }
        });

        let publish_row = add_row(&mut tree, ROW_HEIGHT + 4.0);
        let mut publish_typed_button = Entity::default();
        let mut publish_raw_button = Entity::default();
        let mut publish_binary_button = Entity::default();
        tree.in_parent(publish_row, |tree| {
            publish_typed_button = tree.add_button("Publish Typed");
            publish_raw_button = tree.add_button("Publish Raw");
            publish_binary_button = tree.add_button("Publish Binary");
            for button in [
                publish_typed_button,
                publish_raw_button,
                publish_binary_button,
            ] {
                set_flow_fraction(tree.world_mut(), button, 33.3, 28.0);
            }
        });

        add_label(&mut tree, "Received", 14.0, ThemeColor::Text);
        let binary_label = add_label(
            &mut tree,
            "Binary messages: 0 (last 0 bytes)",
            13.0,
            ThemeColor::TextDisabled,
        );
        let no_received_label = add_label(
            &mut tree,
            "No text messages received",
            13.0,
            ThemeColor::TextDisabled,
        );
        let received_scroll = tree.add_scroll_area(vec2(0.0, 120.0));
        let received_content = widget::<UiScrollAreaData>(tree.world_mut(), received_scroll)
            .map(|data| data.content_entity)
            .unwrap_or(received_scroll);
        let mut received_rows = Vec::new();
        tree.in_parent(received_content, |tree| {
            for _ in 0..MAX_RECEIVED_MESSAGES {
                let received_row = tree
                    .add_node()
                    .size(100.pct(), (18.0).px())
                    .with_text("", 12.0)
                    .text_left()
                    .fg(ThemeColor::TextAccent)
                    .entity();
                ui_set_visible(tree.world_mut(), received_row, false);
                received_rows.push(received_row);
            }
        });
        let clear_received_button = tree.add_button("Clear Received");
        tree.add_separator();

        add_label(&mut tree, "Notifications", 14.0, ThemeColor::Text);
        let notify_row = add_row(&mut tree, ROW_HEIGHT + 4.0);
        let mut notify_info_button = Entity::default();
        let mut notify_success_button = Entity::default();
        let mut notify_warning_button = Entity::default();
        let mut notify_error_button = Entity::default();
        tree.in_parent(notify_row, |tree| {
            notify_info_button = tree.add_button("Info");
            notify_success_button = tree.add_button("Success");
            notify_warning_button = tree.add_button("Warning");
            notify_error_button = tree.add_button("Error");
            for button in [
                notify_info_button,
                notify_success_button,
                notify_warning_button,
                notify_error_button,
            ] {
                set_flow_fraction(tree.world_mut(), button, 25.0, 28.0);
            }
        });
        tree.add_separator();

        add_label(&mut tree, "Files", 14.0, ThemeColor::Text);
        let files_row = add_row(&mut tree, ROW_HEIGHT + 4.0);
        let mut pick_file_button = Entity::default();
        let mut pick_folder_button = Entity::default();
        let mut save_file_button = Entity::default();
        tree.in_parent(files_row, |tree| {
            pick_file_button = tree.add_button("Pick File");
            pick_folder_button = tree.add_button("Pick Folder");
            save_file_button = tree.add_button("Save File");
            for button in [pick_file_button, pick_folder_button, save_file_button] {
                set_flow_fraction(tree.world_mut(), button, 33.3, 28.0);
            }
        });
        let picked_file_label = add_label(&mut tree, "", 13.0, ThemeColor::TextDisabled);
        let picked_folder_label = add_label(&mut tree, "", 13.0, ThemeColor::TextDisabled);
        let saved_file_label = add_label(&mut tree, "", 13.0, ThemeColor::TextDisabled);
        for label in [picked_file_label, picked_folder_label, saved_file_label] {
            ui_set_visible(tree.world_mut(), label, false);
        }
        tree.add_separator();

        add_label(&mut tree, "Modal", 14.0, ThemeColor::Text);
        let show_modal_button = tree.add_button("Show Modal");
        let modal_result_label = add_label(
            &mut tree,
            "No modal result yet",
            13.0,
            ThemeColor::TextDisabled,
        );

        tree.pop_parent();
        tree.finish_subtree();

        Self {
            rpc,
            pane_id,
            content_entity,
            subscribed: false,
            auto_subscribe_pending: false,
            outgoing_text: String::new(),
            received_text: Vec::new(),
            received_binary_count: 0,
            last_binary_length: 0,
            picked_file: None,
            picked_folder: None,
            saved_file: None,
            last_modal_result: None,
            rendered_connected: None,
            subscriptions_dirty: true,
            received_dirty: true,
            files_dirty: false,
            modal_dirty: false,
            connection_label,
            topic_labels,
            no_subscriptions_label,
            subscription_button,
            message_input,
            publish_typed_button,
            publish_raw_button,
            publish_binary_button,
            binary_label,
            received_rows,
            no_received_label,
            clear_received_button,
            notify_info_button,
            notify_success_button,
            notify_warning_button,
            notify_error_button,
            pick_file_button,
            pick_folder_button,
            save_file_button,
            picked_file_label,
            picked_folder_label,
            saved_file_label,
            show_modal_button,
            modal_result_label,
        }
    }

    pub fn receive_message(&mut self, message: &Message) {
        self.rpc.receive_message(message);
    }

    pub fn drain_messages(&mut self) -> Vec<Message> {
        self.rpc.drain_messages()
    }

    fn subscribe(&mut self) {
        self.rpc
            .subscribe_to_topics(&[TEXT_TOPIC.to_string(), BINARY_TOPIC.to_string()]);
        self.subscribed = true;
        self.subscriptions_dirty = true;
    }

    pub fn handle_ui_event(&mut self, event: &UiEvent) -> bool {
        match event {
            UiEvent::ButtonClicked(entity) => {
                let entity = *entity;
                if entity == self.subscription_button {
                    if self.subscribed {
                        self.rpc.unsubscribe_from_topics(&[
                            TEXT_TOPIC.to_string(),
                            BINARY_TOPIC.to_string(),
                        ]);
                        self.subscribed = false;
                        self.auto_subscribe_pending = true;
                    } else {
                        self.subscribe();
                        self.auto_subscribe_pending = false;
                    }
                    self.subscriptions_dirty = true;
                } else if entity == self.publish_typed_button {
                    let note = TemplateNote {
                        text: self.outgoing_text.clone(),
                    };
                    self.rpc.publish(TEXT_TOPIC, &note);
                } else if entity == self.publish_raw_button {
                    let payload_json = format!("{{\"raw\":\"{}\"}}", self.outgoing_text);
                    self.rpc.publish_json(TEXT_TOPIC, &payload_json);
                } else if entity == self.publish_binary_button {
                    let bytes = self.outgoing_text.clone().into_bytes();
                    self.rpc.publish_bytes(BINARY_TOPIC, &bytes);
                } else if entity == self.clear_received_button {
                    self.received_text.clear();
                    self.received_binary_count = 0;
                    self.last_binary_length = 0;
                    self.received_dirty = true;
                } else if entity == self.notify_info_button {
                    self.rpc
                        .notify("An informational toast", NotificationKind::Info, 3.0);
                } else if entity == self.notify_success_button {
                    self.rpc
                        .notify("A success toast", NotificationKind::Success, 3.0);
                } else if entity == self.notify_warning_button {
                    self.rpc
                        .notify("A warning toast", NotificationKind::Warning, 3.0);
                } else if entity == self.notify_error_button {
                    self.rpc
                        .notify("An error toast", NotificationKind::Error, 3.0);
                } else if entity == self.pick_file_button {
                    self.rpc.pick_file(PICK_FILE_TAG, "", Vec::new());
                } else if entity == self.pick_folder_button {
                    self.rpc.pick_directory(PICK_FOLDER_TAG);
                } else if entity == self.save_file_button {
                    let bytes = self.outgoing_text.clone().into_bytes();
                    self.rpc
                        .save_file(SAVE_FILE_TAG, bytes, "text", vec!["txt".to_string()]);
                } else if entity == self.show_modal_button {
                    if !self.rpc.has_open_modal() {
                        self.rpc.show_modal(
                            "Template Modal",
                            "Confirm the template action?",
                            Some("Yes, proceed".to_string()),
                            Some("No, cancel".to_string()),
                        );
                        self.modal_dirty = true;
                    }
                } else {
                    return false;
                }
                true
            }
            UiEvent::TextInputChanged { entity, text } if *entity == self.message_input => {
                self.outgoing_text = text.clone();
                true
            }
            _ => false,
        }
    }

    pub fn update(&mut self, world: &mut World, connected: bool) {
        self.rpc.update(connected);

        if !self.subscribed && !self.auto_subscribe_pending && connected {
            self.subscribe();
        }

        self.process_messages();
        self.process_file_results();
        if let Some(result) = self.rpc.take_modal_result() {
            self.last_modal_result = Some(result);
            self.modal_dirty = true;
        }

        if self.rendered_connected != Some(connected) {
            self.rendered_connected = Some(connected);
            let theme = world
                .resources
                .retained_ui
                .theme_state
                .active_theme()
                .clone();
            let (text, color) = if connected {
                ("Connected to broker", theme.success_color)
            } else {
                ("Disconnected from broker", theme.error_color)
            };
            ui_set_text(world, self.connection_label, text);
            if let Some(node_color) = world.ui.get_ui_node_color_mut(self.connection_label) {
                node_color.colors[UiBase::INDEX] = Some(color);
            }
            for button in [
                self.publish_typed_button,
                self.publish_raw_button,
                self.publish_binary_button,
            ] {
                ui_set_disabled(world, button, !connected);
            }
        }

        if self.subscriptions_dirty {
            self.subscriptions_dirty = false;
            let topics = self.rpc.subscribed_topics();
            for (index, label) in self.topic_labels.clone().into_iter().enumerate() {
                if let Some(topic) = topics.get(index) {
                    ui_set_text(world, label, topic);
                    ui_set_visible(world, label, true);
                } else {
                    ui_set_visible(world, label, false);
                }
            }
            ui_set_visible(world, self.no_subscriptions_label, topics.is_empty());
            let button_text = if self.subscribed {
                "Unsubscribe"
            } else {
                "Subscribe"
            };
            ui_button_set_text(world, self.subscription_button, button_text);
        }

        if self.received_dirty {
            self.received_dirty = false;
            ui_set_text(
                world,
                self.binary_label,
                &format!(
                    "Binary messages: {} (last {} bytes)",
                    self.received_binary_count, self.last_binary_length
                ),
            );
            ui_set_visible(world, self.no_received_label, self.received_text.is_empty());
            let rows = self.received_rows.clone();
            for (row_index, row) in rows.into_iter().enumerate() {
                let message_index = self.received_text.len().checked_sub(row_index + 1);
                if let Some(message_index) = message_index {
                    let payload = &self.received_text[message_index];
                    ui_set_text(world, row, &format!("[{message_index}] {payload}"));
                    ui_set_visible(world, row, true);
                } else {
                    ui_set_visible(world, row, false);
                }
            }
        }

        if self.files_dirty {
            self.files_dirty = false;
            if let Some((path, byte_count)) = &self.picked_file {
                ui_set_text(
                    world,
                    self.picked_file_label,
                    &format!("Picked file: {path} ({byte_count} bytes)"),
                );
                ui_set_visible(world, self.picked_file_label, true);
            }
            if let Some(path) = &self.picked_folder {
                ui_set_text(
                    world,
                    self.picked_folder_label,
                    &format!("Picked folder: {path}"),
                );
                ui_set_visible(world, self.picked_folder_label, true);
            }
            if let Some(path) = &self.saved_file {
                ui_set_text(world, self.saved_file_label, &format!("Saved file: {path}"));
                ui_set_visible(world, self.saved_file_label, true);
            }
        }

        if self.modal_dirty {
            self.modal_dirty = false;
            ui_set_disabled(world, self.show_modal_button, self.rpc.has_open_modal());
            let theme = world
                .resources
                .retained_ui
                .theme_state
                .active_theme()
                .clone();
            let (text, color) = match &self.last_modal_result {
                Some(ModalResult::Confirmed) => {
                    ("Last result: confirmed", Some(theme.success_color))
                }
                Some(ModalResult::Cancelled) => {
                    ("Last result: cancelled", Some(theme.warning_color))
                }
                None => ("No modal result yet", None),
            };
            ui_set_text(world, self.modal_result_label, text);
            if let Some(color) = color
                && let Some(node_color) = world.ui.get_ui_node_color_mut(self.modal_result_label)
            {
                node_color.colors[UiBase::INDEX] = Some(color);
            }
        }
    }

    fn process_messages(&mut self) {
        if let Some(messages) = self.rpc.get_messages_for_topic(TEXT_TOPIC) {
            for payload in messages.clone() {
                self.received_text.push(payload);
            }
            if self.received_text.len() > MAX_RECEIVED_MESSAGES {
                let excess = self.received_text.len() - MAX_RECEIVED_MESSAGES;
                self.received_text.drain(0..excess);
            }
            self.rpc.clear_topic_messages(&[TEXT_TOPIC]);
            self.received_dirty = true;
        }

        if let Some(binary_messages) = self.rpc.get_binary_messages_for_topic(BINARY_TOPIC) {
            self.received_binary_count += binary_messages.len();
            if let Some(last_message) = binary_messages.last() {
                self.last_binary_length = last_message.len();
            }
            self.rpc.clear_binary_topic_messages(&[BINARY_TOPIC]);
            self.received_dirty = true;
        }
    }

    fn process_file_results(&mut self) {
        while let Some(result) = self.rpc.next_file_result(PICK_FILE_TAG) {
            if let FileSystemSuccess::File { path, bytes, .. } = result {
                self.picked_file = Some((path, bytes.len()));
                self.files_dirty = true;
            }
        }
        while let Some(result) = self.rpc.next_file_result(PICK_FOLDER_TAG) {
            if let FileSystemSuccess::Folder { path, .. } = result {
                self.picked_folder = Some(path);
                self.files_dirty = true;
            }
        }
        while let Some(result) = self.rpc.next_file_result(SAVE_FILE_TAG) {
            if let FileSystemSuccess::File { path, .. } = result {
                self.saved_file = Some(path);
                self.files_dirty = true;
            }
        }
    }
}
