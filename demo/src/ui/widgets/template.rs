use crate::prelude::*;

const TEXT_TOPIC: &str = "template/text";
const BINARY_TOPIC: &str = "template/binary";
const PICK_FILE_TAG: &str = "template-file";
const PICK_FOLDER_TAG: &str = "template-folder";
const SAVE_FILE_TAG: &str = "template-save";
const MAX_RECEIVED_MESSAGES: usize = 25;

#[derive(Serialize)]
struct TemplateNote {
    text: String,
}

#[derive(Default, Debug, Clone)]
pub struct TemplateWidget {
    pub rpc: WidgetRpc,
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
}

impl MessageHandler for TemplateWidget {
    fn receive_message(&mut self, message: &Message) {
        self.rpc.receive_message(message);
    }

    fn drain_messages(&mut self) -> Vec<Message> {
        self.rpc.drain_messages()
    }
}

impl Widget for TemplateWidget {
    fn title(&self) -> String {
        "Template".to_string()
    }

    fn ui(&mut self, ui: &mut egui::Ui, context: &WidgetContext) {
        self.rpc.update(context);
        let is_connected = self.rpc.is_connected();

        if !self.subscribed && !self.auto_subscribe_pending && is_connected {
            self.subscribe();
        }

        self.process_messages();
        self.process_file_results();
        if let Some(result) = self.rpc.take_modal_result() {
            self.last_modal_result = Some(result);
        }

        ui.heading("Template Widget");
        ui.label(format!("Widget id: {}", self.rpc.widget_id()));
        if is_connected {
            ui.colored_label(ui.visuals().hyperlink_color, "Connected to broker");
        } else {
            ui.colored_label(ui.visuals().error_fg_color, "Disconnected from broker");
        }
        ui.separator();

        self.subscription_section(ui);
        ui.separator();
        self.publish_section(ui, is_connected);
        ui.separator();
        self.notification_section(ui);
        ui.separator();
        self.filesystem_section(ui);
        ui.separator();
        self.modal_section(ui);
    }
}

impl TemplateWidget {
    fn subscribe(&mut self) {
        self.rpc
            .subscribe_to_topics(&[TEXT_TOPIC.to_string(), BINARY_TOPIC.to_string()]);
        self.subscribed = true;
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
        }

        if let Some(binary_messages) = self.rpc.get_binary_messages_for_topic(BINARY_TOPIC) {
            self.received_binary_count += binary_messages.len();
            if let Some(last_message) = binary_messages.last() {
                self.last_binary_length = last_message.len();
            }
            self.rpc.clear_binary_topic_messages(&[BINARY_TOPIC]);
        }
    }

    fn process_file_results(&mut self) {
        while let Some(result) = self.rpc.next_file_result(PICK_FILE_TAG) {
            if let FileSystemSuccess::File { path, bytes, .. } = result {
                self.picked_file = Some((path, bytes.len()));
            }
        }
        while let Some(result) = self.rpc.next_file_result(PICK_FOLDER_TAG) {
            if let FileSystemSuccess::Folder { path, .. } = result {
                self.picked_folder = Some(path);
            }
        }
        while let Some(result) = self.rpc.next_file_result(SAVE_FILE_TAG) {
            if let FileSystemSuccess::File { path, .. } = result {
                self.saved_file = Some(path);
            }
        }
    }

    fn subscription_section(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Subscriptions").strong());
        let subscribed_topics = self.rpc.subscribed_topics();
        if subscribed_topics.is_empty() {
            ui.label("No active subscriptions");
        } else {
            for topic in &subscribed_topics {
                ui.monospace(topic);
            }
        }
        ui.horizontal(|ui| {
            if self.subscribed {
                if ui.button("Unsubscribe").clicked() {
                    self.rpc.unsubscribe_from_topics(&[
                        TEXT_TOPIC.to_string(),
                        BINARY_TOPIC.to_string(),
                    ]);
                    self.subscribed = false;
                    self.auto_subscribe_pending = true;
                }
            } else if ui.button("Subscribe").clicked() {
                self.subscribe();
                self.auto_subscribe_pending = false;
            }
        });
    }

    fn publish_section(&mut self, ui: &mut egui::Ui, is_connected: bool) {
        ui.label(egui::RichText::new("Publish").strong());
        ui.horizontal(|ui| {
            ui.label("Message:");
            ui.text_edit_singleline(&mut self.outgoing_text);
        });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(is_connected, egui::Button::new("Publish Typed"))
                .clicked()
            {
                let note = TemplateNote {
                    text: self.outgoing_text.clone(),
                };
                self.rpc.publish(TEXT_TOPIC, &note);
            }
            if ui
                .add_enabled(is_connected, egui::Button::new("Publish Raw"))
                .clicked()
            {
                let payload_json = format!("{{\"raw\":\"{}\"}}", self.outgoing_text);
                self.rpc.publish_json(TEXT_TOPIC, &payload_json);
            }
            if ui
                .add_enabled(is_connected, egui::Button::new("Publish Binary"))
                .clicked()
            {
                let bytes = self.outgoing_text.clone().into_bytes();
                self.rpc.publish_bytes(BINARY_TOPIC, &bytes);
            }
        });

        ui.label(egui::RichText::new("Received").strong());
        ui.label(format!(
            "Binary messages: {} (last {} bytes)",
            self.received_binary_count, self.last_binary_length
        ));
        if self.received_text.is_empty() {
            ui.label("No text messages received");
        } else {
            egui::ScrollArea::vertical()
                .max_height(120.0)
                .show(ui, |ui| {
                    for (message_index, payload) in self.received_text.iter().enumerate().rev() {
                        ui.horizontal(|ui| {
                            ui.monospace(format!("[{message_index}]"));
                            ui.colored_label(ui.visuals().hyperlink_color, payload);
                        });
                    }
                });
            if ui.button("Clear Received").clicked() {
                self.received_text.clear();
                self.received_binary_count = 0;
                self.last_binary_length = 0;
            }
        }
    }

    fn notification_section(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Notifications").strong());
        ui.horizontal(|ui| {
            if ui.button("Info").clicked() {
                self.rpc
                    .notify("An informational toast", NotificationKind::Info, 3.0);
            }
            if ui.button("Success").clicked() {
                self.rpc
                    .notify("A success toast", NotificationKind::Success, 3.0);
            }
            if ui.button("Warning").clicked() {
                self.rpc
                    .notify("A warning toast", NotificationKind::Warning, 3.0);
            }
            if ui.button("Error").clicked() {
                self.rpc
                    .notify("An error toast", NotificationKind::Error, 3.0);
            }
        });
    }

    fn filesystem_section(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Files").strong());
        ui.horizontal(|ui| {
            if ui.button("Pick File").clicked() {
                self.rpc.pick_file(PICK_FILE_TAG, "", Vec::new());
            }
            if ui.button("Pick Folder").clicked() {
                self.rpc.pick_directory(PICK_FOLDER_TAG);
            }
            if ui.button("Save File").clicked() {
                let bytes = self.outgoing_text.clone().into_bytes();
                self.rpc
                    .save_file(SAVE_FILE_TAG, bytes, "text", vec!["txt".to_string()]);
            }
        });
        if let Some((path, byte_count)) = &self.picked_file {
            ui.label(format!("Picked file: {path} ({byte_count} bytes)"));
        }
        if let Some(path) = &self.picked_folder {
            ui.label(format!("Picked folder: {path}"));
        }
        if let Some(path) = &self.saved_file {
            ui.label(format!("Saved file: {path}"));
        }
    }

    fn modal_section(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Modal").strong());
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.rpc.has_open_modal(), egui::Button::new("Show Modal"))
                .clicked()
            {
                self.rpc.show_modal(
                    "Template Modal",
                    "Confirm the template action?",
                    Some("Yes, proceed".to_string()),
                    Some("No, cancel".to_string()),
                );
            }
            match &self.last_modal_result {
                Some(ModalResult::Confirmed) => {
                    ui.colored_label(ui.visuals().hyperlink_color, "Last result: confirmed");
                }
                Some(ModalResult::Cancelled) => {
                    ui.colored_label(ui.visuals().warn_fg_color, "Last result: cancelled");
                }
                None => {
                    ui.label("No modal result yet");
                }
            }
        });
    }
}
