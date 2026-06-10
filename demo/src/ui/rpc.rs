use crate::prelude::*;

#[derive(Debug, Clone)]
pub struct WidgetRpc {
    subscribed_topics: HashSet<String>,
    received_messages: HashMap<String, Vec<String>>,
    received_binary_messages: HashMap<String, Vec<Vec<u8>>>,
    file_results: HashMap<String, Vec<FileSystemSuccess>>,
    current_modal_result: Option<bool>,
    last_modal_id: Option<String>,
    modal_counter: u32,
    widget_id: String,
    messages: Vec<Message>,
    connected: bool,
}

impl Default for WidgetRpc {
    fn default() -> Self {
        Self {
            subscribed_topics: HashSet::new(),
            received_messages: HashMap::new(),
            received_binary_messages: HashMap::new(),
            file_results: HashMap::new(),
            current_modal_result: None,
            last_modal_id: None,
            modal_counter: 0,
            widget_id: uuid::Uuid::new_v4().to_string(),
            messages: Vec::new(),
            connected: false,
        }
    }
}

impl MessageHandler for WidgetRpc {
    fn receive_message(&mut self, message: &Message) {
        match message {
            Message::Topic {
                topic,
                payload,
                bytes,
            } => {
                self.process_topic_message(topic, payload);
                if let Some(bytes) = bytes {
                    self.process_binary_topic_message(topic, bytes);
                }
            }
            Message::Modal {
                message: ModalServiceMessage::ModalResult { id, confirmed },
            } => {
                if let Some(last_modal_id) = &self.last_modal_id
                    && id == last_modal_id
                {
                    self.current_modal_result = Some(*confirmed);
                }
            }
            Message::Filesystem {
                message: FileSystemMessage::Result(FileSystemResult::Success(success)),
            } => match success {
                FileSystemSuccess::File { tag, .. } | FileSystemSuccess::Folder { tag, .. } => {
                    let prefix = format!("{}::", self.widget_id);
                    if tag.starts_with(&prefix) {
                        self.file_results
                            .entry(tag.clone())
                            .or_default()
                            .push(success.clone());
                    }
                }
                FileSystemSuccess::Empty => {}
            },
            Message::ConnectionStatus { connected } => {
                self.connected = *connected;
            }
            _ => {}
        }
    }

    fn drain_messages(&mut self) -> Vec<Message> {
        self.messages.drain(..).collect()
    }
}

impl WidgetRpc {
    pub fn widget_id(&self) -> &str {
        &self.widget_id
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn update(&mut self, context: &WidgetContext) -> bool {
        let was_connected = self.connected;
        self.connected = context.is_connected;
        if !was_connected && self.connected {
            self.register_default_subscriptions();
        }
        self.connected
    }

    fn register_default_subscriptions(&mut self) {
        let topics: Vec<String> = self.subscribed_topics.iter().cloned().collect();
        if !topics.is_empty() {
            self.messages.push(Message::Broker {
                message: BrokerServiceMessage::Subscribe {
                    topics,
                    widget_id: self.widget_id.clone(),
                },
            });
        }
    }

    fn add_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    pub fn subscribe_to_topics(&mut self, topics: &[String]) {
        for topic in topics {
            self.subscribe_to_topic(topic);
        }
    }

    pub fn subscribe_to_topic(&mut self, topic: &str) {
        if self.subscribed_topics.contains(topic) {
            return;
        }
        self.subscribed_topics.insert(topic.to_string());
        self.messages.push(Message::Broker {
            message: BrokerServiceMessage::Subscribe {
                topics: vec![topic.to_string()],
                widget_id: self.widget_id.clone(),
            },
        });
    }

    pub fn unsubscribe_from_topics(&mut self, topics: &[String]) {
        for topic in topics {
            self.subscribed_topics.remove(topic);
        }
        self.messages.push(Message::Broker {
            message: BrokerServiceMessage::Unsubscribe {
                topics: topics.to_vec(),
                widget_id: self.widget_id.clone(),
            },
        });
    }

    pub fn subscribed_topics(&self) -> Vec<String> {
        let mut topics: Vec<String> = self.subscribed_topics.iter().cloned().collect();
        topics.sort();
        topics
    }

    fn process_topic_message(&mut self, topic: &str, payload: &str) {
        if !self.subscribed_topics.contains(topic) {
            return;
        }
        self.received_messages
            .entry(topic.to_string())
            .or_default()
            .push(payload.to_string());
    }

    fn process_binary_topic_message(&mut self, topic: &str, bytes: &[u8]) {
        if !self.subscribed_topics.contains(topic) {
            return;
        }
        self.received_binary_messages
            .entry(topic.to_string())
            .or_default()
            .push(bytes.to_vec());
    }

    pub fn get_messages_for_topic(&self, topic: &str) -> Option<&Vec<String>> {
        self.received_messages.get(topic)
    }

    pub fn clear_topic_messages(&mut self, topics: &[&str]) {
        for topic in topics {
            self.received_messages.remove(*topic);
        }
    }

    pub fn get_binary_messages_for_topic(&self, topic: &str) -> Option<&Vec<Vec<u8>>> {
        self.received_binary_messages.get(topic)
    }

    pub fn clear_binary_topic_messages(&mut self, topics: &[&str]) {
        for topic in topics {
            self.received_binary_messages.remove(*topic);
        }
    }

    pub fn publish<T: Serialize>(&mut self, topic: &str, payload: &T) {
        match serde_json::to_string(payload) {
            Ok(payload_json) => {
                self.publish_json(topic, &payload_json);
            }
            Err(error) => {
                bevy::log::error!("Failed to serialize payload: {error}");
            }
        }
    }

    pub fn publish_json(&mut self, topic: &str, payload_json: &str) {
        self.messages.push(Message::Broker {
            message: BrokerServiceMessage::Publish {
                topic: topic.to_string(),
                message: payload_json.to_string(),
            },
        });
    }

    pub fn publish_bytes(&mut self, topic: &str, bytes: &[u8]) {
        self.messages.push(Message::Broker {
            message: BrokerServiceMessage::PublishBytes {
                topic: topic.to_string(),
                bytes: bytes.to_vec(),
            },
        });
    }

    pub fn notify(&mut self, text: &str, kind: NotificationKind, duration: f64) {
        self.messages.push(Message::Notify {
            message: NotificationServiceMessage::Show {
                text: text.to_string(),
                kind,
                duration_in_seconds: duration,
            },
        });
    }

    fn scoped_tag(&self, tag: &str) -> String {
        format!("{}::{}", self.widget_id, tag)
    }

    pub fn pick_file(&mut self, tag: &str, filter_name: &str, extensions: Vec<String>) {
        let tag = self.scoped_tag(tag);
        self.messages.push(Message::Filesystem {
            message: FileSystemMessage::Command(FileSystemCommand::PickFile {
                tag,
                filter_name: filter_name.to_string(),
                extensions,
            }),
        });
    }

    pub fn pick_directory(&mut self, tag: &str) {
        let tag = self.scoped_tag(tag);
        self.messages.push(Message::Filesystem {
            message: FileSystemMessage::Command(FileSystemCommand::PickFolder { tag }),
        });
    }

    pub fn save_file(
        &mut self,
        tag: &str,
        bytes: Vec<u8>,
        filter_name: &str,
        extensions: Vec<String>,
    ) {
        let tag = self.scoped_tag(tag);
        self.messages.push(Message::Filesystem {
            message: FileSystemMessage::Command(FileSystemCommand::SaveFile {
                tag,
                bytes,
                filter_name: filter_name.to_string(),
                extensions,
            }),
        });
    }

    pub fn next_file_result(&mut self, tag: &str) -> Option<FileSystemSuccess> {
        let tag = self.scoped_tag(tag);
        if let Some(results) = self.file_results.get_mut(&tag)
            && !results.is_empty()
        {
            return Some(results.remove(0));
        }
        None
    }

    pub fn show_modal(
        &mut self,
        title: &str,
        body: &str,
        confirm_text: Option<String>,
        cancel_text: Option<String>,
    ) {
        self.modal_counter += 1;
        let modal_id = format!("modal_{}_{}", self.widget_id, self.modal_counter);
        self.last_modal_id = Some(modal_id.clone());
        self.current_modal_result = None;

        self.add_message(Message::Modal {
            message: ModalServiceMessage::ShowConfirm {
                id: modal_id,
                title: title.to_string(),
                body: body.to_string(),
                confirm_text,
                cancel_text,
            },
        });
    }

    pub fn take_modal_result(&mut self) -> Option<ModalResult> {
        if let Some(result) = self.current_modal_result.take() {
            self.last_modal_id = None;
            Some(ModalResult::from(result))
        } else {
            None
        }
    }

    pub fn has_open_modal(&self) -> bool {
        self.last_modal_id.is_some()
    }
}
