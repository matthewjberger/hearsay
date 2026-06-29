use crate::prelude::*;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

pub const BROKER_ADDRESS: &str = "127.0.0.1:9612";

#[derive(Resource, Clone, Debug, PartialEq)]
pub enum WindowRole {
    Primary,
    Child { broker_address: String },
}

impl WindowRole {
    pub fn detect() -> Self {
        match std::env::var(hearsay::BROKER_ADDRESS_VARIABLE) {
            Ok(broker_address) => Self::Child { broker_address },
            Err(_) => Self::Primary,
        }
    }

    pub fn is_primary(&self) -> bool {
        matches!(self, Self::Primary)
    }
}

#[derive(Resource, Default, Clone, Debug)]
pub struct BrokerConnectionStatus {
    pub connected: bool,
    pub address: String,
    pub client_id: String,
}

#[derive(Default, Event, Debug, Serialize, Deserialize, Clone, EnumStr, Gui)]
pub enum BrokerServiceMessage {
    Publish {
        topic: String,
        message: String,
    },
    PublishBytes {
        topic: String,
        bytes: Vec<u8>,
    },
    Subscribe {
        topics: Vec<String>,
        widget_id: String,
    },
    Unsubscribe {
        topics: Vec<String>,
        widget_id: String,
    },
    WidgetRemoved {
        widget_id: String,
    },
    SpawnWindow,
    #[default]
    #[serde(other)]
    #[enum2str("")]
    #[enum2egui(skip)]
    Empty,
}

#[derive(Event, Debug, Clone)]
pub struct TopicEvent {
    pub topic: String,
    pub payload: String,
    pub bytes: Option<Vec<u8>>,
}

pub(crate) enum RuntimeCommand {
    Publish { topic: String, message: String },
    PublishBytes { topic: String, bytes: Vec<u8> },
    Subscribe { topics: Vec<String> },
    Unsubscribe { topics: Vec<String> },
    SpawnWindow,
}

pub(crate) enum RuntimeEvent {
    Connected {
        client_id: String,
        address: String,
    },
    Disconnected,
    Failed {
        reason: String,
    },
    Inbound {
        topic: String,
        payload: String,
        bytes: Option<Vec<u8>>,
    },
}

#[derive(Resource)]
pub struct BrokerLink {
    pub(crate) command_sender: UnboundedSender<RuntimeCommand>,
    pub(crate) event_receiver: UnboundedReceiver<RuntimeEvent>,
}

#[derive(Resource, Default)]
pub struct SubscriptionRegistry {
    pub topic_subscribers: HashMap<String, Vec<String>>,
}

pub struct BrokerPlugin;

impl Plugin for BrokerPlugin {
    fn build(&self, app: &mut App) {
        let role = app.world().resource::<WindowRole>().clone();
        let (command_sender, command_receiver) = unbounded_channel();
        let (event_sender, event_receiver) = unbounded_channel();
        std::thread::spawn(move || run_broker_runtime(role, command_receiver, event_sender));
        app.insert_resource(BrokerLink {
            command_sender,
            event_receiver,
        })
        .init_resource::<SubscriptionRegistry>()
        .init_resource::<BrokerConnectionStatus>()
        .add_event::<BrokerServiceMessage>()
        .add_event::<TopicEvent>()
        .add_systems(
            Update,
            (process_broker_service_messages, drain_runtime_events),
        );
    }
}

fn run_broker_runtime(
    role: WindowRole,
    command_receiver: UnboundedReceiver<RuntimeCommand>,
    event_sender: UnboundedSender<RuntimeEvent>,
) {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = event_sender.send(RuntimeEvent::Failed {
                reason: error.to_string(),
            });
            return;
        }
    };
    runtime.block_on(broker_runtime_loop(role, command_receiver, event_sender));
}

async fn broker_runtime_loop(
    role: WindowRole,
    mut command_receiver: UnboundedReceiver<RuntimeCommand>,
    event_sender: UnboundedSender<RuntimeEvent>,
) {
    let (broker, address) = match role {
        WindowRole::Primary => match hearsay::start_broker(BROKER_ADDRESS).await {
            Ok(broker) => (Some(broker), BROKER_ADDRESS.to_string()),
            Err(error) => {
                let _ = event_sender.send(RuntimeEvent::Failed {
                    reason: error.to_string(),
                });
                return;
            }
        },
        WindowRole::Child { broker_address } => (None, broker_address),
    };

    let client = hearsay::create_client("window", hearsay::ClientSettings::default());
    if let Err(error) = hearsay::connect(&client, &address).await {
        let _ = event_sender.send(RuntimeEvent::Failed {
            reason: error.to_string(),
        });
        return;
    }
    let client_id = hearsay::client_id(&client).await;
    let _ = event_sender.send(RuntimeEvent::Connected {
        client_id,
        address: address.clone(),
    });

    let mut window_counter: u32 = 0;
    let mut connected = true;
    loop {
        if connected {
            tokio::select! {
                command = command_receiver.recv() => match command {
                    Some(command) => {
                        execute_runtime_command(command, &broker, &client, &mut window_counter).await;
                    }
                    None => break,
                },
                inbound = hearsay::next_message(&client) => match inbound {
                    Some(message) => {
                        let (payload, bytes) = match message.body {
                            hearsay::Body::Text(text) => (text, None),
                            hearsay::Body::Binary(bytes) => (String::new(), Some(bytes)),
                        };
                        let _ = event_sender.send(RuntimeEvent::Inbound {
                            topic: message.topic,
                            payload,
                            bytes,
                        });
                    }
                    None => {
                        connected = false;
                        let _ = event_sender.send(RuntimeEvent::Disconnected);
                    }
                },
            }
        } else {
            tokio::select! {
                command = command_receiver.recv() => match command {
                    Some(command) => {
                        execute_runtime_command(command, &broker, &client, &mut window_counter).await;
                    }
                    None => break,
                },
                _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {
                    if hearsay::is_connected(&client).await {
                        connected = true;
                        let client_id = hearsay::client_id(&client).await;
                        let _ = event_sender.send(RuntimeEvent::Connected {
                            client_id,
                            address: address.clone(),
                        });
                    }
                },
            }
        }
    }
}

async fn execute_runtime_command(
    command: RuntimeCommand,
    broker: &Option<hearsay::Broker>,
    client: &hearsay::Client,
    window_counter: &mut u32,
) {
    match command {
        RuntimeCommand::Publish { topic, message } => {
            let _ = hearsay::publish_json(client, &topic, &message, hearsay::Route::Global).await;
        }
        RuntimeCommand::PublishBytes { topic, bytes } => {
            let _ = hearsay::publish_bytes(client, &topic, &bytes, hearsay::Route::Global).await;
        }
        RuntimeCommand::Subscribe { topics } => {
            let topic_references: Vec<&str> = topics.iter().map(String::as_str).collect();
            let _ = hearsay::subscribe(client, &topic_references).await;
        }
        RuntimeCommand::Unsubscribe { topics } => {
            let topic_references: Vec<&str> = topics.iter().map(String::as_str).collect();
            let _ = hearsay::unsubscribe(client, &topic_references).await;
        }
        RuntimeCommand::SpawnWindow => {
            let Some(broker) = broker else {
                return;
            };
            let Ok(executable) = std::env::current_exe() else {
                return;
            };
            *window_counter += 1;
            let _ = hearsay::spawn_app(
                broker,
                hearsay::App {
                    name: format!("window-{window_counter}"),
                    path: executable.display().to_string(),
                    restart_policy: hearsay::RestartPolicy::Never,
                    ..Default::default()
                },
            )
            .await;
        }
    }
}

fn process_broker_service_messages(
    mut events: EventReader<BrokerServiceMessage>,
    link: Res<BrokerLink>,
    mut registry: ResMut<SubscriptionRegistry>,
) {
    for event in events.read() {
        match event {
            BrokerServiceMessage::Publish { topic, message } => {
                let _ = link.command_sender.send(RuntimeCommand::Publish {
                    topic: topic.clone(),
                    message: message.clone(),
                });
            }
            BrokerServiceMessage::PublishBytes { topic, bytes } => {
                let _ = link.command_sender.send(RuntimeCommand::PublishBytes {
                    topic: topic.clone(),
                    bytes: bytes.clone(),
                });
            }
            BrokerServiceMessage::Subscribe { topics, widget_id } => {
                for topic in topics {
                    let subscribers = registry.topic_subscribers.entry(topic.clone()).or_default();
                    if !subscribers.contains(widget_id) {
                        subscribers.push(widget_id.clone());
                    }
                }
                let _ = link.command_sender.send(RuntimeCommand::Subscribe {
                    topics: topics.clone(),
                });
            }
            BrokerServiceMessage::Unsubscribe { topics, widget_id } => {
                let mut orphaned_topics = Vec::new();
                for topic in topics {
                    if let Some(subscribers) = registry.topic_subscribers.get_mut(topic) {
                        subscribers.retain(|subscriber| subscriber != widget_id);
                        if subscribers.is_empty() {
                            registry.topic_subscribers.remove(topic);
                            orphaned_topics.push(topic.clone());
                        }
                    }
                }
                if !orphaned_topics.is_empty() {
                    let _ = link.command_sender.send(RuntimeCommand::Unsubscribe {
                        topics: orphaned_topics,
                    });
                }
            }
            BrokerServiceMessage::WidgetRemoved { widget_id } => {
                let mut orphaned_topics = Vec::new();
                registry.topic_subscribers.retain(|topic, subscribers| {
                    subscribers.retain(|subscriber| subscriber != widget_id);
                    if subscribers.is_empty() {
                        orphaned_topics.push(topic.clone());
                        false
                    } else {
                        true
                    }
                });
                if !orphaned_topics.is_empty() {
                    let _ = link.command_sender.send(RuntimeCommand::Unsubscribe {
                        topics: orphaned_topics,
                    });
                }
            }
            BrokerServiceMessage::SpawnWindow => {
                let _ = link.command_sender.send(RuntimeCommand::SpawnWindow);
            }
            BrokerServiceMessage::Empty => {}
        }
    }
}

fn drain_runtime_events(
    mut link: ResMut<BrokerLink>,
    mut status: ResMut<BrokerConnectionStatus>,
    mut topic_events: EventWriter<TopicEvent>,
    mut message_bus_events: EventWriter<MessageBusEvent>,
    role: Res<WindowRole>,
    mut exit_events: EventWriter<AppExit>,
) {
    while let Ok(event) = link.event_receiver.try_recv() {
        match event {
            RuntimeEvent::Connected { client_id, address } => {
                status.connected = true;
                status.address = address;
                status.client_id = client_id;
                message_bus_events.send(MessageBusEvent::RouteMessage(Message::ConnectionStatus {
                    connected: true,
                }));
            }
            RuntimeEvent::Disconnected => {
                status.connected = false;
                message_bus_events.send(MessageBusEvent::RouteMessage(Message::ConnectionStatus {
                    connected: false,
                }));
                if !role.is_primary() {
                    exit_events.send(AppExit::Success);
                }
            }
            RuntimeEvent::Failed { reason } => {
                status.connected = false;
                message_bus_events.send(MessageBusEvent::RouteMessage(Message::Notify {
                    message: NotificationServiceMessage::Show {
                        text: format!("Broker runtime failed: {reason}"),
                        kind: NotificationKind::Error,
                        duration_in_seconds: 8.0,
                    },
                }));
                if !role.is_primary() {
                    exit_events.send(AppExit::Success);
                }
            }
            RuntimeEvent::Inbound {
                topic,
                payload,
                bytes,
            } => {
                topic_events.send(TopicEvent {
                    topic,
                    payload,
                    bytes,
                });
            }
        }
    }
}
