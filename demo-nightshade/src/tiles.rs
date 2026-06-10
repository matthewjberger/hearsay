use crate::messages::Message;
use crate::messages::*;
use crate::widget::TemplateWidget;
use nightshade::prelude::*;
use std::collections::VecDeque;

pub const EMPTY_PANE_TITLE: &str = "Empty";
pub const TEMPLATE_PANE_TITLE: &str = "Template";
pub const WIDGET_KINDS: [&str; 1] = [TEMPLATE_PANE_TITLE];

pub struct TilesState {
    pub container: Entity,
    pub widgets: Vec<TemplateWidget>,
    pub empty_pane: Option<(TileId, Entity)>,
    pub snapshot: String,
}

pub fn build_tile_area(tree: &mut UiTreeBuilder) -> TilesState {
    let container = tree.add_tile_container(vec2(0.0, 0.0));

    let mut tiles = TilesState {
        container,
        widgets: Vec::new(),
        empty_pane: None,
        snapshot: String::new(),
    };
    tiles.add_empty_pane(tree.world_mut());
    tiles
}

pub fn prepare_pane_content(world: &mut World, content_entity: Entity) {
    if let Some(node) = world.ui.get_ui_layout_node_mut(content_entity) {
        node.flow_layout = None;
        node.clip_content = true;
    }
}

fn build_empty_pane_content(world: &mut World, content_entity: Entity) {
    prepare_pane_content(world, content_entity);
    let mut tree = UiTreeBuilder::from_parent(world, content_entity);
    let column = tree
        .add_node()
        .boundary(
            Ab(vec2(12.0, 12.0)),
            Ab(vec2(-12.0, -12.0)) + Rl(vec2(100.0, 100.0)),
        )
        .flow_vertical()
        .padding(0.0)
        .gap(6.0)
        .entity();
    tree.in_parent(column, |tree| {
        tree.add_node()
            .size(100.pct(), (22.0).px())
            .with_text("This pane is empty", 15.0)
            .text_left()
            .fg(ThemeColor::Text)
            .entity();
        tree.add_node()
            .size(100.pct(), (20.0).px())
            .with_text("Use the Add Panel button to insert a widget.", 13.0)
            .text_left()
            .fg(ThemeColor::TextDisabled)
            .entity();
    });
    tree.finish_subtree();
}

impl TilesState {
    pub fn update_rect(&self, world: &mut World) {
        let Some((width, height)) = world.resources.window.cached_viewport_size else {
            return;
        };
        let reserved = ui_reserved_areas(world).clone();
        let dpi = world.resources.window.cached_scale_factor.max(0.0001);
        let position = vec2(reserved.left, reserved.top) / dpi;
        let size = vec2(
            (width as f32 - reserved.left - reserved.right).max(0.0),
            (height as f32 - reserved.top - reserved.bottom).max(0.0),
        ) / dpi;
        if let Some(node) = world.ui.get_ui_layout_node_mut(self.container)
            && let Some(UiLayoutType::Window(window)) = node.base_layout.as_mut()
        {
            window.position = Ab(position).into();
            window.size = Ab(size).into();
        }
    }

    pub fn pane_count(&self, world: &World) -> usize {
        world
            .ui
            .get_ui_tile_container(self.container)
            .map(|data| {
                data.tiles
                    .iter()
                    .filter(|tile| matches!(tile, Some(TileNode::Pane { .. })))
                    .count()
            })
            .unwrap_or(0)
    }

    pub fn add_empty_pane(&mut self, world: &mut World) {
        let Some((pane_id, content_entity)) =
            ui_tile_add_pane(world, self.container, EMPTY_PANE_TITLE)
        else {
            return;
        };
        build_empty_pane_content(world, content_entity);
        self.empty_pane = Some((pane_id, content_entity));
    }

    pub fn add_template_pane(&mut self, world: &mut World) {
        let Some((pane_id, content_entity)) =
            ui_tile_add_pane(world, self.container, TEMPLATE_PANE_TITLE)
        else {
            return;
        };
        prepare_pane_content(world, content_entity);
        let widget = TemplateWidget::build(world, pane_id, content_entity);
        self.widgets.push(widget);
        ui_tile_set_active(world, self.container, pane_id);

        if let Some((empty_id, empty_content)) = self.empty_pane.take() {
            ui_tile_remove(world, self.container, empty_id);
            ui_despawn_node(world, empty_content);
        }
    }

    pub fn handle_tab_closed(
        &mut self,
        world: &mut World,
        pane_id: TileId,
        bus: &mut VecDeque<Message>,
    ) {
        if let Some((empty_id, empty_content)) = self.empty_pane
            && empty_id == pane_id
        {
            ui_despawn_node(world, empty_content);
            self.empty_pane = None;
        } else if let Some(index) = self
            .widgets
            .iter()
            .position(|widget| widget.pane_id == pane_id)
        {
            let widget = self.widgets.remove(index);
            bus.push_back(Message::Broker {
                message: BrokerServiceMessage::WidgetRemoved {
                    widget_id: widget.rpc.widget_id().to_string(),
                },
            });
            ui_despawn_node(world, widget.content_entity);
        }

        if self.pane_count(world) == 0 {
            self.add_empty_pane(world);
        }
    }

    pub fn remove_all_widget_subscriptions(&self, bus: &mut VecDeque<Message>) {
        for widget in &self.widgets {
            bus.push_back(Message::Broker {
                message: BrokerServiceMessage::WidgetRemoved {
                    widget_id: widget.rpc.widget_id().to_string(),
                },
            });
        }
    }

    pub fn reset(&mut self, world: &mut World, bus: &mut VecDeque<Message>) {
        self.remove_all_widget_subscriptions(bus);
        for widget in &self.widgets {
            ui_despawn_node(world, widget.content_entity);
        }
        self.widgets.clear();
        if let Some((_, empty_content)) = self.empty_pane.take() {
            ui_despawn_node(world, empty_content);
        }
        ui_tile_reset_to_panes(world, self.container, &[]);
        self.add_empty_pane(world);
        self.refresh_snapshot(world);
    }

    pub fn apply_layout(
        &mut self,
        world: &mut World,
        layout: &TileLayout,
        bus: &mut VecDeque<Message>,
    ) {
        self.remove_all_widget_subscriptions(bus);
        self.widgets.clear();
        self.empty_pane = None;

        let mappings = ui_tile_load_layout(world, self.container, layout);
        for (pane_id, content_entity) in mappings {
            let title = ui_tile_pane_title(world, self.container, pane_id).unwrap_or_default();
            if title == TEMPLATE_PANE_TITLE {
                prepare_pane_content(world, content_entity);
                let widget = TemplateWidget::build(world, pane_id, content_entity);
                self.widgets.push(widget);
            } else {
                build_empty_pane_content(world, content_entity);
                self.empty_pane = Some((pane_id, content_entity));
            }
        }

        if self.pane_count(world) == 0 {
            self.add_empty_pane(world);
        }
        self.refresh_snapshot(world);
    }

    pub fn current_layout(&self, world: &World) -> Option<TileLayout> {
        ui_tile_save_layout(world, self.container)
    }

    pub fn refresh_snapshot(&mut self, world: &World) {
        self.snapshot = self
            .current_layout(world)
            .and_then(|layout| serde_json::to_string(&layout).ok())
            .unwrap_or_default();
    }

    pub fn detect_layout_change(&mut self, world: &World) -> bool {
        let current = self
            .current_layout(world)
            .and_then(|layout| serde_json::to_string(&layout).ok())
            .unwrap_or_default();
        if current != self.snapshot {
            self.snapshot = current;
            true
        } else {
            false
        }
    }
}
