# Phase 16: recall subcommand - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Add `mnemonic recall` subcommand for listing and retrieving memories from the terminal. DB-only fast path — no embedding model loaded. Supports listing recent memories, fetching a single memory by ID, and filtering by agent_id/session_id/limit. Must complete in under 100ms.

</domain>

<decisions>
## Implementation Decisions

### Data access pattern
- **D-01:** Recall uses the DB-only fast path — same init as `keys` (register_sqlite_vec → load_config → apply --db → open DB). No MemoryService, no embedding, no LLM, no server.
- **D-02:** Recall handlers take `Arc<Connection>` directly and run raw SQL queries, matching the `KeyService` pattern where the service wraps `Arc<Connection>` without needing embedding.
- **D-03:** A `get_memory(id)` query must be implemented — `get_memory_agent_id()` exists in MemoryService but returns only agent_id, not the full memory. The recall handler needs a full-memory fetch by ID.

### DB init deduplication
- **D-04:** Extract a shared DB init helper (`init_db` or similar) that encapsulates: register_sqlite_vec → load_config → apply --db override → open DB → return `(Arc<Connection>, Config)`. Both Keys and Recall match arms call this instead of duplicating 10 lines.
- **D-05:** The helper lives in `cli.rs` or a new utility — NOT in main.rs. Keep main.rs as pure dispatch.

### CLI args structure
- **D-06:** `Recall` variant in `Commands` enum wraps a `RecallArgs` struct with flat optional flags (not nested subcommands like Keys):
  - `--id <UUID>` — fetch a single memory (mutually exclusive with filter flags)
  - `--agent-id <ID>` — filter by agent
  - `--session-id <ID>` — filter by session
  - `--limit <N>` — max results (default 20)
- **D-07:** Bare `mnemonic recall` (no flags) lists the 20 most recent memories across all agents — useful for a quick "what's in here?" scan.
- **D-08:** No `--offset` or `--page` flag — keep it simple, `--limit` is enough for v1.3. Pagination is a v1.4 concern.

### List output format
- **D-09:** Table columns: `ID` (first 8 chars of UUID), `CONTENT` (truncated to 60 chars), `AGENT` (truncated to 15 chars), `CREATED` (datetime truncated to 19 chars)
- **D-10:** Footer line: `"Showing {count} of {total} memories"` — gives user awareness of how many are hidden by the limit
- **D-11:** Empty state: `"No memories found."` — simple, no suggestion to use `remember` (Phase 17 doesn't exist yet at ship time)
- **D-12:** Reuse the `truncate()` helper already in `cli.rs` (line 154)

### Single-memory display (--id)
- **D-13:** Full key-value detail format, not a one-row table:
  ```
  ID:       <full uuid>
  Content:  <full content, no truncation>
  Agent:    <agent_id or (none)>
  Session:  <session_id or (none)>
  Tags:     <comma-separated or (none)>
  Model:    <embedding_model>
  Created:  <created_at>
  Updated:  <updated_at or (never)>
  ```
- **D-14:** If `--id` is provided and the memory is not found, print `"No memory found with ID <id>"` to stderr and exit with code 1.

### Error handling and exit codes
- **D-15:** Exit code 0 on success, exit code 1 on any error (DB failure, not found) — matches Phase 20 requirements (OUT-03) early.
- **D-16:** Errors print to stderr, data prints to stdout — matches Phase 20 requirements (OUT-04) early.

### Dispatch entry point
- **D-17:** Add `run_recall(args: RecallArgs, conn: Arc<Connection>)` function in `cli.rs`, parallel to `run_keys()`.
- **D-18:** main.rs gets a new match arm: `Some(Commands::Recall(recall_args))` → calls shared DB init helper → calls `run_recall()`.

### Claude's Discretion
- Exact SQL queries (can mirror `list_memories` SQL from service.rs or simplify)
- Whether `RecallArgs` uses clap groups to enforce `--id` mutual exclusivity with filters, or just runtime check
- Test structure and whether to extract DB-query functions into testable units
- Column widths and exact spacing in table output

</decisions>

<specifics>
## Specific Ideas

- The recall init path should feel identical to `keys` — fast, quiet, no tracing init, no model loading messages
- Table format should match the `keys list` visual style — consistent column alignment with dashes separator under header
- `--id` flag (not positional arg) because recall's default behavior is listing, not fetching — positional would feel wrong for "show me recent memories"
- The 100ms target is easily achievable — SQLite queries on a local file return in <10ms, the bottleneck would only be model loading (which we skip)

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` §Recall — RCL-01 (list recent), RCL-02 (--id retrieval), RCL-03 (filter flags)
- `.planning/REQUIREMENTS.md` §Output — OUT-03, OUT-04 (exit codes, stderr/stdout split — implement early)

### CLI patterns (must match)
- `src/cli.rs` — `Commands` enum (line 22), `KeysArgs` struct (line 30), `run_keys()` entry point (line 57), `truncate()` helper (line 154)
- `src/main.rs` — Keys fast-path init (lines 24-53), match dispatch (line 24)

### Data access
- `src/service.rs` — `list_memories()` SQL (lines 229-293) for query pattern reference, `Memory` struct (line 58), `ListParams` (line 48), `ListResponse` (line 71)
- `src/service.rs` — `get_memory_agent_id()` (line 298) — exists but insufficient; need full-memory version
- `src/service.rs` — `delete_memory()` (line 311) — contains inline full-memory fetch SQL that can be referenced

### Prior phase decisions
- `.planning/phases/15-serve-subcommand/15-CONTEXT.md` — D-04/D-08/D-09 (dispatch pattern, deferred helper extraction)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `cli::truncate(s, max_len)` — string truncation with "..." suffix, reuse for content/agent columns
- `cli::Commands` enum — add `Recall(RecallArgs)` variant
- `cli::is_display_id()` — not needed for recall but shows the pattern for ID validation
- `db::register_sqlite_vec()` and `db::open()` — reused in DB init helper
- `config::load_config()` — reused in DB init helper (skip `validate_config` like keys does)

### Established Patterns
- Keys fast-path: register_sqlite_vec → load_config (no validate) → apply --db → open → Arc<Connection> → service → run → exit
- Error handling: `eprintln!("error: ...")` + `std::process::exit(1)` for failures
- Table output: padded columns with `format!("{:<width}")`, header + dashes separator
- Handler naming: `cmd_create`, `cmd_list`, `cmd_revoke` → recall would use `cmd_list_memories`, `cmd_get_memory` or similar

### Integration Points
- `main.rs` line 24: `match cli_args.command` — add `Some(Commands::Recall(recall_args))` arm
- `cli.rs` line 22: `Commands` enum — add `Recall(RecallArgs)` variant
- `service.rs` line 58: `Memory` struct — recall output should match these fields

</code_context>

<deferred>
## Deferred Ideas

- `--json` flag for machine-readable output — Phase 20 (OUT-02) handles this across all subcommands
- `--tag` filter flag — could be added but not in RCL-03 requirements; keep scope tight
- `--offset` / `--page` for pagination — v1.4 concern
- Content search/grep within recall results — that's what `search` is for (Phase 18)
- Color-coded output — v1.4 (CLR-01 in future requirements)

</deferred>

---

*Phase: 16-recall-subcommand*
*Context gathered: 2026-03-21*
