# ABI Reference

The long-term contract for Greentic MCP tools is the component world described
in [`greentic-interfaces`](https://docs.rs/greentic-interfaces). Hosts can call
the generated bindings to invoke the `component-api::invoke` function, fulfill
the `runner-host-v1` imports (HTTP/KV), and query `describe-json` so
schema/default metadata stays bundled with each component.

Secrets are exposed only through the `greentic:secrets/store@1.0.0` bytes API.
Legacy `secret_get` host imports are removed; callers must wire a secrets-store
binding that applies runtime TenantCtx/session scope when resolving names.

Until every tool is published with those bindings, the executor supports a
lightweight Preview2 component ABI that trades in UTF-8 JSON strings instead of
raw pointer/length pairs. Each tool compiled for `wasm32-wasip2` must export:

```
world tool {
    export func tool_invoke(input: string) -> string
}
```

- The host converts the invocation payload to a JSON string and calls
  `tool_invoke`.
- The guest returns a JSON string describing the response payload.
- Traps are classified as transient errors and retried according to the tool
  policy.

When the full Greentic component export is available it takes precedence over
this string-based entrypoint and enables both `describe-v1` and the richer
host callback set.
