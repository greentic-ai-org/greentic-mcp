#!/usr/bin/env bash
set -euo pipefail

if [ "${DEBUG:-0}" = "1" ]; then
  set -x
fi

trap 'echo "error: failed at line $LINENO: $BASH_COMMAND" >&2' ERR

ROOT_DIR="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ADAPTER_TARGET_DIR="${ROOT_DIR}/target/adapter-build"
TARGET_TRIPLE="wasm32-wasip2"
OUT_DIR=""
BIN_WASM=""
COMP_WASM=""

if [ "${DEBUG:-0}" = "1" ]; then
  set -x
fi

ACTIVE_TOOLCHAIN="$(rustup show active-toolchain | awk '{print $1}')"
echo "==> rustup version: $(rustup --version)"
echo "==> rustc version:  $(rustc -V || true)"
echo "==> Ensuring wasm32-wasip2 target for toolchain ${ACTIVE_TOOLCHAIN}"

set +e
rustup target add --toolchain "${ACTIVE_TOOLCHAIN}" "${TARGET_TRIPLE}"
status=$?
set -e

echo "==> rustup target add exit code: ${status}"
echo "==> rustup target list (installed):"
rustup target list --toolchain "${ACTIVE_TOOLCHAIN}" --installed || true

if [ "${status}" -ne 0 ]; then
  if rustup target list --toolchain "${ACTIVE_TOOLCHAIN}" --installed | grep -q "^${TARGET_TRIPLE}\$"; then
    echo "warn: rustup returned ${status}, but ${TARGET_TRIPLE} is installed; continuing" >&2
  else
    echo "error: rustup target add ${TARGET_TRIPLE} failed (exit ${status})" >&2
    echo "==> Available wasm targets for this toolchain:" >&2
    rustup target list --toolchain "${ACTIVE_TOOLCHAIN}" | grep -E '^wasm32-' >&2 || true
    exit "${status}"
  fi
fi

echo "==> wasm32-wasip2 target confirmed; continuing"

OUT_DIR="${ADAPTER_TARGET_DIR}/${TARGET_TRIPLE}/release"
BIN_WASM="$OUT_DIR/greentic_mcp_adapter.wasm"
COMP_WASM="$OUT_DIR/mcp_adapter_25_06_18.component.wasm"

ensure_bindings() {
  if [ -n "${GREENTIC_INTERFACES_BINDINGS:-}" ] && [ -d "${GREENTIC_INTERFACES_BINDINGS}" ]; then
    echo "==> Using GREENTIC_INTERFACES_BINDINGS=${GREENTIC_INTERFACES_BINDINGS}"
    return
  fi

  local iface_version iface_src
  iface_version="$(python3 - <<'PY' || true
import tomllib
from pathlib import Path
lock = Path('Cargo.lock')
data = tomllib.loads(lock.read_text())
print(next(p['version'] for p in data['package'] if p['name']=='greentic-interfaces'))
PY
)"
  if [ -z "${iface_version}" ]; then
    echo "error: unable to discover greentic-interfaces version from Cargo.lock" >&2
    exit 1
  fi

  iface_src="$(ls -d "${CARGO_HOME:-$HOME/.cargo}"/registry/src/*/greentic-interfaces-"${iface_version}" 2>/dev/null | head -n1)"
  if [ -z "${iface_src}" ]; then
    echo "error: unable to locate greentic-interfaces-${iface_version} in cargo registry" >&2
    exit 1
  fi

  echo "==> Generating greentic-interfaces bindings for host (bindings-rust)"
  CARGO_TARGET_DIR="${ADAPTER_TARGET_DIR}" "cargo" "+${ACTIVE_TOOLCHAIN}" build --locked --manifest-path "${iface_src}/Cargo.toml"

  local candidate
  candidate="$(ls -d "${ADAPTER_TARGET_DIR}"/debug/build/greentic-interfaces-*/out/bindings 2>/dev/null | sort | tail -n1)"
  if [ -z "${candidate}" ] || [ ! -d "${candidate}" ]; then
    echo "error: unable to locate generated greentic-interfaces bindings (set GREENTIC_INTERFACES_BINDINGS)" >&2
    exit 1
  fi

  export GREENTIC_INTERFACES_BINDINGS="${candidate}"
  echo "==> Using GREENTIC_INTERFACES_BINDINGS=${GREENTIC_INTERFACES_BINDINGS}"
}

echo "==> Step: ensure_bindings"
ensure_bindings

echo "==> Step: build adapter crate"
echo "==> Using target: ${TARGET_TRIPLE}"
CARGO_TARGET_DIR="${ADAPTER_TARGET_DIR}" "cargo" "+${ACTIVE_TOOLCHAIN}" build --release --locked --target "${TARGET_TRIPLE}" -p greentic-mcp-adapter

echo "==> Step: componentize"
if ! wasm-tools component new "$BIN_WASM" -o "$COMP_WASM" 2>"/tmp/componentize.err.$$"; then
  if grep -q "decoding a component is not supported" "/tmp/componentize.err.$$"; then
    # Already a component; just copy it.
    cp "$BIN_WASM" "$COMP_WASM"
  else
    cat "/tmp/componentize.err.$$"
    rm -f "/tmp/componentize.err.$$"
    exit 1
  fi
fi
rm -f "/tmp/componentize.err.$$"

VERSION="$(cargo metadata --format-version 1 --no-deps \
  | jq -r '.packages[] | select(.name=="greentic-mcp-adapter") | .version')"

echo "Built adapter:"
echo "  wasm:      ${BIN_WASM}"
echo "  component: ${COMP_WASM}"
echo "Intended OCI ref:"
echo "  ghcr.io/greentic-ai/greentic-mcp-adapter:25.06.18-v${VERSION}"
