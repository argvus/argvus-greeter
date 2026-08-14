# Argvus Greeter

Argvus Greeter is a lightweight GTK4 graphical frontend for
[greetd](https://git.sr.ht/~kennylevinsen/greetd) built for the Argvus Desktop
Environment.

It is designed for Wayland and for Argvus' Hyprland-based desktop, while still
discovering installed Wayland sessions instead of hardcoding a single session.

## Screenshot

Screenshot placeholder.

## Architecture

```text
systemd
   -> greetd
   -> minimal Wayland compositor session
   -> argvus-greeter
   -> selected Wayland desktop session
```

The greeter does not authenticate users itself. It uses the greetd IPC socket
from `GREETD_SOCK`; greetd then delegates authentication to PAM.

## Requirements

- Rust 1.92 or newer
- GTK 4
- greetd
- A Wayland compositor suitable for running the greeter, such as Hyprland
- `argvus-appearance` for the default wallpaper path
- systemd/logind for the power menu

## Building

```sh
cargo build --release
```

The binary is produced at `target/release/argvus-greeter`.

## Installation

Expected package locations:

- `/usr/bin/argvus-greeter`
- `/etc/argvus/greeter.toml`
- `/usr/share/argvus-greeter/`
- `/usr/share/backgrounds/argvus/`
- `/usr/share/wayland-sessions/argvus.desktop`

Arch packaging can install this repository's `packaging/greetd` examples as
documentation or adapt them into package defaults.

Arch packaging is owned by this repository through `packaging/PKGBUILD`. Tag
pushes build and publish signed `.pkg.tar.zst` packages to the shared
`argvus/packages` repository.

## greetd Configuration

See:

- `packaging/greetd/config.toml`
- `packaging/greetd/hyprland-argvus-greeter.lua`

The example starts a dedicated minimal Hyprland instance for the greeter
through `argvus-greeter-session` and does not start the user's normal Argvus
desktop configuration before authentication. The launcher clears VT1 and
redirects compositor output to the greeter user's state directory, or to a
per-UID fallback under `/tmp` when greetd does not provide a writable `HOME`, so
boot or terminal logs do not remain visible under the login screen.
The greeter compositor still sets `XDG_CURRENT_DESKTOP=Hyprland` to satisfy
Hyprland's startup checks, while `XDG_SESSION_DESKTOP=argvus-greeter` identifies
the login environment. When authentication succeeds, the greeter app exits and
the minimal compositor is stopped immediately before greetd hands over to the
real user session.

When installed from the Arch package, apply the Argvus greetd configuration
with:

```sh
sudo argvus-greeter-setup --enable
```

Use `--now` instead of `--enable` to restart `greetd.service` immediately.
The helper backs up an existing `/etc/greetd/config.toml` before replacing it.
If `/etc/greetd/config.toml` still points to `Hyprland --config ...conf`, run
the helper again so greetd uses the packaged `argvus-greeter-session` + Lua
configuration. Older direct commands can leave terminal output visible while
the graphical greeter starts.

## Configuration

System configuration is read from `/etc/argvus/greeter.toml`.

```toml
[appearance]
wallpaper = "/usr/share/backgrounds/argvus/default.png"
show_clock = true
show_date = true

[session]
default = "argvus"
```

If the file does not exist, safe defaults are used.

## Development

Useful checks:

```sh
cargo build --release --locked
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
```

The greeter expects `GREETD_SOCK` to be set by greetd for real authentication.
Without greetd it can still be compiled and inspected, but login cannot proceed.

## Security Notes

- Passwords and other PAM responses are never logged.
- Authentication is handled only through greetd IPC.
- The greeter never reads `/etc/shadow`.
- Session commands come from standard Wayland `.desktop` files and are parsed
  into argv vectors rather than passed through a shell.
- Power operations use systemd/logind over D-Bus.
- User discovery intentionally filters obvious service accounts and avoids
  depending on user home directories.
