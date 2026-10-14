use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::instance::Instance;
#[cfg(target_os = "linux")]
use crate::paths;

fn launcher() -> Result<PathBuf> {
    if let Some(image) = std::env::var_os("APPIMAGE").filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(image));
    }
    Ok(std::env::current_exe()?)
}

#[cfg(target_os = "linux")]
const FALLBACK_ICON: &[u8] = include_bytes!("../icons/128x128.png");

#[cfg(target_os = "linux")]
fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\\\\\"),
            '"' | '`' | '$' => {
                out.push('\\');
                out.push(c);
            }
            '%' => out.push_str("%%"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(target_os = "linux")]
fn entry_value(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

#[cfg(target_os = "linux")]
fn file_name(id: &str) -> String {
    format!("piuma-{id}.desktop")
}

#[cfg(target_os = "linux")]
fn targets() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = dirs::data_dir().map(|d| d.join("applications")).into_iter().collect();
    out.extend(dirs::desktop_dir().filter(|d| d.is_dir()));
    out
}

#[cfg(target_os = "linux")]
pub async fn create(client: &reqwest::Client, inst: &Instance) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let exe = launcher()?;
    let icons = paths::root().join("icons");
    std::fs::create_dir_all(&icons)?;
    let icon_path = icons.join(format!("{}.png", inst.id));
    let png = match &inst.icon_url {
        Some(url) => crate::thumb::get(client, url, 128).await.ok().and_then(|data| to_png(&data)),
        None => None,
    };
    std::fs::write(&icon_path, png.as_deref().unwrap_or(FALLBACK_ICON))?;

    let body = desktop_entry(&exe, inst, &icon_path);
    let mut written = 0;
    for dir in targets() {
        if std::fs::create_dir_all(&dir).is_err() {
            continue;
        }
        let file = dir.join(file_name(&inst.id));
        if std::fs::write(&file, &body).is_ok() {
            let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755));
            let _ = std::process::Command::new("gio")
                .args(["set", &file.to_string_lossy(), "metadata::trusted", "true"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            written += 1;
        }
    }
    if written == 0 {
        return Err(Error::msg("Couldn't create the shortcut"));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn desktop_entry(exe: &std::path::Path, inst: &Instance, icon: &std::path::Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName={}\nComment=Minecraft {} · PiumaLauncher\nExec={} --instance {}\nIcon={}\nTerminal=false\nCategories=Game;\nStartupWMClass=piuma-launcher\n",
        entry_value(&inst.name),
        entry_value(&inst.mc_version),
        quote(&exe.to_string_lossy()),
        inst.id,
        icon.to_string_lossy(),
    )
}

#[cfg(target_os = "linux")]
fn to_png(data_url: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    let raw = base64::engine::general_purpose::STANDARD.decode(data_url.split_once(",")?.1).ok()?;
    if raw.starts_with(b"\x89PNG") {
        return Some(raw);
    }
    let img = image::load_from_memory(&raw).ok()?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

#[cfg(target_os = "linux")]
pub fn forget(id: &str) {
    for dir in targets() {
        let _ = std::fs::remove_file(dir.join(file_name(id)));
    }
    let _ = std::fs::remove_file(paths::root().join("icons").join(format!("{id}.png")));
}

#[cfg(windows)]
pub async fn create(_client: &reqwest::Client, inst: &Instance) -> Result<()> {
    let exe = launcher()?;
    let desktop = dirs::desktop_dir().ok_or_else(|| Error::msg("Desktop folder not found"))?;
    let safe: String = inst
        .name
        .chars()
        .map(|c| if c.is_alphanumeric() || " -_.()".contains(c) { c } else { '_' })
        .collect();
    let safe = safe.trim().trim_end_matches('.');
    let link = desktop.join(format!("{} (Piuma).lnk", if safe.is_empty() { inst.id.as_str() } else { safe }));
    let ps = |s: &str| s.replace('\'', "''");
    let script = format!(
        "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}');$s.TargetPath='{}';$s.Arguments='--instance {}';$s.WorkingDirectory='{}';$s.IconLocation='{},0';$s.Description='Minecraft {}';$s.Save()",
        ps(&link.to_string_lossy()),
        ps(&exe.to_string_lossy()),
        inst.id,
        ps(&exe.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()),
        ps(&exe.to_string_lossy()),
        ps(&inst.mc_version),
    );
    let mut cmd = tokio::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    cmd.creation_flags(0x0800_0000);
    let ok = cmd.status().await.map(|s| s.success()).unwrap_or(false);
    if !ok || !link.is_file() {
        return Err(Error::msg("Couldn't create the shortcut"));
    }
    Ok(())
}

#[cfg(windows)]
pub fn forget(_id: &str) {}

#[cfg(not(any(target_os = "linux", windows)))]
pub async fn create(_client: &reqwest::Client, _inst: &Instance) -> Result<()> {
    Err(Error::msg("Shortcuts aren't supported on this system"))
}

#[cfg(not(any(target_os = "linux", windows)))]
pub fn forget(_id: &str) {}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn exec_paths_are_quoted() {
        assert_eq!(quote("/home/a b/Piuma $x.AppImage"), "\"/home/a b/Piuma \\$x.AppImage\"");
        assert_eq!(quote("/a\\b%c"), "\"/a\\\\\\\\b%%c\"");
        assert_eq!(entry_value("Bad\nName"), "BadName");
    }

    #[test]
    #[ignore]
    fn entry_passes_desktop_file_validate() {
        let inst: Instance = serde_json::from_value(serde_json::json!({ "id": "abc-123", "name": "Cobblemon \"Deluxe\" 100%", "mcVersion": "1.21.1" })).unwrap();
        let file = std::env::temp_dir().join("piuma-abc-123.desktop");
        std::fs::write(&file, desktop_entry(std::path::Path::new("/opt/My Apps/Piuma $1.AppImage"), &inst, std::path::Path::new("/tmp/i.png"))).unwrap();
        let out = std::process::Command::new("desktop-file-validate").arg(&file).output().unwrap();
        eprintln!("{}", std::fs::read_to_string(&file).unwrap());
        assert!(out.status.success() && out.stdout.is_empty(), "{}", String::from_utf8_lossy(&out.stdout));
    }
}
