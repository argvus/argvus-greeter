#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ -f "$ROOT_DIR/packaging/arch/PKGBUILD.local" ]]; then
  PACKAGING_DIR="$ROOT_DIR/packaging/arch"
elif [[ -f "$ROOT_DIR/packaging/PKGBUILD.local" ]]; then
  PACKAGING_DIR="$ROOT_DIR/packaging"
else
  echo "PKGBUILD.local not found under packaging/arch or packaging." >&2
  exit 1
fi

BUILD_SCRIPT="$PACKAGING_DIR/PKGBUILD.local"
metadata="$({ cd "$PACKAGING_DIR" && bash -c 'source "$1"; printf "%s\n%s\n" "$pkgname" "$pkgver"' bash "$BUILD_SCRIPT"; })"
pkgname="$(printf '%s\n' "$metadata" | sed -n '1p')"
pkgver="$(printf '%s\n' "$metadata" | sed -n '2p')"
archive="$PACKAGING_DIR/${pkgname}-${pkgver}.tar.gz"

if grep -q "${pkgname}-\${pkgver}.tar.gz\|\${pkgname}-\${pkgver}.tar.gz\|${pkgname}-${pkgver}.tar.gz" "$BUILD_SCRIPT"; then
  echo "Creating local source archive: $archive"
  staging_dir="$(mktemp -d)"
  trap 'rm -rf "$staging_dir"' EXIT

  mkdir -p "$staging_dir/${pkgname}-${pkgver}" "$staging_dir/argvus-i18n"

  tar -cf - \
    --exclude='./.git' \
    --exclude='./.release' \
    --exclude='./packages-repo' \
    --exclude='./target' \
    --exclude='./pkg' \
    --exclude='./packaging/pkg' \
    --exclude='./packaging/src' \
    --exclude='./packaging/arch/pkg' \
    --exclude='./packaging/arch/src' \
    --exclude='./packaging/*.pkg.tar*' \
    --exclude='./packaging/arch/*.pkg.tar*' \
    --exclude="./${pkgdir:-pkg}" \
    --exclude="./${pkgname}-${pkgver}.tar.gz" \
    --exclude="./packaging/${pkgname}-${pkgver}.tar.gz" \
    --exclude="./packaging/arch/${pkgname}-${pkgver}.tar.gz" \
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
    -C "$i18n_root" . \
    | tar -xf - -C "$staging_dir/argvus-i18n"

  tar -czf "$archive" \
    -C "$staging_dir" "${pkgname}-${pkgver}" argvus-i18n
fi

if [[ -n "${MAKEPKG_FLAGS:-}" ]]; then
  # shellcheck disable=SC2206
  flags=(${MAKEPKG_FLAGS})
elif [[ "$pkgname" == "argvus-waybar" ]]; then
  flags=(--syncdeps --noconfirm --needed --cleanbuild --clean --force)
else
  flags=(--nodeps --noconfirm --needed --cleanbuild --clean --force)
fi

cd "$PACKAGING_DIR"
makepkg -p PKGBUILD.local "${flags[@]}" "$@"

packages="$(find "$PACKAGING_DIR" -maxdepth 1 -type f -name "${pkgname}-*.pkg.tar.zst" -print | sort)"
if [[ -n "$packages" ]]; then
  printf 'Packages created:\n%s\n' "$packages"

  DIST_DIR="$ROOT_DIR/dist"
  mkdir -p "$DIST_DIR"
  mv -f $packages "$DIST_DIR/"
  printf 'Moved to %s:\n' "$DIST_DIR"
  printf '%s\n' "$packages" | xargs -I{} basename {}
fi
