mod api;
mod services;
mod ui;
extern crate alloc;

pub(crate) mod prelude {
    pub use super::{api::*, services::*, ui::*};
    pub use bevy::prelude::*;
    pub use bevy_egui::*;
    pub use enum2egui::{Gui, GuiInspect};
    pub use enum2str::EnumStr;
    pub use serde::{Deserialize, Serialize};
    pub use std::{
        collections::{HashMap, HashSet},
        fs,
    };
}

use prelude::*;

fn main() {
    let role = WindowRole::detect();

    let mut app_builder = App::new();
    app_builder
        .add_plugins(bevy::DefaultPlugins.set(bevy::window::WindowPlugin {
            primary_window: Some(bevy::window::Window {
                title: "Hearsay Demo".to_string(),
                name: Some("hearsay.demo".into()),
                present_mode: bevy::window::PresentMode::AutoNoVsync,
                window_theme: Some(bevy::window::WindowTheme::Dark),
                ..Default::default()
            }),
            ..Default::default()
        }))
        .insert_resource(role.clone())
        .add_plugins(bevy_egui::EguiPlugin)
        .add_plugins(MessageBusPlugin);

    if role.is_primary() {
        load_startup_project(&mut app_builder);
    }

    app_builder.run();
}

fn load_startup_project(app_builder: &mut App) {
    let argument_path = std::env::args().nth(1);
    let settings_path = app_builder
        .world()
        .resource::<UserSettings>()
        .default_project_path
        .clone();
    let Some(project_path) = argument_path.or(settings_path) else {
        return;
    };

    if !std::path::Path::new(&project_path).exists() {
        bevy::log::warn!("Startup project file not found: {project_path}");
        return;
    }

    match fs::read_to_string(&project_path) {
        Ok(json) => match serde_json::from_str::<ProjectSaveFile>(&json) {
            Ok(save_file) => {
                let trees: Vec<egui_tiles::Tree<Pane>> = save_file
                    .windows
                    .iter()
                    .map(|window_tree| window_tree.tree.clone())
                    .collect();
                let layout_names: Vec<String> = save_file
                    .windows
                    .iter()
                    .map(|window_tree| window_tree.layout_name.clone())
                    .collect();
                app_builder.insert_resource(ProjectLoadData {
                    trees,
                    project_name: save_file.project_name,
                    layout_names,
                    path: project_path,
                });
                app_builder.add_systems(Startup, send_project_loaded_message);
            }
            Err(error) => {
                bevy::log::error!("Failed to parse startup project file: {error}");
            }
        },
        Err(error) => {
            bevy::log::error!("Failed to read startup project file: {error}");
        }
    }
}
