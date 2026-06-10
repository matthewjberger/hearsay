use hearsay::{
    Client, ClientSettings, Interrupt, Lifecycle, LifecycleSettings, Message, Route,
    broker_is_running, connect, create_client,
    enum2contract::{self, EnumContract},
    next_message, publish, run_lifecycle, start_broker, subscribe,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[cfg(feature = "spawn")]
use hearsay::{App, AppStatus, app_status, drain_output, spawn_app};

const BROKER_ADDRESS: &str = "127.0.0.1:9612";

#[derive(Debug, EnumContract, Serialize, Deserialize, Default)]
pub enum PingPongContract {
    #[topic("pingpong/command-{channel}")]
    Command { command: PingPongCommand },

    #[topic("pingpong/event-{channel}")]
    Event { event: PingPongEvent },

    #[topic("")]
    #[default]
    #[serde(other)]
    Empty,
}

impl PingPongContract {
    pub const BROADCAST: &'static str = "broadcast";
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum PingPongCommand {
    Ping,

    #[default]
    #[serde(other)]
    Empty,
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum PingPongEvent {
    Pong {
        value: u32,
    },

    #[default]
    #[serde(other)]
    Empty,
}

struct Responder;

impl Lifecycle for Responder {
    async fn initialize(&mut self, client: &mut Client) {
        let topic = PingPongContract::command_topic(PingPongContract::BROADCAST);
        let _ = subscribe(client, &[&topic]).await;
    }

    async fn receive_message(
        &mut self,
        message: &Message,
        client: &mut Client,
    ) -> Option<Interrupt> {
        match message.topic.as_str() {
            topic if PingPongContract::command_topic(PingPongContract::BROADCAST) == topic => {
                let Ok(payload) = CommandPayload::from_json(&message.payload) else {
                    return None;
                };
                if matches!(payload.command, PingPongCommand::Ping) {
                    println!("[responder] received ping, sending pong");
                    let (event_topic, mut event_payload) =
                        PingPongContract::event(PingPongContract::BROADCAST);
                    event_payload.event = PingPongEvent::Pong { value: 42 };
                    let _ = publish(client, &event_topic, &event_payload, Route::Global).await;
                }
            }
            _ => {}
        }
        None
    }
}

#[tokio::main]
async fn main() -> hearsay::Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("broker") => run_broker().await,
        Some("responder") => run_lifecycle(Responder, responder_settings()).await,
        Some("requester") => run_requester().await,
        #[cfg(feature = "spawn")]
        Some("host") => run_host().await,
        Some(role) => Err(format!("unknown role: {role}").into()),
        None => run_all_in_one().await,
    }
}

#[cfg(feature = "spawn")]
async fn run_host() -> hearsay::Result<()> {
    let broker = start_broker(BROKER_ADDRESS).await?;
    let executable = std::env::current_exe()?.display().to_string();
    spawn_app(
        &broker,
        App {
            name: "responder".to_string(),
            path: executable.clone(),
            args: "responder".to_string(),
            ..Default::default()
        },
    )
    .await?;
    spawn_app(
        &broker,
        App {
            name: "requester".to_string(),
            path: executable,
            args: "requester".to_string(),
            ..Default::default()
        },
    )
    .await?;

    loop {
        for line in drain_output(&broker).await {
            println!("[host] {}: {}", line.app_name, line.line);
        }
        if app_status(&broker, "requester").await == AppStatus::ExitedSuccessfully {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn responder_settings() -> LifecycleSettings {
    LifecycleSettings {
        name: "responder".to_string(),
        broker_address: BROKER_ADDRESS.to_string(),
        update_interval: Duration::from_millis(50),
    }
}

async fn run_broker() -> hearsay::Result<()> {
    let broker = start_broker(BROKER_ADDRESS).await?;
    println!("[broker] listening on {BROKER_ADDRESS}");
    while broker_is_running(&broker) {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Ok(())
}

async fn run_requester() -> hearsay::Result<()> {
    let mut requester = create_client("requester", ClientSettings::default());
    connect(&mut requester, BROKER_ADDRESS).await?;
    subscribe(
        &mut requester,
        &[&PingPongContract::event_topic(PingPongContract::BROADCAST)],
    )
    .await?;

    loop {
        let (command_topic, mut command_payload) =
            PingPongContract::command(PingPongContract::BROADCAST);
        command_payload.command = PingPongCommand::Ping;
        publish(&requester, &command_topic, &command_payload, Route::Global).await?;

        let received =
            tokio::time::timeout(Duration::from_millis(100), next_message(&mut requester)).await;
        let Ok(Some(message)) = received else {
            continue;
        };
        if message.topic != PingPongContract::event_topic(PingPongContract::BROADCAST) {
            continue;
        }
        let Ok(event_payload) = EventPayload::from_json(&message.payload) else {
            continue;
        };
        if let PingPongEvent::Pong { value } = event_payload.event {
            println!("[requester] received pong with value {value}");
            return Ok(());
        }
    }
}

async fn run_all_in_one() -> hearsay::Result<()> {
    let _broker = start_broker(BROKER_ADDRESS).await?;
    tokio::spawn(run_lifecycle(Responder, responder_settings()));
    run_requester().await
}
