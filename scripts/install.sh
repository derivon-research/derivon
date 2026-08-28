#!/bin/sh
set -eu

repository="derivon-research/derivon"
formula="derivon-research/tap/derivon"
version="${DERIVON_VERSION:-latest}"
install_dir="${DERIVON_INSTALL_DIR:-$HOME/.local/bin}"
method="${DERIVON_INSTALL_METHOD:-auto}"

fail() {
  printf 'derivon installer: %s\n' "$*" >&2
  exit 1
}

command_exists() {
  command -v "$1" >/dev/null 2>&1
}

case "$method" in
  auto|brew|cargo|binary) ;;
  *) fail "DERIVON_INSTALL_METHOD must be auto, brew, cargo, or binary" ;;
esac

case "$version" in
  latest) ;;
  *[!0-9.]*|.*|*.) fail "DERIVON_VERSION must be latest or an exact SemVer" ;;
  *)
    old_ifs=$IFS
    IFS=.
    set -- $version
    IFS=$old_ifs
    [ "$#" -eq 3 ] || fail "DERIVON_VERSION must contain major.minor.patch"
    ;;
esac

os=$(uname -s)
arch=$(uname -m)

install_with_brew() {
  [ "$version" = "latest" ] || fail "Homebrew installs latest; use Cargo for an exact version"
  if brew list --formula derivon >/dev/null 2>&1; then
    brew upgrade "$formula"
  else
    brew install "$formula"
  fi
}

install_with_cargo() {
  command_exists cargo || fail "Cargo is not installed"
  if [ "$version" = "latest" ]; then
    cargo install derivon-cli --locked
  else
    cargo install derivon-cli --version "$version" --locked
  fi
}

if [ "$os" = "Darwin" ]; then
  case "$method" in
    binary) fail "direct macOS binaries are not distributed; use Homebrew or Cargo" ;;
    brew) command_exists brew || fail "Homebrew is not installed"; install_with_brew; exit 0 ;;
    cargo) install_with_cargo; exit 0 ;;
    auto)
      if command_exists brew && [ "$version" = "latest" ]; then
        install_with_brew
      elif command_exists cargo; then
        install_with_cargo
      else
        fail "install Homebrew or Rust/Cargo, then run this installer again"
      fi
      exit 0
      ;;
  esac
fi

[ "$os" = "Linux" ] || fail "unsupported operating system: $os"
if [ "$method" = "brew" ]; then
  fail "Homebrew installation is supported on macOS; use binary or Cargo on Linux"
fi
if [ "$method" = "cargo" ]; then
  install_with_cargo
  exit 0
fi

case "$arch" in
  x86_64|amd64) target="x86_64-unknown-linux-musl" ;;
  aarch64|arm64) target="aarch64-unknown-linux-musl" ;;
  *) fail "unsupported Linux architecture: $arch" ;;
esac

command_exists curl || fail "curl is required"
command_exists tar || fail "tar is required"

if [ "$version" = "latest" ]; then
  effective_url=$(curl -fsSLI -o /dev/null -w '%{url_effective}' \
    "https://github.com/$repository/releases/latest")
  tag=${effective_url##*/}
  case "$tag" in
    derivon-cli-v*) version=${tag#derivon-cli-v} ;;
    *) fail "latest release is not a derivon-cli release" ;;
  esac
fi

tag="derivon-cli-v$version"
archive="derivon-$version-$target.tar.gz"
base_url="https://github.com/$repository/releases/download/$tag"
tmp_dir=$(mktemp -d)
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT HUP INT TERM

curl -fsSL "$base_url/$archive" -o "$tmp_dir/$archive"
curl -fsSL "$base_url/SHA256SUMS" -o "$tmp_dir/SHA256SUMS"
expected=$(awk -v file="$archive" '$2 == file || $2 == "*" file { print $1; exit }' "$tmp_dir/SHA256SUMS")
[ -n "$expected" ] || fail "release checksum is missing for $archive"
if command_exists sha256sum; then
  actual=$(sha256sum "$tmp_dir/$archive" | awk '{print $1}')
elif command_exists shasum; then
  actual=$(shasum -a 256 "$tmp_dir/$archive" | awk '{print $1}')
else
  fail "sha256sum or shasum is required"
fi
[ "$actual" = "$expected" ] || fail "SHA-256 verification failed"

tar -xzf "$tmp_dir/$archive" -C "$tmp_dir"
[ -f "$tmp_dir/derivon" ] || fail "release archive does not contain derivon"
chmod 755 "$tmp_dir/derivon"
"$tmp_dir/derivon" --version >/dev/null

mkdir -p "$install_dir"
staged="$install_dir/.derivon-install-$$"
cp "$tmp_dir/derivon" "$staged"
chmod 755 "$staged"
mv -f "$staged" "$install_dir/derivon"
trap - EXIT HUP INT TERM
cleanup

printf 'installed derivon %s to %s/derivon\n' "$version" "$install_dir"
case ":$PATH:" in
  *":$install_dir:"*) ;;
  *) printf 'add %s to PATH before running derivon\n' "$install_dir" ;;
esac
