use crate::{config::Config, greetd, session::Session, ui, users::User};
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

pub fn run(config: Config, users: Vec<User>, sessions: Vec<Session>) {
    let (command_tx, command_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();
    greetd::spawn_worker(command_rx, event_tx);
    let event_rx = Rc::new(RefCell::new(Some(event_rx)));

    let app = gtk::Application::builder()
        .application_id("sh.argvus.Greeter")
        .build();

    app.connect_activate(move |app| {
        let Some(event_rx) = event_rx.borrow_mut().take() else {
            return;
        };

        ui::style::load();
        ui::login::LoginView::new(
            app,
            config.clone(),
            users.clone(),
            sessions.clone(),
            command_tx.clone(),
            event_rx,
        )
        .present();
    });

    app.run();
}
