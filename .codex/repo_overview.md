# Repo Overview — greentic-mcp (Router Execution Focus)

## Current Execution Model
- **Executor:** `greentic-mcp/src/executor.rs` defines `WasixExecutor`, a blocking/sync Wasmtime component runner. It reads component bytes, instantiates with a `Linker`, and calls a **typed func** using `ToolRef.entry`.
- **Tool registry:** `greentic-mcp/src/types.rs` defines `ToolRef { name, component, entry, ... }`. The executor resolves `ToolRef.component` as a filesystem path and invokes `instance.get_typed_func::<(String,), (String,)>(&..., &tool.entry)`.
- **Entrypoint assumption:** Expects a core/component export named by `ToolRef.entry` (legacy `exec`/`invoke`-style `(String) -> String`). No inspection of worlds; router-only components lacking that export fail with “missing entry”.
- **WASI wiring:** Uses `wasmtime_wasi::p2::add_to_linker_sync` and a `WasiState` with `WasiCtxBuilder` inheriting stdio/env and allowing blocking. No router-host bindings are linked; only WASI preview2 is available.
- **Error classification:** Traps are treated as transient; other Wasmtime errors become fatal `McpError::ExecutionFailed`.

## CLI / Harness Usage
- Top-level helpers in `greentic-mcp/src/lib.rs`:
  - `WasixExecutor::invoke(tool, input)` is the main path for `ToolMap` flows (still entrypoint-based).
  - Retry helpers `exec_with_retries*` delegate to **greentic-mcp-exec** (`greentic_mcp_exec::exec`) for other flows, inheriting its legacy `exec(action, args_json)` assumption.
- Tests (`greentic-mcp/tests/integration_echo.rs`) exercise native backends and the retry wrapper; they do not cover router-world components. Router-only WASMs currently trap because neither path calls router exports.

## Existing Router Bindings in the Workspace
- **WIT:** `crates/mcp-exec/wit/wasix-mcp-25.6.18/package.wit` (vendored for publishing) and a copied fixture in `crates/mcp-exec/tests/router_echo/wit/...`.
- **Host bindings:** `crates/mcp-exec/src/router.rs` uses `wasmtime::component::bindgen!` against the router WIT and invokes `call_call_tool`/`list_tools` with WASI p2 linked.
- **Guest bindings:** `crates/mcp-adapter` imports `wasix:mcp/router@25.6.18` on the guest side.
- **greentic-interfaces** currently does not expose router host bindings; reuse would require depending on `greentic-mcp-exec` or vendoring WIT.

## Gap vs Router-First MCP
- No world detection; assumes a single core/component export named by `ToolRef.entry`.
- No router executor; no path to call `router.call-tool`/`list-tools`.
- CLI/tests don’t surface router responses or tool lists; errors for missing exports are opaque Wasmtime failures.

## Proposed Refactor (Router-First)
1) **World detection / dispatch**
   - After loading the `Component`, inspect exports. If `wasix:mcp/router@25.6.18` is present, choose RouterExecutor; else, if a legacy entry matches `ToolRef.entry`, use LegacyExecutor; otherwise, return an error listing exports found.
   - Router takes precedence when both are present.
2) **RouterExecutor**
   - Link WASI p2 (`add_to_linker_sync`) and instantiate router bindings (reuse the router WIT).
   - Implement `list_tools` and `call_tool(tool_name, args_json)` using the generated host bindings.
   - Return structured `router::Response` (completed/elicitation) and map `ToolError::*` to user-facing errors.
3) **Legacy fallback**
   - Keep current typed-func invocation using `ToolRef.entry` for components without router exports.
4) **Unified API**
   - Add `execute_router_tool(wasm_path, tool_name, args_json)` and `execute_any(...)` that dispatches based on exports. Update `WasixExecutor::invoke` (and retry helpers) to call this dispatcher.
5) **CLI/tests**
   - Update harness to accept `--tool/--operation` and `--input` and call `execute_any`.
   - Improve errors: if tool not found, list available tools from `router.list-tools`; render elicitation vs completed responses; preserve `is-error` when present.
   - Add an offline router-only fixture test (e.g., reuse `router_echo` fixture) covering `list-tools`, `tool not found`, and successful call.

## Files to Touch
- `greentic-mcp/src/executor.rs` (add RouterExecutor + dispatcher, WASI/linker wiring)
- `greentic-mcp/src/types.rs` (document legacy `entry`; possibly make optional for router path)
- `greentic-mcp/src/lib.rs` (route `invoke_with_map`/retry helpers through dispatcher)
- `greentic-mcp/tests/...` (add router fixture tests; update harness expectations)
- `greentic-mcp/wit/` (if vendoring router WIT) or dependency wiring if reusing existing bindings.

## Decisions Needed (WIT/Bindings)
- **Option A: Reuse `greentic-mcp-exec` router bindings** — avoids duplicating WIT, but couples crates and pulls in exec-specific deps.
- **Option B: Vendor router WIT under `greentic-mcp/wit/` and generate host bindings locally** — keeps publish boundary clean and avoids cross-crate leakage at the cost of a small WIT copy.
- **Recommendation:** Option B. Vendoring the router WIT within `greentic-mcp` keeps the publish story simple and avoids bringing in the full `greentic-mcp-exec` dependency tree; the WIT is already present in the workspace and stable. Clearly document the WIT location for future updates.
