//! "> command": run shell commands, either inline (output streams into the panel)
//! or in the user's terminal emulator.

use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::env::{clean_env, path_executables, shell_quote, spawn_detached, which};
use crate::fuzzy::fuzzy_match;
use crate::store::Settings;
use crate::types::*;
use crate::xdg;

type Args = fn(&str, &[String]) -> Vec<String>;

fn with_sep(mut base: Vec<String>, sep: &str, argv: &[String]) -> Vec<String> {
    if !argv.is_empty() {
        if !sep.is_empty() {
            base.push(sep.into());
        }
        base.extend(argv.iter().cloned());
    }
    base
}

const TERMINALS: &[(&str, Args)] = &[
    ("ptyxis", |cwd, argv| with_sep(vec!["--new-window".into(), "-d".into(), cwd.into()], "--", argv)),
    ("gnome-terminal", |cwd, argv| with_sep(vec![format!("--working-directory={cwd}")], "--", argv)),
    ("kgx", |cwd, argv| with_sep(vec![format!("--working-directory={cwd}")], "--", argv)),
    ("konsole", |cwd, argv| with_sep(vec!["--workdir".into(), cwd.into()], "-e", argv)),
    ("xfce4-terminal", |cwd, argv| with_sep(vec![format!("--working-directory={cwd}")], "-x", argv)),
    ("tilix", |cwd, argv| with_sep(vec!["-w".into(), cwd.into()], "-x", argv)),
    ("ghostty", |cwd, argv| with_sep(vec![format!("--working-directory={cwd}")], "-e", argv)),
    ("wezterm", |cwd, argv| with_sep(vec!["start".into(), "--cwd".into(), cwd.into()], "--", argv)),
    ("kitty", |cwd, argv| with_sep(vec!["--directory".into(), cwd.into()], "", argv)),
    ("alacritty", |cwd, argv| with_sep(vec!["--working-directory".into(), cwd.into()], "-e", argv)),
    ("foot", |cwd, argv| with_sep(vec!["-D".into(), cwd.into()], "", argv)),
    ("x-terminal-emulator", |_, argv| with_sep(vec![], "-e", argv)),
    ("xterm", |_, argv| with_sep(vec![], "-e", argv)),
];

/// Terminal emulators Lumen knows how to drive, in order of preference.
pub fn known_terminals() -> impl Iterator<Item = &'static str> {
    TERMINALS.iter().map(|(b, _)| *b)
}

/// Choices for the Settings dropdown.
pub const TERMINAL_CHOICES: &[&str] =
    &["auto", "ptyxis", "gnome-terminal", "kgx", "konsole", "kitty", "alacritty", "wezterm", "ghostty", "foot", "tilix", "xfce4-terminal", "xterm"];

fn pick_terminal(preferred: &str) -> Option<(String, Args)> {
    let generic: Args = |_, argv| with_sep(vec![], "-e", argv);
    if !preferred.is_empty() && preferred != "auto" {
        if let Some((bin, args)) = TERMINALS.iter().find(|(b, _)| *b == preferred)
            && which(bin).is_some()
        {
            return Some((bin.to_string(), *args));
        }
        // A custom terminal binary: assume the common `-e` convention.
        if which(preferred).is_some() {
            return Some((preferred.to_string(), generic));
        }
    }
    if let Ok(from_env) = std::env::var("TERMINAL")
        && let Some((bin, args)) = TERMINALS.iter().find(|(b, _)| *b == from_env)
        && which(bin).is_some()
    {
        return Some((bin.to_string(), *args));
    }
    TERMINALS.iter().find(|(b, _)| which(b).is_some()).map(|(b, a)| (b.to_string(), *a))
}

pub fn user_shell() -> String {
    std::env::var("SHELL").ok().filter(|s| std::path::Path::new(s).exists()).unwrap_or_else(|| "/bin/bash".into())
}

pub fn home_str() -> String {
    xdg::home().to_string_lossy().into_owned()
}

/// Open the user's terminal at `cwd`, optionally running `command` and keeping the shell open afterwards.
pub fn open_in_terminal(settings: &Settings, command: Option<&str>, cwd: Option<&str>) -> Result<(), String> {
    let (bin, args) = pick_terminal(&settings.terminal).ok_or("No terminal emulator found")?;
    let cwd = cwd.map(String::from).unwrap_or_else(home_str);
    let shell = user_shell();
    let argv: Vec<String> = match command {
        Some(c) => vec![shell.clone(), "-ic".into(), format!("{c}; exec {}", shell_quote(&shell))],
        None => vec![],
    };
    spawn_detached(&bin, &args(&cwd, &argv), Some(&cwd)).map_err(|e| format!("Couldn't start {bin}: {e}"))
}

// Programs that need a real TTY; these always run in a terminal window.
const INTERACTIVE: &[&str] = &[
    "vim", "vi", "nvim", "nano", "emacs", "micro", "helix", "hx", "htop", "top", "btop", "atop", "nvtop", "less", "more", "man", "ssh", "mosh",
    "tmux", "screen", "mc", "ranger", "nnn", "lf", "yazi", "sudo", "su", "passwd", "watch", "bash", "zsh", "fish", "sh", "python", "python3", "node",
    "irb", "psql", "mysql", "sqlite3", "ftp", "telnet", "claude", "lazygit",
];

pub fn needs_terminal(command: &str) -> bool {
    let first = command.split_whitespace().next().unwrap_or("");
    if INTERACTIVE.contains(&first) {
        return true;
    }
    let follows = command.split_whitespace().any(|w| w == "-f" || w == "--follow") || command.contains("| less") || command.contains("|less");
    follows && ["journalctl", "tail", "less"].iter().any(|p| command.contains(p))
}

fn command_item(command: &str, score: f64, from_history: bool) -> ResultItem {
    let interactive = needs_terminal(command);
    let in_terminal = Action::RunCommand { command: command.into(), mode: RunMode::Terminal };
    let inline = Action::RunCommand { command: command.into(), mode: RunMode::Inline };
    let mut item =
        ResultItem::new(format!("cmd:{command}"), ResultKind::Command, command, if interactive { in_terminal.clone() } else { inline.clone() });
    item.subtitle = Some(
        if interactive {
            "Run in terminal"
        } else if from_history {
            "Recent command · Enter shows output here"
        } else {
            "Run and show output here"
        }
        .into(),
    );
    item.icon = Some(IconSpec::Glyph(Glyph::Terminal));
    item.score = score;
    item.actions = if interactive {
        vec![SecondaryAction::new("Run here and show output", inline).glyph(Glyph::Terminal)]
    } else {
        vec![SecondaryAction::new("Run in terminal", in_terminal).hint("Ctrl+Enter").glyph(Glyph::Terminal)]
    };
    item
}

/// Results for an explicit "> command" query.
pub fn command_results(command: &str, history: &[String]) -> Vec<ResultItem> {
    let cmd = command.trim();
    let mut items = Vec::new();
    if !cmd.is_empty() {
        items.push(command_item(cmd, 2000.0, false));
    }
    for h in history {
        if h == cmd {
            continue;
        }
        let score = if cmd.is_empty() { Some(500.0) } else { fuzzy_match(cmd, h).map(|m| m.score) };
        if let Some(s) = score
            && s > 300.0
        {
            items.push(command_item(h, 1000.0 + s / 10.0, true));
        }
        if items.len() >= 7 {
            break;
        }
    }
    if cmd.is_empty() {
        let mut open = ResultItem::new("cmd:open-terminal", ResultKind::Command, "Open Terminal", Action::OpenTerminalAt(home_str()));
        open.subtitle = Some("Type a command after \">\" to run it".into());
        open.icon = Some(IconSpec::Glyph(Glyph::Terminal));
        open.score = 1500.0;
        items.insert(0, open);
    }
    items
}

/// A low-priority "run it" suggestion when the first word is a program on PATH.
pub fn bare_command_result(query: &str) -> Option<ResultItem> {
    let q = query.trim();
    let first = q.split_whitespace().next()?;
    if first.chars().count() < 2 || !path_executables().contains(first) {
        return None;
    }
    let mut item = command_item(q, 300.0, false);
    item.subtitle = Some(format!("Run “{first}” · tip: start with > to run commands"));
    Some(item)
}

// ── Inline runs ────────────────────────────────────────────────────────────

pub enum RunEvent {
    Output { text: String, stderr: bool },
    Done { exit_code: Option<i32> },
}

const MAX_OUTPUT: usize = 512 * 1024;

/// Turns byte chunks into text without splitting a UTF-8 character across two chunks.
struct Utf8Chunker(Vec<u8>);

impl Utf8Chunker {
    fn push(&mut self, bytes: &[u8]) -> String {
        self.0.extend_from_slice(bytes);
        let valid = match std::str::from_utf8(&self.0) {
            Ok(_) => self.0.len(),
            Err(e) if e.error_len().is_none() => e.valid_up_to(),
            Err(_) => self.0.len(),
        };
        let rest = self.0.split_off(valid);
        let text = String::from_utf8_lossy(&self.0).into_owned();
        self.0 = rest;
        text
    }
}

/// Run `command` with the user's shell, streaming output to `emit` from background threads.
/// Returns the process id (also the process group id, for stopping it).
pub fn run_inline(command: &str, emit: impl Fn(RunEvent) + Send + Sync + 'static) -> std::io::Result<u32> {
    let mut env = clean_env();
    env.retain(|(k, _)| !matches!(k.as_str(), "TERM" | "PAGER" | "GIT_PAGER" | "SYSTEMD_PAGER"));
    env.extend([("TERM", "dumb"), ("PAGER", "cat"), ("GIT_PAGER", "cat"), ("SYSTEMD_PAGER", "")].map(|(k, v)| (k.to_string(), v.to_string())));
    let mut child = Command::new(user_shell())
        .args(["-c", command])
        .current_dir(home_str())
        .env_clear()
        .envs(env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    let pid = child.id();
    let emit = Arc::new(emit);
    let sent = Arc::new(AtomicUsize::new(0));
    let reader = |mut pipe: Box<dyn Read + Send>, stderr: bool| {
        let emit = emit.clone();
        let sent = sent.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            let mut chunker = Utf8Chunker(Vec::new());
            loop {
                let n = match pipe.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                let before = sent.fetch_add(n, Ordering::Relaxed);
                if before > MAX_OUTPUT {
                    continue; // keep draining so the program isn't blocked on a full pipe
                }
                let mut text = chunker.push(&buf[..n]);
                if before + n > MAX_OUTPUT {
                    text.push_str("\n… output truncated\n");
                }
                if !text.is_empty() {
                    emit(RunEvent::Output { text, stderr });
                }
            }
        })
    };
    let out = reader(Box::new(child.stdout.take().expect("piped stdout")), false);
    let err = reader(Box::new(child.stderr.take().expect("piped stderr")), true);
    std::thread::spawn(move || {
        let _ = out.join();
        let _ = err.join();
        let code = child.wait().ok().and_then(|s| s.code());
        emit(RunEvent::Done { exit_code: code });
    });
    Ok(pid)
}

/// Stop an inline run and everything it started.
pub fn kill_run(pid: u32) {
    // SAFETY: plain syscalls on a process group we created.
    unsafe {
        if libc::kill(-(pid as i32), libc::SIGTERM) != 0 {
            libc::kill(pid as i32, libc::SIGTERM);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn interactive_programs_go_to_a_terminal() {
        assert!(needs_terminal("htop"));
        assert!(needs_terminal("journalctl -f"));
        assert!(needs_terminal("tail -f /var/log/syslog"));
        assert!(!needs_terminal("ls -la"));
        assert!(!needs_terminal("tail -n 5 file"));
    }

    #[test]
    fn explicit_commands_lead() {
        let r = command_results("ls", &["ls -la".into(), "git status".into()]);
        assert_eq!(r[0].title, "ls");
        assert_eq!(r[1].title, "ls -la");
        let empty = command_results("", &[]);
        assert_eq!(empty[0].title, "Open Terminal");
    }

    #[test]
    fn inline_runs_stream_and_finish() {
        let (tx, rx) = mpsc::channel();
        let tx = std::sync::Mutex::new(tx);
        run_inline("printf 'héllo'; echo oops >&2; exit 3", move |e| {
            let _ = tx.lock().unwrap().send(e);
        })
        .unwrap();
        let (mut out, mut err, mut code) = (String::new(), String::new(), None);
        while let Ok(e) = rx.recv_timeout(std::time::Duration::from_secs(5)) {
            match e {
                RunEvent::Output { text, stderr: false } => out.push_str(&text),
                RunEvent::Output { text, stderr: true } => err.push_str(&text),
                RunEvent::Done { exit_code } => {
                    code = exit_code;
                    break;
                }
            }
        }
        assert_eq!(out, "héllo");
        assert_eq!(err, "oops\n");
        assert_eq!(code, Some(3));
    }

    #[test]
    fn utf8_chunks_are_not_split() {
        let mut c = Utf8Chunker(Vec::new());
        let bytes = "é".as_bytes();
        assert_eq!(c.push(&bytes[..1]), "");
        assert_eq!(c.push(&bytes[1..]), "é");
    }
}
