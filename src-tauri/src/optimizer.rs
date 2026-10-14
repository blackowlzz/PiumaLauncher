use serde::Serialize;

use crate::flags::{FlagProfile, Gc, Tuning};
use crate::instance::Instance;
use crate::paths;
use crate::settings::Settings;

#[derive(Debug, Clone, Copy)]
pub struct Hardware {
    pub total_mb: u64,
    pub available_mb: u64,
    pub cores: usize,
}

pub fn hardware() -> Hardware {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    Hardware {
        total_mb: sys.total_memory() / 1_048_576,
        available_mb: sys.available_memory() / 1_048_576,
        cores: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
    }
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Weak,
    Modest,
    Capable,
    Strong,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub tier: Tier,
    pub memory_max_mb: u32,
    pub memory_min_mb: u32,
    pub flag_profile: FlagProfile,
    pub tuning: Tuning,
    pub reasons: Vec<&'static str>,
}

fn tier_of(hw: &Hardware) -> Tier {
    if hw.total_mb <= 4096 || hw.cores <= 2 {
        Tier::Weak
    } else if hw.total_mb <= 8192 || hw.cores < 4 {
        Tier::Modest
    } else if hw.total_mb >= 16384 && hw.cores >= 6 {
        Tier::Strong
    } else {
        Tier::Capable
    }
}

fn round_down(mb: u64, step: u64) -> u64 {
    mb / step * step
}

pub fn recommend(hw: &Hardware, mod_count: u32) -> Recommendation {
    let tier = tier_of(hw);
    let mut reasons = Vec::new();

    let mut max = match tier {
        Tier::Weak => (hw.total_mb * 45 / 100).min(2048),
        Tier::Modest => 3072,
        Tier::Capable => {
            if hw.total_mb >= 12288 {
                6144
            } else {
                4096
            }
        }
        Tier::Strong => {
            if hw.total_mb >= 24576 {
                8192
            } else {
                6144
            }
        }
    };

    if mod_count >= 200 {
        max += 2048;
        reasons.push("A large mod list needs extra memory.");
    } else if mod_count >= 80 {
        max += 1024;
        reasons.push("Many mods are installed, so some extra memory was added.");
    }

    let cap = (hw.total_mb * 65 / 100).min((hw.available_mb * 85 / 100).max(1024));
    if max > cap {
        max = cap;
        reasons.push("Memory was limited to what your computer can spare right now.");
    }
    let memory_max_mb = round_down(max, 256).clamp(1024, 65_536) as u32;

    let memory_min_mb = match tier {
        Tier::Weak => round_down(memory_max_mb as u64 / 4, 128).max(256) as u32,
        Tier::Modest | Tier::Capable | Tier::Strong => round_down((memory_max_mb as u64 / 2).min(2048), 128) as u32,
    };

    let flag_profile = match tier {
        Tier::Weak => FlagProfile::Low,
        Tier::Modest | Tier::Capable => FlagProfile::Balanced,
        Tier::Strong => FlagProfile::Performance,
    };

    let gc_threads = match tier {
        Tier::Weak => u32::from(hw.cores >= 2),
        Tier::Modest => 0,
        Tier::Capable | Tier::Strong => (hw.cores / 2).clamp(2, 8) as u32,
    };

    let tuning = Tuning {
        gc: Gc::Auto,
        pause_ms: 0,
        gc_threads,
        pre_touch: false,
        disable_explicit_gc: true,
        string_dedup: matches!(tier, Tier::Weak) || (matches!(tier, Tier::Modest) && mod_count >= 80),
    };

    reasons.push(match tier {
        Tier::Weak => "Low memory or few cores: a small heap and the lightest Java profile keep the game responsive.",
        Tier::Modest => "A modest computer: balanced memory and a gentle garbage collector.",
        Tier::Capable => "A solid computer: generous memory with the balanced profile.",
        Tier::Strong => "A powerful computer: a large heap and a low-pause collector for the smoothest play.",
    });

    Recommendation { tier, memory_max_mb, memory_min_mb, flag_profile, tuning, reasons }
}

pub fn count_mods(instance_id: &str) -> u32 {
    std::fs::read_dir(paths::game_dir(instance_id).join("mods"))
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|x| x == "jar"))
                .count() as u32
        })
        .unwrap_or(0)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Effective {
    pub memory_max_mb: u32,
    pub memory_min_mb: u32,
    pub flag_profile: FlagProfile,
    #[serde(skip)]
    pub tuning: Tuning,
}

pub fn effective(inst: &Instance, settings: &Settings) -> Effective {
    let (base_max, base_min, base_profile) = if inst.custom_performance {
        (inst.memory_max_mb, inst.memory_min_mb, inst.flag_profile)
    } else {
        (settings.default_memory_max_mb, settings.default_memory_min_mb, settings.default_flag_profile)
    };
    if !settings.auto_optimize {
        return Effective {
            memory_max_mb: base_max,
            memory_min_mb: base_min.min(base_max),
            flag_profile: base_profile,
            tuning: settings.tuning.clone(),
        };
    }
    let hw = hardware();
    let rec = recommend(&hw, count_mods(&inst.id));
    let ceiling = (hw.total_mb * 75 / 100).max(1024) as u32;
    let memory_max_mb = rec.memory_max_mb.max(base_max.min(ceiling));
    let memory_min_mb = rec.memory_min_mb.min(memory_max_mb);
    let flag_profile = if base_profile == FlagProfile::None { FlagProfile::None } else { rec.flag_profile };
    let mut tuning = rec.tuning;
    tuning.gc = settings.tuning.gc;
    tuning.pause_ms = settings.tuning.pause_ms;
    Effective { memory_max_mb, memory_min_mb, flag_profile, tuning }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hw(total_mb: u64, cores: usize) -> Hardware {
        Hardware { total_mb, available_mb: total_mb * 7 / 10, cores }
    }

    #[test]
    fn old_hardware_gets_the_lightest_setup() {
        let r = recommend(&hw(4096, 2), 0);
        assert_eq!(r.tier, Tier::Weak);
        assert_eq!(r.flag_profile, FlagProfile::Low);
        assert!(r.memory_max_mb <= 2048);
        assert!(r.memory_min_mb < r.memory_max_mb);
    }

    #[test]
    fn strong_hardware_gets_low_pause_collector() {
        let r = recommend(&hw(32768, 12), 0);
        assert_eq!(r.tier, Tier::Strong);
        assert_eq!(r.flag_profile, FlagProfile::Performance);
        assert_eq!(r.memory_max_mb, 8192);
    }

    #[test]
    fn memory_never_exceeds_what_is_free() {
        let h = Hardware { total_mb: 16384, available_mb: 3000, cores: 8 };
        let r = recommend(&h, 0);
        assert!(r.memory_max_mb as u64 <= 3000);
        assert!(r.memory_min_mb <= r.memory_max_mb);
    }

    #[test]
    fn many_mods_add_memory() {
        let plain = recommend(&hw(16384, 8), 0).memory_max_mb;
        let heavy = recommend(&hw(16384, 8), 150).memory_max_mb;
        assert!(heavy > plain);
    }
}
