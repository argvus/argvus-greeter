//! Discovery and validation of installed Wayland login sessions.
//!
//! Session desktop files are external input. This module parses only the
//! `[Desktop Entry]` section, resolves executable names against a fixed safe
//! system path, and rejects invalid entries before they reach greetd.

use anyhow::{Context, bail};
use std::{
  collections::BTreeMap,
  fs,
  path::{Path, PathBuf},
};

const WAYLAND_SESSION_DIRS: &[&str] = &[
  // Prefer the standard system directory while retaining the conventional
  // local administrator directory for development/custom installations.
  "/usr/share/wayland-sessions",
  "/usr/local/share/wayland-sessions",
];

/// A validated Wayland session available to greetd.
#[derive(Debug, Clone)]
pub struct Session {
  /// Stable desktop-file identifier used for selection and logging.
  pub id: String,
  /// Human-readable name shown in the TUI.
  pub name: String,
  /// Executable plus arguments passed to greetd's `StartSession` request.
  pub command: Vec<String>,
  /// Whether this entry matches the configured default preference.
  pub is_default: bool,
}

/// Discovers, deduplicates, validates, and sorts installed Wayland sessions.
pub fn discover_sessions(default_id: &str) -> anyhow::Result<Vec<Session>> {
  // BTreeMap provides deterministic ordering and lets the first valid entry
  // win when multiple search directories contain the same session id.
  let mut sessions = BTreeMap::new();

  for dir in WAYLAND_SESSION_DIRS {
    let path = Path::new(dir);
    // Missing optional directories are normal on minimal systems; unreadable
    // directories are errors because discovery would otherwise be incomplete.
    if !path.is_dir() {
      continue;
    }

    for entry in fs::read_dir(path).with_context(|| format!("reading {}", path.display()))? {
      let entry = entry?;
      let path = entry.path();
      // Only Wayland desktop entries are considered; unrelated desktop files
      // must never become login commands accidentally.
      if path.extension().and_then(|value| value.to_str()) != Some("desktop") {
        continue;
      }

      match parse_session_file(&path, default_id) {
        Ok(session) => {
          sessions.entry(session.id.clone()).or_insert(session);
        }
        Err(error) => {
          // One malformed session must not hide all other usable sessions.
          tracing::warn!(path = %path.display(), %error, "ignoring invalid session file");
        }
      }
    }
  }

  let mut sessions = sessions.into_values().collect::<Vec<_>>();
  // Defaults come first, followed by a stable alphabetical presentation.
  sessions.sort_by(|a, b| b.is_default.cmp(&a.is_default).then(a.name.cmp(&b.name)));
  tracing::info!(count = sessions.len(), "wayland sessions discovered");
  Ok(sessions)
}

/// Parses and validates one Wayland desktop entry.
fn parse_session_file(path: &Path, default_id: &str) -> anyhow::Result<Session> {
  let contents = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
  // Desktop entry keys are kept as strings because only a small subset is
  // relevant to greetd and preserving unknown keys has no security benefit.
  let values = parse_desktop_entry(&contents);

  let name = values
    .get("Name")
    .filter(|value| !value.trim().is_empty())
    .context("missing Name")?
    .to_string();
  let exec = values
    .get("Exec")
    .filter(|value| !value.trim().is_empty())
    .context("missing Exec")?;
  // The command is validated before the session is exposed to the UI.
  let mut command = parse_exec(exec)?;
  validate_command(&mut command)?;

  let id = path
    .file_stem()
    .and_then(|value| value.to_str())
    .context("session filename is not valid UTF-8")?
    .to_string();
  // The desktop filename is the stable id used by configuration and logs.

  let desktop_names = values
    .get("DesktopNames")
    .map(|value| {
      value
        .split(';')
        .filter(|item| !item.trim().is_empty())
        .map(|item| item.trim().to_string())
        .collect::<Vec<_>>()
    })
    .unwrap_or_default();

  Ok(Session {
    is_default: is_default_session(&id, &name, &desktop_names, default_id),
    id,
    name,
    command,
  })
}

/// Reads key/value pairs only while inside `[Desktop Entry]`.
///
/// This intentionally does not implement the full desktop-entry standard; the
/// Greeter needs a narrow, predictable subset and rejects missing essentials.
fn parse_desktop_entry(contents: &str) -> BTreeMap<String, String> {
  let mut in_desktop_entry = false;
  let mut values = BTreeMap::new();

  for raw_line in contents.lines() {
    let line = raw_line.trim();
    // Comments and blank lines do not carry launch metadata.
    if line.is_empty() || line.starts_with('#') {
      continue;
    }

    if line.starts_with('[') && line.ends_with(']') {
      // Stop collecting when another desktop-entry group begins.
      in_desktop_entry = line == "[Desktop Entry]";
      continue;
    }

    if !in_desktop_entry {
      continue;
    }

    if let Some((key, value)) = line.split_once('=') {
      values.insert(key.trim().to_string(), value.trim().to_string());
    }
  }

  values
}

/// Splits an `Exec` value while removing freedesktop field codes.
fn parse_exec(exec: &str) -> anyhow::Result<Vec<String>> {
  // Field codes such as `%U` describe desktop-launch context unavailable to a
  // login manager. Removing them before shell-like parsing avoids passing
  // meaningless tokens to the session process.
  let cleaned = exec
    .split_whitespace()
    .filter(|part| !part.starts_with('%'))
    .collect::<Vec<_>>()
    .join(" ");

  let command = shlex::split(&cleaned).context("could not parse Exec command")?;
  if command.is_empty() {
    bail!("Exec command is empty");
  }
  Ok(command)
}

/// Verifies the executable and canonicalizes PATH lookups to absolute paths.
fn validate_command(command: &mut [String]) -> anyhow::Result<()> {
  let executable = command.first().context("empty command")?.clone();
  if executable.contains('/') {
    // Absolute paths are accepted only when they are real files.
    let path = PathBuf::from(executable);
    if path.is_file() {
      return Ok(());
    }
    bail!("session executable does not exist: {}", path.display());
  }

  if let Some(path) = find_in_path(&executable) {
    // Bare names are intentionally resolved against fixed system directories,
    // not the potentially manipulated greeter user's PATH.
    command[0] = path.to_string_lossy().into_owned();
    Ok(())
  } else {
    bail!("session executable not found in PATH: {executable}");
  }
}

/// Searches the fixed executable directories used for session launch.
fn find_in_path(executable: &str) -> Option<PathBuf> {
  ["/usr/local/bin", "/usr/bin", "/bin"]
    .into_iter()
    .map(PathBuf::from)
    .map(|dir| dir.join(executable))
    .find(|candidate| candidate.is_file())
}

/// Applies the configured default preference to one discovered session.
fn is_default_session(id: &str, name: &str, desktop_names: &[String], configured: &str) -> bool {
  // Case-insensitive matching is user-friendly while retaining exact field
  // boundaries; no substring matching is performed.
  let configured = configured.to_ascii_lowercase();
  id.eq_ignore_ascii_case(&configured)
    || name.eq_ignore_ascii_case(&configured)
    || desktop_names
      .iter()
      .any(|value| value.eq_ignore_ascii_case(&configured))
    || (configured == "argvus"
      && (id.eq_ignore_ascii_case("argvus")
        || name.eq_ignore_ascii_case("argvus")
        || desktop_names
          .iter()
          .any(|value| value.eq_ignore_ascii_case("hyprland"))))
}
