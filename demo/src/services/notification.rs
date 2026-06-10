use crate::prelude::*;
use egui::{Align2, Direction, RichText};
use egui_toast::{Toast, ToastKind, ToastOptions, Toasts};

#[derive(Resource)]
struct NotificationService {
    toasts: Toasts,
}

impl Default for NotificationService {
    fn default() -> Self {
        Self {
            toasts: Toasts::new()
                .anchor(Align2::RIGHT_BOTTOM, (-10.0, -10.0))
                .direction(Direction::BottomUp),
        }
    }
}

#[derive(Default, Event, Debug, Serialize, Deserialize, Clone, Gui, EnumStr)]
pub enum NotificationServiceMessage {
    Show {
        text: String,
        kind: NotificationKind,
        duration_in_seconds: f64,
    },
    #[default]
    #[serde(other)]
    #[enum2egui(skip)]
    #[enum2str("")]
    Empty,
}

#[derive(Default, Debug, Serialize, Deserialize, Clone, Copy, Gui, EnumStr, PartialEq, Eq)]
pub enum NotificationKind {
    Info,
    Warning,
    Error,
    Success,
    #[default]
    #[serde(other)]
    #[enum2egui(skip)]
    #[enum2str("")]
    Empty,
}

impl From<NotificationKind> for ToastKind {
    fn from(kind: NotificationKind) -> Self {
        match kind {
            NotificationKind::Info => ToastKind::Info,
            NotificationKind::Warning => ToastKind::Warning,
            NotificationKind::Error => ToastKind::Error,
            NotificationKind::Success => ToastKind::Success,
            NotificationKind::Empty => ToastKind::Info,
        }
    }
}

pub struct NotificationServicePlugin;

impl Plugin for NotificationServicePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NotificationService>()
            .add_event::<NotificationServiceMessage>()
            .add_systems(
                Update,
                receive_messages.after(EguiPreUpdateSet::InitContexts),
            );
    }
}

fn receive_messages(
    mut messages: EventReader<NotificationServiceMessage>,
    mut contexts: Query<&mut EguiContext, With<bevy::window::PrimaryWindow>>,
    mut notification_service: ResMut<NotificationService>,
) {
    for message in messages.read() {
        if let NotificationServiceMessage::Show {
            text,
            kind,
            duration_in_seconds,
        } = message
        {
            let duration = if *duration_in_seconds < 0.01 {
                None
            } else {
                Some(std::time::Duration::from_secs_f64(*duration_in_seconds))
            };
            notification_service.toasts.add(Toast {
                text: RichText::new(text).strong().into(),
                kind: (*kind).into(),
                options: ToastOptions::default()
                    .duration(duration)
                    .show_progress(true)
                    .show_icon(true),
                ..Default::default()
            });
        }
    }

    let Ok(mut context) = contexts.get_single_mut() else {
        return;
    };
    notification_service.toasts.show(context.get_mut());
}
