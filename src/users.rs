use anyhow::Context;
use std::{
    fs,
    path::{Path, PathBuf},
};

const PASSWD_PATH: &str = "/etc/passwd";
const MIN_LOGIN_UID: u32 = 1000;
const ACCOUNTS_SERVICE_DIR: &str = "/var/lib/AccountsService";
/// Avatar file managed by `argvus-accounts` inside the user's home directory.
const FACE_FILENAME: &str = ".face";

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
            avatar: avatar_for(&entry),
            username: entry.username,
        })
        .collect::<Vec<_>>();

    users.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    tracing::info!(count = users.len(), "local login users discovered");
    Ok(users)
}

/// Returns the avatar image for an account.
///
/// Resolution order:
///
/// 1. `$HOME/.face` — the location written and validated by
///    [`argvus-accounts`](https://github.com/argvus/argvus-accounts), the
///    official account-metadata source of the Argvus desktop. The file is a
///    regular, user-owned PNG (256x256, mode 0644) when deployed by that tool.
/// 2. `/var/lib/AccountsService/icons/<username>` — freedesktop AccountsService
///    convention, kept for interoperability with GNOME/KDE and other display
///    managers.
/// 3. The `Icon=` path declared in `/var/lib/AccountsService/users/<username>`.
///
/// Only existing files that are readable by the greeter user qualify.
/// Symlinks are never followed (the check uses `lstat`), so a hostile or
/// stale `.face` symlink cannot make the greeter read arbitrary paths.
/// Only existing files that are readable by the greeter user qualify.
/// Symlinks are never followed (the check uses `lstat`), so a hostile or
/// stale `.face` symlink cannot make the greeter read arbitrary paths.
fn avatar_for(entry: &PasswdEntry) -> Option<PathBuf> {
    avatar_for_in(Path::new(ACCOUNTS_SERVICE_DIR), entry)
}

/// Same as [`avatar_for`] with an injectable AccountsService directory.
fn avatar_for_in(accounts_dir: &Path, entry: &PasswdEntry) -> Option<PathBuf> {
    let from_home = (!entry.home.as_os_str().is_empty())
        .then(|| avatar_in_home(Path::new(&entry.home), &entry.username))
        .flatten();
    from_home.or_else(|| avatar_in_accounts(accounts_dir, &entry.username))
}

/// Resolves `$HOME/.face` for `username`, ignoring anything that is not a
/// plain readable regular file.
pub fn avatar_in_home(home: &Path, username: &str) -> Option<PathBuf> {
    if !is_safe_username(username) {
        return None;
    }
    let face = home.join(FACE_FILENAME);
    is_regular_readable_file(&face).then_some(face)
}

fn avatar_in_accounts(accounts_dir: &Path, username: &str) -> Option<PathBuf> {
    let icon = accounts_dir.join("icons").join(username);
    if is_regular_readable_file(&icon) {
        return Some(icon);
    }

    let user_file = accounts_dir.join("users").join(username);
    let contents = fs::read_to_string(user_file).ok()?;
    contents.lines().find_map(|line| {
        line.strip_prefix("Icon=")
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .filter(|path| is_regular_readable_file(path))
    })
}

/// Defense-in-depth: usernames coming from `/etc/passwd` must be safe to use
/// as a single path component before being joined into any directory.
fn is_safe_username(username: &str) -> bool {
    !username.is_empty()
        && username.len() <= 32
        && username != "."
        && username != ".."
        && !username.chars().any(|c| c == '/' || c.is_control())
}

fn is_regular_readable_file(path: &Path) -> bool {
    // `symlink_metadata` does not follow symlinks; `is_file()` on its result
    // is true only for real regular files.
    path.symlink_metadata()
        .map(|meta| meta.is_file())
        .unwrap_or(false)
        && fs::File::open(path).is_ok()
}

#[derive(Debug)]
struct PasswdEntry {
    username: String,
    gecos: String,
    uid: u32,
    home: PathBuf,
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
        home: PathBuf::from(fields[5]),
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

    fn temp_home_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "argvus-greeter-home-{}-{}",
            label,
            std::process::id(),
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn running_as_root() -> bool {
        fs::metadata("/proc/self")
            .map(|meta| meta.uid())
            .unwrap_or(0)
            == 0
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
            format!(
                "[User]\nIcon={}\nSystemAccount=false\n",
                referenced.display()
            ),
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

    #[test]
    fn face_file_in_home_is_resolved() {
        let home = temp_home_dir("face-hit");
        let face = home.join(FACE_FILENAME);
        fs::write(&face, b"png-bytes").unwrap();

        assert_eq!(avatar_in_home(&home, "alice"), Some(face));
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn missing_face_file_returns_none() {
        let home = temp_home_dir("face-missing");
        assert_eq!(avatar_in_home(&home, "alice"), None);
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn face_takes_priority_over_accounts_service() {
        let accounts = temp_accounts_dir("face-priority");
        let home = temp_home_dir("face-priority");
        let icon = accounts.join("icons").join("alice");
        fs::write(&icon, b"accounts-png").unwrap();
        let face = home.join(FACE_FILENAME);
        fs::write(&face, b"face-png").unwrap();

        let entry = PasswdEntry {
            username: "alice".to_string(),
            gecos: String::new(),
            uid: 1000,
            home: home.clone(),
            shell: "/bin/bash".to_string(),
        };
        assert_eq!(avatar_for_in(Path::new(&accounts), &entry), Some(face));
        fs::remove_dir_all(accounts).unwrap();
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn accounts_service_is_used_when_no_face_exists() {
        let accounts = temp_accounts_dir("face-fallback");
        let home = temp_home_dir("face-fallback");
        let icon = accounts.join("icons").join("bob");
        fs::write(&icon, b"accounts-png").unwrap();

        let entry = PasswdEntry {
            username: "bob".to_string(),
            gecos: String::new(),
            uid: 1001,
            home: home.clone(),
            shell: "/bin/bash".to_string(),
        };
        assert_eq!(avatar_for_in(Path::new(&accounts), &entry), Some(icon));
        fs::remove_dir_all(accounts).unwrap();
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn symlinked_face_file_is_ignored() {
        let home = temp_home_dir("face-symlink");
        let target = std::env::temp_dir().join(format!("face-target-{}", std::process::id()));
        fs::write(&target, b"secret").unwrap();
        std::os::unix::fs::symlink(&target, home.join(FACE_FILENAME)).unwrap();

        assert_eq!(avatar_in_home(&home, "alice"), None);
        fs::remove_dir_all(home).unwrap();
        fs::remove_file(target).unwrap();
    }

    #[test]
    fn directory_named_face_is_ignored() {
        let home = temp_home_dir("face-dir");
        fs::create_dir_all(home.join(FACE_FILENAME)).unwrap();

        assert_eq!(avatar_in_home(&home, "alice"), None);
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn unreadable_face_file_returns_none() {
        if running_as_root() {
            return;
        }
        let home = temp_home_dir("face-unreadable");
        let face = home.join(FACE_FILENAME);
        fs::write(&face, b"png-bytes").unwrap();
        fs::set_permissions(&face, Permissions::from_mode(0o000)).unwrap();

        assert_eq!(avatar_in_home(&home, "alice"), None);
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn unsafe_usernames_never_reach_the_filesystem() {
        let home = temp_home_dir("face-unsafe");
        fs::write(home.join(FACE_FILENAME), b"png-bytes").unwrap();

        for username in ["", "..", ".", "a/b", "traversal/../x"] {
            assert_eq!(
                avatar_in_home(&home, username),
                None,
                "username {username:?} must be rejected"
            );
        }
        // Even with a `.face` present, nothing was read through a path join.
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn empty_home_directory_field_still_falls_back_to_accounts_service() {
        let accounts = temp_accounts_dir("empty-home-fallback");
        let icon = accounts.join("icons").join("carol");
        fs::write(&icon, b"accounts-png").unwrap();

        let entry = PasswdEntry {
            username: "carol".to_string(),
            gecos: String::new(),
            uid: 1002,
            home: PathBuf::new(),
            shell: "/bin/bash".to_string(),
        };
        assert_eq!(avatar_for_in(Path::new(&accounts), &entry), Some(icon));
        fs::remove_dir_all(accounts).unwrap();
    }
}
