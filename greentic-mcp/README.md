# Greentic MCP Tool Bridge

`greentic-mcp` provides the host-side tool registry and WASIX/WASI execution
bridge used by flows to invoke MCP tools compiled to WebAssembly. It loads a
declarative tool map, resolves tools by logical name, and executes them through
Wasmtime with per-tool timeouts, retry hints, and transient error handling.

The crate leans on the shared contracts published in
[`greentic-types`](https://docs.rs/greentic-types) and the WIT definitions plus
generated bindings in [`greentic-interfaces`](https://docs.rs/greentic-interfaces).
Tools are compiled for the WASI Preview2 target (`wasm32-wasip2`) and export a
canonical entrypoint that accepts and returns UTF-8 JSON payloads, so hosts no
longer have to marshal pointer/length pairs manually. When tools additionally
export the `greentic:component/component@1.0.0` world, the bridge calls
`describe-json` to fetch the node schema/defaults that live with the tool
component rather than mirroring them in this repository.

Runtime callbacks (HTTP + KV) are satisfied through the `runner-host-v1`
helpers shipped by `greentic-interfaces`, which keeps the host implementation
aligned with the latest Greentic runner contracts. Secrets must travel through
the `greentic:secrets/store@1.0.0` bytes API (no env/host-import fallback); the
legacy `secret_get` hook is removed and callers should provide a store binding
that enforces runtime TenantCtx scope.

## Tool map configuration

Tool metadata is loaded from JSON or YAML. Each entry records where the tool
artifact lives, the export to call, and optional execution hints.

```yaml
tools:
  - name: echo
    component: ./tools/echo.wasm
    entry: tool_invoke
    timeout_ms: 1000
    max_retries: 2
    retry_backoff_ms: 200
```

Use `greentic_mcp::load_tool_map` to parse the file and build a `ToolMap`.

```rust,no_run
use greentic_mcp::{invoke_with_map, load_tool_map, WasixExecutor};
use serde_json::json;
use std::path::Path;

# #[tokio::main]
# async fn main() -> Result<(), Box<dyn std::error::Error>> {
let map = load_tool_map(Path::new("toolmap.yaml"))?;
let executor = WasixExecutor::new()?;

let echoed = invoke_with_map(&map, &executor, "echo", json!({"hello": "world"}))
    .await?;

assert_eq!(echoed, json!({"hello": "world"}));
# Ok(())
# }
```

`WasixExecutor` ensures that traps bubble up as transient errors, applies
exponential backoff with jitter between retries, and converts wall-clock
timeouts into `McpError::Timeout`.

## ABI contracts

See [ABI.md](ABI.md) for the exact contract implemented by the integration
tests. When a tool exports the richer Greentic component API, use the bindings
from `greentic-interfaces` to avoid manual JSON marshalling and to work with
the `describe-v1`/`runner-host-v1` WIT worlds directly.

## Secret requirements

`describe_tool` now normalizes any secret descriptors in tool metadata to
`Vec<SecretRequirement>`:

- Prefer `secret_requirements` returned by `describe-json`.
- Fall back to legacy `list_secrets` payloads and emit a warning.
- Keys validated via `greentic-types`; unknown formats default to `text`.
- Default scope uses the runtime sentinel `{ env: "runtime", tenant: "runtime" }`
  so TenantCtx/session identity can override at execution time.
