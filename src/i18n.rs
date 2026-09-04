//! Minimal greeter localization.
//!
//! ARGVUS only needs Portuguese and English here. The selected language follows
//! the login environment locale, falling back to English.

use std::borrow::Cow;

pub fn is_pt() -> bool {
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"]
        .iter()
        .find_map(|key| std::env::var(key).ok().filter(|value| !value.is_empty()));

    locale
        .map(|value| locale_value_is_pt(&value))
        .unwrap_or(false)
}

pub fn label<'a>(pt_text: &'a str, en_text: &'a str) -> &'a str {
    if is_pt() { pt_text } else { en_text }
}

fn locale_value_is_pt(value: &str) -> bool {
    value
        .split(':')
        .any(|part| part.trim().to_lowercase().starts_with("pt"))
}

pub fn auth_message(message: &str) -> Cow<'_, str> {
    if !is_pt() {
        return Cow::Borrowed(message);
    }

    translate_auth_message_pt(message)
}

fn translate_auth_message_pt(message: &str) -> Cow<'_, str> {
    let normalized = message
        .trim()
        .trim_end_matches([':', '.'])
        .trim()
        .to_lowercase();

    match normalized.as_str() {
        "password" => Cow::Borrowed("Senha"),
        "authentication failed" | "login incorrect" => Cow::Borrowed("Autenticação falhou."),
        "no active authentication prompt" => {
            Cow::Borrowed("Nenhuma solicitação de autenticação ativa.")
        }
        "greetd_sock is not set" => Cow::Borrowed("GREETD_SOCK não está definido."),
        "greetd requested authentication after startsession" => {
            Cow::Borrowed("O greetd solicitou autenticação depois de iniciar a sessão.")
        }
        _ => Cow::Borrowed(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_portuguese_locale_values() {
        assert!(locale_value_is_pt("pt_BR.UTF-8"));
        assert!(locale_value_is_pt("en_US.UTF-8:pt_BR.UTF-8"));
        assert!(!locale_value_is_pt("en_US.UTF-8"));
    }

    #[test]
    fn auth_message_translation_is_conservative() {
        assert_eq!(translate_auth_message_pt("Password:").as_ref(), "Senha");
        assert_eq!(
            translate_auth_message_pt("Login incorrect").as_ref(),
            "Autenticação falhou."
        );
        assert_eq!(
            translate_auth_message_pt("Custom PAM text").as_ref(),
            "Custom PAM text"
        );
    }
}
