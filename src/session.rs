use anyhow::{Context, bail};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

const WAYLAND_SESSION_DIRS: &[&str] = &[
    "/usr/share/wayland-sessions",
    "/usr/local/share/wayland-sessions",
];

#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub command: Vec<String>,
    pub is_default: bool,
}

pub fn discover_sessions(default_id: &str) -> anyhow::Result<Vec<Session>> {
    let mut sessions = BTreeMap::new();

    for dir in WAYLAND_SESSION_DIRS {
        let path = Path::new(dir);
        if !path.is_dir() {
            continue;
        }

        for entry in fs::read_dir(path).with_context(|| format!("reading {}", path.display()))? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("desktop") {
                continue;
            }

            match parse_session_file(&path, default_id) {
                Ok(session) => {
                    sessions.entry(session.id.clone()).or_insert(session);
                }
                Err(error) => {
                    tracing::warn!(path = %path.display(), %error, "ignoring invalid session file");
                }
            }
        }
    }

    let mut sessions = sessions.into_values().collect::<Vec<_>>();
    sessions.sort_by(|a, b| b.is_default.cmp(&a.is_default).then(a.name.cmp(&b.name)));
    tracing::info!(count = sessions.len(), "wayland sessions discovered");
    Ok(sessions)
}

fn parse_session_file(path: &Path, default_id: &str) -> anyhow::Result<Session> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
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
    let mut command = parse_exec(exec)?;
    validate_command(&mut command)?;

    let id = path
        .file_stem()
        .and_then(|value| value.to_str())
        .context("session filename is not valid UTF-8")?
        .to_string();

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

fn parse_desktop_entry(contents: &str) -> BTreeMap<String, String> {
    let mut in_desktop_entry = false;
    let mut values = BTreeMap::new();

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
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

fn parse_exec(exec: &str) -> anyhow::Result<Vec<String>> {
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

fn validate_command(command: &mut [String]) -> anyhow::Result<()> {
    let executable = command.first().context("empty command")?.clone();
    if executable.contains('/') {
        let path = PathBuf::from(executable);
        if path.is_file() {
            return Ok(());
        }
        bail!("session executable does not exist: {}", path.display());
    }

    if let Some(path) = find_in_path(&executable) {
        command[0] = path.to_string_lossy().into_owned();
        Ok(())
    } else {
        bail!("session executable not found in PATH: {executable}");
    }
}

fn find_in_path(executable: &str) -> Option<PathBuf> {
    ["/usr/local/bin", "/usr/bin", "/bin"]
        .into_iter()
        .map(PathBuf::from)
        .map(|dir| dir.join(executable))
        .find(|candidate| candidate.is_file())
}

fn is_default_session(id: &str, name: &str, desktop_names: &[String], configured: &str) -> bool {
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
