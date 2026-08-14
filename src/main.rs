mod app;
mod config;
mod greetd;
mod power;
mod session;
mod ui;
mod users;

use anyhow::Context;
use tracing_subscriber::{EnvFilter, fmt};

fn main() -> anyhow::Result<()> {
    init_logging();

    let config = config::Config::load().context("failed to load greeter configuration")?;
    let users = users::discover_users().context("failed to discover local users")?;
    let sessions = session::discover_sessions(&config.session.default)
        .context("failed to discover graphical sessions")?;

    app::run(config, users, sessions);
    Ok(())
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("argvus_greeter=info,warn"));

    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}
