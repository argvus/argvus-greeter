---
title: Greeter
description: Configure the ARGVUS greetd login interface.
---

`argvus-greeter` is the keyboard-first TUI greeter used by the greetd login service. Authentication is performed by greetd through PAM; the greeter presents account and session selection but does not implement password authentication itself.

Its setup helper is `argvus-greeter-setup`. On an installed system, use:

```sh
sudo argvus-greeter-setup --enable
```

Use `sudo argvus-greeter-setup --now` when greetd should be restarted immediately. The helper backs up an existing `/etc/greetd/config.toml` before installing the ARGVUS configuration. Use `argvus-greeter-setup --help` to inspect installed options.

The greeter reads local account metadata from `argvus-accounts`, authenticates through greetd and hands the selected user context to the session. Account avatars are resolved from the user's `.face` file, the AccountsService icon location or its declared `Icon=` entry. `argvus-accounts` can create a validated `.face` image; the greeter itself does not edit avatars.

The system greeter configuration is `/etc/argvus/greeter.toml`. To refresh the public theme projections used before authentication without changing greetd configuration, run:

```sh
sudo argvus-greeter-setup --sync-themes
```

Before authentication, the greeter does not read the selected user's private home directory. Theme selection through ARGVUS appearance publishes the validated theme to `/var/lib/argvus/greeter/themes/<uid>` as an atomic, UID-owned projection. `argvus-greeter-setup --sync-themes` remains the administrative resynchronization command. The login TUI and the post-login handoff use that projection, including the optional accent in `<uid>.accent`; `.active-theme` and `.accent-color` remain compatibility fallbacks. If the selected theme is unavailable, the greeter uses the deterministic packaged `argvus-dark` fallback rather than failing.

The administrative `/etc/argvus/greeter.toml` independently controls the
pre-login surface. Under `[appearance]`, use
`transparency_enabled`, `transparency_value`, `blur_enabled`, and `blur_value`:

```toml
[appearance]
transparency_enabled = true
transparency_value = 50
blur_enabled = true
blur_value = 50
```

Percentage values are clamped to `0..=100`. The typed Rust loader passes the
normalized values to Kitty and the minimal Hyprland compositor. The user's
`~/.config/argvus/config.json` is not read before authentication. Blur is only
visually apparent when content exists behind the Kitty surface.

Greeter, session-loading overlay and boot splash are separate components. See the developer [startup subsystem](../../developer-guide/subsystems/greeter-lock-and-splash/).
