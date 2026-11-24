//! Best-effort integrations with Termux and the Termux:API app.
//!
//! Everything in this module is deliberately side-effect-only: failures are
//! ignored and nothing is ever written to stdout, so machine-readable output
//! (`--json` / `--stream-jsonl`) stays untouched. Termux detection works with
//! forks such as `com.involvex.termux_app`, not just the stock `com.termux`.

use std::collections::HashSet;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::cli::TermuxMode;

const TERMUX_TOOLS: &[&str] = &[
    "termux-clipboard-set",
    "termux-open",
    "termux-toast",
    "termux-notification",
    "termux-wake-lock",
    "termux-wake-unlock",
];

/// Resolved Termux environment with the Termux:API tools that are actually
/// installed. `None` means "behave exactly like a desktop terminal".
#[derive(Clone, Debug)]
pub struct Termux {
    tools: HashSet<String>,
}

impl Termux {
    pub fn resolve(mode: TermuxMode) -> Option<Self> {
        match mode {
            TermuxMode::Off => None,
            TermuxMode::On => Some(Self::probe()),
            TermuxMode::Auto => {
                if detect_from_env(|key| std::env::var(key).ok()) {
                    Some(Self::probe())
                } else {
                    None
                }
            }
        }
    }

    pub fn has(&self, tool: &str) -> bool {
        self.tools.contains(tool)
    }

    pub fn is_api_available(&self) -> bool {
        TERMUX_TOOLS.iter().any(|tool| self.has(tool))
    }

    pub fn tools(&self) -> Vec<String> {
        let mut tools: Vec<String> = self.tools.iter().cloned().collect();
        tools.sort();
        tools
    }

    fn probe() -> Self {
        let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
            .map(|path| std::env::split_paths(&path).collect())
            .unwrap_or_default();
        if let Some(prefix) = std::env::var_os("PREFIX") {
            dirs.push(PathBuf::from(prefix).join("bin"));
        }
        let tools = TERMUX_TOOLS
            .iter()
            .filter(|tool| dirs.iter().any(|dir| is_executable(&dir.join(tool))))
            .map(|tool| (*tool).to_string())
            .collect();
        Self { tools }
    }

    fn spawn(&self, tool: &str, args: &[&str], stdin_text: Option<&str>) {
        if !self.has(tool) {
            return;
        }
        let mut command = Command::new(tool);
        command
            .args(args)
            .stdin(if stdin_text.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Ok(mut child) = command.spawn() {
            if let (Some(text), Some(stdin)) = (stdin_text, child.stdin.take()) {
                let mut stdin = stdin;
                let _ = stdin.write_all(text.as_bytes());
            }
            // Reap in the background so long-running commands do not leak
            // zombies, without ever blocking on a Termux:API broadcast.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }

    pub fn copy(&self, text: &str) {
        // Text via stdin: termux-clipboard-set has no `--` separator and would
        // otherwise treat a leading `-` as an option.
        self.spawn("termux-clipboard-set", &[], Some(text));
    }

    pub fn open(&self, path_or_url: &str) {
        self.spawn("termux-open", &["--view", path_or_url], None);
    }

    pub fn toast(&self, text: &str) {
        // No arguments: termux-toast reads the text from stdin, so message
        // contents can never be mistaken for options.
        self.spawn("termux-toast", &[], Some(text));
    }

    pub fn notify(&self, title: &str, content: &str) {
        // A stable id overwrites the previous notification instead of piling
        // up one notification per message.
        self.spawn(
            "termux-notification",
            &["-i", "whatshell", "-t", title, "-c", content],
            None,
        );
    }

    pub fn wake_lock(&self) -> WakeLock {
        let active = self.has("termux-wake-lock") && self.has("termux-wake-unlock");
        if active {
            self.spawn("termux-wake-lock", &[], None);
        }
        WakeLock { active }
    }
}

/// Holds a Termux wake lock until dropped, so Android Doze cannot suspend a
/// long-running WhatsApp session (auth, sync --follow, listen, inbox TUI).
#[derive(Debug)]
pub struct WakeLock {
    active: bool,
}

impl Drop for WakeLock {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        if let Ok(mut child) = Command::new("termux-wake-unlock")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }
}

/// Coalesces bursts of incoming messages into at most one notification every
/// few seconds. Shared by `listen`, `sync --follow`, and the inbox TUI.
#[derive(Debug)]
pub struct MessageNotifier {
    termux: Option<Termux>,
    enabled: bool,
    interval: Duration,
    state: Mutex<NotifyState>,
}

#[derive(Debug, Default)]
struct NotifyState {
    count: usize,
    last_chat: String,
    last: Option<Instant>,
}

impl MessageNotifier {
    pub fn new(termux: Option<Termux>, enabled: bool) -> Self {
        Self {
            termux,
            enabled,
            interval: Duration::from_secs(3),
            state: Mutex::new(NotifyState::default()),
        }
    }

    pub fn note(&self, chat: &str) {
        if !self.enabled {
            return;
        }
        let mut state = self.lock();
        state.count += 1;
        state.last_chat = chat.to_string();
        let due = state
            .last
            .is_none_or(|last| last.elapsed() >= self.interval);
        if due {
            self.flush_state(&state);
            state.count = 0;
            state.last = Some(Instant::now());
        }
    }

    pub fn flush(&self) {
        if !self.enabled {
            return;
        }
        let mut state = self.lock();
        self.flush_state(&state);
        state.count = 0;
        state.last = Some(Instant::now());
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, NotifyState> {
        self.state.lock().unwrap_or_else(|err| err.into_inner())
    }

    fn flush_state(&self, state: &NotifyState) {
        if state.count == 0 {
            return;
        }
        let Some(termux) = self.termux.as_ref() else {
            return;
        };
        let content = if state.count == 1 {
            format!("New message in {}", state.last_chat)
        } else {
            format!("{} new messages (latest: {})", state.count, state.last_chat)
        };
        termux.notify("whatshell · WhatsApp", &content);
    }
}

/// Clipboard and `termux-open` side effects should only happen when a human is
/// watching the terminal; agents parsing `--json` must never be interrupted.
pub fn human_interactive(json: bool) -> bool {
    !json && io::stdout().is_terminal()
}

/// Fork-agnostic Termux detection.
///
/// `TERMUX_VERSION` is set by Termux and its forks (including
/// `com.involvex.termux_app`). The fallback checks the characteristic
/// `$PREFIX/usr` + sibling `$HOME` layout on Android, without hardcoding a
/// package name.
pub fn detect_from_env<F>(get: F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    if get("TERMUX_VERSION").is_some_and(|value| !value.trim().is_empty()) {
        return true;
    }
    let Some(prefix) = get("PREFIX") else {
        return false;
    };
    if prefix.trim().is_empty() || !prefix.ends_with("/usr") {
        return false;
    }
    if get("ANDROID_ROOT").is_none_or(|value| value.trim().is_empty()) {
        return false;
    }
    let Some(home) = get("HOME") else {
        return false;
    };
    let prefix = Path::new(&prefix);
    let sibling_home = prefix.parent().map(|parent| parent.join("home"));
    sibling_home.is_some_and(|expected| Path::new(&home) == expected) && prefix.join("bin").is_dir()
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars<'a>(values: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| {
            values
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| (*value).to_string())
        }
    }

    #[test]
    fn detects_termux_version_from_forks() {
        assert!(detect_from_env(vars(&[("TERMUX_VERSION", "0.210.0")])));
    }

    #[test]
    fn detects_custom_fork_prefix_without_termux_version() {
        let dir = tempfile::tempdir().unwrap();
        let files = dir.path().join("files");
        let prefix = files.join("usr");
        let home = files.join("home");
        std::fs::create_dir_all(prefix.join("bin")).unwrap();
        assert!(detect_from_env(vars(&[
            ("PREFIX", prefix.to_str().unwrap()),
            ("HOME", home.to_str().unwrap()),
            ("ANDROID_ROOT", "/system"),
        ])));
    }

    #[test]
    fn rejects_desktop_environments() {
        assert!(!detect_from_env(vars(&[
            ("PREFIX", "/usr"),
            ("HOME", "/home/user"),
        ])));
    }

    #[test]
    fn off_mode_resolves_to_none() {
        assert!(Termux::resolve(TermuxMode::Off).is_none());
    }

    #[test]
    fn notifier_is_a_noop_when_disabled() {
        let notifier = MessageNotifier::new(None, false);
        notifier.note("+15551234567");
        notifier.flush();
    }
}
