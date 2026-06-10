use crate::prelude::*;
use bevy::ecs::system::SystemParam;

#[derive(Default, Debug, Serialize, Deserialize, Clone, Event, EnumStr, Gui)]
pub enum Message {
    ConnectionStatus {
        connected: bool,
    },

    Topic {
        topic: String,
        payload: String,
        #[serde(default)]
        #[enum2egui(skip)]
        bytes: Option<Vec<u8>>,
    },

    Broker {
        message: BrokerServiceMessage,
    },

    Filesystem {
        message: FileSystemMessage,
    },

    Modal {
        message: ModalServiceMessage,
    },

    Notify {
        message: NotificationServiceMessage,
    },

    Project {
        message: ProjectMessage,
    },

    Tiles {
        message: TileTreeMessage,
    },

    #[default]
    #[serde(other)]
    #[enum2str("")]
    #[enum2egui(skip)]
    Empty,
}

#[derive(Default, Debug, Serialize, Deserialize, Clone, Event, EnumStr, Gui)]
pub enum ProjectMessage {
    #[enum2egui(skip)]
    ProjectLoaded {
        trees: Vec<egui_tiles::Tree<Pane>>,
        project_name: Option<String>,
        layout_names: Vec<String>,
        path: String,
    },
    #[enum2egui(skip)]
    ProjectSaved {
        path: String,
    },
    #[enum2egui(skip)]
    LayoutLoaded {
        tree: Box<egui_tiles::Tree<Pane>>,
        layout_name: Option<String>,
    },
    CloseProject,
    #[default]
    #[serde(other)]
    #[enum2str("")]
    Empty,
}

#[derive(Default, Debug, Serialize, Deserialize, Clone, Event, EnumStr, Gui)]
pub enum MessageBusEvent {
    RouteMessage(Message),
    #[default]
    #[serde(other)]
    #[enum2str("")]
    #[enum2egui(skip)]
    Empty,
}

pub struct MessageBusPlugin;

impl Plugin for MessageBusPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<Message>()
            .add_event::<MessageBusEvent>()
            .add_event::<ProjectMessage>()
            .add_systems(Update, route_messages_to_services)
            .add_plugins((
                SettingsPlugin,
                BrokerPlugin,
                ShellPlugin,
                TileTreePlugin,
                FilesystemServicePlugin,
                FpsPlugin,
                NotificationServicePlugin,
                ModalServicePlugin,
                ThemeServicePlugin,
            ));
    }
}

#[derive(SystemParam)]
pub struct ServiceWriters<'w> {
    pub broker: EventWriter<'w, BrokerServiceMessage>,
    pub filesystem: EventWriter<'w, FileSystemCommand>,
    pub tiles: EventWriter<'w, TileTreeMessage>,
    pub modal: EventWriter<'w, ModalServiceMessage>,
    pub notification: EventWriter<'w, NotificationServiceMessage>,
    pub project: EventWriter<'w, ProjectMessage>,
    pub topics: EventWriter<'w, TopicEvent>,
}

fn route_messages_to_services(
    mut message_bus_events: EventReader<MessageBusEvent>,
    mut messages: EventWriter<Message>,
    mut writers: ServiceWriters,
) {
    for event in message_bus_events.read() {
        match event {
            MessageBusEvent::RouteMessage(message) => {
                messages.send(message.clone());
                match message {
                    Message::Broker { message } => {
                        writers.broker.send(message.clone());
                    }
                    Message::Filesystem { message } => match message {
                        FileSystemMessage::Command(command) => {
                            writers.filesystem.send(command.clone());
                        }
                        FileSystemMessage::Result(result) => {
                            writers
                                .tiles
                                .send(TileTreeMessage::ProcessFileResult(result.clone()));
                        }
                        FileSystemMessage::Empty => {}
                    },
                    Message::Modal { message } => match message {
                        ModalServiceMessage::ShowConfirm { .. }
                        | ModalServiceMessage::CloseModal(_) => {
                            writers.modal.send(message.clone());
                        }
                        ModalServiceMessage::ModalResult { .. } | ModalServiceMessage::Empty => {}
                    },
                    Message::Notify { message } => {
                        writers.notification.send(message.clone());
                    }
                    Message::Project { message } => {
                        writers.project.send(message.clone());
                    }
                    Message::Tiles { message } => {
                        writers.tiles.send(message.clone());
                    }
                    Message::Topic {
                        topic,
                        payload,
                        bytes,
                    } => {
                        writers.topics.send(TopicEvent {
                            topic: topic.clone(),
                            payload: payload.clone(),
                            bytes: bytes.clone(),
                        });
                    }
                    Message::ConnectionStatus { .. } | Message::Empty => {}
                }
            }
            MessageBusEvent::Empty => {}
        }
    }
}
