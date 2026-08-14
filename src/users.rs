use anyhow::Context;
use std::{fs, path::PathBuf};

const PASSWD_PATH: &str = "/etc/passwd";
const MIN_LOGIN_UID: u32 = 1000;

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

fn avatar_for(username: &str) -> Option<PathBuf> {
    let accounts_icon = PathBuf::from(format!("/var/lib/AccountsService/icons/{username}"));
    if accounts_icon.is_file() {
        return Some(accounts_icon);
    }

    let accounts_user = PathBuf::from(format!("/var/lib/AccountsService/users/{username}"));
    let contents = fs::read_to_string(accounts_user).ok()?;
    contents.lines().find_map(|line| {
        line.strip_prefix("Icon=")
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .filter(|path| path.is_file())
    })
}
