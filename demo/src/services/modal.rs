use crate::prelude::*;

#[derive(Default, Event, Debug, Serialize, Deserialize, Clone, Gui, EnumStr)]
pub enum ModalServiceMessage {
    ShowConfirm {
        id: String,
        title: String,
        body: String,
        confirm_text: Option<String>,
        cancel_text: Option<String>,
    },
    CloseModal(String),
    ModalResult {
        id: String,
        confirmed: bool,
    },
    #[default]
    #[serde(other)]
    #[enum2egui(skip)]
    #[enum2str("")]
    Empty,
}

#[derive(Default, Debug, Serialize, Deserialize, Clone, Gui, EnumStr, PartialEq)]
pub enum ModalResult {
    Confirmed,
    #[default]
    Cancelled,
}

impl From<bool> for ModalResult {
    fn from(value: bool) -> Self {
        if value {
            ModalResult::Confirmed
        } else {
            ModalResult::Cancelled
        }
    }
}

#[derive(Clone, Debug)]
pub struct ModalData {
    pub title: String,
    pub body: String,
    pub confirm_text: String,
    pub cancel_text: String,
}

#[derive(Resource, Default)]
pub struct ActiveModals {
    pub modals: HashMap<String, ModalData>,
}

pub struct ModalServicePlugin;

impl Plugin for ModalServicePlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<ModalServiceMessage>()
            .init_resource::<ActiveModals>()
            .add_systems(
                Update,
                (receive_messages, process_modals).after(EguiPreUpdateSet::InitContexts),
            );
    }
}

fn receive_messages(
    mut messages: EventReader<ModalServiceMessage>,
    mut active_modals: ResMut<ActiveModals>,
) {
    for message in messages.read() {
        match message {
            ModalServiceMessage::ShowConfirm {
                id,
                title,
                body,
                confirm_text,
                cancel_text,
            } => {
                active_modals.modals.insert(
                    id.clone(),
                    ModalData {
                        title: title.clone(),
                        body: body.clone(),
                        confirm_text: confirm_text
                            .clone()
                            .unwrap_or_else(|| "Confirm".to_string()),
                        cancel_text: cancel_text.clone().unwrap_or_else(|| "Cancel".to_string()),
                    },
                );
            }
            ModalServiceMessage::CloseModal(id) => {
                active_modals.modals.remove(id);
            }
            ModalServiceMessage::ModalResult { .. } | ModalServiceMessage::Empty => {}
        }
    }
}

fn process_modals(
    mut active_modals: ResMut<ActiveModals>,
    mut message_bus: EventWriter<MessageBusEvent>,
    mut contexts: Query<&mut EguiContext, With<bevy::window::PrimaryWindow>>,
) {
    if active_modals.modals.is_empty() {
        return;
    }
    let Ok(mut context) = contexts.get_single_mut() else {
        return;
    };
    let context = context.get_mut();

    let mut modals_to_remove = Vec::new();
    for (id, modal) in active_modals.modals.iter() {
        let modal_response = egui::Modal::new(egui::Id::new(id.as_str())).show(&*context, |ui| {
            ui.set_min_width(280.0);
            ui.heading(&modal.title);
            ui.separator();
            ui.label(&modal.body);
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            ui.allocate_ui_with_layout(
                egui::Vec2::new(ui.available_width(), 28.0),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    let clicked_confirm = ui.button(&modal.confirm_text).clicked();
                    ui.add_space(8.0);
                    let clicked_cancel = ui.button(&modal.cancel_text).clicked();
                    (clicked_confirm, clicked_cancel)
                },
            )
            .inner
        });

        let (clicked_confirm, clicked_cancel) = modal_response.inner;
        if clicked_confirm || clicked_cancel {
            message_bus.send(MessageBusEvent::RouteMessage(Message::Modal {
                message: ModalServiceMessage::ModalResult {
                    id: id.clone(),
                    confirmed: clicked_confirm,
                },
            }));
            modals_to_remove.push(id.clone());
        }
    }

    for id in modals_to_remove {
        active_modals.modals.remove(&id);
    }
}
