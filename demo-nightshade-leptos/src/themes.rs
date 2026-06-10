//! The same eleven palettes as the native demos, applied as CSS variables on
//! the document root and persisted to local storage.

use wasm_bindgen::JsCast;

pub struct Theme {
    pub name: &'static str,
    pub background: &'static str,
    pub panel: &'static str,
    pub panel_border: &'static str,
    pub text: &'static str,
    pub text_dim: &'static str,
    pub accent: &'static str,
    pub input_background: &'static str,
    pub success: &'static str,
    pub warning: &'static str,
    pub error: &'static str,
}

pub const THEMES: [Theme; 11] = [
    Theme {
        name: "Deep Ocean",
        background: "#0b1220",
        panel: "#111b2d",
        panel_border: "#344966",
        text: "#becdde",
        text_dim: "#7e8da0",
        accent: "#38bdb2",
        input_background: "#070c16",
        success: "#60c878",
        warning: "#f0ad4e",
        error: "#e55f66",
    },
    Theme {
        name: "Dracula",
        background: "#282a36",
        panel: "#21222c",
        panel_border: "#6272a4",
        text: "#f8f8f2",
        text_dim: "#9aa0b6",
        accent: "#bd93f9",
        input_background: "#191a21",
        success: "#50fa7b",
        warning: "#f1fa8c",
        error: "#ff5555",
    },
    Theme {
        name: "Nord",
        background: "#2e3440",
        panel: "#3b4252",
        panel_border: "#4c566a",
        text: "#d8dee9",
        text_dim: "#94a0b3",
        accent: "#88c0d0",
        input_background: "#242933",
        success: "#a3be8c",
        warning: "#ebcb8b",
        error: "#bf616a",
    },
    Theme {
        name: "Gruvbox Dark",
        background: "#282828",
        panel: "#3c3836",
        panel_border: "#665c54",
        text: "#ebdbb2",
        text_dim: "#a89984",
        accent: "#fe8019",
        input_background: "#1d2021",
        success: "#b8bb26",
        warning: "#fabd2f",
        error: "#fb4934",
    },
    Theme {
        name: "Monokai",
        background: "#272822",
        panel: "#1e1f19",
        panel_border: "#75715e",
        text: "#f8f8f2",
        text_dim: "#a59f85",
        accent: "#66d9ef",
        input_background: "#181914",
        success: "#a6e22e",
        warning: "#e6db74",
        error: "#f92672",
    },
    Theme {
        name: "One Dark",
        background: "#282c34",
        panel: "#21252b",
        panel_border: "#4b5263",
        text: "#abb2bf",
        text_dim: "#7f8694",
        accent: "#61afef",
        input_background: "#181a1f",
        success: "#98c379",
        warning: "#e5c07b",
        error: "#e06c75",
    },
    Theme {
        name: "Tokyo Night",
        background: "#1a1b26",
        panel: "#24283b",
        panel_border: "#3b4261",
        text: "#c0caf5",
        text_dim: "#8089b3",
        accent: "#7aa2f7",
        input_background: "#16161e",
        success: "#9ece6a",
        warning: "#e0af68",
        error: "#f7768e",
    },
    Theme {
        name: "Catppuccin Mocha",
        background: "#1e1e2e",
        panel: "#181825",
        panel_border: "#45475a",
        text: "#cdd6f4",
        text_dim: "#8d92ad",
        accent: "#cba6f7",
        input_background: "#11111b",
        success: "#a6e3a1",
        warning: "#f9e2af",
        error: "#f38ba8",
    },
    Theme {
        name: "Solarized Dark",
        background: "#002b36",
        panel: "#073642",
        panel_border: "#586e75",
        text: "#839496",
        text_dim: "#657b83",
        accent: "#2aa198",
        input_background: "#00212b",
        success: "#859900",
        warning: "#b58900",
        error: "#dc322f",
    },
    Theme {
        name: "Sandstone",
        background: "#f4ede0",
        panel: "#ede3d1",
        panel_border: "#a89376",
        text: "#4d3e2d",
        text_dim: "#7d6c56",
        accent: "#bc5c3a",
        input_background: "#e4d7c0",
        success: "#2e8c46",
        warning: "#b0791e",
        error: "#b23438",
    },
    Theme {
        name: "Solarized Light",
        background: "#fdf6e3",
        panel: "#eee8d5",
        panel_border: "#b5ad94",
        text: "#657b83",
        text_dim: "#93a1a1",
        accent: "#268bd2",
        input_background: "#fffcf2",
        success: "#859900",
        warning: "#b58900",
        error: "#dc322f",
    },
];

const STORAGE_KEY: &str = "hearsay-demo-leptos-theme";

pub fn apply_theme(index: usize) {
    let theme = &THEMES[index % THEMES.len()];
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let Some(root) = document.document_element() else {
        return;
    };
    let Ok(element) = root.dyn_into::<web_sys::HtmlElement>() else {
        return;
    };
    let style = element.style();
    for (variable, value) in [
        ("--bg", theme.background),
        ("--panel", theme.panel),
        ("--panel-border", theme.panel_border),
        ("--text", theme.text),
        ("--text-dim", theme.text_dim),
        ("--accent", theme.accent),
        ("--input-bg", theme.input_background),
        ("--success", theme.success),
        ("--warning", theme.warning),
        ("--error", theme.error),
    ] {
        let _ = style.set_property(variable, value);
    }
}

pub fn save_theme(index: usize) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(STORAGE_KEY, THEMES[index % THEMES.len()].name);
    }
}

pub fn load_theme() -> usize {
    let Some(storage) = local_storage() else {
        return 0;
    };
    let Ok(Some(name)) = storage.get_item(STORAGE_KEY) else {
        return 0;
    };
    THEMES
        .iter()
        .position(|theme| theme.name == name)
        .unwrap_or(0)
}

pub fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|window| window.local_storage().ok().flatten())
}
