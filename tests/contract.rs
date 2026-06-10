use hearsay::{
    ClientSettings, Route, connect, create_client,
    enum2contract::{self, EnumContract},
    next_message, publish, start_broker, subscribe,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, EnumContract, Serialize, Deserialize, Default)]
pub enum CounterContract {
    #[topic("counter/command-{channel}")]
    Command { command: CounterCommand },

    #[topic("counter/event-{channel}")]
    Event { event: CounterEvent },

    #[topic("")]
    #[default]
    #[serde(other)]
    Empty,
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum CounterCommand {
    Increment {
        amount: u32,
    },

    #[default]
    #[serde(other)]
    Empty,
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum CounterEvent {
    Incremented {
        total: u32,
    },

    #[default]
    #[serde(other)]
    Empty,
}

fn test_settings() -> ClientSettings {
    ClientSettings {
        autoreconnect: false,
        ..Default::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn contract_command_and_event_round_trip() -> hearsay::Result<()> {
    let broker_address = "127.0.0.1:9938";
    let _broker = start_broker(broker_address).await?;
    let channel = "all";

    let mut service = create_client("service", test_settings());
    connect(&mut service, broker_address).await?;
    subscribe(&mut service, &[&CounterContract::command_topic(channel)]).await?;

    let mut operator = create_client("operator", test_settings());
    connect(&mut operator, broker_address).await?;
    subscribe(&mut operator, &[&CounterContract::event_topic(channel)]).await?;

    tokio::time::sleep(Duration::from_millis(250)).await;

    let (command_topic, mut command_payload) = CounterContract::command(channel);
    command_payload.command = CounterCommand::Increment { amount: 5 };
    publish(&operator, &command_topic, &command_payload, Route::Global).await?;

    let message = tokio::time::timeout(Duration::from_secs(5), next_message(&mut service))
        .await?
        .expect("expected a command message");
    assert_eq!(message.topic, CounterContract::command_topic(channel));
    let received_command = CommandPayload::from_json(&message.payload)?;
    assert_eq!(
        received_command.command,
        CounterCommand::Increment { amount: 5 }
    );

    let CounterCommand::Increment { amount } = received_command.command else {
        panic!("expected an increment command");
    };
    let (event_topic, mut event_payload) = CounterContract::event(channel);
    event_payload.event = CounterEvent::Incremented { total: amount };
    publish(&service, &event_topic, &event_payload, Route::Global).await?;

    let message = tokio::time::timeout(Duration::from_secs(5), next_message(&mut operator))
        .await?
        .expect("expected an event message");
    assert_eq!(message.topic, CounterContract::event_topic(channel));
    let received_event = EventPayload::from_json(&message.payload)?;
    assert_eq!(received_event.event, CounterEvent::Incremented { total: 5 });
    Ok(())
}
