#!/bin/sh
# Mosaic CLI installer — macOS + Linux
# Usage: curl -LsSf https://mosaicvideo.github.io/mosaic/install.sh | sh
# Env:
#   MOSAIC_INSTALL_DIR  target dir (default: $HOME/.local/bin)
#   MOSAIC_VERSION      tag or "latest" (default: latest)

set -eu

: "${MOSAIC_INSTALL_DIR:=$HOME/.local/bin}"
: "${MOSAIC_VERSION:=latest}"

REPO="mosaicvideo/mosaic"
GH_API="https://api.github.com/repos/${REPO}"
DOCS_URL="https://mosaicvideo.github.io/mosaic/cli.html"

say() { printf 'mosaic-cli: %s\n' "$*" >&2; }

os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Darwin) asset="mosaic-cli-macos-universal" ;;
  Linux)
    case "$arch" in
      x86_64|amd64) asset="mosaic-cli-linux-x86_64" ;;
      aarch64|arm64)
        say "Linux aarch64 builds aren't published yet. See ${DOCS_URL}#troubleshooting"
        exit 1 ;;
      *)
        say "Unsupported Linux arch: $arch"
        exit 1 ;;
    esac ;;
  *)
    say "Unsupported OS: $os. On Windows, use install.ps1."
    exit 1 ;;
esac

if command -v curl >/dev/null 2>&1; then
  dl() { curl -fL --proto '=https' --tlsv1.2 -o "$2" "$1"; }
  fetch() { curl -fsSL --proto '=https' --tlsv1.2 "$1"; }
elif command -v wget >/dev/null 2>&1; then
  dl() { wget -qO "$2" "$1"; }
  fetch() { wget -qO- "$1"; }
else
  say "Neither curl nor wget found on PATH."
  exit 1
fi

if [ "$MOSAIC_VERSION" = "latest" ]; then
  say "resolving latest release tag..."
  tag="$(fetch "${GH_API}/releases/latest" 2>/dev/null \
    | sed -nE 's/.*"tag_name"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p' \
    | head -n1)"
  if [ -z "${tag:-}" ]; then
    say "could not resolve latest release tag from ${GH_API}/releases/latest"
    say "(try setting MOSAIC_VERSION=vX.Y.Z explicitly)"
    exit 1
  fi
else
  tag="$MOSAIC_VERSION"
fi

tmp="$(mktemp -d 2>/dev/null || mktemp -d -t mosaic-cli)"
trap 'rm -rf "$tmp"' EXIT

base_url="https://github.com/${REPO}/releases/download/${tag}"
say "downloading ${asset} (${tag})"

if ! dl "${base_url}/${asset}" "${tmp}/${asset}"; then
  say "download failed: ${base_url}/${asset}"
  exit 1
fi
if ! dl "${base_url}/SHA256SUMS" "${tmp}/SHA256SUMS"; then
  say "download failed: ${base_url}/SHA256SUMS"
  say "(this release may predate SHA256SUMS; try MOSAIC_VERSION=v0.1.5 or later)"
  exit 1
fi

say "verifying checksum..."
(
  cd "$tmp"
  expected_line="$(grep " ${asset}\$" SHA256SUMS || true)"
  if [ -z "$expected_line" ]; then
    echo "mosaic-cli: asset ${asset} not listed in SHA256SUMS" >&2
    exit 1
  fi
  if command -v shasum >/dev/null 2>&1; then
    printf '%s\n' "$expected_line" | shasum -a 256 -c - >/dev/null
  elif command -v sha256sum >/dev/null 2>&1; then
    printf '%s\n' "$expected_line" | sha256sum -c - >/dev/null
  else
    echo "mosaic-cli: neither shasum nor sha256sum found — cannot verify" >&2
    exit 1
  fi
) || {
  say "checksum verification failed"
  exit 1
}

mkdir -p "$MOSAIC_INSTALL_DIR"
install -m 755 "${tmp}/${asset}" "${MOSAIC_INSTALL_DIR}/mosaic-cli"

ver="$("${MOSAIC_INSTALL_DIR}/mosaic-cli" --version 2>/dev/null | awk '{print $2}')" || {
  say "installed binary failed to run. Try:"
  say "  ${MOSAIC_INSTALL_DIR}/mosaic-cli --version"
  exit 1
}

printf '\n'
printf 'Installed mosaic-cli %s\n' "${ver:-unknown}"
printf '  -> %s/mosaic-cli\n' "$MOSAIC_INSTALL_DIR"
printf '\n'

shell_name="$(basename "${SHELL:-sh}")"

case ":${PATH}:" in
  *":${MOSAIC_INSTALL_DIR}:"*) ;;
  *)
    # Tildes here are literal advice text for the user to read and
    # re-type into their rc file — not a path we need to expand.
    # shellcheck disable=SC2088
    case "$shell_name" in
      zsh) rc='~/.zshrc' ;;
      bash) rc='~/.bashrc (macOS: ~/.bash_profile)' ;;
      fish) rc='~/.config/fish/config.fish' ;;
      *) rc="your shell rc file" ;;
    esac
    printf 'Add this to %s:\n' "$rc"
    # $PATH is a literal in the printed advice, not a variable to expand.
    # shellcheck disable=SC2016
    printf '  export PATH="%s:$PATH"\n\n' "$MOSAIC_INSTALL_DIR"
    ;;
esac

case "$shell_name" in
  zsh)
    printf 'Enable zsh completions:\n'
    printf '  mkdir -p ~/.zfunc && mosaic-cli completions zsh > ~/.zfunc/_mosaic-cli\n'
    printf "  # ensure 'fpath=(~/.zfunc \$fpath)' is in ~/.zshrc before 'compinit'\n"
    printf '\n'
    ;;
  bash)
    printf 'Enable bash completions:\n'
    printf '  mkdir -p ~/.local/share/bash-completion/completions\n'
    printf '  mosaic-cli completions bash > ~/.local/share/bash-completion/completions/mosaic-cli\n\n'
    ;;
  fish)
    printf 'Enable fish completions:\n'
    printf '  mkdir -p ~/.config/fish/completions\n'
    printf '  mosaic-cli completions fish > ~/.config/fish/completions/mosaic-cli.fish\n\n'
    ;;
esac

printf 'Docs: %s\n' "$DOCS_URL"
