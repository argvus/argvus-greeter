use anyhow::Context;
use std::{
    fs,
    path::{Path, PathBuf},
};

const PASSWD_PATH: &str = "/etc/passwd";
const MIN_LOGIN_UID: u32 = 1000;
const ACCOUNTS_SERVICE_DIR: &str = "/var/lib/AccountsService";

#[derive(Debug, Clone)]
pub struct User {
    pub username: String,
    pub display_name: String,
    pub avatar: Option<PathBuf>,
}

pub fn discover_users() -> anyhow::Result<Vec<User>> {
    let passwd = fs::read_to_string(PASSWD_PATH).context("reading /etc/passwd")?;
    let mut users = passwd
        .lines()
        .filter_map(parse_passwd_line)
        .filter(is_login_user)
        .map(|entry| User {
            display_name: display_name(&entry),
            avatar: avatar_for(&entry.username),
            username: entry.username,
        })
        .collect::<Vec<_>>();

    users.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    tracing::info!(count = users.len(), "local login users discovered");
    Ok(users)
}

/// Returns the avatar image path registered in AccountsService for `username`.
///
/// AccountsService stores per-account icons under `<accounts>/icons/<username>`
/// and an optional custom `Icon=` path inside `<accounts>/users/<username>`.
/// Paths that do not exist or cannot be read by the greeter user are treated
/// as "no avatar"; home directories are never consulted so no extra
/// permissions are required beyond reading `/var/lib/AccountsService`.
pub fn avatar_for(username: &str) -> Option<PathBuf> {
    avatar_in_accounts(Path::new(ACCOUNTS_SERVICE_DIR), username)
}

fn avatar_in_accounts(accounts_dir: &Path, username: &str) -> Option<PathBuf> {
    let icon = accounts_dir.join("icons").join(username);
    if is_readable_file(&icon) {
        return Some(icon);
    }

    let user_file = accounts_dir.join("users").join(username);
    let contents = fs::read_to_string(user_file).ok()?;
    contents.lines().find_map(|line| {
        line.strip_prefix("Icon=")
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .filter(|path| is_readable_file(path))
    })
}

fn is_readable_file(path: &Path) -> bool {
    path.is_file() && fs::File::open(path).is_ok()
}

#[derive(Debug)]
struct PasswdEntry {
    username: String,
    gecos: String,
    uid: u32,
    shell: String,
}

fn parse_passwd_line(line: &str) -> Option<PasswdEntry> {
    let fields = line.split(':').collect::<Vec<_>>();
    if fields.len() < 7 {
        return None;
    }

    Some(PasswdEntry {
        username: fields[0].to_string(),
        uid: fields[2].parse().ok()?,
        gecos: fields[4].to_string(),
        shell: fields[6].to_string(),
    })
}

fn is_login_user(entry: &PasswdEntry) -> bool {
    if entry.uid < MIN_LOGIN_UID {
        return false;
    }

    let shell = entry.shell.trim();
    !(shell.ends_with("/false") || shell.ends_with("/nologin") || shell.is_empty())
}

fn display_name(entry: &PasswdEntry) -> String {
    let gecos_name = entry.gecos.split(',').next().unwrap_or("").trim();
    if gecos_name.is_empty() {
        entry.username.clone()
    } else {
        gecos_name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::Permissions;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::sync::atomic::{AtomicU32, Ordering};

    fn temp_accounts_dir(label: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "argvus-greeter-avatar-{}-{}-{}",
            label,
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(dir.join("icons")).unwrap();
        fs::create_dir_all(dir.join("users")).unwrap();
        dir
    }

    fn running_as_root() -> bool {
        fs::metadata("/proc/self").map(|meta| meta.uid()).unwrap_or(0) == 0
    }

    #[test]
    fn avatar_found_in_icons_directory() {
        let dir = temp_accounts_dir("icons-hit");
        let icon = dir.join("icons").join("alice");
        fs::write(&icon, b"png-bytes").unwrap();

        assert_eq!(avatar_in_accounts(&dir, "alice"), Some(icon));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_avatar_returns_none() {
        let dir = temp_accounts_dir("missing");
        assert_eq!(avatar_in_accounts(&dir, "carol"), None);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn nonexistent_accounts_directory_returns_none() {
        assert_eq!(
            avatar_in_accounts(Path::new("/nonexistent/argvus-test"), "alice"),
            None
        );
    }

    #[test]
    fn icon_reference_in_users_file_is_used_as_fallback() {
        let dir = temp_accounts_dir("icon-ref");
        let referenced = dir.join("custom-bob.png");
        fs::write(&referenced, b"png-bytes").unwrap();
        fs::write(
            dir.join("users").join("bob"),
            format!("[User]\nIcon={}\nSystemAccount=false\n", referenced.display()),
        )
        .unwrap();

        assert_eq!(avatar_in_accounts(&dir, "bob"), Some(referenced));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn icon_reference_to_missing_file_is_ignored() {
        let dir = temp_accounts_dir("icon-ref-missing");
        fs::write(
            dir.join("users").join("bob"),
            "[User]\nIcon=/does/not/exist.png\n",
        )
        .unwrap();

        assert_eq!(avatar_in_accounts(&dir, "bob"), None);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn empty_or_malformed_icon_lines_are_ignored() {
        let dir = temp_accounts_dir("icon-ref-empty");
        let referenced = dir.join("custom-carol.png");
        fs::write(&referenced, b"png-bytes").unwrap();
        fs::write(
            dir.join("users").join("carol"),
            format!("Icon=   \nIcon={}\n", referenced.display()),
        )
        .unwrap();

        assert_eq!(avatar_in_accounts(&dir, "carol"), Some(referenced));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn directory_instead_of_icon_file_is_ignored() {
        let dir = temp_accounts_dir("icon-dir");
        fs::create_dir_all(dir.join("icons").join("dave")).unwrap();

        assert_eq!(avatar_in_accounts(&dir, "dave"), None);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unreadable_icon_file_falls_back_to_icon_reference() {
        if running_as_root() {
            return;
        }
        let dir = temp_accounts_dir("unreadable-icon");
        let unreadable = dir.join("icons").join("erin");
        fs::write(&unreadable, b"png-bytes").unwrap();
        fs::set_permissions(&unreadable, Permissions::from_mode(0o000)).unwrap();

        let referenced = dir.join("custom-erin.png");
        fs::write(&referenced, b"png-bytes").unwrap();
        fs::write(
            dir.join("users").join("erin"),
            format!("[User]\nIcon={}\n", referenced.display()),
        )
        .unwrap();

        assert_eq!(avatar_in_accounts(&dir, "erin"), Some(referenced));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unreadable_users_file_returns_none() {
        if running_as_root() {
            return;
        }
        let dir = temp_accounts_dir("unreadable-users");
        let users_file = dir.join("users").join("frank");
        fs::write(&users_file, "[User]\nIcon=/tmp/x.png\n").unwrap();
        fs::set_permissions(&users_file, Permissions::from_mode(0o000)).unwrap();

        assert_eq!(avatar_in_accounts(&dir, "frank"), None);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn usernames_with_valid_special_characters_resolve() {
        let dir = temp_accounts_dir("special-chars");
        let icon = dir.join("icons").join("user.name-1_x");
        fs::write(&icon, b"png-bytes").unwrap();

        assert_eq!(avatar_in_accounts(&dir, "user.name-1_x"), Some(icon));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn switching_users_resolves_distinct_avatars() {
        let dir = temp_accounts_dir("switch");
        let alice_icon = dir.join("icons").join("alice");
        let bob_icon = dir.join("icons").join("bob");
        fs::write(&alice_icon, b"alice-png").unwrap();
        fs::write(&bob_icon, b"bob-png").unwrap();

        assert_eq!(avatar_in_accounts(&dir, "alice"), Some(alice_icon));
        assert_eq!(avatar_in_accounts(&dir, "bob"), Some(bob_icon));
        assert_ne!(
            avatar_in_accounts(&dir, "alice"),
            avatar_in_accounts(&dir, "bob")
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
