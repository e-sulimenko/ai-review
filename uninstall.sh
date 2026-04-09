#!/usr/bin/env bash
set -euo pipefail

# ai-review uninstaller:
# - removes installed binary from a directory on PATH (or prefix)

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_NAME="ai-review"

usage() {
  cat <<'EOF'
Usage: ./uninstall.sh [--prefix DIR] [--force]

Uninstalls `ai-review` by removing the installed binary:
- DIR/bin/ai-review if --prefix is provided
- otherwise: the path returned by `command -v ai-review` (if any)

Options:
  --prefix DIR   Uninstall from DIR/bin
  --force        Remove without prompting
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

resolve_installed_path() {
  if [[ -n "$PREFIX" ]]; then
    echo "${PREFIX%/}/bin/${BIN_NAME}"
    return 0
  fi

  if command -v "${BIN_NAME}" >/dev/null 2>&1; then
    command -v "${BIN_NAME}"
    return 0
  fi

  return 1
}

if ! INSTALLED_PATH="$(resolve_installed_path)"; then
  echo "No installed \`${BIN_NAME}\` found to uninstall." >&2
  echo "If you installed with --prefix, pass the same: ./uninstall.sh --prefix <DIR>" >&2
  exit 1
fi

# Best effort: refuse to delete something that doesn't look like our binary name.
if [[ "$(basename "${INSTALLED_PATH}")" != "${BIN_NAME}" ]]; then
  echo "Refusing to remove unexpected path: ${INSTALLED_PATH}" >&2
  exit 2
fi

if [[ ! -e "${INSTALLED_PATH}" ]]; then
  echo "Path does not exist: ${INSTALLED_PATH}" >&2
  exit 1
fi

if [[ "${FORCE}" != "1" ]]; then
  echo "==> Will remove: ${INSTALLED_PATH}"
  read -r -p "Proceed? [y/N]: " ans
  ans="${ans:-}"
  ans_lc="$(printf '%s' "${ans}" | tr '[:upper:]' '[:lower:]')"
  case "${ans_lc}" in
    y|yes) ;;
    *) echo "Cancelled."; exit 0 ;;
  esac
fi

echo "==> Removing ${INSTALLED_PATH}"
rm -f "${INSTALLED_PATH}"

echo
echo "Uninstalled: ${INSTALLED_PATH}"
echo "Note: build artifacts in this repo (e.g. ${ROOT_DIR}/target/) were not removed."
