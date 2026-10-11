
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

fn g1_aikar() -> Vec<String> {
    [
        "-XX:+UseG1GC",
        "-XX:+ParallelRefProcEnabled",
        "-XX:MaxGCPauseMillis=200",
        "-XX:+UnlockExperimentalVMOptions",
        "-XX:+DisableExplicitGC",
        "-XX:G1NewSizePercent=30",
        "-XX:G1MaxNewSizePercent=40",
        "-XX:G1HeapRegionSize=8M",
        "-XX:G1ReservePercent=20",
        "-XX:G1HeapWastePercent=5",
        "-XX:G1MixedGCCountTarget=4",
        "-XX:InitiatingHeapOccupancyPercent=15",
        "-XX:G1MixedGCLiveThresholdPercent=90",
        "-XX:G1RSetUpdatingPauseTimePercent=5",
        "-XX:SurvivorRatio=32",
        "-XX:MaxTenuringThreshold=1",
        "-XX:-UsePerfData",
    ]
    .map(String::from)
    .to_vec()
}

pub fn jvm_flags(profile: FlagProfile, java_major: u32) -> Vec<String> {
    match profile {
        FlagProfile::None => vec![],
        FlagProfile::Balanced => g1_aikar(),
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
        FlagProfile::Performance => {
            if java_major < 17 {
                return g1_aikar();
            }
            let mut f = vec!["-XX:+UseZGC".to_string()];
            if (21..=22).contains(&java_major) {
                f.push("-XX:+ZGenerational".into());
            }
            f.push("-XX:+DisableExplicitGC".into());
            f.push("-XX:-UsePerfData".into());
            f
        }
    }
}
