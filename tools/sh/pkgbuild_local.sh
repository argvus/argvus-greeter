#!/usr/bin/env bash
set -euo pipefail

# Resolve all paths from the checkout so this helper can be invoked from any
# working directory without changing the package's source layout.
ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"

PACKAGING_DIR="$ROOT_DIR/packaging/arch/local"

BUILD_SCRIPT="$PACKAGING_DIR/PKGBUILD"
# Read package metadata through the PKGBUILD itself instead of duplicating the
# package name/version in this helper.
metadata="$({ cd "$PACKAGING_DIR" && bash -c 'source "$1"; printf "%s\n%s\n" "$pkgname" "$pkgver"' bash "$BUILD_SCRIPT"; })"
pkgname="$(printf '%s\n' "$metadata" | sed -n '1p')"
pkgver="$(printf '%s\n' "$metadata" | sed -n '2p')"
BUILD_DIR="$ROOT_DIR/build"
ARTIFACTS_DIR="$BUILD_DIR/artifacts"
DIST_DIR="$BUILD_DIR/dist"
CARGO_TARGET_DIR="$BUILD_DIR/cargo-target"
archive="$ARTIFACTS_DIR/${pkgname}-${pkgver}.tar.gz"
mkdir -p "$ARTIFACTS_DIR" "$CARGO_TARGET_DIR" "$DIST_DIR"
find "$DIST_DIR" -maxdepth 1 -type f -name "${pkgname}-*.pkg.tar.*" -delete

# Local recipes use a generated source archive so path dependencies beside the
# greeter checkout can be included without publishing temporary archives.
if grep -q "${pkgname}-\${pkgver}.tar.gz\|\${pkgname}-\${pkgver}.tar.gz\|${pkgname}-${pkgver}.tar.gz" "$BUILD_SCRIPT"; then
  echo "Creating local source archive: $archive"
  staging_dir="$(mktemp -d)"
  trap 'rm -rf "$staging_dir"' EXIT

  mkdir -p "$staging_dir/${pkgname}-${pkgver}" "$staging_dir/argvus-i18n" "$staging_dir/argvus-tui"

  tar -cf - \
    --exclude='./.git' \
    --exclude='./.release' \
    --exclude='./packages-repo' \
    --exclude='./target' \
    --exclude='*/target' \
    --exclude='./build' \
    --exclude='*/build' \
    --exclude='./dist' \
    --exclude='*/dist' \
    --exclude='./pkg' \
    --exclude='./packaging/pkg' \
    --exclude='./packaging/src' \
    --exclude='./packaging/arch/ci/pkg' \
    --exclude='./packaging/arch/ci/src' \
    --exclude='./packaging/arch/local/pkg' \
    --exclude='./packaging/arch/local/src' \
    --exclude='./packaging/*.pkg.tar*' \
    --exclude='./packaging/arch/ci/*.pkg.tar*' \
    --exclude='./packaging/arch/local/*.pkg.tar*' \
    --exclude="./${pkgdir:-pkg}" \
    --exclude="./${pkgname}-${pkgver}.tar.gz" \
    --exclude="./packaging/${pkgname}-${pkgver}.tar.gz" \
    --exclude="./build/artifacts/${pkgname}-${pkgver}.tar.gz" \
    -C "$ROOT_DIR" . \
    | tar -xf - -C "$staging_dir/${pkgname}-${pkgver}"

  i18n_root="$ROOT_DIR/../argvus-i18n"
  if [[ ! -f "$i18n_root/Cargo.toml" ]]; then
    echo "argvus-i18n checkout not found beside argvus-greeter: $i18n_root" >&2
    exit 1
  fi

  tar -cf - \
    --exclude='./.git' \
    --exclude='./target' \
    --exclude='*/target' \
    --exclude='./build' \
    --exclude='*/build' \
    --exclude='./dist' \
    --exclude='*/dist' \
    -C "$i18n_root" . \
    | tar -xf - -C "$staging_dir/argvus-i18n"

  tui_root="$ROOT_DIR/../argvus-tui"
  if [[ ! -f "$tui_root/Cargo.toml" ]]; then
    echo "argvus-tui checkout not found beside argvus-greeter: $tui_root" >&2
    exit 1
  fi

  tar -cf - \
    --exclude='./.git' \
    --exclude='./target' \
    --exclude='*/target' \
    --exclude='./build' \
    --exclude='*/build' \
    --exclude='./dist' \
    --exclude='*/dist' \
    -C "$tui_root" . \
    | tar -xf - -C "$staging_dir/argvus-tui"

  tar -czf "$archive" \
    -C "$staging_dir" "${pkgname}-${pkgver}" argvus-i18n argvus-tui
fi

 # Respect caller-supplied flags; otherwise use a clean, non-network local
 # package build suitable for the development checkout.
if [[ -n "${MAKEPKG_FLAGS:-}" ]]; then
  # shellcheck disable=SC2206
  flags=(${MAKEPKG_FLAGS})
elif [[ "$pkgname" == "argvus-waybar" ]]; then
  flags=(--syncdeps --noconfirm --needed --cleanbuild --clean --force)
else
  flags=(--nodeps --noconfirm --needed --cleanbuild --clean --force)
fi

cd "$PACKAGING_DIR"
export BUILDDIR="$ARTIFACTS_DIR"
export SRCDEST="$ARTIFACTS_DIR"
export PKGDEST="$DIST_DIR" CARGO_TARGET_DIR
cp "$BUILD_SCRIPT" "$PACKAGING_DIR/PKGBUILD.local"
trap 'rm -f "$PACKAGING_DIR/PKGBUILD.local"' EXIT
sha256="$(sha256sum "$archive" | awk '{print $1}')"
sed -i "s/^sha256sums=.*/sha256sums=(\"${sha256}\")/" "$PACKAGING_DIR/PKGBUILD.local"
makepkg -p PKGBUILD.local "${flags[@]}" "$@"

printf 'Packages created in %s:\n' "$DIST_DIR"
find "$DIST_DIR" -maxdepth 1 -type f -name "${pkgname}-*.pkg.tar.zst" -printf '  %f\n' | sort
