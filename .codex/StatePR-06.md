# StatePR-06 — greentic-mcp: Remove placeholder mcp-component; update mcp-adapter to component/node@0.5.0 and align capabilities

## Repo
`greentic-mcp`

## Goal
1) Remove or quarantine `mcp-component` (placeholder) so it does not confuse the ecosystem.
2) Update `mcp-adapter` to export the current Greentic component interface (e.g. `greentic:component/node@0.5.0`).
3) Confirm `mcp-adapter` behavior remains compatible with the new payload templating approach (runner passes JSON input).
4) Ensure state capability declarations are correct:
   - If adapter does not need persistent state, it should NOT declare it.
   - If it needs state (e.g., caching or elicitation continuity), declare state-store explicitly and rely on runner capability gating.

## Non-goals
- Do not change MCP protocol or router interface (`wasix:mcp@25.06.18`) unless required for version alignment.
- Do not add state usage to the adapter unless there is a concrete requirement.

---

## Work Items

### 1) mcp-component cleanup
- Determine whether `crates/mcp-component` is referenced by builds, workspace members, CI, or published artifacts.
- If unused placeholder:
  - remove it from workspace membership and docs, or
  - keep it but clearly label as non-buildable staging area and exclude from publish/build/test.
Prefer removal from builds to avoid confusion.

### 2) Update mcp-adapter export interface version
- Update the adapter to export `greentic:component/node@0.5.0` (or the current canonical component node interface used elsewhere).
- Ensure any required manifest/schema metadata is updated to match.

### 3) Confirm payload compatibility
- Adapter accepts `{operation?, tool?, arguments?}` JSON.
- Under the new runner model, node config templating will produce this JSON input and pass it to the adapter.
- Ensure adapter continues to behave correctly when values are typed (numbers/bools/objects) and not stringified.

### 4) Capability declarations
- Ensure adapter manifest does NOT declare state-store unless it truly needs it.
- If it does need state for a documented reason, declare read/write/delete appropriately and add tests that enforce runner gating.

### 5) Composition sanity
- Ensure pack-build time composition (adapter + router) still works after export version bump.
- Add/adjust a minimal build/test that composes adapter + a tiny router and executes one `list-tools` and one `call-tool` path.

## Acceptance Criteria
- Placeholder `mcp-component` no longer confuses builds/artifacts (removed or excluded).
- `mcp-adapter` exports the current component node interface version.
- No unintended state capability declaration.
- Composition tests pass.
