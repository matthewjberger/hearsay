use crate::messages::Message;
use crate::messages::*;
use nightshade::prelude::*;

pub const API_TEMPLATES: [&str; 15] = [
    "Topic",
    "Broker / Publish",
    "Broker / PublishBytes",
    "Broker / Subscribe",
    "Broker / Unsubscribe",
    "Broker / WidgetRemoved",
    "Broker / SpawnWindow",
    "Filesystem / PickFile",
    "Filesystem / PickFolder",
    "Filesystem / SaveFile",
    "Modal / ShowConfirm",
    "Modal / CloseModal",
    "Notify / Show",
    "Project / CloseProject",
    "ConnectionStatus",
];

pub fn template_message(index: usize) -> Message {
    match index {
        0 => Message::Topic {
            topic: "template/text".to_string(),
            payload: "{\"text\":\"hello\"}".to_string(),
            bytes: None,
        },
        1 => Message::Broker {
            message: BrokerServiceMessage::Publish {
                topic: "template/text".to_string(),
                message: "{\"text\":\"hello\"}".to_string(),
            },
        },
        2 => Message::Broker {
            message: BrokerServiceMessage::PublishBytes {
                topic: "template/binary".to_string(),
                bytes: vec![1, 2, 3],
            },
        },
        3 => Message::Broker {
            message: BrokerServiceMessage::Subscribe {
                topics: vec!["template/text".to_string()],
                widget_id: "api-window".to_string(),
            },
        },
        4 => Message::Broker {
            message: BrokerServiceMessage::Unsubscribe {
                topics: vec!["template/text".to_string()],
                widget_id: "api-window".to_string(),
            },
        },
        5 => Message::Broker {
            message: BrokerServiceMessage::WidgetRemoved {
                widget_id: "api-window".to_string(),
            },
        },
        6 => Message::Broker {
            message: BrokerServiceMessage::SpawnWindow,
        },
        7 => Message::Filesystem {
            message: FileSystemMessage::Command(FileSystemCommand::PickFile {
                tag: "api".to_string(),
                filter_name: "text".to_string(),
                extensions: vec!["txt".to_string()],
            }),
        },
        8 => Message::Filesystem {
            message: FileSystemMessage::Command(FileSystemCommand::PickFolder {
                tag: "api".to_string(),
            }),
        },
        9 => Message::Filesystem {
            message: FileSystemMessage::Command(FileSystemCommand::SaveFile {
                tag: "api".to_string(),
                bytes: vec![104, 105],
                filter_name: "text".to_string(),
                extensions: vec!["txt".to_string()],
            }),
        },
        10 => Message::Modal {
            message: ModalServiceMessage::ShowConfirm {
                id: "api-modal".to_string(),
                title: "Api Modal".to_string(),
                body: "Sent from the Api window".to_string(),
                confirm_text: Some("Yes".to_string()),
                cancel_text: Some("No".to_string()),
            },
        },
        11 => Message::Modal {
            message: ModalServiceMessage::CloseModal("api-modal".to_string()),
        },
        12 => Message::Notify {
            message: NotificationServiceMessage::Show {
                text: "Hello from the Api window".to_string(),
                kind: NotificationKind::Info,
                duration_in_seconds: 3.0,
            },
        },
        13 => Message::Project {
            message: ProjectMessage::CloseProject,
        },
        _ => Message::ConnectionStatus { connected: true },
    }
}

pub fn template_json(index: usize) -> String {
    serde_json::to_string_pretty(&template_message(index)).unwrap_or_default()
}

pub struct ApiPanel {
    pub panel: Entity,
    pub dropdown: Entity,
    pub text_area: Entity,
    pub send_button: Entity,
    pub status_label: Entity,
    pub visible: bool,
}

pub fn build_api_panel(tree: &mut UiTreeBuilder) -> ApiPanel {
    let panel = tree.add_floating_panel("api-window", "Api", Rect::new(340.0, 90.0, 460.0, 420.0));
    let content = widget::<UiPanelData>(tree.world_mut(), panel)
        .map(|data| data.content_entity)
        .unwrap_or(panel);

    let mut dropdown = Entity::default();
    let mut text_area = Entity::default();
    let mut send_button = Entity::default();
    let mut status_label = Entity::default();
    tree.in_parent(content, |tree| {
        tree.add_node()
            .size(100.pct(), (20.0).px())
            .with_text("Compose a bus message as JSON and send it.", 13.0)
            .text_left()
            .fg(ThemeColor::TextDisabled)
            .entity();
        dropdown = tree.add_dropdown(&API_TEMPLATES, 0);
        text_area = tree.add_text_area_with_value("Message JSON...", 10, &template_json(0));
        send_button = tree.add_button("Send Message");
        status_label = tree
            .add_node()
            .size(100.pct(), (36.0).px())
            .with_text("", 12.0)
            .with_text_wrap()
            .with_text_alignment(TextAlignment::Left, VerticalAlignment::Top)
            .fg(ThemeColor::TextDisabled)
            .entity();
    });

    ui_set_visible(tree.world_mut(), panel, false);

    ApiPanel {
        panel,
        dropdown,
        text_area,
        send_button,
        status_label,
        visible: false,
    }
}
