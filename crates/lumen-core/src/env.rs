//! Launching programs the way the desktop would.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Variables that describe how *Lumen* was started and must not leak into programs it launches.
const DROP_EXACT: &[&str] = &[
    "APPDIR",
    "APPIMAGE",
    "ARGV0",
    "OWD",
    "NO_AT_BRIDGE",
    "GDK_BACKEND",
    "ORIGINAL_XDG_CURRENT_DESKTOP",
    "GIO_LAUNCHED_DESKTOP_FILE",
    "GIO_LAUNCHED_DESKTOP_FILE_PID",
    "DESKTOP_STARTUP_ID",
    "XDG_ACTIVATION_TOKEN",
    "GTK_DEBUG",
    "GSK_RENDERER",
];
const DROP_PREFIX: &[&str] = &["LUMEN_", "ELECTRON_", "CHROME_"];

/// Environment for programs we launch: Lumen's own variables removed, and AppImage paths (if
/// Lumen runs from one) filtered out, so launched apps behave as if started from the desktop.
pub fn clean_env() -> Vec<(String, String)> {
    let app_dir = std::env::var("APPDIR").ok().filter(|d| !d.is_empty());
    let mut out = Vec::new();
    for (k, v) in std::env::vars() {
        if DROP_EXACT.contains(&k.as_str()) || DROP_PREFIX.iter().any(|p| k.starts_with(p)) {
            continue;
        }
        let mut v = v;
        if let Some(dir) = &app_dir
            && ["LD_LIBRARY_PATH", "PATH", "XDG_DATA_DIRS", "PYTHONPATH", "PERL5LIB", "GSETTINGS_SCHEMA_DIR", "QT_PLUGIN_PATH"].contains(&k.as_str())
        {
            let kept: Vec<&str> = v.split(':').filter(|p| !p.is_empty() && !p.starts_with(dir.as_str())).collect();
            if kept.is_empty() {
                continue;
            }
            v = kept.join(":");
        }
        out.push((k, v));
    }
    if let Ok(orig) = std::env::var("ORIGINAL_XDG_CURRENT_DESKTOP") {
        out.retain(|(k, _)| k != "XDG_CURRENT_DESKTOP");
        out.push(("XDG_CURRENT_DESKTOP".into(), orig));
    }
    out
}

/// The names `clean_env` removes, for callers that adjust an existing environment (GIO launch contexts).
pub fn dropped_env_names() -> Vec<String> {
    std::env::vars().map(|(k, _)| k).filter(|k| DROP_EXACT.contains(&k.as_str()) || DROP_PREFIX.iter().any(|p| k.starts_with(p))).collect()
}

fn path_dirs() -> Vec<String> {
    clean_env()
        .into_iter()
        .find(|(k, _)| k == "PATH")
        .map(|(_, v)| v.split(':').filter(|d| !d.is_empty()).map(String::from).collect())
        .unwrap_or_else(|| vec!["/usr/local/bin".into(), "/usr/bin".into(), "/bin".into()])
}

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata().map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

static WHICH: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

/// Full path of a program on PATH (or the path itself if it's executable).
pub fn which(bin: &str) -> Option<String> {
    let cache = WHICH.get_or_init(Default::default);
    if let Some(hit) = cache.lock().unwrap().get(bin) {
        return hit.clone();
    }
    let found = if bin.contains('/') {
        is_executable(Path::new(bin)).then(|| bin.to_string())
    } else {
        path_dirs().into_iter().map(|d| format!("{d}/{bin}")).find(|p| is_executable(Path::new(p)))
    };
    // Only remember hits: a program installed later (ydotool, say) should be found without a restart.
    if found.is_some() {
        cache.lock().unwrap().insert(bin.to_string(), found.clone());
    }
    found
}

static EXECUTABLES: OnceLock<HashSet<String>> = OnceLock::new();

/// Every program name on PATH, used to suggest "Run" for bare commands.
pub fn path_executables() -> &'static HashSet<String> {
    EXECUTABLES.get_or_init(|| {
        let mut set = HashSet::new();
        for dir in path_dirs() {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for e in entries.flatten() {
                    set.insert(e.file_name().to_string_lossy().into_owned());
                }
            }
        }
        set
    })
}

pub struct RunOutput {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Run a short command and capture its output, killing it after `timeout`.
pub fn run_timeout(cmd: &str, args: &[&str], timeout: Duration) -> RunOutput {
    let fail = |e: String| RunOutput { ok: false, stdout: String::new(), stderr: e };
    let mut child =
        match Command::new(cmd).args(args).env_clear().envs(clean_env()).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
            Ok(c) => c,
            Err(e) => return fail(e.to_string()),
        };
    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let out_t = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(p) = out_pipe.as_mut() {
            let _ = p.read_to_string(&mut s);
        }
        s
    });
    let err_t = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(p) = err_pipe.as_mut() {
            let _ = p.read_to_string(&mut s);
        }
        s
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) if started.elapsed() < timeout => std::thread::sleep(Duration::from_millis(5)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let stdout = out_t.join().unwrap_or_default().trim().to_string();
    let stderr = err_t.join().unwrap_or_default().trim().to_string();
    RunOutput { ok: status.is_some_and(|s| s.success()), stdout, stderr }
}

pub fn run(cmd: &str, args: &[&str]) -> RunOutput {
    run_timeout(cmd, args, Duration::from_secs(3))
}

/// Start a program fully detached from Lumen (its own session), so closing Lumen never closes it.
pub fn spawn_detached(cmd: &str, args: &[String], cwd: Option<&str>) -> std::io::Result<()> {
    let mut command = Command::new(cmd);
    command.args(args).env_clear().envs(clean_env()).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    // SAFETY: setsid is async-signal-safe and touches no Rust state.
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let mut child = command.spawn()?;
    // Reap it when it exits so it never lingers as a zombie.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

pub fn desktop_name() -> String {
    std::env::var("ORIGINAL_XDG_CURRENT_DESKTOP")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("XDG_CURRENT_DESKTOP").ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "unknown".into())
}

pub fn is_gnome() -> bool {
    let d = desktop_name().to_lowercase();
    d.contains("gnome") || d.contains("unity") || d.contains("ubuntu")
}

pub fn is_wayland() -> bool {
    std::env::var("XDG_SESSION_TYPE").is_ok_and(|s| s == "wayland")
}

pub fn session_type() -> String {
    std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".into())
}

/// What users and keybindings should run to start Lumen: the installed binary (or AppImage).
pub fn launcher_path() -> String {
    for var in ["LUMEN_LAUNCHER", "APPIMAGE"] {
        if let Ok(v) = std::env::var(var)
            && !v.is_empty()
        {
            return v;
        }
    }
    std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| "lumen".into())
}

/// Quote for a POSIX shell, leaving simple words alone.
pub fn shell_quote(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "@%+=:,./-_".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}
