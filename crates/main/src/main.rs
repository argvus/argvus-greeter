//! Process entry point for the Argvus greetd frontend.
//!
//! This module deliberately keeps startup orchestration small. Discovery and
//! policy live in dedicated modules so the binary can be audited as a login
//! component without mixing terminal rendering, authentication, and system
//! configuration concerns.

mod app;
mod config;
mod greetd;
mod i18n;
mod power;
mod session;
mod ui;
mod users;

use anyhow::Context;
use tracing_subscriber::{EnvFilter, fmt};

fn main() -> anyhow::Result<()> {
  // Logging is initialized before any discovery work so configuration and
  // environment failures remain visible when the greeter runs under greetd.
  init_logging();

  // These objects are collected once at startup. The login UI then owns the
  // immutable discovery snapshot while only the selected indices change.
  let config = config::Config::load().context("failed to load greeter configuration")?;
  let users = users::discover_users().context("failed to discover local users")?;
  let sessions = session::discover_sessions(&config.session.default)
    .context("failed to discover graphical sessions")?;
  let i18n = i18n::load().context("failed to load greeter translations")?;

  app::run(config, users, sessions, i18n)
}

fn init_logging() {
  // Respect RUST_LOG when operators need diagnostics, while keeping normal
  // login screens quiet by default.
  let filter = EnvFilter::try_from_default_env()
    .unwrap_or_else(|_| EnvFilter::new("argvus_greeter=info,warn"));

  fmt()
    .with_env_filter(filter)
    .with_target(false)
    .compact()
    .init();
}
