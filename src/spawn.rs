use crate::{Broker, Result};
use std::{
    collections::HashMap,
    process::Stdio,
    sync::{Arc, Weak},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::{Child, Command},
    sync::{
        Mutex,
        mpsc::{self, UnboundedReceiver, UnboundedSender},
    },
};

pub const BROKER_ADDRESS_VARIABLE: &str = "HEARSAY_BROKER";

const SUPERVISION_INTERVAL: Duration = Duration::from_millis(500);

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Default)]
pub struct App {
    pub name: String,
    pub path: String,
    pub args: String,
    pub environment_variables: HashMap<String, String>,
    pub restart_policy: RestartPolicy,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RestartPolicy {
    #[default]
    Never,
    OnFailure,
    Always,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppStatus {
    NotFound,
    Running,
    Stopped,
    ExitedSuccessfully,
    ExitedWithError(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone)]
pub struct OutputLine {
    pub app_name: String,
    pub stream: OutputStream,
    pub line: String,
    pub timestamp_ms: u64,
}

pub(crate) struct Spawner {
    state: Arc<Mutex<SpawnerState>>,
}

struct SpawnerState {
    broker_address: String,
    apps: HashMap<String, ManagedApp>,
    output_sender: UnboundedSender<OutputLine>,
    output_receiver: UnboundedReceiver<OutputLine>,
}

struct ManagedApp {
    descriptor: App,
    child: Option<Child>,
    last_status: AppStatus,
}

pub(crate) fn create_spawner(broker_address: &str) -> Spawner {
    let (output_sender, output_receiver) = mpsc::unbounded_channel();
    let state = Arc::new(Mutex::new(SpawnerState {
        broker_address: broker_address.to_string(),
        apps: HashMap::new(),
        output_sender,
        output_receiver,
    }));
    tokio::spawn(supervision_task(Arc::downgrade(&state)));
    Spawner { state }
}

pub async fn spawn_app(broker: &Broker, app: App) -> Result<()> {
    let mut state = broker.spawner.state.lock().await;
    if let Some(managed) = state.apps.get_mut(&app.name)
        && matches!(poll_status(managed), AppStatus::Running)
    {
        return Err(format!("app '{}' is already running", app.name).into());
    }
    let child = launch(&app, &state.broker_address, &state.output_sender)?;
    state.apps.insert(
        app.name.clone(),
        ManagedApp {
            descriptor: app,
            child: Some(child),
            last_status: AppStatus::Running,
        },
    );
    Ok(())
}

pub async fn stop_app(broker: &Broker, name: &str) -> Result<()> {
    let mut state = broker.spawner.state.lock().await;
    let Some(managed) = state.apps.get_mut(name) else {
        return Err(format!("app '{name}' not found").into());
    };
    if let Some(child) = managed.child.as_mut() {
        let _ = child.kill().await;
    }
    managed.child = None;
    managed.last_status = AppStatus::Stopped;
    Ok(())
}

pub async fn restart_app(broker: &Broker, name: &str) -> Result<()> {
    let mut state = broker.spawner.state.lock().await;
    let Some(managed) = state.apps.get_mut(name) else {
        return Err(format!("app '{name}' not found").into());
    };
    if let Some(child) = managed.child.as_mut() {
        let _ = child.kill().await;
    }
    managed.child = None;
    let descriptor = managed.descriptor.clone();
    let broker_address = state.broker_address.clone();
    let output_sender = state.output_sender.clone();
    let child = launch(&descriptor, &broker_address, &output_sender)?;
    if let Some(managed) = state.apps.get_mut(name) {
        managed.child = Some(child);
        managed.last_status = AppStatus::Running;
    }
    Ok(())
}

pub async fn app_status(broker: &Broker, name: &str) -> AppStatus {
    let mut state = broker.spawner.state.lock().await;
    match state.apps.get_mut(name) {
        Some(managed) => poll_status(managed),
        None => AppStatus::NotFound,
    }
}

pub async fn app_statuses(broker: &Broker) -> Vec<(String, AppStatus)> {
    let mut state = broker.spawner.state.lock().await;
    let mut statuses: Vec<(String, AppStatus)> = state
        .apps
        .iter_mut()
        .map(|(name, managed)| (name.clone(), poll_status(managed)))
        .collect();
    statuses.sort_by(|left, right| left.0.cmp(&right.0));
    statuses
}

pub async fn drain_output(broker: &Broker) -> Vec<OutputLine> {
    let mut state = broker.spawner.state.lock().await;
    let mut lines = Vec::new();
    while let Ok(line) = state.output_receiver.try_recv() {
        lines.push(line);
    }
    lines
}

fn launch(
    app: &App,
    broker_address: &str,
    output_sender: &UnboundedSender<OutputLine>,
) -> Result<Child> {
    let mut command = Command::new(&app.path);
    if !app.args.is_empty() {
        command.args(app.args.split_whitespace());
    }
    command.envs(&app.environment_variables);
    command.env(BROKER_ADDRESS_VARIABLE, broker_address);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.kill_on_drop(true);
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);
    let mut child = command.spawn()?;
    if let Some(stdout) = child.stdout.take() {
        tokio::spawn(read_output(
            stdout,
            app.name.clone(),
            OutputStream::Stdout,
            output_sender.clone(),
        ));
    }
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(read_output(
            stderr,
            app.name.clone(),
            OutputStream::Stderr,
            output_sender.clone(),
        ));
    }
    Ok(child)
}

async fn read_output(
    stream: impl AsyncRead + Unpin + Send + 'static,
    app_name: String,
    output_stream: OutputStream,
    sender: UnboundedSender<OutputLine>,
) {
    let mut lines = BufReader::new(stream).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis() as u64);
        let output_line = OutputLine {
            app_name: app_name.clone(),
            stream: output_stream,
            line,
            timestamp_ms,
        };
        if sender.send(output_line).is_err() {
            break;
        }
    }
}

fn poll_status(managed: &mut ManagedApp) -> AppStatus {
    let Some(child) = managed.child.as_mut() else {
        return managed.last_status.clone();
    };
    match child.try_wait() {
        Ok(None) => AppStatus::Running,
        Ok(Some(exit_status)) => {
            let status = if exit_status.success() {
                AppStatus::ExitedSuccessfully
            } else {
                AppStatus::ExitedWithError(exit_status.to_string())
            };
            managed.child = None;
            managed.last_status = status.clone();
            status
        }
        Err(error) => {
            let status = AppStatus::ExitedWithError(error.to_string());
            managed.child = None;
            managed.last_status = status.clone();
            status
        }
    }
}

async fn supervision_task(state: Weak<Mutex<SpawnerState>>) {
    loop {
        tokio::time::sleep(SUPERVISION_INTERVAL).await;
        let Some(state_handle) = state.upgrade() else {
            break;
        };
        let mut state = state_handle.lock().await;
        supervise(&mut state);
    }
}

fn supervise(state: &mut SpawnerState) {
    let broker_address = state.broker_address.clone();
    let output_sender = state.output_sender.clone();
    for managed in state.apps.values_mut() {
        if managed.child.is_none() {
            continue;
        }
        let status = poll_status(managed);
        let should_restart = matches!(
            (&status, managed.descriptor.restart_policy),
            (AppStatus::ExitedSuccessfully, RestartPolicy::Always)
                | (
                    AppStatus::ExitedWithError(_),
                    RestartPolicy::Always | RestartPolicy::OnFailure
                )
        );
        if should_restart
            && let Ok(child) = launch(&managed.descriptor, &broker_address, &output_sender)
        {
            managed.child = Some(child);
            managed.last_status = AppStatus::Running;
        }
    }
}
