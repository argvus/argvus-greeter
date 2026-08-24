use crate::{
    config::Config,
    greetd::{AuthPrompt, Command, Event},
    power::{self, PowerAction},
    session::Session,
    users::User,
};
use gtk::{gdk, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
    sync::mpsc::{Receiver, Sender},
    time::Duration,
};

const DEFAULT_AVATAR: &[u8] = include_bytes!("../../assets/avatar-default.svg");

pub struct LoginView {
    window: gtk::ApplicationWindow,
}

impl LoginView {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        app: &gtk::Application,
        config: Config,
        users: Vec<User>,
        sessions: Vec<Session>,
        commands: Sender<Command>,
        events: Receiver<Event>,
    ) -> Self {
        let widgets = Widgets::new(app, &config, &users, &sessions);
        let state = Rc::new(State {
            users,
            sessions,
            commands,
            events: RefCell::new(events),
            waiting_for_prompt: Cell::new(false),
            active_prompt: RefCell::new(None),
            pending_secret_response: RefCell::new(None),
        });

        wire_clock(&widgets, &config);
        wire_user_selection(&widgets, state.clone());
        wire_login(&widgets, state.clone());
        poll_events(&widgets, state);

        Self {
            window: widgets.window,
        }
    }

    pub fn present(&self) {
        self.window.present();
    }
}

struct State {
    users: Vec<User>,
    sessions: Vec<Session>,
    commands: Sender<Command>,
    events: RefCell<Receiver<Event>>,
    waiting_for_prompt: Cell<bool>,
    active_prompt: RefCell<Option<AuthPrompt>>,
    pending_secret_response: RefCell<Option<String>>,
}

#[derive(Clone)]
struct Widgets {
    window: gtk::ApplicationWindow,
    user_dropdown: gtk::DropDown,
    session_dropdown: gtk::DropDown,
    avatar: gtk::Picture,
    default_avatar: Option<gdk::Texture>,
    display_name: gtk::Label,
    prompt_label: gtk::Label,
    auth_entry: gtk::Entry,
    submit_button: gtk::Button,
    status_label: gtk::Label,
    clock_label: gtk::Label,
    date_label: gtk::Label,
}

impl Widgets {
    fn new(app: &gtk::Application, config: &Config, users: &[User], sessions: &[Session]) -> Self {
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("Argvus Greeter")
            .default_width(1280)
            .default_height(720)
            .build();
        window.fullscreen();

        let overlay = gtk::Overlay::new();
        window.set_child(Some(&overlay));

        let background = if config.appearance.wallpaper.is_file() {
            gtk::Picture::for_filename(&config.appearance.wallpaper)
        } else {
            gtk::Picture::new()
        };
        background.set_can_shrink(false);
        background.set_content_fit(gtk::ContentFit::Cover);
        overlay.set_child(Some(&background));

        let tint = gtk::Box::new(gtk::Orientation::Vertical, 0);
        tint.add_css_class("background-tint");
        overlay.add_overlay(&tint);

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("screen");
        overlay.add_overlay(&root);

        let top = gtk::Box::new(gtk::Orientation::Vertical, 2);
        top.set_halign(gtk::Align::Center);
        top.set_margin_top(52);
        top.add_css_class("clock");

        let clock_label = gtk::Label::new(None);
        clock_label.add_css_class("clock-time");
        let date_label = gtk::Label::new(None);
        date_label.add_css_class("clock-date");
        top.append(&clock_label);
        top.append(&date_label);
        root.append(&top);

        let center = gtk::CenterBox::new();
        center.set_vexpand(true);
        root.append(&center);

        let form = gtk::Box::new(gtk::Orientation::Vertical, 16);
        form.set_width_request(340);
        form.set_halign(gtk::Align::Center);
        form.set_valign(gtk::Align::Center);
        form.add_css_class("login-panel");
        center.set_center_widget(Some(&form));

        let avatar = gtk::Picture::new();
        avatar.set_size_request(96, 96);
        avatar.set_content_fit(gtk::ContentFit::Cover);
        avatar.set_halign(gtk::Align::Center);
        avatar.add_css_class("avatar");
        form.append(&avatar);
        let default_avatar = load_default_avatar();

        let display_name = gtk::Label::new(None);
        display_name.add_css_class("display-name");
        form.append(&display_name);

        let user_names = users
            .iter()
            .map(|user| user.display_name.as_str())
            .collect::<Vec<_>>();
        let user_dropdown = gtk::DropDown::from_strings(&user_names);
        user_dropdown.set_hexpand(true);
        user_dropdown.add_css_class("compact-select");
        form.append(&user_dropdown);

        let prompt_label = gtk::Label::new(Some("Password"));
        prompt_label.add_css_class("prompt-label");
        prompt_label.set_halign(gtk::Align::Start);
        form.append(&prompt_label);

        let auth_entry = gtk::Entry::new();
        auth_entry.set_visibility(false);
        auth_entry.set_placeholder_text(Some("Password"));
        auth_entry.set_activates_default(true);
        form.append(&auth_entry);

        let submit_button = gtk::Button::with_label("Log In");
        submit_button.add_css_class("suggested-action");
        submit_button.set_receives_default(true);
        form.append(&submit_button);
        window.set_default_widget(Some(&submit_button));

        let status_label = gtk::Label::new(None);
        status_label.set_wrap(true);
        status_label.add_css_class("status");
        form.append(&status_label);

        let bottom = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        bottom.set_margin_bottom(36);
        bottom.set_margin_start(36);
        bottom.set_margin_end(36);
        bottom.add_css_class("bottom-bar");
        root.append(&bottom);

        let session_names = sessions
            .iter()
            .map(|session| session.name.as_str())
            .collect::<Vec<_>>();
        let session_dropdown = gtk::DropDown::from_strings(&session_names);
        let default_session = sessions
            .iter()
            .position(|session| session.is_default)
            .unwrap_or(0);
        session_dropdown.set_selected(default_session as u32);
        session_dropdown.add_css_class("session-select");
        bottom.append(&session_dropdown);

        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        bottom.append(&spacer);

        let power_button = power_menu();
        bottom.append(&power_button);

        let widgets = Self {
            window,
            user_dropdown,
            session_dropdown,
            avatar,
            default_avatar,
            display_name,
            prompt_label,
            auth_entry,
            submit_button,
            status_label,
            clock_label,
            date_label,
        };
        update_selected_user(&widgets, users);
        update_empty_state(&widgets, users, sessions);
        widgets
    }
}

fn wire_clock(widgets: &Widgets, config: &Config) {
    if !config.appearance.show_clock {
        widgets.clock_label.set_visible(false);
        widgets.date_label.set_visible(false);
        return;
    }

    widgets.date_label.set_visible(config.appearance.show_date);
    update_clock(widgets);

    let widgets = widgets.clone();
    glib::timeout_add_local(Duration::from_secs(1), move || {
        update_clock(&widgets);
        glib::ControlFlow::Continue
    });
}

fn update_clock(widgets: &Widgets) {
    match glib::DateTime::now_local() {
        Ok(now) => {
            if let Ok(time) = now.format("%H:%M") {
                widgets.clock_label.set_text(&time);
            }
            if let Ok(date) = now.format("%a, %d %B") {
                widgets.date_label.set_text(&date);
            }
        }
        Err(error) => tracing::warn!(%error, "could not read local time"),
    }
}

fn wire_user_selection(widgets: &Widgets, state: Rc<State>) {
    let widgets = widgets.clone();
    widgets
        .user_dropdown
        .clone()
        .connect_selected_notify(move |_| {
            update_selected_user(&widgets, &state.users);
        });
}

fn wire_login(widgets: &Widgets, state: Rc<State>) {
    let submit_widgets = widgets.clone();
    let submit_state = state.clone();
    widgets.submit_button.connect_clicked(move |_| {
        submit(&submit_widgets, &submit_state);
    });

    let entry_widgets = widgets.clone();
    widgets.auth_entry.connect_activate(move |_| {
        submit(&entry_widgets, &state);
    });
}

fn submit(widgets: &Widgets, state: &State) {
    if state.waiting_for_prompt.get() {
        let response = widgets.auth_entry.text().to_string();
        widgets.auth_entry.set_text("");
        widgets.submit_button.set_sensitive(false);
        widgets.status_label.set_text("");
        state.waiting_for_prompt.set(false);

        if state
            .commands
            .send(Command::AuthResponse(Some(response)))
            .is_err()
        {
            widgets
                .status_label
                .set_text("Internal greeter channel is unavailable.");
        }
        return;
    }

    let Some(user) = selected_user(widgets, &state.users) else {
        widgets.status_label.set_text("No login user is available.");
        return;
    };
    let Some(session) = selected_session(widgets, &state.sessions) else {
        widgets
            .status_label
            .set_text("No Wayland session is available.");
        return;
    };

    let initial_response = widgets.auth_entry.text().to_string();
    widgets.auth_entry.set_text("");
    widgets.auth_entry.set_sensitive(false);
    widgets.submit_button.set_sensitive(false);
    widgets.status_label.set_text("Starting authentication...");
    if initial_response.is_empty() {
        state.pending_secret_response.replace(None);
    } else {
        state
            .pending_secret_response
            .replace(Some(initial_response));
    }
    tracing::info!(user = %user.username, session = %session.id, "selected session");

    if state
        .commands
        .send(Command::Begin {
            username: user.username.clone(),
            session: session.clone(),
        })
        .is_err()
    {
        widgets
            .status_label
            .set_text("Internal greeter channel is unavailable.");
    }
}

fn poll_events(widgets: &Widgets, state: Rc<State>) {
    let widgets = widgets.clone();
    glib::timeout_add_local(Duration::from_millis(50), move || {
        while let Ok(event) = state.events.borrow_mut().try_recv() {
            handle_event(&widgets, &state, event);
        }
        glib::ControlFlow::Continue
    });
}

fn handle_event(widgets: &Widgets, state: &State, event: Event) {
    match event {
        Event::Ready => {}
        Event::AuthMessage(prompt) => {
            if prompt.secret
                && let Some(response) = state.pending_secret_response.borrow_mut().take()
            {
                if state
                    .commands
                    .send(Command::AuthResponse(Some(response)))
                    .is_err()
                {
                    widgets
                        .status_label
                        .set_text("Internal greeter channel is unavailable.");
                }
                return;
            }

            widgets.auth_entry.set_sensitive(true);
            widgets.auth_entry.set_visibility(!prompt.secret);
            widgets
                .auth_entry
                .set_placeholder_text(Some(&prompt.message));
            widgets.prompt_label.set_text(&prompt.message);
            widgets.submit_button.set_sensitive(true);
            widgets.status_label.set_text("");
            widgets.auth_entry.grab_focus();
            state.waiting_for_prompt.set(true);
            state.active_prompt.replace(Some(prompt));
        }
        Event::Info(message) => widgets.status_label.set_text(&message),
        Event::Error(message) => {
            widgets.auth_entry.set_text("");
            widgets.auth_entry.set_sensitive(true);
            widgets.submit_button.set_sensitive(true);
            widgets.status_label.set_text(&message);
            state.waiting_for_prompt.set(false);
            state.active_prompt.replace(None);
            state.pending_secret_response.replace(None);
        }
        Event::AuthFailed(message) => {
            widgets.auth_entry.set_text("");
            widgets.auth_entry.set_sensitive(true);
            widgets.submit_button.set_sensitive(true);
            widgets.status_label.set_text(if message.is_empty() {
                "Authentication failed."
            } else {
                &message
            });
            state.waiting_for_prompt.set(false);
            state.active_prompt.replace(None);
            state.pending_secret_response.replace(None);
            widgets.auth_entry.grab_focus();
        }
        Event::SessionStarting => {
            widgets.status_label.set_text("Starting session...");
            widgets.auth_entry.set_sensitive(false);
            widgets.submit_button.set_sensitive(false);
        }
        Event::SessionStarted => {
            widgets.status_label.set_text("Session started.");
            let window = widgets.window.clone();
            if let Some(app) = window.application() {
                app.quit();
            } else {
                window.close();
            }
        }
    }
}

fn selected_user<'a>(widgets: &Widgets, users: &'a [User]) -> Option<&'a User> {
    users.get(widgets.user_dropdown.selected() as usize)
}

fn selected_session<'a>(widgets: &Widgets, sessions: &'a [Session]) -> Option<&'a Session> {
    sessions.get(widgets.session_dropdown.selected() as usize)
}

fn update_selected_user(widgets: &Widgets, users: &[User]) {
    let Some(user) = selected_user(widgets, users) else {
        widgets.display_name.set_text("Argvus");
        apply_avatar(widgets, None);
        return;
    };

    widgets.display_name.set_text(&user.display_name);
    apply_avatar(widgets, user.avatar.as_deref());
}

fn apply_avatar(widgets: &Widgets, path: Option<&Path>) {
    let texture = path
        .and_then(load_user_avatar)
        .or_else(|| widgets.default_avatar.clone());

    if let Some(texture) = texture {
        widgets.avatar.remove_css_class("avatar-fallback");
        widgets.avatar.set_paintable(Some(&texture));
    } else {
        widgets.avatar.add_css_class("avatar-fallback");
    }
}

fn load_user_avatar(path: &Path) -> Option<gdk::Texture> {
    match gdk::Texture::from_filename(path) {
        Ok(texture) => Some(texture),
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "user avatar could not be loaded");
            None
        }
    }
}

fn load_default_avatar() -> Option<gdk::Texture> {
    match gdk::Texture::from_bytes(&glib::Bytes::from_static(DEFAULT_AVATAR)) {
        Ok(texture) => Some(texture),
        Err(error) => {
            tracing::warn!(%error, "default avatar asset could not be decoded");
            None
        }
    }
}

fn update_empty_state(widgets: &Widgets, users: &[User], sessions: &[Session]) {
    if users.is_empty() || sessions.is_empty() {
        widgets.auth_entry.set_sensitive(false);
        widgets.submit_button.set_sensitive(false);
        widgets.status_label.set_text(if users.is_empty() {
            "No local login users were found."
        } else {
            "No Wayland sessions were found."
        });
    }
}

fn power_menu() -> gtk::MenuButton {
    let menu = gtk::MenuButton::builder()
        .icon_name("system-shutdown-symbolic")
        .tooltip_text("Power")
        .build();
    menu.add_css_class("power-button");

    let popover = gtk::Popover::new();
    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 4);
    box_.set_margin_top(8);
    box_.set_margin_bottom(8);
    box_.set_margin_start(8);
    box_.set_margin_end(8);

    for action in [
        PowerAction::Shutdown,
        PowerAction::Restart,
        PowerAction::Suspend,
    ] {
        let button = gtk::Button::with_label(action.label());
        button.set_halign(gtk::Align::Fill);
        button.connect_clicked(move |_| {
            if let Err(error) = power::request(action) {
                tracing::warn!(%error, "power action failed");
            }
        });
        box_.append(&button);
    }

    popover.set_child(Some(&box_));
    menu.set_popover(Some(&popover));
    menu
}
