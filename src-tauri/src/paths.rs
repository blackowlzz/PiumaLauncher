
use std::path::PathBuf;

pub fn root() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("PiumaLauncher")
}

pub fn versions() -> PathBuf {
    root().join("versions")
}

pub fn libraries() -> PathBuf {
    root().join("libraries")
}

pub fn assets() -> PathBuf {
    root().join("assets")
}

pub fn natives(instance_id: &str) -> PathBuf {
    root().join("natives").join(instance_id)
}

pub fn runtimes() -> PathBuf {
    root().join("runtimes")
}

pub fn instances() -> PathBuf {
    root().join("instances")
}

pub fn instance_dir(id: &str) -> PathBuf {
    instances().join(id)
}

pub fn game_dir(id: &str) -> PathBuf {
    instance_dir(id).join("game")
}

pub fn backups() -> PathBuf {
    root().join("backups")
}

pub fn logs() -> PathBuf {
    root().join("logs")
}

pub fn shim_dir() -> PathBuf {
    root().join("shim")
}

pub fn accounts_file() -> PathBuf {
    root().join("accounts.json")
}

pub fn microsoft_file() -> PathBuf {
    root().join("microsoft.json")
}

pub fn manifest_cache() -> PathBuf {
    root().join("version_manifest.json")
}
