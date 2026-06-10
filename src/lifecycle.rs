use crate::{Client, ClientSettings, Message, Result, connect, create_client, next_message};
use std::{
    future::Future,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interrupt {
    UpdateImmediately,
    Stop,
}

pub trait Lifecycle: Send {
    fn initialize(&mut self, _client: &mut Client) -> impl Future<Output = ()> + Send {
        std::future::ready(())
    }

    fn update(&mut self, _client: &mut Client) -> impl Future<Output = ()> + Send {
        std::future::ready(())
    }

    fn receive_message(
        &mut self,
        _message: &Message,
        _client: &mut Client,
    ) -> impl Future<Output = Option<Interrupt>> + Send {
        std::future::ready(None)
    }
}

#[derive(Debug, Clone)]
pub struct LifecycleSettings {
    pub name: String,
    pub broker_address: String,
    pub update_interval: Duration,
}

pub async fn run_lifecycle(
    mut lifecycle: impl Lifecycle,
    settings: LifecycleSettings,
) -> Result<()> {
    let mut client = create_client(&settings.name, ClientSettings::default());
    connect(&mut client, &settings.broker_address).await?;
    lifecycle.initialize(&mut client).await;
    loop {
        let deadline = Instant::now() + settings.update_interval;
        let interrupt =
            receive_messages_until_deadline(&mut lifecycle, &mut client, deadline).await;
        if interrupt == Some(Interrupt::Stop) {
            return Ok(());
        }
        lifecycle.update(&mut client).await;
    }
}

async fn receive_messages_until_deadline(
    lifecycle: &mut impl Lifecycle,
    client: &mut Client,
    deadline: Instant,
) -> Option<Interrupt> {
    loop {
        let now = Instant::now();
        if now >= deadline {
            return None;
        }
        let remaining = deadline - now;
        match tokio::time::timeout(remaining, next_message(client)).await {
            Ok(Some(message)) => match lifecycle.receive_message(&message, client).await {
                Some(Interrupt::UpdateImmediately) => {
                    return Some(Interrupt::UpdateImmediately);
                }
                Some(Interrupt::Stop) => return Some(Interrupt::Stop),
                None => {}
            },
            _ => return None,
        }
    }
}
