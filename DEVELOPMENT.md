# Development

Argvus Greeter is a Rust Ratatui application used as the TUI greetd frontend
for Argvus.

## Requirements

Install Rust and the terminal/runtime dependencies used by the TUI greeter:

```sh
cargo --version
pkg-config --version
```

On Arch Linux, the runtime/build dependencies are represented by
`packaging/arch/ci/PKGBUILD` and `packaging/arch/local/PKGBUILD`.

## Commands

```sh
make check
make package
```

The greeter expects `GREETD_SOCK` to be set by greetd for real authentication.
Without greetd it can still be compiled and inspected, but login cannot proceed.

## Testing

### TUI checks without greetd

```sh
GREETD_SOCK=/dev/null cargo run --release
```

This opens the greeter in the current terminal. Verify user discovery, avatars,
session list, clock, focus traversal and semantic TUI styling. Authentication
cannot proceed because there is no real greetd socket; submit attempts only show
an error in the status line.

CSS and image assets are compiled into the binary (`include_str!` /
`include_bytes!`), so visual changes always require rebuilding before they
show up. User accounts and their avatars are discovered once at startup, so a
photo changed under `/var/lib/AccountsService/icons/` is reflected by quitting
and rerunning the command.

### Full authentication flow with greetd on a spare VT

A second greetd instance can run manually against a throwaway configuration,
without rebooting, logging out or touching `/etc/greetd/config.toml`:

```sh
cat > /tmp/greetd-test.toml <<'EOF'
[terminal]
vt = 3

[default_session]
command = "argvus-greeter-session"
user = "greeter"
EOF

sudo greetd --config /tmp/greetd-test.toml &
echo $! # keep the PID to stop it later
```

Switch to the configured VT with `Ctrl+Alt+F3` and back with `Ctrl+Alt+F1`.
This exercises the real greetd IPC path and PAM authentication; a successful
login starts the chosen Wayland session on that VT while the desktop session
stays untouched on its own VT. Stop the test instance afterwards:

```sh
sudo kill <PID>
```

Notes:

- Pick a VT that is not in use; `sudo cat /sys/class/tty/tty0/active` shows
  the active one.
- `command = "argvus-greeter-session"` runs the installed script and greeter
  binary. To exercise a local build instead, shadow the packaged one and
  remove it after testing:
  `sudo install -Dm755 target/release/argvus-greeter /usr/local/bin/`
- Greeter session logs are written to
  `$XDG_RUNTIME_DIR/argvus-greeter/session.log`.
- Rust authentication and discovery logs are written separately to
  `$XDG_RUNTIME_DIR/argvus-greeter/greeter.log`; they are never emitted to the
  Kitty terminal surface used by the TUI.

## Package Contents

The Arch package installs:

```text
/usr/bin/argvus-greeter
/usr/bin/argvus-greeter-tui
/usr/bin/argvus-greeter-setup
/usr/bin/argvus-greeter-session
/usr/lib/argvus/greeter/handoff
/usr/lib/tmpfiles.d/argvus-greeter.conf
/etc/argvus/greeter.toml
/etc/argvus/hyprland-argvus-greeter.conf
/etc/argvus/hyprland-argvus-greeter.lua
/usr/share/doc/argvus-greeter/greetd-config.toml
/usr/share/doc/argvus-greeter/README.md
/var/lib/argvus/greeter/themes/<uid>
/var/lib/argvus/greeter/themes/<uid>.accent
```

The per-UID theme files contain only validated official theme identifiers. The
matching `.accent` files contain only validated opaque `#RRGGBB` colors. Both
are updated by `argvus-appearance` and initialized for existing users by
`argvus-greeter-setup --sync-themes`. The greeter verifies file ownership
against the selected account before reading either projection.

The package declares `argvus-appearance` and `argvus-session` as runtime
dependencies because the official TUI themes and session handoff rely on those
component packages. The release workflow installs the official build
dependencies and runs `makepkg --nodeps` so the greeter package can be built
before every Argvus component package is present in the public repository.

## Release Flow

1. Update the workspace version in `Cargo.toml`.
2. Run tests and clippy.
3. Commit the version change.
4. Tag `vX.Y.Z` and push the tag.
5. Confirm the package workflow builds `argvus-greeter-X.Y.Z-1-x86_64.pkg.tar.zst` and its `.sig`.
6. Confirm the workflow publishes both files to `argvus/packages` under `public/arch/x86_64/` and updates the Arch repository database.

The project does not create GitHub Releases for package distribution. The built
`.pkg.tar.zst` and `.sig` are kept as GitHub Actions artifacts for one day only;
the permanent package copies live in `argvus/packages`.
