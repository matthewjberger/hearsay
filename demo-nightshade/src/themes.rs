use nightshade::ecs::ui::theme::ThemePalette;
use nightshade::prelude::*;

struct DemoPalette {
    name: &'static str,
    dark_mode: bool,
    background: Vec4,
    panel: Vec4,
    recessed: Vec4,
    widget: Vec4,
    widget_hovered: Vec4,
    widget_active: Vec4,
    outline: Vec4,
    text: Vec4,
    accent: Vec4,
    selection: Vec4,
    warning: Vec4,
    error: Vec4,
}

fn rgb(red: u8, green: u8, blue: u8) -> Vec4 {
    vec4(
        red as f32 / 255.0,
        green as f32 / 255.0,
        blue as f32 / 255.0,
        1.0,
    )
}

fn mix(first: Vec4, second: Vec4, amount: f32) -> Vec4 {
    first + (second - first) * amount
}

fn with_alpha(color: Vec4, alpha: f32) -> Vec4 {
    vec4(color.x, color.y, color.z, alpha)
}

fn nudge(color: Vec4, amount: f32) -> Vec4 {
    vec4(
        (color.x + amount).clamp(0.0, 1.0),
        (color.y + amount).clamp(0.0, 1.0),
        (color.z + amount).clamp(0.0, 1.0),
        color.w,
    )
}

fn build_theme(palette: DemoPalette) -> UiTheme {
    let success = if palette.dark_mode {
        rgb(96, 200, 120)
    } else {
        rgb(46, 140, 70)
    };
    UiTheme::from_palette(ThemePalette {
        name: palette.name,
        dark_mode: palette.dark_mode,
        text_color: palette.text,
        text_color_disabled: mix(palette.text, palette.background, 0.45),
        text_color_accent: palette.accent,
        background_color: palette.background,
        background_color_hovered: palette.widget_hovered,
        background_color_active: palette.widget_active,
        panel_color: palette.panel,
        panel_header_color: palette.widget,
        border_color: palette.outline,
        border_color_focused: palette.accent,
        accent_color: palette.accent,
        accent_color_hovered: nudge(palette.accent, 0.08),
        accent_color_active: nudge(palette.accent, -0.08),
        success_color: success,
        warning_color: palette.warning,
        error_color: palette.error,
        slider_track_color: palette.recessed,
        slider_fill_color: palette.accent,
        input_background_color: palette.recessed,
        input_background_focused: nudge(palette.recessed, 0.04),
        selection_color: with_alpha(palette.selection, 0.6),
        scrollbar_color: with_alpha(palette.outline, 0.5),
        scrollbar_color_hovered: with_alpha(palette.outline, 0.8),
    })
}

fn deep_ocean() -> DemoPalette {
    DemoPalette {
        name: "Deep Ocean",
        dark_mode: true,
        background: rgb(11, 18, 32),
        panel: rgb(17, 27, 45),
        recessed: rgb(7, 12, 22),
        widget: rgb(30, 45, 70),
        widget_hovered: rgb(42, 61, 92),
        widget_active: rgb(54, 78, 115),
        outline: rgb(52, 73, 102),
        text: rgb(190, 205, 221),
        accent: rgb(56, 189, 178),
        selection: rgb(18, 90, 84),
        warning: rgb(240, 173, 78),
        error: rgb(229, 95, 102),
    }
}

fn dracula() -> DemoPalette {
    DemoPalette {
        name: "Dracula",
        dark_mode: true,
        background: rgb(40, 42, 54),
        panel: rgb(33, 34, 44),
        recessed: rgb(25, 26, 33),
        widget: rgb(68, 71, 90),
        widget_hovered: rgb(86, 90, 117),
        widget_active: rgb(98, 114, 164),
        outline: rgb(98, 114, 164),
        text: rgb(248, 248, 242),
        accent: rgb(189, 147, 249),
        selection: rgb(68, 71, 90),
        warning: rgb(241, 250, 140),
        error: rgb(255, 85, 85),
    }
}

fn nord() -> DemoPalette {
    DemoPalette {
        name: "Nord",
        dark_mode: true,
        background: rgb(46, 52, 64),
        panel: rgb(59, 66, 82),
        recessed: rgb(36, 41, 51),
        widget: rgb(67, 76, 94),
        widget_hovered: rgb(76, 86, 106),
        widget_active: rgb(94, 129, 172),
        outline: rgb(76, 86, 106),
        text: rgb(216, 222, 233),
        accent: rgb(136, 192, 208),
        selection: rgb(67, 76, 94),
        warning: rgb(235, 203, 139),
        error: rgb(191, 97, 106),
    }
}

fn gruvbox_dark() -> DemoPalette {
    DemoPalette {
        name: "Gruvbox Dark",
        dark_mode: true,
        background: rgb(40, 40, 40),
        panel: rgb(60, 56, 54),
        recessed: rgb(29, 32, 33),
        widget: rgb(80, 73, 69),
        widget_hovered: rgb(102, 92, 84),
        widget_active: rgb(124, 111, 100),
        outline: rgb(102, 92, 84),
        text: rgb(235, 219, 178),
        accent: rgb(254, 128, 25),
        selection: rgb(102, 92, 84),
        warning: rgb(250, 189, 47),
        error: rgb(251, 73, 52),
    }
}

fn monokai() -> DemoPalette {
    DemoPalette {
        name: "Monokai",
        dark_mode: true,
        background: rgb(39, 40, 34),
        panel: rgb(30, 31, 25),
        recessed: rgb(24, 25, 20),
        widget: rgb(73, 72, 62),
        widget_hovered: rgb(94, 92, 77),
        widget_active: rgb(117, 113, 94),
        outline: rgb(117, 113, 94),
        text: rgb(248, 248, 242),
        accent: rgb(102, 217, 239),
        selection: rgb(73, 72, 62),
        warning: rgb(230, 219, 116),
        error: rgb(249, 38, 114),
    }
}

fn one_dark() -> DemoPalette {
    DemoPalette {
        name: "One Dark",
        dark_mode: true,
        background: rgb(40, 44, 52),
        panel: rgb(33, 37, 43),
        recessed: rgb(24, 26, 31),
        widget: rgb(62, 68, 81),
        widget_hovered: rgb(75, 82, 99),
        widget_active: rgb(92, 99, 112),
        outline: rgb(75, 82, 99),
        text: rgb(171, 178, 191),
        accent: rgb(97, 175, 239),
        selection: rgb(62, 68, 81),
        warning: rgb(229, 192, 123),
        error: rgb(224, 108, 117),
    }
}

fn tokyo_night() -> DemoPalette {
    DemoPalette {
        name: "Tokyo Night",
        dark_mode: true,
        background: rgb(26, 27, 38),
        panel: rgb(36, 40, 59),
        recessed: rgb(22, 22, 30),
        widget: rgb(41, 46, 66),
        widget_hovered: rgb(59, 66, 97),
        widget_active: rgb(65, 72, 104),
        outline: rgb(59, 66, 97),
        text: rgb(192, 202, 245),
        accent: rgb(122, 162, 247),
        selection: rgb(40, 52, 87),
        warning: rgb(224, 175, 104),
        error: rgb(247, 118, 142),
    }
}

fn catppuccin_mocha() -> DemoPalette {
    DemoPalette {
        name: "Catppuccin Mocha",
        dark_mode: true,
        background: rgb(30, 30, 46),
        panel: rgb(24, 24, 37),
        recessed: rgb(17, 17, 27),
        widget: rgb(49, 50, 68),
        widget_hovered: rgb(69, 71, 90),
        widget_active: rgb(88, 91, 112),
        outline: rgb(69, 71, 90),
        text: rgb(205, 214, 244),
        accent: rgb(203, 166, 247),
        selection: rgb(69, 71, 90),
        warning: rgb(249, 226, 175),
        error: rgb(243, 139, 168),
    }
}

fn solarized_dark() -> DemoPalette {
    DemoPalette {
        name: "Solarized Dark",
        dark_mode: true,
        background: rgb(0, 43, 54),
        panel: rgb(7, 54, 66),
        recessed: rgb(0, 33, 43),
        widget: rgb(13, 69, 84),
        widget_hovered: rgb(23, 87, 104),
        widget_active: rgb(88, 110, 117),
        outline: rgb(88, 110, 117),
        text: rgb(131, 148, 150),
        accent: rgb(42, 161, 152),
        selection: rgb(13, 69, 84),
        warning: rgb(181, 137, 0),
        error: rgb(220, 50, 47),
    }
}

fn sandstone() -> DemoPalette {
    DemoPalette {
        name: "Sandstone",
        dark_mode: false,
        background: rgb(244, 237, 224),
        panel: rgb(237, 227, 209),
        recessed: rgb(228, 215, 192),
        widget: rgb(223, 208, 183),
        widget_hovered: rgb(211, 192, 162),
        widget_active: rgb(199, 177, 142),
        outline: rgb(168, 147, 118),
        text: rgb(77, 62, 45),
        accent: rgb(188, 92, 58),
        selection: rgb(232, 196, 168),
        warning: rgb(176, 121, 30),
        error: rgb(178, 52, 56),
    }
}

fn solarized_light() -> DemoPalette {
    DemoPalette {
        name: "Solarized Light",
        dark_mode: false,
        background: rgb(253, 246, 227),
        panel: rgb(238, 232, 213),
        recessed: rgb(255, 252, 242),
        widget: rgb(228, 221, 200),
        widget_hovered: rgb(216, 207, 181),
        widget_active: rgb(203, 192, 163),
        outline: rgb(181, 173, 148),
        text: rgb(101, 123, 131),
        accent: rgb(38, 139, 210),
        selection: rgb(199, 219, 239),
        warning: rgb(181, 137, 0),
        error: rgb(220, 50, 47),
    }
}

pub fn theme_presets() -> Vec<UiTheme> {
    [
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
    ]
    .into_iter()
    .map(|palette| {
        let mut theme = build_theme(palette);
        theme.font_size = 15.0;
        theme.button_height = 28.0;
        theme
    })
    .collect()
}

pub fn install_themes(world: &mut World, saved_theme_name: Option<&str>) {
    let presets = theme_presets();
    let selected_index = saved_theme_name
        .and_then(|name| presets.iter().position(|preset| preset.name == name))
        .unwrap_or(0);
    let theme_state = &mut world.resources.retained_ui.theme_state;
    theme_state.presets = presets;
    theme_state.current_theme = theme_state.presets[selected_index].clone();
    theme_state.selected_preset_index = Some(selected_index);
    theme_state.generation += 1;
}
