use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::instance::Instance;
use crate::{diag, paths, settings};

pub const CATEGORIES: [&str; 4] = ["controls", "audio", "interface", "video"];

const CONTROLS: &[&str] = &[
    "mouseSensitivity",
    "invertYMouse",
    "autoJump",
    "toggleCrouch",
    "toggleSprint",
    "rawMouseInput",
    "mouseWheelSensitivity",
    "discrete_mouse_scroll",
    "touchscreen",
];

const AUDIO: &[&str] = &["soundDevice", "showSubtitles", "directionalAudio"];

const INTERFACE: &[&str] = &[
    "lang",
    "guiScale",
    "mainHand",
    "attackIndicator",
    "chatScale",
    "chatOpacity",
    "chatLineSpacing",
    "chatHeightFocused",
    "chatHeightUnfocused",
    "chatWidth",
    "chatDelay",
    "chatColors",
    "chatLinks",
    "chatLinksPrompt",
    "chatVisibility",
    "textBackgroundOpacity",
    "backgroundForChatOnly",
    "forceUnicodeFont",
    "japaneseGlyphVariants",
    "hideMatchedNames",
    "narrator",
    "darkMojangStudiosBackground",
    "hideLightningFlashes",
    "damageTiltStrength",
    "screenEffectScale",
    "fovEffectScale",
    "glintSpeed",
    "glintStrength",
    "highContrast",
    "notificationDisplayTime",
    "showAutosaveIndicator",
    "advancedItemTooltips",
    "heldItemTooltips",
    "hideServerAddress",
    "reducedDebugInfo",
    "tutorialStep",
    "skipMultiplayerWarning",
    "joinedFirstServer",
    "onboardAccessibility",
];

const VIDEO: &[&str] = &[
    "renderDistance",
    "simulationDistance",
    "fov",
    "gamma",
    "maxFps",
    "graphicsMode",
    "fancyGraphics",
    "renderClouds",
    "ao",
    "biomeBlendRadius",
    "enableVsync",
    "entityShadows",
    "entityDistanceScaling",
    "particles",
    "mipmapLevels",
    "bobView",
    "prioritizeChunkUpdates",
    "menuBackgroundBlurriness",
];

fn category(key: &str) -> Option<&'static str> {
    if key.starts_with("key_") || CONTROLS.contains(&key) {
        Some("controls")
    } else if key.starts_with("soundCategory_") || AUDIO.contains(&key) {
        Some("audio")
    } else if key.starts_with("modelPart_") || INTERFACE.contains(&key) {
        Some("interface")
    } else if VIDEO.contains(&key) {
        Some("video")
    } else {
        None
    }
}

fn synced(key: &str, enabled: &[String]) -> bool {
    category(key).is_some_and(|c| enabled.iter().any(|e| e == c))
}

fn modern(inst: &Instance) -> bool {
    let mut parts = inst.mc_version.split(['.', '-', ' ']);
    match (parts.next().and_then(|p| p.parse::<u32>().ok()), parts.next().and_then(|p| p.parse::<u32>().ok())) {
        (Some(1), Some(minor)) => minor >= 13,
        (Some(major), _) => major > 1,
        _ => !inst.mc_version.contains("1."),
    }
}

fn shared_file(inst: &Instance) -> PathBuf {
    paths::root().join("synced").join(if modern(inst) { "options.txt" } else { "options-legacy.txt" })
}

fn instance_file(inst: &Instance) -> PathBuf {
    paths::game_dir(&inst.id).join("options.txt")
}

fn read(path: &Path) -> Vec<(String, String)> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn write(path: &Path, lines: &[(String, String)]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let body: String = lines.iter().map(|(k, v)| format!("{k}:{v}\n")).collect();
    let tmp = path.with_extension("txt.piuma");
    std::fs::write(&tmp, body)?;
    crate::fsx::retry(|| std::fs::rename(&tmp, path))
}

fn merge(base: &[(String, String)], incoming: &[(String, String)], keep: impl Fn(&str) -> bool) -> Option<Vec<(String, String)>> {
    let wanted: HashMap<&str, &str> = incoming.iter().filter(|(k, _)| keep(k)).map(|(k, v)| (k.as_str(), v.as_str())).collect();
    if wanted.is_empty() {
        return None;
    }
    let mut changed = false;
    let mut out: Vec<(String, String)> = base
        .iter()
        .map(|(k, v)| match wanted.get(k.as_str()) {
            Some(new) if *new != v => {
                changed = true;
                (k.clone(), new.to_string())
            }
            _ => (k.clone(), v.clone()),
        })
        .collect();
    for (k, v) in incoming.iter().filter(|(k, _)| keep(k)) {
        if !out.iter().any(|(o, _)| o == k) {
            changed = true;
            out.push((k.clone(), v.clone()));
        }
    }
    changed.then_some(out)
}

pub fn apply(inst: &Instance) {
    let prefs = settings::load();
    if !prefs.sync_options {
        return;
    }
    let shared = read(&shared_file(inst));
    let target = instance_file(inst);
    let current = read(&target);
    if let Some(next) = merge(&current, &shared, |k| synced(k, &prefs.sync_categories)) {
        match write(&target, &next) {
            Ok(()) => diag::log(format!("{}: applied synced options", inst.id)),
            Err(e) => diag::log(format!("{}: couldn't apply synced options: {e}", inst.id)),
        }
    }
}

pub fn capture(inst: &Instance) {
    let prefs = settings::load();
    if !prefs.sync_options {
        return;
    }
    let source = read(&instance_file(inst));
    let file = shared_file(inst);
    let shared = read(&file);
    if let Some(next) = merge(&shared, &source, |k| category(k).is_some()) {
        if let Err(e) = write(&file, &next) {
            diag::log(format!("{}: couldn't save synced options: {e}", inst.id));
        }
    }
}

pub fn seed(inst: &Instance) {
    if !shared_file(inst).is_file() {
        capture(inst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(s: &str) -> Vec<(String, String)> {
        s.lines().filter_map(|l| l.split_once(':')).map(|(k, v)| (k.into(), v.into())).collect()
    }

    #[test]
    fn categories_cover_the_usual_keys() {
        assert_eq!(category("key_key.jump"), Some("controls"));
        assert_eq!(category("mouseSensitivity"), Some("controls"));
        assert_eq!(category("soundCategory_music"), Some("audio"));
        assert_eq!(category("lang"), Some("interface"));
        assert_eq!(category("modelPart_cape"), Some("interface"));
        assert_eq!(category("renderDistance"), Some("video"));
        assert_eq!(category("resourcePacks"), None);
        assert_eq!(category("version"), None);
    }

    #[test]
    fn merge_only_touches_enabled_keys() {
        let instance = pairs("version:3955\nkey_key.jump:key.keyboard.space\nrenderDistance:8\nresourcePacks:[\"vanilla\",\"file/x.zip\"]\n");
        let shared = pairs("key_key.jump:key.keyboard.j\nrenderDistance:16\nsoundCategory_music:0.0\nresourcePacks:[]\n");
        let enabled = vec!["controls".to_string(), "audio".to_string()];
        let out = merge(&instance, &shared, |k| synced(k, &enabled)).unwrap();
        let get = |k: &str| out.iter().find(|(o, _)| o == k).map(|(_, v)| v.as_str());
        assert_eq!(get("key_key.jump"), Some("key.keyboard.j"));
        assert_eq!(get("renderDistance"), Some("8"));
        assert_eq!(get("soundCategory_music"), Some("0.0"));
        assert_eq!(get("resourcePacks"), Some("[\"vanilla\",\"file/x.zip\"]"));
        assert_eq!(out[0], ("version".to_string(), "3955".to_string()));
        assert!(merge(&out, &shared, |k| synced(k, &enabled)).is_none());
    }

    #[test]
    fn legacy_and_modern_versions_are_kept_apart() {
        let mut inst = crate::instance::Instance {
            id: "x".into(),
            name: "x".into(),
            mc_version: "1.8.9".into(),
            loader: crate::loaders::Loader::Vanilla,
            loader_version: None,
            launch_version: None,
            custom_performance: false,
            memory_max_mb: 2048,
            memory_min_mb: 512,
            flag_profile: Default::default(),
            java_path: None,
            extra_jvm_args: String::new(),
            icon_url: None,
            banner_url: None,
        };
        assert!(!modern(&inst));
        inst.mc_version = "1.12.2".into();
        assert!(!modern(&inst));
        for v in ["1.13", "1.21.11", "26.3", "26.4-snapshot-4", "24w14a"] {
            inst.mc_version = v.into();
            assert!(modern(&inst), "{v}");
        }
    }
}
