use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::accounts::{self, AccountKind};
use crate::{instance, optimizer, paths, settings};

const MAX_BYTES: u64 = 512 * 1024;
const REPORT_LINES: usize = 200;

static LOCK: Mutex<()> = Mutex::new(());

fn file() -> PathBuf {
    paths::logs().join("launcher.log")
}

fn old_file() -> PathBuf {
    paths::logs().join("launcher.old.log")
}

fn timestamp() -> String {
    format_time(SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64)
}

fn format_time(secs: i64) -> String {
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

pub fn redact(text: &str) -> String {
    match dirs::home_dir().map(|h| h.to_string_lossy().into_owned()).filter(|h| h.len() > 3) {
        Some(home) => text.replace(&home, "~"),
        None => text.to_string(),
    }
}

pub fn log(line: impl AsRef<str>) {
    let line = redact(line.as_ref());
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = file();
    let _ = std::fs::create_dir_all(paths::logs());
    let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) else { return };
    for l in line.lines() {
        let _ = writeln!(f, "{} {l}", timestamp());
    }
    let full = f.metadata().is_ok_and(|m| m.len() > MAX_BYTES);
    drop(f);
    if full {
        let _ = std::fs::rename(&path, old_file());
    }
}

fn os() -> String {
    let name = sysinfo::System::long_os_version().unwrap_or_else(|| std::env::consts::OS.to_string());
    format!("{name} ({})", sysinfo::System::cpu_arch())
}

pub fn started() {
    log(format!("PiumaLauncher {} started on {}", env!("CARGO_PKG_VERSION"), os()));
}

fn recent_lines() -> Vec<String> {
    let mut lines: Vec<String> = [old_file(), file()]
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .flat_map(|t| t.lines().map(String::from).collect::<Vec<_>>())
        .collect();
    let skip = lines.len().saturating_sub(REPORT_LINES);
    lines.drain(..skip);
    lines
}

fn runtimes() -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(paths::runtimes())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.'))
        .collect();
    out.sort();
    out
}

pub fn report(running: &[String]) -> String {
    let hw = optimizer::hardware();
    let prefs = settings::load();
    let account = match accounts::active().map(|a| a.kind) {
        Some(AccountKind::Microsoft) => "Microsoft",
        Some(AccountKind::Offline) => "offline",
        None => "none",
    };
    let mut out = vec![
        format!("PiumaLauncher {}", env!("CARGO_PKG_VERSION")),
        format!("OS: {}", os()),
        format!("RAM: {} MB ({} MB free) · CPU threads: {}", hw.total_mb, hw.available_mb, hw.cores),
        format!("Data folder: {}", paths::root().display()),
        format!("Account: {account}"),
        format!("Default Java: {}", prefs.preferred_java.as_deref().unwrap_or("automatic")),
        format!("Installed Java: {}", Some(runtimes().join(", ")).filter(|s| !s.is_empty()).unwrap_or_else(|| "none".into())),
        format!("Running: {}", if running.is_empty() { "nothing".into() } else { running.join(", ") }),
        String::new(),
        "Instances:".into(),
    ];
    for inst in instance::list() {
        let eff = optimizer::effective(&inst, &prefs);
        out.push(format!(
            "- {} · {} {:?} {} · {}-{} MB · {:?} flags · Java {}{}",
            inst.id,
            inst.mc_version,
            inst.loader,
            inst.loader_version.as_deref().unwrap_or(""),
            eff.memory_min_mb,
            eff.memory_max_mb,
            eff.flag_profile,
            if inst.java_path.as_deref().is_some_and(|p| !p.trim().is_empty()) { "custom" } else { "automatic" },
            if inst.extra_jvm_args.trim().is_empty() { "" } else { " · extra JVM args" },
        ));
    }
    out.push(String::new());
    out.push("Recent launcher log:".into());
    out.extend(recent_lines());
    redact(&out.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_utc_times() {
        assert_eq!(format_time(0), "1970-01-01 00:00:00Z");
        assert_eq!(format_time(951_782_400), "2000-02-29 00:00:00Z");
        assert_eq!(format_time(1_792_235_867), "2026-10-17 11:17:47Z");
    }

    #[test]
    #[ignore]
    fn report_preview() {
        started();
        log(format!("path {}", dirs::home_dir().unwrap().join("x").display()));
        let r = report(&[]);
        eprintln!("{r}");
        assert!(r.contains("PiumaLauncher") && r.contains("path ~"));
    }
}
