use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::flags::FlagProfile;
use crate::loaders::Loader;
use crate::paths;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    #[serde(default)]
    pub loader: Loader,
    #[serde(default)]
    pub loader_version: Option<String>,
    #[serde(default)]
    pub launch_version: Option<String>,
    #[serde(default)]
    pub custom_performance: bool,
    #[serde(default = "default_max")]
    pub memory_max_mb: u32,
    #[serde(default = "default_min")]
    pub memory_min_mb: u32,
    #[serde(default)]
    pub flag_profile: FlagProfile,
    #[serde(default)]
    pub java_path: Option<String>,
    #[serde(default)]
    pub extra_jvm_args: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub banner_url: Option<String>,
    #[serde(default)]
    pub pack: Option<PackSource>,
    #[serde(default)]
    pub launch: LaunchOptions,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct LaunchOptions {
    pub wrapper: String,
    pub env: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fullscreen: bool,
    pub skip_defaults: bool,
}

impl LaunchOptions {
    pub fn merged(&self, defaults: &LaunchOptions) -> LaunchOptions {
        if self.skip_defaults {
            return self.clone();
        }
        let own_window = self.fullscreen || (self.width.is_some() && self.height.is_some());
        let window = if own_window { self } else { defaults };
        LaunchOptions {
            wrapper: if self.wrapper.trim().is_empty() { defaults.wrapper.clone() } else { self.wrapper.clone() },
            env: [defaults.env.as_str(), self.env.as_str()].iter().filter(|e| !e.trim().is_empty()).copied().collect::<Vec<_>>().join("\n"),
            width: window.width,
            height: window.height,
            fullscreen: window.fullscreen,
            skip_defaults: false,
        }
    }

    pub fn with_gpu(mut self, card: Option<&crate::gpu::Dedicated>) -> LaunchOptions {
        if let Some(card) = card {
            let vars: String = crate::gpu::offload_env(card).iter().map(|(k, v)| format!("{k}={v}\n")).collect();
            self.env = format!("{vars}{}", self.env);
        }
        self
    }

    pub fn wrapper_command(&self) -> Vec<String> {
        split_command(&self.wrapper)
    }

    pub fn env_vars(&self) -> Vec<(String, String)> {
        self.env
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .filter(|(k, _)| {
                let mut chars = k.chars();
                chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
            })
            .collect()
    }

    pub fn game_args(&self) -> Vec<String> {
        if self.fullscreen {
            return vec!["--fullscreen".into()];
        }
        let valid = |v: Option<u32>| v.filter(|n| (320..=16384).contains(n));
        match (valid(self.width), valid(self.height)) {
            (Some(w), Some(h)) => vec!["--width".into(), w.to_string(), "--height".into(), h.to_string()],
            _ => vec![],
        }
    }
}

fn split_command(text: &str) -> Vec<String> {
    let mut out = vec![];
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    for c in text.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => current.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                started = true;
            }
            (None, c) if c.is_whitespace() => {
                if started || !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            (None, c) => current.push(c),
        }
    }
    if started || !current.is_empty() {
        out.push(current);
    }
    out
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PackSource {
    pub project_id: String,
    pub version_id: String,
    pub version_number: String,
}

fn default_max() -> u32 {
    2048
}
fn default_min() -> u32 {
    512
}

pub fn check_id(id: &str) -> Result<()> {
    let ok = !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok { Ok(()) } else { Err(Error::msg("invalid instance id")) }
}

fn slug(name: &str) -> String {
    let s: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "instance".into() } else { s.chars().take(32).collect() }
}

fn file(id: &str) -> std::path::PathBuf {
    paths::instance_dir(id).join("instance.json")
}

pub fn list() -> Vec<Instance> {
    let mut out: Vec<Instance> = std::fs::read_dir(paths::instances())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| std::fs::read_to_string(e.path().join("instance.json")).ok())
        .filter_map(|t| serde_json::from_str(&t).ok())
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

pub fn get(id: &str) -> Result<Instance> {
    check_id(id)?;
    let text = std::fs::read_to_string(file(id)).map_err(|_| Error::msg("Instance not found"))?;
    Ok(serde_json::from_str(&text)?)
}

pub fn save(inst: &Instance) -> Result<()> {
    check_id(&inst.id)?;
    if inst.name.trim().is_empty() {
        return Err(Error::msg("The name can't be empty"));
    }
    if inst.memory_min_mb == 0 || inst.memory_min_mb > inst.memory_max_mb {
        return Err(Error::msg("Invalid memory: the minimum can't exceed the maximum"));
    }
    std::fs::create_dir_all(paths::instance_dir(&inst.id))?;
    std::fs::write(file(&inst.id), serde_json::to_string_pretty(inst)?)?;
    Ok(())
}

impl Instance {
    pub fn launch_id(&self) -> &str {
        self.launch_version.as_deref().unwrap_or(&self.mc_version)
    }

    pub fn effective_launch(&self) -> LaunchOptions {
        let prefs = crate::settings::load();
        let gpu = if prefs.dedicated_gpu { crate::gpu::dedicated() } else { None };
        self.launch.merged(&prefs.default_launch).with_gpu(gpu.as_ref())
    }
}

pub fn create(
    name: &str,
    mc_version: &str,
    loader: Loader,
    loader_version: Option<String>,
    launch_version: Option<String>,
) -> Result<Instance> {
    let suffix = &uuid::Uuid::new_v4().simple().to_string()[..4];
    let defaults = crate::settings::load();
    let inst = Instance {
        id: format!("{}-{suffix}", slug(name)),
        name: name.trim().to_string(),
        mc_version: mc_version.to_string(),
        loader,
        loader_version,
        launch_version,
        custom_performance: false,
        memory_max_mb: defaults.default_memory_max_mb,
        memory_min_mb: defaults.default_memory_min_mb,
        flag_profile: defaults.default_flag_profile,
        java_path: None,
        extra_jvm_args: defaults.default_extra_jvm_args.clone(),
        icon_url: None,
        banner_url: None,
        pack: None,
        launch: LaunchOptions::default(),
    };
    save(&inst)?;
    std::fs::create_dir_all(paths::game_dir(&inst.id))?;
    Ok(inst)
}

fn pack_files_file(id: &str) -> std::path::PathBuf {
    paths::instance_dir(id).join("pack-files.json")
}

pub fn pack_files(id: &str) -> BTreeMap<String, String> {
    if check_id(id).is_err() {
        return BTreeMap::new();
    }
    std::fs::read_to_string(pack_files_file(id)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save_pack_files(id: &str, files: &BTreeMap<String, String>) -> Result<()> {
    check_id(id)?;
    std::fs::write(pack_files_file(id), serde_json::to_string_pretty(files)?)?;
    Ok(())
}

pub fn delete(id: &str) -> Result<()> {
    check_id(id)?;
    let dir = paths::instance_dir(id);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    let _ = std::fs::remove_dir_all(paths::natives(id));
    let _ = std::fs::remove_dir_all(paths::backups().join(id));
    Ok(())
}

#[cfg(test)]
pub(crate) fn tests_instance(id: &str) -> Instance {
    serde_json::from_value(serde_json::json!({ "id": id, "name": id, "mcVersion": "1.21.1" })).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_options_parse() {
        let o = LaunchOptions {
            wrapper: r#"prime-run  "my wrapper" 'a b' mangohud"#.into(),
            env: "MESA_GL_VERSION_OVERRIDE=4.6\n# note\n\nbad key=1\n1X=2\n__GL_THREADED_OPTIMIZATIONS = 1\nnoequals".into(),
            width: Some(1280),
            height: Some(720),
            fullscreen: false,
            skip_defaults: false,
        };
        assert_eq!(o.wrapper_command(), ["prime-run", "my wrapper", "a b", "mangohud"]);
        assert_eq!(
            o.env_vars(),
            [("MESA_GL_VERSION_OVERRIDE".to_string(), "4.6".to_string()), ("__GL_THREADED_OPTIMIZATIONS".to_string(), "1".to_string())]
        );
        assert_eq!(o.game_args(), ["--width", "1280", "--height", "720"]);
        assert_eq!(LaunchOptions { fullscreen: true, ..o.clone() }.game_args(), ["--fullscreen"]);
        assert!(LaunchOptions { width: Some(10), ..o.clone() }.game_args().is_empty());
        assert!(LaunchOptions::default().wrapper_command().is_empty());
        assert_eq!(split_command(r#"a "" b"#), ["a", "", "b"]);
    }

    #[test]
    fn defaults_fill_only_what_the_instance_leaves_empty() {
        let defaults = LaunchOptions {
            wrapper: "prime-run".into(),
            env: "A=1\nB=1".into(),
            width: Some(1600),
            height: Some(900),
            fullscreen: false,
            skip_defaults: false,
        };
        let empty = LaunchOptions::default().merged(&defaults);
        assert_eq!(empty.wrapper_command(), ["prime-run"]);
        assert_eq!(empty.game_args(), ["--width", "1600", "--height", "900"]);

        let own = LaunchOptions { wrapper: "gamemoderun".into(), env: "B=2".into(), fullscreen: true, ..Default::default() }.merged(&defaults);
        assert_eq!(own.wrapper_command(), ["gamemoderun"]);
        assert_eq!(own.game_args(), ["--fullscreen"]);
        let env: std::collections::HashMap<_, _> = own.env_vars().into_iter().collect();
        assert_eq!((env["A"].as_str(), env["B"].as_str()), ("1", "2"));
        assert_eq!(own.env_vars().last().unwrap(), &("B".to_string(), "2".to_string()));

        let half = LaunchOptions { width: Some(800), ..Default::default() }.merged(&defaults);
        assert_eq!(half.game_args(), ["--width", "1600", "--height", "900"]);

        let skipped = LaunchOptions { skip_defaults: true, ..Default::default() }.merged(&defaults);
        assert!(skipped.wrapper_command().is_empty() && skipped.env_vars().is_empty() && skipped.game_args().is_empty());

        assert_eq!(LaunchOptions::default().merged(&LaunchOptions::default()), LaunchOptions::default());
    }

    #[test]
    fn dedicated_gpu_variables_come_first_so_instances_can_override_them() {
        let own = LaunchOptions { env: "__NV_PRIME_RENDER_OFFLOAD=0".into(), ..Default::default() };
        let env = own.clone().with_gpu(Some(&crate::gpu::test_card(true))).env_vars();
        assert_eq!(env.first().unwrap(), &("__NV_PRIME_RENDER_OFFLOAD".to_string(), "1".to_string()));
        assert_eq!(env.last().unwrap(), &("__NV_PRIME_RENDER_OFFLOAD".to_string(), "0".to_string()));
        assert!(env.iter().any(|(k, v)| k == "__GLX_VENDOR_LIBRARY_NAME" && v == "nvidia"));
        assert_eq!(own.clone().with_gpu(None), own);
        let mesa = LaunchOptions::default().with_gpu(Some(&crate::gpu::test_card(false))).env_vars();
        assert_eq!(mesa, [("DRI_PRIME".to_string(), "1".to_string())]);
    }

    #[test]
    fn old_settings_still_load() {
        let s: crate::settings::Settings = serde_json::from_str(r#"{"theme":"dark"}"#).unwrap();
        assert_eq!(s.default_launch, LaunchOptions::default());
    }

    #[test]
    fn old_instances_still_load() {
        let old = r#"{"id":"x-1","name":"X","mcVersion":"1.21.1"}"#;
        let inst: Instance = serde_json::from_str(old).unwrap();
        assert_eq!(inst.launch, LaunchOptions::default());
        assert!(inst.pack.is_none());
    }
}
