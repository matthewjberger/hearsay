use crate::settings::UserSettings;
use nightshade::prelude::*;

pub const PROJECT_MENU_NEW: u32 = 1;
pub const PROJECT_MENU_LOAD: u32 = 2;
pub const PROJECT_MENU_SAVE_AS: u32 = 3;
pub const PROJECT_MENU_SAVE: u32 = 4;
pub const PROJECT_MENU_SET_STARTUP: u32 = 5;
pub const PROJECT_MENU_UNSET_STARTUP: u32 = 6;
pub const PROJECT_MENU_CLEAR_RECENT: u32 = 7;
pub const PROJECT_MENU_RECENT_BASE: u32 = 100;

pub const LAYOUT_MENU_SAVE: u32 = 1;
pub const LAYOUT_MENU_LOAD: u32 = 2;
pub const LAYOUT_MENU_RESET: u32 = 3;

pub const VIEW_MENU_TOGGLE_API: u32 = 1;

pub struct Chrome {
    pub menu_parent: Entity,
    pub project_button: Entity,
    pub project_menu: Entity,
    pub menu_recent_paths: Vec<String>,
    pub menu_signature: (Vec<String>, Option<String>),
    pub project_label: Entity,
    pub project_edit_button: Entity,
    pub project_edit_input: Entity,
    pub project_edit_ok: Entity,
    pub project_edit_cancel: Entity,
    pub editing_project: bool,
    pub layout_button: Entity,
    pub layout_menu: Entity,
    pub layout_label: Entity,
    pub layout_edit_button: Entity,
    pub layout_edit_input: Entity,
    pub layout_edit_ok: Entity,
    pub layout_edit_cancel: Entity,
    pub editing_layout: bool,
    pub view_button: Entity,
    pub view_menu: Entity,
    pub new_window_button: Entity,
    pub add_panel_button: Entity,
    pub fps_label: Entity,
    pub address_label: Entity,
    pub role_label: Entity,
    pub theme_dropdown: Entity,
    pub rendered_connected: Option<bool>,
}

const BAR_ITEM_HEIGHT: f32 = 24.0;

fn flow_button(tree: &mut UiTreeBuilder, label: &str) -> Entity {
    let transparent = vec4(0.0, 0.0, 0.0, 0.0);
    let font = tree.active_theme().font_size;
    let row = tree
        .add_node()
        .size((0.0).px(), BAR_ITEM_HEIGHT.px())
        .auto_size(AutoSizeMode::Width)
        .auto_size_padding(vec2(10.0, 0.0))
        .with_rect(4.0, 0.0, transparent)
        .color_raw::<UiBase>(transparent)
        .fg_hover(ThemeColor::BackgroundHover)
        .with_interaction()
        .with_transition::<UiHover>(8.0, 6.0)
        .with_cursor_icon(winit::window::CursorIcon::Pointer)
        .flow(FlowDirection::Horizontal, 0.0, 6.0)
        .entity();
    tree.in_parent(row, |tree| {
        tree.add_node()
            .size((0.0).px(), BAR_ITEM_HEIGHT.px())
            .auto_size(AutoSizeMode::Width)
            .with_text(label, font * 0.85)
            .text_left()
            .fg(ThemeColor::Text)
            .entity();
    });
    row
}

fn bar_label(
    tree: &mut UiTreeBuilder,
    text: &str,
    width: f32,
    role: ThemeColor,
    align_right: bool,
) -> Entity {
    let font = tree.active_theme().font_size;
    let builder = tree
        .add_node()
        .size(width.px(), (18.0).px())
        .with_text(text, font * 0.85)
        .with_text_overflow(TextOverflow::Ellipsis)
        .fg(role);
    if align_right {
        builder.text_right().entity()
    } else {
        builder.text_left().entity()
    }
}

pub fn project_menu_builder(settings: &UserSettings) -> (ContextMenuBuilder, Vec<String>) {
    let mut builder = ContextMenuBuilder::new()
        .item_tagged("New Project", "", PROJECT_MENU_NEW)
        .item_tagged("Load Project...", "", PROJECT_MENU_LOAD)
        .separator()
        .item_tagged("Save As Project...", "", PROJECT_MENU_SAVE_AS)
        .item_tagged("Save Project", "", PROJECT_MENU_SAVE)
        .separator()
        .item_tagged("Set as Startup Project", "", PROJECT_MENU_SET_STARTUP)
        .item_tagged("Unset Startup Project", "", PROJECT_MENU_UNSET_STARTUP);

    let mut recent_paths = Vec::new();
    if !settings.recent_projects.is_empty() {
        builder = builder.separator();
        for (index, path) in settings.recent_projects.iter().enumerate() {
            let name = UserSettings::get_recent_project_name(path);
            let is_startup = settings.default_project_path.as_deref() == Some(path.as_str());
            let label = if is_startup {
                format!("Open: {name} (startup)")
            } else {
                format!("Open: {name}")
            };
            builder = builder.item_tagged(&label, "", PROJECT_MENU_RECENT_BASE + index as u32);
            recent_paths.push(path.clone());
        }
        builder =
            builder
                .separator()
                .item_tagged("Clear Recent Projects", "", PROJECT_MENU_CLEAR_RECENT);
    }
    (builder, recent_paths)
}

pub fn rebuild_project_menu(world: &mut World, chrome: &mut Chrome, settings: &UserSettings) {
    if chrome.project_menu != Entity::default() {
        ui_despawn_node(world, chrome.project_menu);
    }
    let (builder, recent_paths) = project_menu_builder(settings);
    let mut tree = UiTreeBuilder::from_parent(world, chrome.menu_parent);
    let menu = tree.add_context_menu_from_builder(builder);
    tree.finish_subtree();
    chrome.project_menu = menu;
    chrome.menu_recent_paths = recent_paths;
    chrome.menu_signature = (
        settings.recent_projects.clone(),
        settings.default_project_path.clone(),
    );
}

pub fn build_chrome(tree: &mut UiTreeBuilder, is_primary: bool) -> Chrome {
    let panel = tree.add_docked_panel_top("top_bar", "", 36.0);
    ui_panel_set_header_visible(tree.world_mut(), panel, false);
    if let Some(data) = tree.world_mut().ui.get_ui_panel_mut(panel) {
        data.min_size = vec2(0.0, 36.0);
        data.resizable = false;
    }

    let content = widget::<UiPanelData>(tree.world_mut(), panel)
        .map(|data| data.content_entity)
        .unwrap_or(panel);
    if let Some(node) = tree.world_mut().ui.get_ui_layout_node_mut(content) {
        node.flow_layout = Some(FlowLayout {
            direction: FlowDirection::Horizontal,
            padding: 6.0,
            spacing: 4.0,
            alignment: FlowAlignment::Start,
            cross_alignment: FlowAlignment::Center,
            wrap: false,
        });
    }

    let mut chrome = Chrome {
        menu_parent: tree.root_entity(),
        project_button: Entity::default(),
        project_menu: Entity::default(),
        menu_recent_paths: Vec::new(),
        menu_signature: (Vec::new(), None),
        project_label: Entity::default(),
        project_edit_button: Entity::default(),
        project_edit_input: Entity::default(),
        project_edit_ok: Entity::default(),
        project_edit_cancel: Entity::default(),
        editing_project: false,
        layout_button: Entity::default(),
        layout_menu: Entity::default(),
        layout_label: Entity::default(),
        layout_edit_button: Entity::default(),
        layout_edit_input: Entity::default(),
        layout_edit_ok: Entity::default(),
        layout_edit_cancel: Entity::default(),
        editing_layout: false,
        view_button: Entity::default(),
        view_menu: Entity::default(),
        new_window_button: Entity::default(),
        add_panel_button: Entity::default(),
        fps_label: Entity::default(),
        address_label: Entity::default(),
        role_label: Entity::default(),
        theme_dropdown: Entity::default(),
        rendered_connected: None,
    };

    tree.in_parent(content, |tree| {
        if is_primary {
            chrome.project_button = flow_button(tree, "Project");
        } else {
            bar_label(tree, "Window", 56.0, ThemeColor::Text, false);
        }
        chrome.project_label = bar_label(
            tree,
            "Untitled Project",
            140.0,
            ThemeColor::TextDisabled,
            false,
        );
        chrome.project_edit_button = flow_button(tree, "Edit");
        chrome.project_edit_input = tree.add_text_input("Project name");
        if let Some(node) = tree
            .world_mut()
            .ui
            .get_ui_layout_node_mut(chrome.project_edit_input)
        {
            node.flow_child_size = Some(Ab(vec2(160.0, BAR_ITEM_HEIGHT)).into());
        }
        chrome.project_edit_ok = flow_button(tree, "OK");
        chrome.project_edit_cancel = flow_button(tree, "X");

        chrome.layout_button = flow_button(tree, "Layout");
        chrome.layout_label = bar_label(
            tree,
            "Default Layout",
            130.0,
            ThemeColor::TextDisabled,
            false,
        );
        chrome.layout_edit_button = flow_button(tree, "Edit");
        chrome.layout_edit_input = tree.add_text_input("Layout name");
        if let Some(node) = tree
            .world_mut()
            .ui
            .get_ui_layout_node_mut(chrome.layout_edit_input)
        {
            node.flow_child_size = Some(Ab(vec2(160.0, BAR_ITEM_HEIGHT)).into());
        }
        chrome.layout_edit_ok = flow_button(tree, "OK");
        chrome.layout_edit_cancel = flow_button(tree, "X");

        chrome.view_button = flow_button(tree, "View");
        chrome.new_window_button = flow_button(tree, "New Window");
        chrome.add_panel_button = flow_button(tree, "Add Panel");

        tree.add_spring();

        bar_label(
            tree,
            concat!("v", env!("CARGO_PKG_VERSION")),
            44.0,
            ThemeColor::TextDisabled,
            true,
        );
        chrome.fps_label = bar_label(tree, "0 FPS", 56.0, ThemeColor::TextDisabled, true);
        chrome.address_label = bar_label(tree, "disconnected", 120.0, ThemeColor::Error, true);
        chrome.role_label = bar_label(
            tree,
            if is_primary { "Primary" } else { "Window" },
            54.0,
            ThemeColor::Text,
            true,
        );
        let theme_holder = tree
            .add_node()
            .size((170.0).px(), (28.0).px())
            .flow_vertical()
            .padding(0.0)
            .gap(0.0)
            .entity();
        chrome.theme_dropdown = tree.in_parent(theme_holder, |tree| tree.add_theme_dropdown());
    });

    chrome.layout_menu = tree.add_context_menu_from_builder(
        ContextMenuBuilder::new()
            .item_tagged("Save Layout...", "", LAYOUT_MENU_SAVE)
            .item_tagged("Load Layout...", "", LAYOUT_MENU_LOAD)
            .separator()
            .item_tagged("Reset Layout", "", LAYOUT_MENU_RESET),
    );
    chrome.view_menu = tree.add_context_menu_from_builder(ContextMenuBuilder::new().item_tagged(
        "Toggle Api Window",
        "",
        VIEW_MENU_TOGGLE_API,
    ));

    let world = tree.world_mut();
    for entity in [
        chrome.project_edit_input,
        chrome.project_edit_ok,
        chrome.project_edit_cancel,
        chrome.layout_edit_input,
        chrome.layout_edit_ok,
        chrome.layout_edit_cancel,
    ] {
        ui_set_visible(world, entity, false);
    }

    chrome
}

pub fn open_menu_at_button(world: &mut World, button: Entity, menu: Entity) {
    let position = world
        .ui
        .get_ui_layout_node(button)
        .map(|node| vec2(node.computed_rect.min.x, node.computed_rect.max.y))
        .unwrap_or_default();
    ui_show_context_menu(world, menu, position);
}

impl Chrome {
    pub fn set_project_editing(&mut self, world: &mut World, editing: bool, initial: &str) {
        self.editing_project = editing;
        ui_set_visible(world, self.project_label, !editing);
        ui_set_visible(world, self.project_edit_button, !editing);
        ui_set_visible(world, self.project_edit_input, editing);
        ui_set_visible(world, self.project_edit_ok, editing);
        ui_set_visible(world, self.project_edit_cancel, editing);
        if editing {
            ui_text_input_set_value(world, self.project_edit_input, initial);
            ui_focus(world, self.project_edit_input);
        }
    }

    pub fn set_layout_editing(&mut self, world: &mut World, editing: bool, initial: &str) {
        self.editing_layout = editing;
        ui_set_visible(world, self.layout_label, !editing);
        ui_set_visible(world, self.layout_edit_button, !editing);
        ui_set_visible(world, self.layout_edit_input, editing);
        ui_set_visible(world, self.layout_edit_ok, editing);
        ui_set_visible(world, self.layout_edit_cancel, editing);
        if editing {
            ui_text_input_set_value(world, self.layout_edit_input, initial);
            ui_focus(world, self.layout_edit_input);
        }
    }
}
