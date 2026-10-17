use std::path::Path;
use std::time::{Duration, Instant};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::error::{Error, Result};
use crate::paths;

fn norm(p: &Path) -> String {
    let s = p.to_string_lossy();
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s).trim_end_matches(['/', '\\']);
    if cfg!(windows) { s.replace('/', "\\").to_lowercase() } else { s.to_string() }
}

fn is_java(name: &str) -> bool {
    let name = name.to_lowercase();
    let stem = name.strip_suffix(".exe").unwrap_or(&name);
    stem == "java" || stem == "javaw"
}

fn scan(instance_id: &str) -> (System, Vec<Pid>) {
    let game = norm(&paths::game_dir(instance_id));
    let natives = norm(&paths::natives(instance_id));
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let javas: Vec<Pid> = sys
        .processes()
        .iter()
        .filter(|(_, p)| is_java(&p.name().to_string_lossy()))
        .map(|(pid, _)| *pid)
        .collect();
    if javas.is_empty() {
        return (sys, vec![]);
    }
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&javas),
        true,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always).with_cwd(UpdateKind::Always),
    );
    let own = Pid::from_u32(std::process::id());
    let found = javas
        .into_iter()
        .filter(|pid| *pid != own)
        .filter(|pid| {
            let Some(p) = sys.process(*pid) else { return false };
            p.cwd().is_some_and(|c| norm(c) == game)
                || p.cmd().iter().any(|a| {
                    let a = Path::new(a);
                    let s = norm(a);
                    s == game || s.strip_prefix("-djava.library.path=").or_else(|| s.strip_prefix("-Djava.library.path=")) == Some(natives.as_str())
                })
        })
        .collect();
    (sys, found)
}

pub fn leftovers(instance_id: &str) -> Vec<u32> {
    scan(instance_id).1.into_iter().map(|p| p.as_u32()).collect()
}

pub fn close_leftovers(instance_id: &str) -> Result<usize> {
    let (sys, pids) = scan(instance_id);
    for pid in &pids {
        if let Some(p) = sys.process(*pid) {
            p.kill();
        }
    }
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        let alive = leftovers(instance_id);
        if alive.is_empty() {
            return Ok(pids.len());
        }
        if Instant::now() >= deadline {
            let list = alive.iter().map(u32::to_string).collect::<Vec<_>>().join(", ");
            return Err(Error::msg(format!(
                "Couldn't close the Minecraft still running for this instance (PID {list}). End it from Task Manager and try again"
            )));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_names() {
        assert!(is_java("java"));
        assert!(is_java("javaw.exe"));
        assert!(is_java("JAVAW.EXE"));
        assert!(!is_java("javascript"));
        assert!(!is_java("piuma-launcher"));
    }

    #[cfg(unix)]
    #[test]
    #[ignore]
    fn finds_and_closes_leftover() {
        let id = "leftover-probe";
        let game = paths::game_dir(id);
        std::fs::create_dir_all(&game).unwrap();
        let fake = paths::root().join("java");
        let _ = std::fs::remove_file(&fake);
        std::os::unix::fs::symlink("/bin/sleep", &fake).unwrap();
        let mut child = std::process::Command::new(&fake).arg("60").current_dir(&game).spawn().unwrap();
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(leftovers(id), vec![child.id()]);
        assert!(leftovers("some-other-instance").is_empty());
        let reaper = std::thread::spawn(move || child.wait());
        assert_eq!(close_leftovers(id).unwrap(), 1);
        assert!(reaper.join().unwrap().is_ok());
        assert!(leftovers(id).is_empty());
        let _ = std::fs::remove_file(&fake);
        let _ = std::fs::remove_dir_all(paths::instance_dir(id));
    }

    #[test]
    fn norm_strips_verbatim_and_trailing() {
        let n = norm(Path::new(r"\\?\C:\Games\x\"));
        assert!(n.ends_with("x") && !n.starts_with(r"\\?\"));
    }
}
