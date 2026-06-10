use crate::messages::BrokerServiceMessage;
use std::collections::HashMap;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

pub const BROKER_ADDRESS: &str = "127.0.0.1:9612";
pub const WEBSOCKET_ADDRESS: &str = "127.0.0.1:9613";

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Default, Clone, Debug)]
pub struct BrokerConnectionStatus {
    pub connected: bool,
    pub address: String,
    pub client_id: String,
}

pub enum RuntimeCommand {
    Publish { topic: String, message: String },
    PublishBytes { topic: String, bytes: Vec<u8> },
    Subscribe { topics: Vec<String> },
    Unsubscribe { topics: Vec<String> },
    SpawnWindow,
}

pub enum RuntimeEvent {
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

pub struct BrokerLink {
    pub command_sender: UnboundedSender<RuntimeCommand>,
    pub event_receiver: UnboundedReceiver<RuntimeEvent>,
}

impl BrokerLink {
    pub fn drain_events(&mut self) -> Vec<RuntimeEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_receiver.try_recv() {
            events.push(event);
        }
        events
    }
}

#[derive(Default)]
pub struct SubscriptionRegistry {
    pub topic_subscribers: HashMap<String, Vec<String>>,
}

pub fn start_broker_runtime(role: WindowRole) -> BrokerLink {
    let (command_sender, command_receiver) = unbounded_channel();
    let (event_sender, event_receiver) = unbounded_channel();
    std::thread::spawn(move || run_broker_runtime(role, command_receiver, event_sender));
    BrokerLink {
        command_sender,
        event_receiver,
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
            Ok(broker) => {
                let _ = hearsay::start_websocket_listener(&broker, WEBSOCKET_ADDRESS).await;
                (Some(broker), BROKER_ADDRESS.to_string())
            }
            Err(error) => {
                let _ = event_sender.send(RuntimeEvent::Failed {
                    reason: error.to_string(),
                });
                return;
            }
        },
        WindowRole::Child { broker_address } => (None, broker_address),
    };

    let mut client = hearsay::create_client("window", hearsay::ClientSettings::default());
    if let Err(error) = hearsay::connect(&mut client, &address).await {
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
                        execute_runtime_command(command, &broker, &mut client, &mut window_counter).await;
                    }
                    None => break,
                },
                inbound = hearsay::next_message(&mut client) => match inbound {
                    Some(message) => {
                        let _ = event_sender.send(RuntimeEvent::Inbound {
                            topic: message.topic,
                            payload: message.payload,
                            bytes: message.bytes,
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
                        execute_runtime_command(command, &broker, &mut client, &mut window_counter).await;
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
    client: &mut hearsay::Client,
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

pub fn process_broker_service_message(
    message: &BrokerServiceMessage,
    link: &BrokerLink,
    registry: &mut SubscriptionRegistry,
) {
    match message {
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
