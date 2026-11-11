//! Shell helpers for terminal sessions. `build_cd_command` mirrors `frontend/src/shell.ts`
//! so opening a saved path in a new session types exactly what jumping to it in an open
//! terminal does.

use std::time::Duration;
use tokio::time::Instant;

fn is_safe_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c)
}

/// POSIX shell quoting, equivalent to Python's `shlex.quote`.
pub fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    if value.chars().all(is_safe_char) {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

/// Quotes a remote path while keeping a leading `~` expandable by the shell.
pub fn quote_remote_path(path: &str) -> String {
    if path.is_empty() || path == "~" {
        return "~".to_string();
    }
    if let Some(rest) = path.strip_prefix("~/") {
        return format!("~/{}", shell_quote(rest));
    }
    shell_quote(path)
}

pub fn build_cd_command(path: &str, startup: &str) -> String {
    let mut command = format!("cd -- {} && clear", quote_remote_path(path.trim()));
    let startup = startup.trim();
    if !startup.is_empty() {
        command.push_str(" && ");
        command.push_str(startup);
    }
    command
}

/// Output must pause this long before the shell counts as waiting at its prompt.
pub const STARTUP_QUIET: Duration = Duration::from_millis(600);
/// Upper bound on waiting for the prompt (silent shells, endless MOTD output).
pub const STARTUP_MAX_WAIT: Duration = Duration::from_secs(6);

/// Decides when a saved path's `cd` may be typed into a fresh shell. Input sent before the
/// login shell is ready can be discarded by the remote side, so the command waits until the
/// login banner/prompt has been printed and output has gone quiet.
pub struct StartupGate {
    deadline: Instant,
    due: Instant,
}

impl StartupGate {
    pub fn new(shell_started: Instant) -> Self {
        let deadline = shell_started + STARTUP_MAX_WAIT;
        Self { deadline, due: deadline }
    }

    /// Call whenever the shell prints something.
    pub fn on_output(&mut self, now: Instant) {
        self.due = (now + STARTUP_QUIET).min(self.deadline);
    }

    pub fn due_at(&self) -> Instant {
        self.due
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cd_command_quotes_like_python_backend() {
        assert_eq!(build_cd_command("/var/www/my app", ""), "cd -- '/var/www/my app' && clear");
        assert_eq!(build_cd_command("/srv/it's", ""), "cd -- '/srv/it'\"'\"'s' && clear");
        assert_eq!(build_cd_command("~", ""), "cd -- ~ && clear");
        assert_eq!(build_cd_command("", ""), "cd -- ~ && clear");
        assert_eq!(
            build_cd_command("~/proj a", "git status"),
            "cd -- ~/'proj a' && clear && git status"
        );
        assert_eq!(build_cd_command("/home/u/webku/bos", "  "), "cd -- /home/u/webku/bos && clear");
        assert_eq!(build_cd_command("/x/$(rm -rf ~)", ""), "cd -- '/x/$(rm -rf ~)' && clear");
    }

    #[test]
    fn startup_gate_waits_for_quiet_output_but_not_forever() {
        let t0 = Instant::now();
        let mut gate = StartupGate::new(t0);
        assert_eq!(gate.due_at(), t0 + STARTUP_MAX_WAIT);

        gate.on_output(t0 + Duration::from_millis(100));
        assert_eq!(gate.due_at(), t0 + Duration::from_millis(100) + STARTUP_QUIET);

        gate.on_output(t0 + Duration::from_millis(900));
        assert_eq!(gate.due_at(), t0 + Duration::from_millis(900) + STARTUP_QUIET);

        // Output that never stops is capped by the deadline.
        gate.on_output(t0 + STARTUP_MAX_WAIT - Duration::from_millis(10));
        assert_eq!(gate.due_at(), t0 + STARTUP_MAX_WAIT);
    }
}
