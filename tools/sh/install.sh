#!/usr/bin/env sh
# Install or uninstall argvus-greeter directly on this machine, bypassing the
# Arch package flow (PKGBUILD/makepkg). Intended for development machines.
#
# Usage:
#   tools/sh/install.sh install
#   tools/sh/install.sh uninstall
#
# The release binary is built as the invoking user; sudo is only requested
# when the destination directories are not writable by the current user.
# Overridable environment variables: PREFIX (/usr), CONFIG_DIR (/etc/argvus).
# Installing to non-default locations never restarts greetd.

set -eu

# Keep installation paths overridable for development/staging while retaining
# the distribution defaults used by the Arch package.
PREFIX="${PREFIX:-/usr}"
CONFIG_DIR="${CONFIG_DIR:-/etc/argvus}"
BIN_DIR="$PREFIX/bin"
DOC_DIR="$PREFIX/share/doc/argvus-greeter"
LICENSE_DIR="$PREFIX/share/licenses/argvus-greeter"

log() {
	printf '%s\n' "$*"
}

usage() {
	printf 'usage: %s [install|uninstall]\n' "$0" >&2
	exit 2
}

build() {
	# Build as the invoking user so Cargo caches and generated artifacts retain
	# the correct ownership before any privileged install step.
	log "==> Building release binary"
	cargo build --release --locked
}

install_files() {
	# Binaries and wrappers are installed together because greetd invokes the
	# session wrapper, which then invokes the TUI terminal launcher.
	log "==> Installing binaries to $BIN_DIR"
	install -Dm755 target/release/argvus-greeter "$BIN_DIR/argvus-greeter"
	install -Dm755 packaging/greetd/argvus-greeter-setup "$BIN_DIR/argvus-greeter-setup"
	install -Dm755 packaging/greetd/argvus-greeter-session "$BIN_DIR/argvus-greeter-session"
	install -Dm755 packaging/greetd/argvus-greeter-tui "$BIN_DIR/argvus-greeter-tui"

	log "==> Installing configuration to $CONFIG_DIR (existing files are overwritten)"
	for file in greeter.toml hyprland-argvus-greeter.conf hyprland-argvus-greeter.lua; do
		if [ -e "$CONFIG_DIR/$file" ]; then
			log "    overwriting existing $CONFIG_DIR/$file"
		fi
		install -Dm644 "packaging/greetd/$file" "$CONFIG_DIR/$file"
	done

	log "==> Installing documentation and license"
	install -Dm644 packaging/greetd/config.toml "$DOC_DIR/greetd-config.toml"
	install -Dm644 README.md "$DOC_DIR/README.md"
	install -Dm644 LICENSE "$LICENSE_DIR/LICENSE"
}

remove_files() {
	# Removal is limited to paths owned by this project; unrelated greetd state
	# and user configuration are intentionally left untouched.
	log "==> Removing binaries"
	rm -f "$BIN_DIR/argvus-greeter"
	rm -f "$BIN_DIR/argvus-greeter-setup"
	rm -f "$BIN_DIR/argvus-greeter-session"
	rm -f "$BIN_DIR/argvus-greeter-tui"

	log "==> Removing configuration"
	for file in greeter.toml hyprland-argvus-greeter.conf hyprland-argvus-greeter.lua; do
		rm -f "$CONFIG_DIR/$file"
	done
	rmdir --ignore-fail-on-non-empty "$CONFIG_DIR" 2>/dev/null || true

	log "==> Removing documentation and license"
	rm -f "$DOC_DIR/greetd-config.toml"
	rm -f "$DOC_DIR/README.md"
	rmdir --ignore-fail-on-non-empty "$DOC_DIR" 2>/dev/null || true
	rm -f "$LICENSE_DIR/LICENSE"
	rmdir --ignore-fail-on-non-empty "$LICENSE_DIR" 2>/dev/null || true
}

reset_greetd() {
	# Development installs restart only the default system integration. Custom
	# prefixes are safe to inspect without affecting the running greeter.
	if [ "$PREFIX" != "/usr" ] || [ "$CONFIG_DIR" != "/etc/argvus" ]; then
		log "==> Non-default install location; greetd was left untouched"
		return
	fi

	if command -v systemctl >/dev/null 2>&1 &&
		systemctl is-enabled --quiet greetd.service 2>/dev/null; then
		log "==> Restarting greetd.service"
		systemctl restart greetd.service
		log "    login screen now runs the freshly installed build"
	else
		log "==> greetd.service is not enabled; start it manually to see the new build"
	fi
}

writable_ancestor() {
	# Find the nearest existing directory because install targets may not exist
	# yet on a fresh development machine.
	dir=$1
	while [ ! -d "$dir" ]; do
		dir=$(dirname "$dir")
	done
	[ -w "$dir" ]
}

needs_sudo() {
	# Check every destination before escalating so the privileged phase is
	# deterministic and does not partially install files.
	[ "$(id -u)" -eq 0 ] && return 1
	for dir in "$BIN_DIR" "$CONFIG_DIR" "$DOC_DIR" "$LICENSE_DIR"; do
		if ! writable_ancestor "$dir"; then
			return 0
		fi
	done
	return 1
}

perform() {
	# Keep the command dispatch separate from privilege detection for easier
	# review of the destructive uninstall path.
	if [ "$1" = "install" ]; then
		install_files
		reset_greetd
	else
		remove_files
	fi
}

main() {
	# Parse the small command surface explicitly; --privileged is internal to the
	# script's own sudo re-exec and is not advertised as a user operation.
	cmd=
	privileged=0
	for arg in "$@"; do
		case "$arg" in
		install | uninstall) cmd=$arg ;;
		--privileged) privileged=1 ;;
		*) usage ;;
		esac
	done
	[ -n "$cmd" ] || usage

	if [ "$privileged" -eq 0 ]; then
		if [ "$cmd" = "install" ]; then
			build
		fi
		if needs_sudo; then
			log "==> Destinations are not writable by $(id -un); escalating with sudo"
			exec sudo env PREFIX="$PREFIX" CONFIG_DIR="$CONFIG_DIR" "$0" --privileged "$cmd"
		fi
	fi

	perform "$cmd"
	log "==> Done."
}

main "$@"
