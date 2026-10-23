use std::path::Path;

use serde::Serialize;

const NVIDIA: &str = "0x10de";

#[derive(Debug, Clone, PartialEq)]
struct Card {
    driver: String,
    vendor: String,
    boot: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dedicated {
    pub name: String,
    proprietary: bool,
}

fn cards_in(root: &Path) -> Vec<Card> {
    let Ok(entries) = std::fs::read_dir(root) else { return vec![] };
    entries
        .flatten()
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.starts_with("card") && !n.contains('-')))
        .filter_map(|e| {
            let dev = e.path().join("device");
            let driver = std::fs::read_link(dev.join("driver")).ok()?.file_name()?.to_string_lossy().into_owned();
            let vendor = std::fs::read_to_string(dev.join("vendor")).map(|v| v.trim().to_ascii_lowercase()).unwrap_or_default();
            let boot = std::fs::read_to_string(dev.join("boot_vga")).is_ok_and(|s| s.trim() == "1");
            Some(Card { driver, vendor, boot })
        })
        .collect()
}

fn primary_is_nvidia_in(cards: &[Card]) -> bool {
    match cards.iter().find(|c| c.boot) {
        Some(c) => c.driver == "nvidia",
        None => !cards.is_empty() && cards.iter().all(|c| c.driver == "nvidia"),
    }
}

fn dedicated_in(cards: &[Card]) -> Option<Dedicated> {
    let primary = cards.iter().find(|c| c.boot)?;
    if primary.vendor == NVIDIA || !matches!(primary.driver.as_str(), "i915" | "xe" | "amdgpu" | "radeon") {
        return None;
    }
    let card = cards.iter().find(|c| !c.boot && c.vendor == NVIDIA && matches!(c.driver.as_str(), "nvidia" | "nouveau"))?;
    Some(Dedicated { name: "NVIDIA".into(), proprietary: card.driver == "nvidia" })
}

pub fn primary_is_nvidia() -> bool {
    cfg!(target_os = "linux") && primary_is_nvidia_in(&cards_in(Path::new("/sys/class/drm")))
}

pub fn dedicated() -> Option<Dedicated> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    dedicated_in(&cards_in(Path::new("/sys/class/drm")))
}

#[cfg(test)]
pub(crate) fn test_card(proprietary: bool) -> Dedicated {
    Dedicated { name: "NVIDIA".into(), proprietary }
}

pub fn offload_env(card: &Dedicated) -> Vec<(&'static str, &'static str)> {
    if card.proprietary {
        vec![("__NV_PRIME_RENDER_OFFLOAD", "1"), ("__GLX_VENDOR_LIBRARY_NAME", "nvidia"), ("__VK_LAYER_NV_optimus", "NVIDIA_only")]
    } else {
        vec![("DRI_PRIME", "1")]
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn fake(name: &str, cards: &[(&str, &str, &str, Option<&str>)]) -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!("piuma-gpu-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let drm = base.join("class/drm");
        std::fs::create_dir_all(&drm).unwrap();
        for (i, (card, driver, vendor, boot)) in cards.iter().enumerate() {
            let dev = base.join(format!("devices/{i}"));
            let drv = base.join(format!("drivers/{driver}"));
            std::fs::create_dir_all(&dev).unwrap();
            std::fs::create_dir_all(&drv).unwrap();
            std::os::unix::fs::symlink(&drv, dev.join("driver")).unwrap();
            std::fs::write(dev.join("vendor"), format!("{vendor}\n")).unwrap();
            if let Some(b) = boot {
                std::fs::write(dev.join("boot_vga"), format!("{b}\n")).unwrap();
            }
            let node = base.join(format!("nodes/{card}"));
            std::fs::create_dir_all(&node).unwrap();
            std::os::unix::fs::symlink(&dev, node.join("device")).unwrap();
            std::os::unix::fs::symlink(&node, drm.join(card)).unwrap();
            let conn = base.join(format!("nodes/{card}-eDP-1"));
            std::fs::create_dir_all(&conn).unwrap();
            std::os::unix::fs::symlink(&conn, drm.join(format!("{card}-eDP-1"))).unwrap();
        }
        drm
    }

    type Layout<'a> = Vec<(&'a str, &'a str, &'a str, Option<&'a str>)>;

    const INTEL: &str = "0x8086";
    const AMD: &str = "0x1002";

    #[test]
    fn finds_the_dedicated_card_only_on_hybrid_laptops() {
        let cases: Vec<(&str, Layout, Option<bool>, bool)> = vec![
            ("intel-nvidia", vec![("card0", "nvidia", NVIDIA, Some("0")), ("card1", "i915", INTEL, Some("1"))], Some(true), false),
            ("xe-nvidia", vec![("card0", "xe", INTEL, Some("1")), ("card1", "nvidia", NVIDIA, Some("0"))], Some(true), false),
            ("amd-nvidia", vec![("card0", "amdgpu", AMD, Some("1")), ("card1", "nvidia", NVIDIA, Some("0"))], Some(true), false),
            ("intel-nouveau", vec![("card0", "i915", INTEL, Some("1")), ("card1", "nouveau", NVIDIA, Some("0"))], Some(false), false),
            ("mux-dgpu", vec![("card0", "i915", INTEL, Some("0")), ("card1", "nvidia", NVIDIA, Some("1"))], None, true),
            ("desktop-nvidia", vec![("card1", "nvidia", NVIDIA, Some("1"))], None, true),
            ("desktop-amd-with-igpu", vec![("card0", "amdgpu", AMD, Some("1")), ("card1", "amdgpu", AMD, Some("0"))], None, false),
            ("desktop-amd-with-intel", vec![("card0", "amdgpu", AMD, Some("1")), ("card1", "i915", INTEL, Some("0"))], None, false),
            ("intel-only", vec![("card1", "i915", INTEL, Some("1"))], None, false),
            ("no-boot-vga", vec![("card0", "i915", INTEL, None), ("card1", "nvidia", NVIDIA, None)], None, false),
            ("nvidia-no-boot-vga", vec![("card0", "nvidia", NVIDIA, None)], None, true),
        ];
        for (name, layout, want, nvidia_first) in cases {
            let drm = fake(name, &layout);
            let cards = cards_in(&drm);
            assert_eq!(dedicated_in(&cards).map(|d| d.proprietary), want, "{name}");
            assert_eq!(primary_is_nvidia_in(&cards), nvidia_first, "{name}");
            let _ = std::fs::remove_dir_all(drm.parent().unwrap().parent().unwrap());
        }
        assert!(cards_in(Path::new("/definitely/missing")).is_empty());
    }

    #[test]
    fn offload_variables_match_the_driver() {
        let keys = |p: bool| offload_env(&Dedicated { name: "NVIDIA".into(), proprietary: p }).into_iter().map(|(k, _)| k).collect::<Vec<_>>();
        assert_eq!(keys(true), ["__NV_PRIME_RENDER_OFFLOAD", "__GLX_VENDOR_LIBRARY_NAME", "__VK_LAYER_NV_optimus"]);
        assert_eq!(keys(false), ["DRI_PRIME"]);
    }
}
