# Development

Argvus Greeter is a Rust GTK4 application used as the graphical greetd frontend
for Argvus.

## Requirements

Install Rust and the system libraries used by the GTK4 greeter:

```sh
cargo --version
pkg-config --version
```

On Arch Linux, the runtime/build dependencies are represented by
`packaging/PKGBUILD`.

## Commands

```sh
cargo build --release --locked
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
```

The greeter expects `GREETD_SOCK` to be set by greetd for real authentication.
Without greetd it can still be compiled and inspected, but login cannot proceed.

## Package Contents

The Arch package installs:

```text
/usr/bin/argvus-greeter
/usr/bin/argvus-greeter-setup
/usr/bin/argvus-greeter-session
/etc/argvus/greeter.toml
/etc/argvus/hyprland-argvus-greeter.conf
/etc/argvus/hyprland-argvus-greeter.lua
/usr/share/doc/argvus-greeter/greetd-config.toml
/usr/share/doc/argvus-greeter/README.md
```

The package declares `argvus-appearance` and `argvus-session` as runtime
dependencies because the default wallpaper and session handoff rely on those
component packages. The release workflow installs the official build
dependencies and runs `makepkg --nodeps` so the greeter package can be built
before every Argvus component package is present in the public repository.

## Release Flow

1. Update `Cargo.toml` version.
2. Run tests and clippy.
3. Commit the version change.
4. Tag `vX.Y.Z` and push the tag.
5. Confirm the package workflow builds `argvus-greeter-X.Y.Z-1-x86_64.pkg.tar.zst` and its `.sig`.
6. Confirm the workflow publishes both files to `argvus/packages` under `public/arch/x86_64/` and updates the Arch repository database.

The project does not create GitHub Releases for package distribution. The built
`.pkg.tar.zst` and `.sig` are kept as GitHub Actions artifacts for one day only;
the permanent package copies live in `argvus/packages`.
