use crate::messages::{FileSystemCommand, FileSystemResult, FileSystemSuccess, FilesystemError};
use std::sync::mpsc::{Receiver, Sender, channel};

pub struct FilesystemService {
    sender: Sender<FileSystemResult>,
    receiver: Receiver<FileSystemResult>,
}

impl Default for FilesystemService {
    fn default() -> Self {
        let (sender, receiver) = channel();
        Self { sender, receiver }
    }
}

impl FilesystemService {
    pub fn poll_results(&self) -> Vec<FileSystemResult> {
        let mut results = Vec::new();
        while let Ok(result) = self.receiver.try_recv() {
            results.push(result);
        }
        results
    }

    pub fn execute(&self, command: &FileSystemCommand) {
        match command {
            FileSystemCommand::None => {}

            FileSystemCommand::PickFile {
                tag,
                filter_name,
                extensions,
            } => {
                let sender = self.sender.clone();
                let tag = tag.clone();
                let dialog = build_dialog(filter_name, extensions);
                std::thread::spawn(move || {
                    let result = match dialog.pick_file() {
                        Some(path) => match std::fs::read(&path) {
                            Ok(bytes) => FileSystemResult::Success(FileSystemSuccess::File {
                                path: path.to_string_lossy().to_string(),
                                bytes,
                                tag,
                            }),
                            Err(error) => FileSystemResult::Error(FilesystemError::ReadError(
                                error.to_string(),
                            )),
                        },
                        None => FileSystemResult::Error(FilesystemError::NoFileSelected),
                    };
                    let _ = sender.send(result);
                });
            }

            FileSystemCommand::PickFolder { tag } => {
                let sender = self.sender.clone();
                let tag = tag.clone();
                std::thread::spawn(move || {
                    let result = match nightshade::prelude::rfd::FileDialog::new().pick_folder() {
                        Some(path) => FileSystemResult::Success(FileSystemSuccess::Folder {
                            path: path.to_string_lossy().to_string(),
                            tag,
                        }),
                        None => FileSystemResult::Error(FilesystemError::NoFileSelected),
                    };
                    let _ = sender.send(result);
                });
            }

            FileSystemCommand::SaveFile {
                tag,
                bytes,
                filter_name,
                extensions,
            } => {
                let sender = self.sender.clone();
                let tag = tag.clone();
                let bytes = bytes.clone();
                let dialog = build_dialog(filter_name, extensions);
                std::thread::spawn(move || {
                    let result = match dialog.save_file() {
                        Some(path) => match std::fs::write(&path, &bytes) {
                            Ok(_) => FileSystemResult::Success(FileSystemSuccess::File {
                                path: path.to_string_lossy().to_string(),
                                bytes: Vec::new(),
                                tag,
                            }),
                            Err(error) => FileSystemResult::Error(FilesystemError::WriteError(
                                error.to_string(),
                            )),
                        },
                        None => FileSystemResult::Error(FilesystemError::NoFileSelected),
                    };
                    let _ = sender.send(result);
                });
            }
        }
    }
}

fn build_dialog(filter_name: &str, extensions: &[String]) -> nightshade::prelude::rfd::FileDialog {
    let mut dialog = nightshade::prelude::rfd::FileDialog::new();
    if !filter_name.is_empty() && !extensions.is_empty() {
        dialog = dialog.add_filter(
            filter_name,
            &extensions
                .iter()
                .map(|extension| extension.as_str())
                .collect::<Vec<_>>(),
        );
    }
    dialog
}
