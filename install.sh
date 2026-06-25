#!/bin/sh
# PHPM installer for macOS and Linux.
#   curl -LsSf https://github.com/lemesdaniel/phpm/releases/latest/download/install.sh | sh
#
# Environment overrides:
#   PHPM_INSTALL_DIR  install location (default: $HOME/.local/bin)
#   PHPM_VERSION      a specific release tag (default: latest), e.g. v0.1.0
set -eu

REPO="lemesdaniel/phpm"
BIN="phpm"
INSTALL_DIR="${PHPM_INSTALL_DIR:-$HOME/.local/bin}"

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
    Linux)  os_target="unknown-linux-gnu" ;;
    Darwin) os_target="apple-darwin" ;;
    *) echo "phpm: unsupported OS: $os" >&2; exit 1 ;;
esac

case "$arch" in
    x86_64 | amd64)  arch_target="x86_64" ;;
    arm64 | aarch64) arch_target="aarch64" ;;
    *) echo "phpm: unsupported architecture: $arch" >&2; exit 1 ;;
esac

target="${arch_target}-${os_target}"
asset="${BIN}-${target}.tar.gz"

if [ "${PHPM_VERSION:-latest}" = "latest" ]; then
    url="https://github.com/${REPO}/releases/latest/download/${asset}"
else
    url="https://github.com/${REPO}/releases/download/${PHPM_VERSION}/${asset}"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "phpm: downloading ${url}"
if ! curl -fsSL "$url" -o "$tmp/${asset}"; then
    echo "phpm: download failed. Is there a release with asset ${asset}?" >&2
    exit 1
fi

tar -xzf "$tmp/${asset}" -C "$tmp"
mkdir -p "$INSTALL_DIR"
install -m 0755 "$tmp/${BIN}" "$INSTALL_DIR/${BIN}"

echo "phpm: installed to ${INSTALL_DIR}/${BIN}"

case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *) echo "phpm: add ${INSTALL_DIR} to your PATH to run 'phpm' from anywhere" >&2 ;;
esac

echo "phpm: run 'phpm --help' to get started (requires composer, php, and git on PATH)"
