# Router Echo Fixture (wasix:mcp/router@25.6.18)

Minimal router-only component used in tests:
- Exports `wasix:mcp/router@25.6.18`
- Lists a single tool `echo`
- `call-tool("echo", args_json)` returns `completed` with the raw `args_json` echoed in a text content block.

Rebuild:
```bash
rustup target add wasm32-wasip2
cargo build -p router-echo --target wasm32-wasip2 --release
```
Artifact: `target/wasm32-wasip2/release/router_echo.wasm`
