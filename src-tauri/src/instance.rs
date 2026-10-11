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
        memory_max_mb: defaults.default_memory_max_mb,
        memory_min_mb: defaults.default_memory_min_mb,
        flag_profile: defaults.default_flag_profile,
        java_path: None,
        extra_jvm_args: String::new(),
        icon_url: None,
        banner_url: None,
    };
    save(&inst)?;
    std::fs::create_dir_all(paths::game_dir(&inst.id))?;
    Ok(inst)
}

pub fn delete(id: &str) -> Result<()> {
    check_id(id)?;
    let dir = paths::instance_dir(id);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    let _ = std::fs::remove_dir_all(paths::natives(id));
    Ok(())
}
