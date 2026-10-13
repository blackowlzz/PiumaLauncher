
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::flags::{FlagProfile, Tuning};
use crate::paths;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub language: Option<String>,
    pub theme: String,
    pub ui_scale: String,
    pub minimize_on_launch: bool,
    pub default_memory_max_mb: u32,
    pub default_memory_min_mb: u32,
    pub default_flag_profile: FlagProfile,
    pub lookup_skins: bool,
    pub reduce_effects: bool,
    pub smooth_scroll: bool,
    pub check_updates: bool,
    pub discord_presence: bool,
    pub preferred_java: Option<String>,
    pub default_extra_jvm_args: String,
    pub tuning: Tuning,
    pub auto_optimize: bool,
    pub live_console: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            language: None,
            theme: "system".into(),
            ui_scale: "auto".into(),
            minimize_on_launch: false,
            default_memory_max_mb: 2048,
            default_memory_min_mb: 512,
            default_flag_profile: FlagProfile::default(),
            lookup_skins: true,
            reduce_effects: false,
            smooth_scroll: true,
            check_updates: true,
            discord_presence: true,
            preferred_java: None,
            default_extra_jvm_args: String::new(),
            tuning: Tuning::default(),
            auto_optimize: false,
            live_console: false,
        }
    }
}

fn file() -> std::path::PathBuf {
    paths::root().join("settings.json")
}

pub fn load() -> Settings {
    std::fs::read_to_string(file())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn valid_language(code: &str) -> bool {
    (2..=7).contains(&code.len()) && code.chars().all(|c| c.is_ascii_lowercase() || c == '-')
}

impl Settings {
    fn sanitized(mut self) -> Result<Self> {
        if let Some(l) = &self.language {
            if !valid_language(l) {
                return Err(Error::msg("invalid language code"));
            }
        }
        if !matches!(self.theme.as_str(), "system" | "light" | "dark") {
            self.theme = "system".into();
        }
        if !matches!(self.ui_scale.as_str(), "auto" | "100" | "125" | "150") {
            self.ui_scale = "auto".into();
        }
        if self.preferred_java.as_deref().is_some_and(|id| !crate::runtime::is_known_id(id)) {
            self.preferred_java = None;
        }
        self.tuning = self.tuning.sanitized();
        self.default_extra_jvm_args = self.default_extra_jvm_args.replace(['\n', '\r'], " ").trim().chars().take(1000).collect();
        self.default_memory_max_mb = self.default_memory_max_mb.clamp(512, 65_536);
        self.default_memory_min_mb = self.default_memory_min_mb.clamp(128, self.default_memory_max_mb);
        Ok(self)
    }
}

pub fn save(settings: Settings) -> Result<()> {
    let s = settings.sanitized()?;
    std::fs::create_dir_all(paths::root())?;
    std::fs::write(file(), serde_json::to_string_pretty(&s)?)?;
    Ok(())
}

pub fn set_language(code: &str) -> Result<()> {
    let mut s = load();
    s.language = Some(code.to_string());
    save(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_odd_language_codes() {
        assert!(set_language("../x").is_err());
        assert!(set_language("e").is_err());
        assert!(set_language("EN").is_err());
    }

    #[test]
    fn unknown_values_fall_back() {
        let s = Settings { theme: "neon".into(), ui_scale: "999".into(), default_memory_max_mb: 1, default_memory_min_mb: 99999, ..Settings::default() }
            .sanitized()
            .unwrap();
        assert_eq!((s.theme.as_str(), s.ui_scale.as_str()), ("system", "auto"));
        assert_eq!(s.default_memory_max_mb, 512);
        assert!(s.default_memory_min_mb <= s.default_memory_max_mb);
    }

    #[test]
    fn old_settings_files_still_load() {
        let s: Settings = serde_json::from_str(r#"{"language":"it"}"#).unwrap();
        assert_eq!(s.language.as_deref(), Some("it"));
        assert!(s.lookup_skins);
    }
}
