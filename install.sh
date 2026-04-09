#!/usr/bin/env bash
set -euo pipefail

# ai-review installer:
# - builds in release mode
# - installs binary into a directory on PATH (prefers /usr/local/bin)

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_NAME="ai-review"

usage() {
  cat <<'EOF'
Usage: ./install.sh [--prefix DIR] [--force]

Builds `ai-review` in release mode and installs it into:
- DIR/bin if --prefix is provided
- otherwise: /usr/local/bin (if writable), else ~/.local/bin

Options:
  --prefix DIR   Install into DIR/bin
  --force        Overwrite existing binary without prompting
  -h, --help     Show this help
EOF
}

PREFIX=""
FORCE="0"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      PREFIX="${2:-}"
      shift 2
      ;;
    --force)
      FORCE="1"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

pick_install_dir() {
  if [[ -n "$PREFIX" ]]; then
    echo "${PREFIX%/}/bin"
    return 0
  fi

  if [[ -d "/usr/local/bin" && -w "/usr/local/bin" ]]; then
    echo "/usr/local/bin"
    return 0
  fi

  echo "${HOME}/.local/bin"
}

ensure_rust() {
  if command -v cargo >/dev/null 2>&1; then
    return 0
  fi

  echo "==> Rust (cargo) not found."
  echo "This project includes rust-toolchain.toml; the recommended setup is rustup."
  echo
  echo "Install Rust using rustup now?"
  echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
  echo
  read -r -p "Proceed? [y/N]: " ans
  ans="${ans:-}"
  ans_lc="$(printf '%s' "${ans}" | tr '[:upper:]' '[:lower:]')"
  case "${ans_lc}" in
    y|yes) ;;
    *) echo "Cancelled."; exit 1 ;;
  esac

  if ! command -v curl >/dev/null 2>&1; then
    echo "Error: curl is required to install rustup." >&2
    exit 1
  fi

  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

  # Best effort: load cargo into current shell session.
  if [[ -f "${HOME}/.cargo/env" ]]; then
    # shellcheck disable=SC1090
    source "${HOME}/.cargo/env"
  fi

  if ! command -v cargo >/dev/null 2>&1; then
    echo "Rust installation finished, but cargo is still not on PATH in this shell." >&2
    echo "Restart your terminal (or source ~/.cargo/env), then re-run ./install.sh" >&2
    exit 1
  fi
}

INSTALL_DIR="$(pick_install_dir)"
SRC_BIN="${ROOT_DIR}/target/release/${BIN_NAME}"
DST_BIN="${INSTALL_DIR}/${BIN_NAME}"

ensure_rust

echo "==> Building ${BIN_NAME} (release)"
cd "${ROOT_DIR}"
cargo build --release

if [[ ! -f "${SRC_BIN}" ]]; then
  echo "Build succeeded but binary not found at ${SRC_BIN}" >&2
  exit 1
fi

mkdir -p "${INSTALL_DIR}"

if [[ -f "${DST_BIN}" && "${FORCE}" != "1" ]]; then
  echo "==> ${DST_BIN} already exists."
  read -r -p "Overwrite? [y/N]: " ans
  ans="${ans:-}"
  ans_lc="$(printf '%s' "${ans}" | tr '[:upper:]' '[:lower:]')"
  case "${ans_lc}" in
    y|yes) ;;
    *) echo "Cancelled."; exit 0 ;;
  esac
fi

echo "==> Installing to ${DST_BIN}"
cp -f "${SRC_BIN}" "${DST_BIN}"
chmod +x "${DST_BIN}"

echo
echo "Installed: ${DST_BIN}"

if command -v "${BIN_NAME}" >/dev/null 2>&1; then
  echo "OK: \`${BIN_NAME}\` is available on PATH."
else
  echo "Note: \`${BIN_NAME}\` is NOT on PATH yet."
  echo "Add this to your shell profile (e.g. ~/.zshrc) and restart shell:"
  echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
fi

echo
echo "Try:"
echo "  ${BIN_NAME} --help"
