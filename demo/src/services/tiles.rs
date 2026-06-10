mod behavior;
mod ui;

pub use self::{behavior::*, ui::*};

use crate::prelude::*;

pub struct TileTreePlugin;

impl Plugin for TileTreePlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<TileTreeCommand>()
            .add_event::<TileTreeMessage>()
            .init_resource::<VisualTree>()
            .init_resource::<ProjectState>()
            .init_resource::<LayoutState>()
            .add_systems(
                Update,
                (
                    (
                        apply_theme_and_widget_context,
                        deliver_dialog_results,
                        sync_modification_flags,
                        draw_shell_ui,
                        flush_widget_outputs,
                    )
                        .chain(),
                    deliver_topic_messages,
                    process_tile_tree_commands,
                    handle_tiletree_messages,
                    update_window_title,
                )
                    .after(EguiPreUpdateSet::InitContexts),
            );
    }
}

#[derive(Resource)]
pub struct ProjectLoadData {
    pub trees: Vec<egui_tiles::Tree<Pane>>,
    pub project_name: Option<String>,
    pub layout_names: Vec<String>,
    pub path: String,
}

pub fn send_project_loaded_message(
    mut message_bus_events: EventWriter<MessageBusEvent>,
    project_data: Res<ProjectLoadData>,
) {
    message_bus_events.send(MessageBusEvent::RouteMessage(Message::Project {
        message: ProjectMessage::ProjectLoaded {
            trees: project_data.trees.clone(),
            project_name: project_data.project_name.clone(),
            layout_names: project_data.layout_names.clone(),
            path: project_data.path.clone(),
        },
    }));
}

#[derive(Default, Debug, Clone, Event, Gui, EnumStr, Serialize, Deserialize)]
pub enum TileTreeMessage {
    ProcessFileResult(FileSystemResult),

    #[default]
    #[serde(other)]
    #[enum2str("")]
    #[enum2egui(skip)]
    Empty,
}
