//! Runtime composition for the terminal application.
//!
//! The application loop is intentionally separate from [`crate::ui::login`]:
//! this module owns terminal lifetime and event polling, while the UI owns
//! presentation state and user interaction.

use crate::{config::Config, greetd, session::Session, ui, users::User};
use argvus_tui::terminal::{TerminalGuard, install_panic_hook};
use crossterm::event::{self, Event};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

pub fn run(
  config: Config,
  users: Vec<User>,
  sessions: Vec<Session>,
  i18n: Arc<argvus_i18n::I18n>,
) -> anyhow::Result<()> {
  // Separate channels keep terminal input synchronous while authentication
  // continues in the greetd worker thread.
  let (command_tx, command_rx) = mpsc::channel();
  let (event_tx, event_rx) = mpsc::channel();
  // The worker is started before the first frame so greetd errors can be
  // reported as soon as the login screen becomes interactive.
  greetd::spawn_worker(command_rx, event_tx);

  // TerminalGuard restores the user's terminal even when rendering or event
  // handling returns an error. The panic hook provides the same guarantee for
  // unexpected failures.
  install_panic_hook();
  let mut terminal = TerminalGuard::new()?;
  let mut app = ui::login::LoginApp::new(config, users, sessions, i18n, command_tx, event_rx);
  loop {
    // Drawing happens before input polling so every state transition is
    // visible without requiring a second key press.
    terminal.terminal_mut().draw(|frame| app.draw(frame))?;
    if app.quit {
      break;
    }
    // A bounded poll keeps the UI responsive while allowing authentication
    // events to be consumed regularly when no key is pressed.
    if event::poll(Duration::from_millis(100))?
      && let Event::Key(key) = event::read()?
    {
      app.handle_key(key);
    }
    app.poll_events();
  }
  Ok(())
}
