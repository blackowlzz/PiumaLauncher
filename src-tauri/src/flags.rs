
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum FlagProfile {
    Low,
    #[default]
    Balanced,
    Performance,
    None,
}

fn g1_client() -> Vec<String> {
    [
        "-XX:+UseG1GC",
        "-XX:+ParallelRefProcEnabled",
        "-XX:MaxGCPauseMillis=50",
        "-XX:+UnlockExperimentalVMOptions",
        "-XX:+DisableExplicitGC",
        "-XX:G1NewSizePercent=23",
        "-XX:G1MaxNewSizePercent=40",
        "-XX:G1HeapRegionSize=8M",
        "-XX:G1ReservePercent=20",
        "-XX:G1HeapWastePercent=5",
        "-XX:G1MixedGCCountTarget=3",
        "-XX:InitiatingHeapOccupancyPercent=15",
        "-XX:G1MixedGCLiveThresholdPercent=90",
        "-XX:G1RSetUpdatingPauseTimePercent=0",
        "-XX:SurvivorRatio=32",
        "-XX:MaxTenuringThreshold=1",
        "-XX:G1SATBBufferEnqueueingThresholdPercent=30",
        "-XX:G1ConcMarkStepDurationMillis=5.0",
        "-XX:-UsePerfData",
    ]
    .map(String::from)
    .to_vec()
}

fn platform_flags(java_major: u32) -> Vec<String> {
    let mut f = Vec::new();
    if cfg!(target_os = "linux") {
        f.push("-XX:+UseTransparentHugePages".into());
    }
    if java_major >= 25 {
        f.push("-XX:+UseCompactObjectHeaders".into());
    }
    f
}

fn zgc_keeps_jit(java_major: u32, graal: bool) -> bool {
    java_major >= 17 && (!graal || java_major >= 25)
}

pub fn jvm_flags(profile: FlagProfile, java_major: u32, graal: bool) -> Vec<String> {
    let mut f = match profile {
        FlagProfile::None => return vec![],
        FlagProfile::Balanced => g1_client(),
        FlagProfile::Low => {
            let mut f: Vec<String> = [
                "-XX:+UseG1GC",
                "-XX:MaxGCPauseMillis=100",
                "-XX:+UseStringDeduplication",
                "-XX:+DisableExplicitGC",
                "-XX:-UsePerfData",
                "-XX:CICompilerCount=2",
                "-Xshare:auto",
            ]
            .map(String::from)
            .to_vec();
            if java_major >= 12 {
                f.push("-XX:G1PeriodicGCInterval=20000".into());
                f.push("-XX:MaxHeapFreeRatio=30".into());
                f.push("-XX:MinHeapFreeRatio=10".into());
            }
            f
        }
        FlagProfile::Performance if !zgc_keeps_jit(java_major, graal) => g1_client(),
        FlagProfile::Performance => {
            let mut f = vec!["-XX:+UseZGC".to_string()];
            if (21..=22).contains(&java_major) {
                f.push("-XX:+ZGenerational".into());
            }
            f.push("-XX:+DisableExplicitGC".into());
            f.push("-XX:-UsePerfData".into());
            f
        }
    };
    f.extend(platform_flags(java_major));
    f
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Gc {
    #[default]
    Auto,
    G1,
    Zgc,
    Parallel,
    Serial,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Tuning {
    pub gc: Gc,
    pub pause_ms: u32,
    pub gc_threads: u32,
    pub pre_touch: bool,
    pub disable_explicit_gc: bool,
    pub string_dedup: bool,
}

impl Tuning {
    pub fn sanitized(mut self) -> Self {
        if self.pause_ms != 0 {
            self.pause_ms = self.pause_ms.clamp(10, 2000);
        }
        self.gc_threads = self.gc_threads.min(64);
        self
    }
}

fn selects_gc(flag: &str) -> bool {
    (flag.starts_with("-XX:+Use") && flag.ends_with("GC")) || flag.contains("G1") || flag.contains("ZGenerational")
}

pub fn launch_flags(profile: FlagProfile, tuning: &Tuning, java_major: u32, graal: bool) -> Vec<String> {
    let mut flags = jvm_flags(profile, java_major, graal);
    let mut gc = if flags.iter().any(|f| f == "-XX:+UseZGC") {
        Gc::Zgc
    } else if flags.iter().any(|f| f == "-XX:+UseG1GC") {
        Gc::G1
    } else {
        Gc::Auto
    };

    if tuning.gc != Gc::Auto {
        gc = match tuning.gc {
            Gc::Zgc if java_major < 17 => Gc::G1,
            other => other,
        };
        flags.retain(|f| !selects_gc(f));
        match gc {
            Gc::G1 => flags.push("-XX:+UseG1GC".into()),
            Gc::Zgc => {
                flags.push("-XX:+UseZGC".into());
                if (21..=22).contains(&java_major) {
                    flags.push("-XX:+ZGenerational".into());
                }
            }
            Gc::Parallel => flags.push("-XX:+UseParallelGC".into()),
            Gc::Serial => flags.push("-XX:+UseSerialGC".into()),
            Gc::Auto => {}
        }
    }

    if tuning.pause_ms > 0 && matches!(gc, Gc::G1 | Gc::Parallel) {
        flags.retain(|f| !f.starts_with("-XX:MaxGCPauseMillis="));
        flags.push(format!("-XX:MaxGCPauseMillis={}", tuning.pause_ms));
    }
    if tuning.gc_threads > 0 && gc != Gc::Serial {
        flags.push(format!("-XX:ParallelGCThreads={}", tuning.gc_threads));
        if matches!(gc, Gc::G1 | Gc::Zgc) {
            flags.push(format!("-XX:ConcGCThreads={}", (tuning.gc_threads / 4).max(1)));
        }
    }
    if tuning.pre_touch {
        flags.push("-XX:+AlwaysPreTouch".into());
    }
    if tuning.disable_explicit_gc {
        flags.push("-XX:+DisableExplicitGC".into());
    }
    if tuning.string_dedup && gc == Gc::G1 {
        flags.push("-XX:+UseStringDeduplication".into());
    }

    let mut seen = std::collections::HashSet::new();
    flags.retain(|f| seen.insert(f.clone()));
    flags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_keeps_profile() {
        assert_eq!(launch_flags(FlagProfile::Balanced, &Tuning::default(), 21, false), jvm_flags(FlagProfile::Balanced, 21, false));
    }

    #[test]
    fn override_replaces_gc() {
        let t = Tuning { gc: Gc::Parallel, pause_ms: 50, ..Tuning::default() };
        let f = launch_flags(FlagProfile::Balanced, &t, 21, false);
        assert!(f.contains(&"-XX:+UseParallelGC".to_string()));
        assert!(!f.iter().any(|x| x.contains("G1")));
        assert_eq!(f.iter().filter(|x| x.starts_with("-XX:MaxGCPauseMillis=")).count(), 1);
    }

    #[test]
    fn zgc_falls_back_on_old_java() {
        let t = Tuning { gc: Gc::Zgc, ..Tuning::default() };
        assert!(launch_flags(FlagProfile::None, &t, 8, false).contains(&"-XX:+UseG1GC".to_string()));
    }

    #[test]
    fn performance_keeps_graal_jit() {
        let has = |f: &[String], x: &str| f.iter().any(|y| y == x);
        let graal21 = jvm_flags(FlagProfile::Performance, 21, true);
        assert!(has(&graal21, "-XX:+UseG1GC") && !has(&graal21, "-XX:+UseZGC"));
        let temurin21 = jvm_flags(FlagProfile::Performance, 21, false);
        assert!(has(&temurin21, "-XX:+UseZGC") && has(&temurin21, "-XX:+ZGenerational"));
        let graal25 = jvm_flags(FlagProfile::Performance, 25, true);
        assert!(has(&graal25, "-XX:+UseZGC") && has(&graal25, "-XX:+UseCompactObjectHeaders"));
        assert!(!jvm_flags(FlagProfile::Balanced, 21, true).iter().any(|x| x.contains("CompactObjectHeaders")));
    }

    #[test]
    fn none_adds_nothing() {
        assert!(jvm_flags(FlagProfile::None, 25, true).is_empty());
    }
}
