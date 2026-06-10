use crate::prelude::*;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Resource, Default)]
#[serde(default)]
pub struct UserSettings {
    pub default_project_path: Option<String>,
    pub recent_projects: Vec<String>,
    pub theme_name: Option<String>,
}

impl UserSettings {
    pub fn load() -> Self {
        let Some(path) = Self::settings_path() else {
            return Self::default();
        };
        if !path.exists() {
            return Self::default();
        }
        match fs::read_to_string(&path) {
            Ok(json) => match serde_json::from_str::<UserSettings>(&json) {
                Ok(settings) => settings,
                Err(error) => {
                    bevy::log::error!("Failed to parse user settings: {error}");
                    Self::default()
                }
            },
            Err(error) => {
                bevy::log::error!("Failed to read user settings file: {error}");
                Self::default()
            }
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let settings_path = Self::settings_path()
            .ok_or_else(|| "Failed to determine settings directory".to_string())?;

        if let Some(parent) = settings_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create settings directory: {error}"))?;
        }

        let json = serde_json::to_string_pretty(self)
            .map_err(|error| format!("Failed to serialize settings: {error}"))?;

        fs::write(&settings_path, json)
            .map_err(|error| format!("Failed to write settings file: {error}"))?;

        Ok(())
    }

    pub fn settings_path() -> Option<PathBuf> {
        dirs::config_dir().map(|config_dir| config_dir.join("hearsay-demo").join("settings.json"))
    }

    pub fn add_recent_project(&mut self, path: String) {
        let canonical_path = std::path::Path::new(&path)
            .canonicalize()
            .ok()
            .and_then(|path_buf| path_buf.to_str().map(|path_str| path_str.to_string()))
            .unwrap_or(path);

        self.recent_projects
            .retain(|project| project != &canonical_path);
        self.recent_projects.insert(0, canonical_path);
        const MAX_RECENT_PROJECTS: usize = 10;
        if self.recent_projects.len() > MAX_RECENT_PROJECTS {
            self.recent_projects.truncate(MAX_RECENT_PROJECTS);
        }
    }

    pub fn clear_recent_projects(&mut self) {
        self.recent_projects.clear();
        if let Some(startup_path) = &self.default_project_path {
            self.recent_projects.push(startup_path.clone());
        }
    }

    pub fn get_recent_project_name(path: &str) -> String {
        if let Ok(json) = fs::read_to_string(path)
            && let Ok(save_file) = serde_json::from_str::<serde_json::Value>(&json)
            && let Some(name) = save_file
                .get("project_name")
                .and_then(|name_value| name_value.as_str())
        {
            return name.to_string();
        }
        PathBuf::from(path)
            .file_stem()
            .and_then(|file_stem| file_stem.to_str())
            .unwrap_or("Unknown Project")
            .to_string()
    }
}

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(UserSettings::load());
    }
}
