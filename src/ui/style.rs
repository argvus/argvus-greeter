use gtk::gdk;

pub fn load() {
    let Some(display) = gdk::Display::default() else {
        tracing::warn!("no GTK display available for CSS provider");
        return;
    };

    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("../../assets/style.css"));

    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
