use nightshade::prelude::*;
use std::collections::HashMap;

struct ModalRecord {
    root: Entity,
    dialog: Entity,
}

#[derive(Default)]
pub struct ModalService {
    modals: HashMap<String, ModalRecord>,
}

impl ModalService {
    pub fn show_confirm(
        &mut self,
        world: &mut World,
        id: &str,
        title: &str,
        body: &str,
        confirm_text: Option<&str>,
        cancel_text: Option<&str>,
    ) {
        self.close_modal(world, id);

        let mut tree = UiTreeBuilder::new(world);
        let root = tree.root_entity();
        let dialog = tree.add_modal_dialog(title, 380.0, 210.0);
        let content = widget::<UiModalDialogData>(tree.world_mut(), dialog)
            .map(|data| data.content_entity)
            .unwrap_or(dialog);
        let accent_color = tree.active_theme().accent_color;

        let mut ok_button = Entity::default();
        let mut cancel_button = Entity::default();
        tree.in_parent(content, |tree| {
            tree.add_node()
                .fill_width()
                .auto_size(AutoSizeMode::Height)
                .with_text(body, 14.0)
                .with_text_wrap()
                .with_text_alignment(TextAlignment::Left, VerticalAlignment::Top)
                .fg(ThemeColor::Text)
                .entity();

            tree.add_spacing(8.0);

            let button_row = tree
                .add_node()
                .size(100.pct(), (36.0).px())
                .flow_horizontal()
                .padding(0.0)
                .gap(8.0)
                .entity();
            tree.in_parent(button_row, |tree| {
                cancel_button = tree.add_button(cancel_text.unwrap_or("Cancel"));
                ok_button =
                    tree.add_button_colored(confirm_text.unwrap_or("Confirm"), accent_color);
                let world = tree.world_mut();
                for button in [cancel_button, ok_button] {
                    if let Some(node) = world.ui.get_ui_layout_node_mut(button) {
                        node.flow_child_size = Some(Rl(vec2(50.0, 0.0)) + Ab(vec2(-4.0, 32.0)));
                    }
                }
            });
        });
        tree.finish();

        if let Some(data) = world.ui.get_ui_modal_dialog_mut(dialog) {
            data.ok_button = Some(ok_button);
            data.cancel_button = Some(cancel_button);
        }
        ui_show_modal(world, dialog);

        self.modals
            .insert(id.to_string(), ModalRecord { root, dialog });
    }

    pub fn close_modal(&mut self, world: &mut World, id: &str) {
        if let Some(record) = self.modals.remove(id) {
            ui_despawn_node(world, record.root);
        }
    }

    pub fn handle_modal_closed(
        &mut self,
        world: &mut World,
        dialog: Entity,
        _confirmed: bool,
    ) -> Option<String> {
        let id = self
            .modals
            .iter()
            .find(|(_, record)| record.dialog == dialog)
            .map(|(id, _)| id.clone())?;
        self.close_modal(world, &id);
        Some(id)
    }
}
