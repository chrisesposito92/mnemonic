# Phase 20: output polish - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

All subcommands produce consistent, machine-composable output — `--json` flag works everywhere, exit codes are correct, and data/errors are split across stdout/stderr. Covers requirements OUT-01, OUT-02, OUT-03, OUT-04.

This is a cross-cutting polish phase that touches every handler in cli.rs plus main.rs dispatch. No new subcommands are added — only output behavior changes.

</domain>

<decisions>
## Implementation Decisions

### --json flag design
- **D-01:** `--json` is a global flag on `Cli` struct (same level as `--db`), not per-subcommand. Avoids duplicating the flag on every Args struct. All handlers receive it from the top-level `cli_args.json` bool.
- **D-02:** `--json` applies to data-producing subcommands: `recall`, `recall --id`, `remember`, `search`, `compact`, `keys list`, `keys create`. For `serve` and `keys revoke`, `--json` is accepted but has no effect (serve produces no stdout; revoke's single-line confirmation is too trivial).
- **D-03:** JSON output is a single JSON object per invocation printed to stdout, never streaming NDJSON. One `serde_json::to_string_pretty()` call at the end.
- **D-04:** JSON output replaces ALL stdout — no table headers, no footers, no progress text. Stderr messages (model loading, audit trail, LLM status) remain unchanged regardless of `--json`.
- **D-05:** When `--json` and an error occurs, errors still go to stderr as plain text and exit 1. JSON is for success-path data only. This matches `jq` ecosystem conventions — stderr is always human-readable.

### JSON output shapes per subcommand
- **D-06:** `recall` (list mode): serialize `ListResponse` directly — `{"memories": [...], "total": N}`. Already derives Serialize.
- **D-07:** `recall --id`: serialize the single `Memory` object directly — `{"id": "...", "content": "...", ...}`. Already derives Serialize.
- **D-08:** `remember`: serialize `{"id": "<full-uuid>"}` — the machine-consumable output is just the ID. Human mode also prints ID on line 1, so `--json` wraps it in an object.
- **D-09:** `search`: serialize `SearchResponse` directly — `{"memories": [{"id": "...", "content": "...", "distance": 0.1234, ...}]}`. Already derives Serialize.
- **D-10:** `compact`: serialize `CompactResponse` directly — includes `run_id`, `clusters_found`, `memories_merged`, `memories_created`, `id_mapping`, `truncated`. The full cluster detail that D-16 in Phase 19 deferred to Phase 20.
- **D-11:** `keys list`: serialize the `Vec<ApiKey>` directly (ApiKey must derive Serialize if not already — check during planning).
- **D-12:** `keys create`: serialize `{"token": "<raw>", "id": "<display_id>", "name": "...", "scope": "..."}` — includes the one-time raw token.

### Exit code audit
- **D-13:** Exit 0 on success for all subcommands — already the case (Rust `main() -> Result<()>` returns 0 on Ok). Handlers that call `std::process::exit(1)` on error are correct.
- **D-14:** Exit 1 on all error paths — already consistent. Every `Err(e)` arm prints to stderr and calls `std::process::exit(1)`. No changes needed for exit codes.
- **D-15:** "Not found" cases (recall --id with bad UUID, keys revoke with no match) correctly use exit 1 + stderr already. No change.

### stdout/stderr split audit
- **D-16:** Current violations to fix:
  - `remember`: `eprintln!("Stored memory {}", short_id)` — this is metadata about the success, not an error. BUT it's correctly on stderr because stdout is reserved for the pipeable UUID. **No fix needed** — this is intentional (line 1 stdout = UUID, stderr = human context).
  - `compact`: `eprintln!("Run: {}", run_id_short)` — audit trail correctly on stderr. **No fix needed.**
  - `keys create`: `eprintln!("Save this key -- it will not be shown again.")` — warning correctly on stderr. **No fix needed.**
- **D-17:** After audit: no stdout/stderr split fixes needed. The existing pattern is already correct — data to stdout, progress/warnings/errors to stderr.

### Implementation approach
- **D-18:** Add `#[arg(long, global = true)]` `pub json: bool` field to `Cli` struct. Pass the bool through to each handler.
- **D-19:** Each handler gains an `if json { ... } else { ... }` branch wrapping the output section. The existing human-readable output stays in the else branch unchanged.
- **D-20:** Handler signatures change to accept the json bool. Minimal approach: pass `json: bool` as an additional parameter to `run_recall()`, `run_remember()`, `run_search()`, `run_compact()`, `run_keys()`. Do NOT restructure into an output trait or strategy pattern — that's overengineering for 6 branches.
- **D-21:** For `recall` list mode, the handler currently does raw SQL. For `--json`, reuse the same query but serialize the result vec + total as `ListResponse` JSON. No need to route through `MemoryService` — the handler already constructs `Memory` structs.

### Claude's Discretion
- JSON pretty-print vs compact (recommend pretty for CLI, but up to implementation)
- Whether `keys revoke` outputs `{"revoked": true}` in JSON mode or stays silent
- ApiKey struct Serialize derivation approach if not already present
- Test strategy for JSON output verification

</decisions>

<specifics>
## Specific Ideas

- `--json` output should be valid JSON that pipes directly to `jq` — e.g., `mnemonic recall --json | jq '.memories[0].id'`
- The `compact --json` output should include the full `id_mapping` cluster detail that was intentionally deferred from Phase 19's human output (D-16 in 19-CONTEXT.md)
- `mnemonic remember "test" --json` should output `{"id": "..."}` so scripts can capture the UUID

</specifics>

<canonical_refs>
## Canonical References

No external specs — requirements are fully captured in decisions above and REQUIREMENTS.md.

### Requirements
- `.planning/REQUIREMENTS.md` — OUT-01 through OUT-04 define the four output requirements

### Prior phase output patterns
- `.planning/phases/18-search-subcommand/18-CONTEXT.md` — D-10 through D-15 define search table format (the human output that --json replaces)
- `.planning/phases/19-compact-subcommand/19-CONTEXT.md` — D-12 through D-16 define compact output format and the explicit deferral of cluster detail to Phase 20's --json

### Existing serialization
- `src/service.rs` lines 58-86 — Memory, ListResponse, SearchResponse, SearchResultItem all derive Serialize
- `src/compaction.rs` lines 20-35 — CompactResponse, ClusterMapping derive Serialize

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- All response types (`Memory`, `ListResponse`, `SearchResponse`, `CompactResponse`) already derive `serde::Serialize` — JSON output is one `serde_json::to_string_pretty()` call away
- `serde_json` is already a dependency (used by API handlers in server.rs)

### Established Patterns
- Global `--db` flag on `Cli` struct with `#[arg(long, global = true)]` — same pattern for `--json`
- `db_override` extracted before match in main.rs — same extraction pattern for `json` bool
- Every handler follows the pattern: success → println to stdout, error → eprintln + exit(1)

### Integration Points
- `Cli` struct in `cli.rs` line 11 — add `json: bool` field here
- `main.rs` dispatch match (lines 24-87) — extract `json` alongside `db_override`, pass to each handler
- Every `run_*` function signature — add `json: bool` parameter
- `auth::ApiKey` — may need `#[derive(serde::Serialize)]` if not already present

</code_context>

<deferred>
## Deferred Ideas

- Color-coded output with owo-colors — CLR-01 in future requirements (v1.4+)
- `--format csv/table/json` multi-format — explicitly out of scope per REQUIREMENTS.md
- `--quiet` / `-q` flag for silent operation — not in v1.3 requirements

</deferred>

---

*Phase: 20-output-polish*
*Context gathered: 2026-03-21*
