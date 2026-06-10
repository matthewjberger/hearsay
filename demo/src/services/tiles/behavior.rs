use crate::prelude::*;
use egui_tiles::{Tile, TileId, Tiles};

pub struct TreeBehavior {
    pub simplification_options: egui_tiles::SimplificationOptions,
    pub tab_bar_height: f32,
    pub gap_width: f32,
    pub add_child_to: Option<(TileId, UiWidgetKind)>,
    pub layout_modified: bool,
    pub project_modified: bool,
    pub removed_widget_ids: Vec<String>,
    pub reset_layout_requested: bool,
}

impl Default for TreeBehavior {
    fn default() -> Self {
        Self {
            simplification_options: egui_tiles::SimplificationOptions {
                all_panes_must_have_tabs: true,
                join_nested_linear_containers: true,
                ..Default::default()
            },
            tab_bar_height: 24.0,
            gap_width: 2.0,
            add_child_to: None,
            layout_modified: false,
            project_modified: false,
            removed_widget_ids: Vec::new(),
            reset_layout_requested: false,
        }
    }
}

impl egui_tiles::Behavior<Pane> for TreeBehavior {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: TileId,
        view: &mut Pane,
    ) -> egui_tiles::UiResponse {
        let widget_context = ui.ctx().data(|data| {
            data.get_temp::<WidgetContext>(egui::Id::new("widget_context"))
                .unwrap_or_default()
        });

        ui.push_id(ui.next_auto_id(), |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                view.widget.ui(ui, &widget_context);
            });
        });

        if ui.input(|input| input.modifiers.shift) {
            let response = ui
                .allocate_rect(ui.max_rect(), egui::Sense::click_and_drag())
                .on_hover_cursor(egui::CursorIcon::Grab);
            if response.dragged() {
                return egui_tiles::UiResponse::DragStarted;
            }
        } else {
            ui.allocate_rect(ui.max_rect(), egui::Sense::hover());
        }
        egui_tiles::UiResponse::None
    }

    fn tab_title_for_pane(&mut self, view: &Pane) -> egui::WidgetText {
        egui::RichText::new(view.widget.title()).into()
    }

    fn top_bar_right_ui(
        &mut self,
        _tiles: &Tiles<Pane>,
        ui: &mut egui::Ui,
        tile_id: TileId,
        _tabs: &egui_tiles::Tabs,
        _scroll_offset: &mut f32,
    ) {
        let button_response = ui.button("➕").on_hover_text("Add new panel");

        let popup_id = egui::Id::new("add_tab_popup").with(tile_id);
        if button_response.clicked() {
            ui.memory_mut(|memory| {
                memory.open_popup(popup_id);
                memory
                    .data
                    .get_temp_mut_or_insert_with::<String>(popup_id, String::new);
                memory
                    .data
                    .get_temp_mut_or_insert_with::<usize>(popup_id.with("selected_index"), || 0);
            });
        }

        egui::popup_below_widget(
            ui,
            popup_id,
            &button_response,
            egui::popup::PopupCloseBehavior::CloseOnClickOutside,
            |ui: &mut egui::Ui| {
                ui.set_min_width(200.0);

                let mut search = ui.memory_mut(|memory| {
                    memory
                        .data
                        .get_temp_mut_or_default::<String>(popup_id)
                        .clone()
                });

                let popup_just_opened = ui.memory_mut(|memory| {
                    let popup_open_key = popup_id.with("was_just_opened");
                    if memory.is_popup_open(popup_id) {
                        let was_open_before = memory
                            .data
                            .get_temp::<bool>(popup_open_key)
                            .unwrap_or(false);
                        if !was_open_before {
                            memory.data.insert_temp(popup_open_key, true);
                            true
                        } else {
                            false
                        }
                    } else {
                        memory.data.insert_temp(popup_open_key, false);
                        false
                    }
                });

                ui.spacing_mut().item_spacing.y = 6.0;

                let search_frame = egui::Frame::NONE
                    .fill(ui.visuals().extreme_bg_color)
                    .inner_margin(egui::Margin::same(8))
                    .stroke(ui.visuals().widgets.noninteractive.bg_stroke);

                search_frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("🔍");
                        let search_response = ui
                            .text_edit_singleline(&mut search)
                            .on_hover_text("Type to filter widgets");

                        let should_focus = popup_just_opened
                            || ui.memory(|memory| {
                                memory.is_popup_open(popup_id)
                                    && !memory.has_focus(search_response.id)
                            });

                        if should_focus {
                            ui.memory_mut(|memory| memory.request_focus(search_response.id));
                            ui.ctx().request_repaint();
                        }

                        if search_response.changed() {
                            ui.memory_mut(|memory| {
                                *memory.data.get_temp_mut_or_default::<String>(popup_id) =
                                    search.clone();
                                *memory.data.get_temp_mut_or_default::<usize>(
                                    popup_id.with("selected_index"),
                                ) = 0;
                            });
                        }

                        if !search.is_empty() && ui.button("✖").clicked() {
                            search.clear();
                            ui.memory_mut(|memory| {
                                *memory.data.get_temp_mut_or_default::<String>(popup_id) =
                                    String::new();
                            });
                        }
                    });
                });

                ui.add_space(4.0);
                ui.separator();

                let mut widgets: Vec<UiWidgetKind> =
                    <UiWidgetKind as strum::IntoEnumIterator>::iter()
                        .filter(|widget| {
                            if matches!(widget, UiWidgetKind::Empty) {
                                return false;
                            }
                            if search.is_empty() {
                                true
                            } else {
                                let widget_instance = UiWidget::from(widget);
                                widget_instance
                                    .title()
                                    .to_lowercase()
                                    .contains(&search.to_lowercase())
                            }
                        })
                        .collect();

                widgets.sort_by_key(|widget_kind| {
                    let widget_instance = UiWidget::from(widget_kind);
                    widget_instance.title().to_lowercase()
                });

                if widgets.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(10.0);
                        ui.label("No matches found");
                        ui.add_space(10.0);
                    });
                    return;
                }

                let mut selected_index = ui.memory_mut(|memory| {
                    *memory
                        .data
                        .get_temp_mut_or_default::<usize>(popup_id.with("selected_index"))
                });

                if selected_index >= widgets.len() {
                    selected_index = widgets.len() - 1;
                    ui.memory_mut(|memory| {
                        *memory
                            .data
                            .get_temp_mut_or_default::<usize>(popup_id.with("selected_index")) =
                            selected_index;
                    });
                }

                if ui.input_mut(|input| {
                    input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
                }) && selected_index < widgets.len() - 1
                {
                    selected_index += 1;
                    ui.memory_mut(|memory| {
                        *memory
                            .data
                            .get_temp_mut_or_default::<usize>(popup_id.with("selected_index")) =
                            selected_index;
                    });
                }

                if ui
                    .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp))
                    && selected_index > 0
                {
                    selected_index -= 1;
                    ui.memory_mut(|memory| {
                        *memory
                            .data
                            .get_temp_mut_or_default::<usize>(popup_id.with("selected_index")) =
                            selected_index;
                    });
                }

                let enter_pressed = ui
                    .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));

                let max_items = 8;
                let item_height = 28.0;
                let max_height = (widgets.len().min(max_items) as f32 * item_height) + 4.0;

                egui::ScrollArea::vertical()
                    .id_salt(ui.next_auto_id())
                    .max_height(max_height)
                    .auto_shrink([false, false])
                    .show_viewport(ui, |ui, _viewport| {
                        ui.style_mut().visuals.button_frame = false;
                        ui.style_mut().spacing.item_spacing.y = 1.0;

                        if enter_pressed
                            || ui.input(|input| {
                                input.key_pressed(egui::Key::ArrowUp)
                                    || input.key_pressed(egui::Key::ArrowDown)
                            })
                        {
                            let selected_position = selected_index as f32 * item_height;
                            ui.scroll_to_rect(
                                egui::Rect::from_min_size(
                                    egui::pos2(0.0, selected_position),
                                    egui::vec2(1.0, item_height),
                                ),
                                None,
                            );
                        }

                        for (item_index, widget_kind) in widgets.iter().enumerate() {
                            let is_selected = item_index == selected_index;

                            let widget = UiWidget::from(widget_kind);
                            let display_name = widget.title();

                            let button = egui::Button::new(
                                egui::RichText::new(display_name)
                                    .size(14.0)
                                    .color(ui.visuals().text_color()),
                            );

                            let button_response = ui.add_sized(
                                [ui.available_width(), item_height],
                                button.fill(if is_selected {
                                    ui.visuals().selection.bg_fill
                                } else {
                                    ui.visuals().extreme_bg_color
                                }),
                            );

                            if button_response.hovered() && !is_selected {
                                selected_index = item_index;
                                ui.memory_mut(|memory| {
                                    *memory.data.get_temp_mut_or_default::<usize>(
                                        popup_id.with("selected_index"),
                                    ) = item_index;
                                });
                            }

                            if button_response.clicked() || (enter_pressed && is_selected) {
                                self.add_child_to = Some((tile_id, *widget_kind));
                                self.layout_modified = true;
                                self.project_modified = true;
                                ui.memory_mut(|memory| {
                                    *memory.data.get_temp_mut_or_default::<usize>(
                                        popup_id.with("selected_index"),
                                    ) = 0;
                                });
                                ui.ctx().request_repaint();
                            }
                        }
                    });

                ui.separator();
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.visuals_mut().widgets.noninteractive.fg_stroke.color =
                        ui.visuals().weak_text_color();
                    ui.label("Up/Down to navigate");
                    ui.add_space(8.0);
                    ui.label("Enter to select");
                });
            },
        );
    }

    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        self.tab_bar_height
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        self.gap_width
    }

    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        self.simplification_options
    }

    fn is_tab_closable(&self, _tiles: &Tiles<Pane>, _tile_id: TileId) -> bool {
        true
    }

    fn on_tab_close(&mut self, tiles: &mut Tiles<Pane>, tile_id: TileId) -> bool {
        self.layout_modified = true;
        self.project_modified = true;

        if let Some(tile) = tiles.get(tile_id) {
            match tile {
                Tile::Pane(pane) => {
                    if let Some(widget_id) = get_widget_id(&pane.widget) {
                        self.removed_widget_ids.push(widget_id.to_string());
                    }

                    let total_pane_count = tiles
                        .iter()
                        .filter(|(_, tile)| matches!(tile, Tile::Pane(_)))
                        .count();

                    if total_pane_count == 1 {
                        self.reset_layout_requested = true;
                    }

                    self.add_child_to = None;
                }
                Tile::Container(container) => {
                    for child_id in container.children() {
                        if let Some(Tile::Pane(pane)) = tiles.get(*child_id)
                            && let Some(widget_id) = get_widget_id(&pane.widget)
                        {
                            self.removed_widget_ids.push(widget_id.to_string());
                        }
                    }
                    self.add_child_to = None;
                }
            }
        }
        true
    }

    fn on_edit(&mut self, edit_action: egui_tiles::EditAction) {
        match edit_action {
            egui_tiles::EditAction::TileDropped | egui_tiles::EditAction::TileResized => {
                self.layout_modified = true;
                self.project_modified = true;
            }
            _ => {}
        }
    }
}
