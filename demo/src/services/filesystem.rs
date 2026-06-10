use crate::prelude::*;
use futures_lite::future;
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize, EnumStr, Clone, PartialEq, Eq, Event, Gui)]
pub enum FileSystemMessage {
    Command(FileSystemCommand),
    Result(FileSystemResult),

    #[default]
    Empty,
}

#[derive(Debug, Default, Serialize, Deserialize, EnumStr, Clone, PartialEq, Eq, Event, Gui)]
pub enum FileSystemCommand {
    #[default]
    None,

    PickFile {
        tag: String,
        filter_name: String,
        extensions: Vec<String>,
    },

    PickFolder {
        tag: String,
    },

    SaveFile {
        tag: String,
        bytes: Vec<u8>,
        filter_name: String,
        extensions: Vec<String>,
    },
}

#[derive(Debug, Serialize, Deserialize, EnumStr, Clone, PartialEq, Eq, Event, Gui)]
pub enum FileSystemResult {
    Success(FileSystemSuccess),
    Error(FilesystemError),
}

impl Default for FileSystemResult {
    fn default() -> Self {
        Self::Success(FileSystemSuccess::Empty)
    }
}

#[derive(Debug, Default, Serialize, Deserialize, EnumStr, Clone, PartialEq, Eq, Event, Gui)]
pub enum FileSystemSuccess {
    #[default]
    Empty,

    File {
        path: String,
        bytes: Vec<u8>,
        tag: String,
    },

    Folder {
        path: String,
        tag: String,
    },
}

#[derive(Default, Debug, Serialize, Deserialize, EnumStr, Clone, PartialEq, Eq, Gui)]
pub enum FilesystemError {
    #[default]
    None,
    ReadError(String),
    WriteError(String),
    NoFileSelected,
}

pub const TAG_PROJECT: &str = "project";
pub const TAG_PROJECT_SAVE: &str = "project_save";
pub const TAG_LAYOUT: &str = "layout";
pub const TAG_LAYOUT_SAVE: &str = "layout_save";

#[derive(Component)]
pub struct PickFileTask {
    pub tag: String,
}

#[derive(Component)]
pub struct PickFolderTask {
    pub tag: String,
}

#[derive(Component)]
pub struct SaveFileTask {
    pub tag: String,
    pub bytes: Vec<u8>,
}

#[derive(Component, Deref, DerefMut)]
pub struct FileTaskComponent(pub bevy::tasks::Task<Option<PathBuf>>);

pub struct FilesystemServicePlugin;

impl Plugin for FilesystemServicePlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<FileSystemCommand>()
            .add_event::<FileSystemResult>()
            .add_event::<FileSystemMessage>()
            .add_systems(Update, process_filesystem_commands)
            .add_systems(Update, poll_pick_file_tasks)
            .add_systems(Update, poll_pick_folder_tasks)
            .add_systems(Update, poll_save_file_tasks);
    }
}

fn process_filesystem_commands(
    mut commands: Commands,
    mut command_events: EventReader<FileSystemCommand>,
) {
    for command in command_events.read() {
        handle_command(&mut commands, command);
    }
}

fn handle_command(commands: &mut Commands, command: &FileSystemCommand) {
    match command {
        FileSystemCommand::None => {}

        FileSystemCommand::PickFile {
            tag,
            filter_name,
            extensions,
        } => {
            let mut dialog = rfd::FileDialog::new();
            if !filter_name.is_empty() && !extensions.is_empty() {
                dialog = dialog.add_filter(
                    filter_name,
                    &extensions
                        .iter()
                        .map(|extension| extension.as_str())
                        .collect::<Vec<_>>(),
                );
            }
            let task =
                bevy::tasks::AsyncComputeTaskPool::get().spawn(async move { dialog.pick_file() });
            commands.spawn((FileTaskComponent(task), PickFileTask { tag: tag.clone() }));
        }

        FileSystemCommand::PickFolder { tag } => {
            let dialog = rfd::FileDialog::new();
            let task =
                bevy::tasks::AsyncComputeTaskPool::get().spawn(async move { dialog.pick_folder() });
            commands.spawn((FileTaskComponent(task), PickFolderTask { tag: tag.clone() }));
        }

        FileSystemCommand::SaveFile {
            tag,
            bytes,
            filter_name,
            extensions,
        } => {
            let mut dialog = rfd::FileDialog::new();
            if !filter_name.is_empty() && !extensions.is_empty() {
                dialog = dialog.add_filter(
                    filter_name,
                    &extensions
                        .iter()
                        .map(|extension| extension.as_str())
                        .collect::<Vec<_>>(),
                );
            }
            let task =
                bevy::tasks::AsyncComputeTaskPool::get().spawn(async move { dialog.save_file() });
            commands.spawn((
                FileTaskComponent(task),
                SaveFileTask {
                    tag: tag.clone(),
                    bytes: bytes.clone(),
                },
            ));
        }
    }
}

fn send_result(message_bus: &mut EventWriter<MessageBusEvent>, result: FileSystemResult) {
    message_bus.send(MessageBusEvent::RouteMessage(Message::Filesystem {
        message: FileSystemMessage::Result(result),
    }));
}

fn poll_pick_file_tasks(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut FileTaskComponent, &PickFileTask)>,
    mut message_bus: EventWriter<MessageBusEvent>,
) {
    for (entity, mut task, pick_file_task) in tasks.iter_mut() {
        let Some(result) = future::block_on(future::poll_once(&mut task.0)) else {
            continue;
        };
        commands.entity(entity).despawn();

        match result {
            Some(path) => match std::fs::read(&path) {
                Ok(bytes) => {
                    send_result(
                        &mut message_bus,
                        FileSystemResult::Success(FileSystemSuccess::File {
                            path: path.to_string_lossy().to_string(),
                            bytes,
                            tag: pick_file_task.tag.clone(),
                        }),
                    );
                }
                Err(error) => {
                    send_result(
                        &mut message_bus,
                        FileSystemResult::Error(FilesystemError::ReadError(error.to_string())),
                    );
                }
            },
            None => {
                send_result(
                    &mut message_bus,
                    FileSystemResult::Error(FilesystemError::NoFileSelected),
                );
            }
        }
    }
}

fn poll_pick_folder_tasks(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut FileTaskComponent, &PickFolderTask)>,
    mut message_bus: EventWriter<MessageBusEvent>,
) {
    for (entity, mut task, pick_folder_task) in tasks.iter_mut() {
        let Some(result) = future::block_on(future::poll_once(&mut task.0)) else {
            continue;
        };
        commands.entity(entity).despawn();

        match result {
            Some(path) => {
                send_result(
                    &mut message_bus,
                    FileSystemResult::Success(FileSystemSuccess::Folder {
                        path: path.to_string_lossy().to_string(),
                        tag: pick_folder_task.tag.clone(),
                    }),
                );
            }
            None => {
                send_result(
                    &mut message_bus,
                    FileSystemResult::Error(FilesystemError::NoFileSelected),
                );
            }
        }
    }
}

fn poll_save_file_tasks(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut FileTaskComponent, &SaveFileTask)>,
    mut message_bus: EventWriter<MessageBusEvent>,
) {
    for (entity, mut task, save_file_task) in tasks.iter_mut() {
        let Some(result) = future::block_on(future::poll_once(&mut task.0)) else {
            continue;
        };
        commands.entity(entity).despawn();

        match result {
            Some(path) => match std::fs::write(&path, &save_file_task.bytes) {
                Ok(_) => {
                    send_result(
                        &mut message_bus,
                        FileSystemResult::Success(FileSystemSuccess::File {
                            path: path.to_string_lossy().to_string(),
                            bytes: Vec::new(),
                            tag: save_file_task.tag.clone(),
                        }),
                    );
                }
                Err(error) => {
                    send_result(
                        &mut message_bus,
                        FileSystemResult::Error(FilesystemError::WriteError(error.to_string())),
                    );
                }
            },
            None => {
                send_result(
                    &mut message_bus,
                    FileSystemResult::Error(FilesystemError::NoFileSelected),
                );
            }
        }
    }
}
