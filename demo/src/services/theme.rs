use crate::prelude::*;

#[derive(Clone, Debug)]
pub struct ThemePalette {
    pub background: egui::Color32,
    pub panel: egui::Color32,
    pub recessed: egui::Color32,
    pub widget: egui::Color32,
    pub widget_hovered: egui::Color32,
    pub widget_active: egui::Color32,
    pub outline: egui::Color32,
    pub text: egui::Color32,
    pub text_strong: egui::Color32,
    pub accent: egui::Color32,
    pub selection: egui::Color32,
    pub selection_text: egui::Color32,
    pub warning: egui::Color32,
    pub error: egui::Color32,
}

#[derive(Clone, Debug)]
pub struct ThemePreset {
    pub name: &'static str,
    pub dark_mode: bool,
    pub palette: ThemePalette,
}

fn deep_ocean() -> ThemePreset {
    ThemePreset {
        name: "Deep Ocean",
        dark_mode: true,
        palette: ThemePalette {
            background: egui::Color32::from_rgb(11, 18, 32),
            panel: egui::Color32::from_rgb(17, 27, 45),
            recessed: egui::Color32::from_rgb(7, 12, 22),
            widget: egui::Color32::from_rgb(30, 45, 70),
            widget_hovered: egui::Color32::from_rgb(42, 61, 92),
            widget_active: egui::Color32::from_rgb(54, 78, 115),
            outline: egui::Color32::from_rgb(52, 73, 102),
            text: egui::Color32::from_rgb(190, 205, 221),
            text_strong: egui::Color32::from_rgb(235, 244, 252),
            accent: egui::Color32::from_rgb(56, 189, 178),
            selection: egui::Color32::from_rgb(18, 90, 84),
            selection_text: egui::Color32::from_rgb(235, 244, 252),
            warning: egui::Color32::from_rgb(240, 173, 78),
            error: egui::Color32::from_rgb(229, 95, 102),
        },
    }
}

fn dracula() -> ThemePreset {
    ThemePreset {
        name: "Dracula",
        dark_mode: true,
        palette: ThemePalette {
            background: egui::Color32::from_rgb(40, 42, 54),
            panel: egui::Color32::from_rgb(33, 34, 44),
            recessed: egui::Color32::from_rgb(25, 26, 33),
            widget: egui::Color32::from_rgb(68, 71, 90),
            widget_hovered: egui::Color32::from_rgb(86, 90, 117),
            widget_active: egui::Color32::from_rgb(98, 114, 164),
            outline: egui::Color32::from_rgb(98, 114, 164),
            text: egui::Color32::from_rgb(248, 248, 242),
            text_strong: egui::Color32::from_rgb(255, 255, 255),
            accent: egui::Color32::from_rgb(189, 147, 249),
            selection: egui::Color32::from_rgb(68, 71, 90),
            selection_text: egui::Color32::from_rgb(248, 248, 242),
            warning: egui::Color32::from_rgb(241, 250, 140),
            error: egui::Color32::from_rgb(255, 85, 85),
        },
    }
}

fn nord() -> ThemePreset {
    ThemePreset {
        name: "Nord",
        dark_mode: true,
        palette: ThemePalette {
            background: egui::Color32::from_rgb(46, 52, 64),
            panel: egui::Color32::from_rgb(59, 66, 82),
            recessed: egui::Color32::from_rgb(36, 41, 51),
            widget: egui::Color32::from_rgb(67, 76, 94),
            widget_hovered: egui::Color32::from_rgb(76, 86, 106),
            widget_active: egui::Color32::from_rgb(94, 129, 172),
            outline: egui::Color32::from_rgb(76, 86, 106),
            text: egui::Color32::from_rgb(216, 222, 233),
            text_strong: egui::Color32::from_rgb(236, 239, 244),
            accent: egui::Color32::from_rgb(136, 192, 208),
            selection: egui::Color32::from_rgb(67, 76, 94),
            selection_text: egui::Color32::from_rgb(236, 239, 244),
            warning: egui::Color32::from_rgb(235, 203, 139),
            error: egui::Color32::from_rgb(191, 97, 106),
        },
    }
}

fn gruvbox_dark() -> ThemePreset {
    ThemePreset {
        name: "Gruvbox Dark",
        dark_mode: true,
        palette: ThemePalette {
            background: egui::Color32::from_rgb(40, 40, 40),
            panel: egui::Color32::from_rgb(60, 56, 54),
            recessed: egui::Color32::from_rgb(29, 32, 33),
            widget: egui::Color32::from_rgb(80, 73, 69),
            widget_hovered: egui::Color32::from_rgb(102, 92, 84),
            widget_active: egui::Color32::from_rgb(124, 111, 100),
            outline: egui::Color32::from_rgb(102, 92, 84),
            text: egui::Color32::from_rgb(235, 219, 178),
            text_strong: egui::Color32::from_rgb(251, 241, 199),
            accent: egui::Color32::from_rgb(254, 128, 25),
            selection: egui::Color32::from_rgb(102, 92, 84),
            selection_text: egui::Color32::from_rgb(251, 241, 199),
            warning: egui::Color32::from_rgb(250, 189, 47),
            error: egui::Color32::from_rgb(251, 73, 52),
        },
    }
}

fn monokai() -> ThemePreset {
    ThemePreset {
        name: "Monokai",
        dark_mode: true,
        palette: ThemePalette {
            background: egui::Color32::from_rgb(39, 40, 34),
            panel: egui::Color32::from_rgb(30, 31, 25),
            recessed: egui::Color32::from_rgb(24, 25, 20),
            widget: egui::Color32::from_rgb(73, 72, 62),
            widget_hovered: egui::Color32::from_rgb(94, 92, 77),
            widget_active: egui::Color32::from_rgb(117, 113, 94),
            outline: egui::Color32::from_rgb(117, 113, 94),
            text: egui::Color32::from_rgb(248, 248, 242),
            text_strong: egui::Color32::from_rgb(255, 255, 255),
            accent: egui::Color32::from_rgb(102, 217, 239),
            selection: egui::Color32::from_rgb(73, 72, 62),
            selection_text: egui::Color32::from_rgb(248, 248, 242),
            warning: egui::Color32::from_rgb(230, 219, 116),
            error: egui::Color32::from_rgb(249, 38, 114),
        },
    }
}

fn one_dark() -> ThemePreset {
    ThemePreset {
        name: "One Dark",
        dark_mode: true,
        palette: ThemePalette {
            background: egui::Color32::from_rgb(40, 44, 52),
            panel: egui::Color32::from_rgb(33, 37, 43),
            recessed: egui::Color32::from_rgb(24, 26, 31),
            widget: egui::Color32::from_rgb(62, 68, 81),
            widget_hovered: egui::Color32::from_rgb(75, 82, 99),
            widget_active: egui::Color32::from_rgb(92, 99, 112),
            outline: egui::Color32::from_rgb(75, 82, 99),
            text: egui::Color32::from_rgb(171, 178, 191),
            text_strong: egui::Color32::from_rgb(220, 223, 228),
            accent: egui::Color32::from_rgb(97, 175, 239),
            selection: egui::Color32::from_rgb(62, 68, 81),
            selection_text: egui::Color32::from_rgb(220, 223, 228),
            warning: egui::Color32::from_rgb(229, 192, 123),
            error: egui::Color32::from_rgb(224, 108, 117),
        },
    }
}

fn tokyo_night() -> ThemePreset {
    ThemePreset {
        name: "Tokyo Night",
        dark_mode: true,
        palette: ThemePalette {
            background: egui::Color32::from_rgb(26, 27, 38),
            panel: egui::Color32::from_rgb(36, 40, 59),
            recessed: egui::Color32::from_rgb(22, 22, 30),
            widget: egui::Color32::from_rgb(41, 46, 66),
            widget_hovered: egui::Color32::from_rgb(59, 66, 97),
            widget_active: egui::Color32::from_rgb(65, 72, 104),
            outline: egui::Color32::from_rgb(59, 66, 97),
            text: egui::Color32::from_rgb(192, 202, 245),
            text_strong: egui::Color32::from_rgb(224, 231, 255),
            accent: egui::Color32::from_rgb(122, 162, 247),
            selection: egui::Color32::from_rgb(40, 52, 87),
            selection_text: egui::Color32::from_rgb(192, 202, 245),
            warning: egui::Color32::from_rgb(224, 175, 104),
            error: egui::Color32::from_rgb(247, 118, 142),
        },
    }
}

fn catppuccin_mocha() -> ThemePreset {
    ThemePreset {
        name: "Catppuccin Mocha",
        dark_mode: true,
        palette: ThemePalette {
            background: egui::Color32::from_rgb(30, 30, 46),
            panel: egui::Color32::from_rgb(24, 24, 37),
            recessed: egui::Color32::from_rgb(17, 17, 27),
            widget: egui::Color32::from_rgb(49, 50, 68),
            widget_hovered: egui::Color32::from_rgb(69, 71, 90),
            widget_active: egui::Color32::from_rgb(88, 91, 112),
            outline: egui::Color32::from_rgb(69, 71, 90),
            text: egui::Color32::from_rgb(205, 214, 244),
            text_strong: egui::Color32::from_rgb(230, 236, 255),
            accent: egui::Color32::from_rgb(203, 166, 247),
            selection: egui::Color32::from_rgb(69, 71, 90),
            selection_text: egui::Color32::from_rgb(205, 214, 244),
            warning: egui::Color32::from_rgb(249, 226, 175),
            error: egui::Color32::from_rgb(243, 139, 168),
        },
    }
}

fn solarized_dark() -> ThemePreset {
    ThemePreset {
        name: "Solarized Dark",
        dark_mode: true,
        palette: ThemePalette {
            background: egui::Color32::from_rgb(0, 43, 54),
            panel: egui::Color32::from_rgb(7, 54, 66),
            recessed: egui::Color32::from_rgb(0, 33, 43),
            widget: egui::Color32::from_rgb(13, 69, 84),
            widget_hovered: egui::Color32::from_rgb(23, 87, 104),
            widget_active: egui::Color32::from_rgb(88, 110, 117),
            outline: egui::Color32::from_rgb(88, 110, 117),
            text: egui::Color32::from_rgb(131, 148, 150),
            text_strong: egui::Color32::from_rgb(238, 232, 213),
            accent: egui::Color32::from_rgb(42, 161, 152),
            selection: egui::Color32::from_rgb(13, 69, 84),
            selection_text: egui::Color32::from_rgb(238, 232, 213),
            warning: egui::Color32::from_rgb(181, 137, 0),
            error: egui::Color32::from_rgb(220, 50, 47),
        },
    }
}

fn sandstone() -> ThemePreset {
    ThemePreset {
        name: "Sandstone",
        dark_mode: false,
        palette: ThemePalette {
            background: egui::Color32::from_rgb(244, 237, 224),
            panel: egui::Color32::from_rgb(237, 227, 209),
            recessed: egui::Color32::from_rgb(228, 215, 192),
            widget: egui::Color32::from_rgb(223, 208, 183),
            widget_hovered: egui::Color32::from_rgb(211, 192, 162),
            widget_active: egui::Color32::from_rgb(199, 177, 142),
            outline: egui::Color32::from_rgb(168, 147, 118),
            text: egui::Color32::from_rgb(77, 62, 45),
            text_strong: egui::Color32::from_rgb(45, 34, 22),
            accent: egui::Color32::from_rgb(188, 92, 58),
            selection: egui::Color32::from_rgb(232, 196, 168),
            selection_text: egui::Color32::from_rgb(45, 34, 22),
            warning: egui::Color32::from_rgb(176, 121, 30),
            error: egui::Color32::from_rgb(178, 52, 56),
        },
    }
}

fn solarized_light() -> ThemePreset {
    ThemePreset {
        name: "Solarized Light",
        dark_mode: false,
        palette: ThemePalette {
            background: egui::Color32::from_rgb(253, 246, 227),
            panel: egui::Color32::from_rgb(238, 232, 213),
            recessed: egui::Color32::from_rgb(255, 252, 242),
            widget: egui::Color32::from_rgb(228, 221, 200),
            widget_hovered: egui::Color32::from_rgb(216, 207, 181),
            widget_active: egui::Color32::from_rgb(203, 192, 163),
            outline: egui::Color32::from_rgb(181, 173, 148),
            text: egui::Color32::from_rgb(101, 123, 131),
            text_strong: egui::Color32::from_rgb(7, 54, 66),
            accent: egui::Color32::from_rgb(38, 139, 210),
            selection: egui::Color32::from_rgb(199, 219, 239),
            selection_text: egui::Color32::from_rgb(7, 54, 66),
            warning: egui::Color32::from_rgb(181, 137, 0),
            error: egui::Color32::from_rgb(220, 50, 47),
        },
    }
}

#[derive(Resource)]
pub struct ThemeState {
    pub presets: Vec<ThemePreset>,
    pub selected_index: usize,
    pub preview_index: Option<usize>,
}

impl Default for ThemeState {
    fn default() -> Self {
        Self {
            presets: vec![
                deep_ocean(),
                dracula(),
                nord(),
                gruvbox_dark(),
                monokai(),
                one_dark(),
                tokyo_night(),
                catppuccin_mocha(),
                solarized_dark(),
                sandstone(),
                solarized_light(),
            ],
            selected_index: 0,
            preview_index: None,
        }
    }
}

pub fn get_active_theme_visuals(theme_state: &ThemeState) -> egui::Visuals {
    let active_index = theme_state
        .preview_index
        .unwrap_or(theme_state.selected_index);
    theme_state
        .presets
        .get(active_index)
        .or_else(|| theme_state.presets.get(theme_state.selected_index))
        .or_else(|| theme_state.presets.first())
        .map(build_visuals)
        .unwrap_or_else(egui::Visuals::dark)
}

fn build_visuals(preset: &ThemePreset) -> egui::Visuals {
    let palette = &preset.palette;
    let mut visuals = if preset.dark_mode {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    visuals.panel_fill = palette.background;
    visuals.window_fill = palette.panel;
    visuals.window_stroke = egui::Stroke::new(1.0, palette.outline);
    visuals.faint_bg_color = palette.panel;
    visuals.extreme_bg_color = palette.recessed;
    visuals.code_bg_color = palette.recessed;
    visuals.hyperlink_color = palette.accent;
    visuals.warn_fg_color = palette.warning;
    visuals.error_fg_color = palette.error;

    visuals.selection.bg_fill = palette.selection;
    visuals.selection.stroke = egui::Stroke::new(1.0, palette.selection_text);

    visuals.widgets.noninteractive.bg_fill = palette.panel;
    visuals.widgets.noninteractive.weak_bg_fill = palette.panel;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, palette.outline);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, palette.text);

    visuals.widgets.inactive.bg_fill = palette.widget;
    visuals.widgets.inactive.weak_bg_fill = palette.widget;
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, palette.text);

    visuals.widgets.hovered.bg_fill = palette.widget_hovered;
    visuals.widgets.hovered.weak_bg_fill = palette.widget_hovered;
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, palette.accent);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.5, palette.text_strong);

    visuals.widgets.active.bg_fill = palette.widget_active;
    visuals.widgets.active.weak_bg_fill = palette.widget_active;
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, palette.accent);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(2.0, palette.text_strong);

    visuals.widgets.open.bg_fill = palette.widget;
    visuals.widgets.open.weak_bg_fill = palette.widget;
    visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0, palette.outline);
    visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0, palette.text_strong);

    visuals
}

pub struct ThemeServicePlugin;

impl Plugin for ThemeServicePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ThemeState>()
            .add_systems(Startup, apply_saved_theme);
    }
}

fn apply_saved_theme(user_settings: Res<UserSettings>, mut theme_state: ResMut<ThemeState>) {
    let Some(theme_name) = &user_settings.theme_name else {
        return;
    };
    if let Some(preset_index) = theme_state
        .presets
        .iter()
        .position(|preset| preset.name == theme_name)
    {
        theme_state.selected_index = preset_index;
    }
}
