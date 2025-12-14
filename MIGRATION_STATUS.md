# Migration Status — greentic-mcp (PR-08)

- What changed: normalized MCP tool metadata into `Vec<SecretRequirement>` (prefers `secret_requirements` from `describe-json`, falls back to legacy `list_secrets`); protocol `Tool` schema now accepts `secret_requirements`; runner host drops the legacy `secret_get` import and exposes greentic:secrets/store@1.0.0 with TenantCtx-scoped host errors when misconfigured.
- What broke: components that still import `secret_get` will fail to link; if `ExecConfig.secrets_store` is `None`, secrets calls return `secrets-unavailable`; callers must provide a store to actually resolve secrets.
- Next repos to update: `greentic-mcp-generator` should emit `secret_requirements`; `greentic-runner`/`greentic-dev`/`greentic-deployer` need to pass secrets-store bindings with runtime TenantCtx scope.
