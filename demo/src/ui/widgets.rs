use crate::prelude::*;

mod template;

pub use self::template::*;

crate::widgets!(Template(TemplateWidget),);

#[derive(Debug, Clone, Default)]
pub struct WidgetContext {
    pub is_connected: bool,
}

pub trait MessageHandler {
    fn receive_message(&mut self, message: &Message);
    fn drain_messages(&mut self) -> Vec<Message>;
}

pub trait Widget {
    fn title(&self) -> String;
    fn ui(&mut self, ui: &mut egui::Ui, context: &WidgetContext);
}

#[macro_export]
macro_rules! widgets {
    ($($enum_variant:ident($widget_type:ident)),* $(,)?) => {
        use strum::{EnumIter, EnumString, AsRefStr};
        use serde::{Serialize, Deserialize};
        use enum2egui::Gui;

        #[derive(Default, EnumIter, EnumString, AsRefStr, Debug, Serialize, Deserialize, Copy, Clone, Event, Gui)]
        pub enum UiWidgetKind {
            $(
                $enum_variant,
            )*

            #[default]
            #[serde(other)]
            #[enum2egui(skip)]
            Empty,
        }

        impl std::fmt::Display for UiWidgetKind {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "{}", self.as_ref())
            }
        }

        #[derive(Default, Debug, Clone)]
        pub enum UiWidget {
            $(
                $enum_variant(Box<$widget_type>),
            )*

            #[default]
            Empty
        }

        pub fn get_widget_id(widget: &UiWidget) -> Option<&str> {
            match widget {
                $(
                    UiWidget::$enum_variant(widget) => Some(widget.rpc.widget_id()),
                )*
                UiWidget::Empty => None,
            }
        }

        impl Widget for UiWidget {
            fn title(&self) -> String {
                match self {
                    $( UiWidget::$enum_variant(widget) => widget.title(), )*
                    UiWidget::Empty => "Empty".to_string(),
                }
            }

            fn ui(&mut self, ui: &mut egui::Ui, context: &WidgetContext) {
                match self {
                    $( UiWidget::$enum_variant(widget) => widget.ui(ui, context), )*
                    UiWidget::Empty => {}
                }
            }
        }

        impl MessageHandler for UiWidget {
            fn receive_message(&mut self, message: &Message) {
                match self {
                    $( UiWidget::$enum_variant(widget) => {
                        widget.receive_message(message);
                    }, )*
                    UiWidget::Empty => {}
                }
            }

            fn drain_messages(&mut self) -> Vec<Message> {
                match self {
                    $( UiWidget::$enum_variant(widget) => widget.rpc.drain_messages(), )*
                    UiWidget::Empty => vec![],
                }
            }
        }

        impl From<&UiWidgetKind> for UiWidget {
            fn from(kind: &UiWidgetKind) -> Self {
                match kind {
                    $( UiWidgetKind::$enum_variant => UiWidget::$enum_variant(Box::default()), )*
                    UiWidgetKind::Empty => UiWidget::Empty,
                }
            }
        }

        impl From<&UiWidget> for UiWidgetKind {
            fn from(widget: &UiWidget) -> Self {
                match widget {
                    $( UiWidget::$enum_variant(..) => UiWidgetKind::$enum_variant, )*
                    UiWidget::Empty => UiWidgetKind::Empty,
                }
            }
        }
    };
}
