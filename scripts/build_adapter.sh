#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="$ROOT_DIR/target/wasm32-wasip2/release"
BIN_WASM="$OUT_DIR/greentic_mcp_adapter.wasm"
COMP_WASM="$OUT_DIR/mcp_adapter_25_06_18.component.wasm"

ACTIVE_TOOLCHAIN="$(rustup show active-toolchain | awk '{print $1}')"
echo "==> Ensuring wasm32-wasip2 target for toolchain ${ACTIVE_TOOLCHAIN}"
rustup target add --toolchain "${ACTIVE_TOOLCHAIN}" wasm32-wasip2 >/dev/null 2>&1 || true

echo "==> Building greentic-mcp-adapter (wasm32-wasip2, release)"
"cargo" "+${ACTIVE_TOOLCHAIN}" build --release --locked --target wasm32-wasip2 -p greentic-mcp-adapter

echo "==> Component-izing adapter to ${COMP_WASM}"
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
