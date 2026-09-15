use anyhow::Result;
use argvus_i18n::I18n;
use std::sync::Arc;

pub fn load() -> Result<Arc<I18n>> {
    Ok(Arc::new(I18n::new("greeter")?))
}

pub fn auth_message(i18n: &I18n, message: &str) -> String {
    let normalized = message
        .trim()
        .trim_end_matches([':', '.'])
        .trim()
        .to_lowercase();

    let key = match normalized.as_str() {
        "password" => Some("password"),
        "authentication failed" | "login incorrect" => Some("authentication_failed"),
        "no active authentication prompt" => Some("no_active_auth_prompt"),
        "greetd_sock is not set" => Some("greetd_socket_missing"),
        "greetd requested authentication after startsession" => {
            Some("authentication_requested_after_session")
        }
        _ => None,
    };

    key.map_or_else(|| message.to_owned(), |key| i18n.tr(key))
}
