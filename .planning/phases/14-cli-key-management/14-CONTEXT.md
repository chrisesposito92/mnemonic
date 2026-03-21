# Phase 14: CLI Key Management - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

`mnemonic keys` subcommand for creating, listing, and revoking API keys from the terminal. The binary must skip embedding model loading on the CLI path for instant (<1s) startup. KeyService methods already exist from Phase 11 — this phase wraps them in CLI UX.

</domain>

<decisions>
## Implementation Decisions

### CLI argument parsing
- **D-01:** Use `clap` crate with derive macros for argument parsing — standard Rust CLI library, gives subcommands, `--help`, `--version`, and proper error messages for free [auto: clap is the Rust standard; derive macros keep it clean]
- **D-02:** Top-level command structure: `mnemonic` with no args starts the server (backward compatible), `mnemonic keys <subcommand>` branches to CLI path [auto: preserves existing behavior, no breaking change]
- **D-03:** Subcommand is optional in clap — `None` = serve mode. No `mnemonic serve` subcommand needed yet (YAGNI) [auto: simplest approach]

### Binary restructuring (dual-mode)
- **D-04:** Parse CLI args at the very top of `main()` before any initialization. If `keys` subcommand detected, take CLI path: register sqlite-vec → load config → open DB → construct KeyService → run command → exit [auto: satisfies SC4 by skipping model loading entirely]
- **D-05:** CLI path skips: embedding model loading, LLM engine init, MemoryService construction, CompactionService construction, server bind/listen — only DB is needed [auto: minimal init for maximum speed]
- **D-06:** CLI path reuses existing `config::load_config()` for DB path resolution — same config mechanism (env vars, TOML) works for CLI mode [auto: no duplication, consistent behavior]
- **D-07:** Add a `--db <path>` global flag as an override for the DB path from config — useful for quick operations against a specific database file without editing config [auto: common CLI pattern for database tools]

### `keys create` command
- **D-08:** Syntax: `mnemonic keys create <name> [--agent-id <agent_id>]` — name is a required positional arg, agent_id is an optional flag [auto: positional for the single required arg is most natural]
- **D-09:** On success: print the raw `mnk_...` token prominently, followed by a "Save this key — it will not be shown again" warning to stderr [auto: matches SC1, key to stderr warning so stdout can be piped]
- **D-10:** Print key metadata (ID, name, scope) alongside the raw token so the user can immediately reference the key [auto: good UX, follows POST /keys response pattern from Phase 13 D-15]

### `keys list` command
- **D-11:** Syntax: `mnemonic keys list` — no arguments [auto: lists all keys, matches KeyService::list() behavior]
- **D-12:** Output as a hand-formatted table with column headers: ID (display_id 8 chars), NAME, SCOPE, CREATED, STATUS [auto: no new dependency needed; the table is simple enough for formatted println!]
- **D-13:** STATUS column shows "active" or "revoked (date)" — more informative than showing raw revoked_at [auto: human-friendly]
- **D-14:** Empty state: print "No API keys found. Create one with: mnemonic keys create <name>" [auto: actionable hint, matches Phase 10 D-11 messaging pattern]

### `keys revoke` command
- **D-15:** Syntax: `mnemonic keys revoke <id>` — accepts either the full UUID or the 8-char display_id [auto: most user-friendly, display_id is what `keys list` shows prominently]
- **D-16:** If input is 8 hex chars, query by display_id prefix. If it matches multiple keys, error with "Ambiguous — multiple keys match prefix. Use full ID." and list the matches [auto: safe default, prevents accidental wrong-key revocation]
- **D-17:** On success: print confirmation "Key <display_id> revoked" [auto: concise, matches KeyService::revoke() idempotent behavior from Phase 11 D-15]
- **D-18:** If key not found: print "No key found with ID <input>" and exit with code 1 [auto: clear error, distinct from success]

### Output style
- **D-19:** All normal output goes to stdout, warnings and errors go to stderr — allows piping (e.g., `mnemonic keys create my-key 2>/dev/null` captures just the token) [auto: Unix convention]
- **D-20:** Exit codes: 0 = success, 1 = error (key not found, DB error, etc.) [auto: standard]
- **D-21:** No tracing/logging initialization on CLI path — clean output, no `INFO mnemonic starting` noise [auto: CLI UX; tracing is for server mode]

### Crate dependencies
- **D-22:** Add `clap` with `derive` feature to Cargo.toml [auto: standard, well-maintained]

### Claude's Discretion
- Exact table column widths and alignment
- Whether to truncate long key names in the table
- Internal code organization (CLI handler functions in main.rs vs separate module)
- Whether `keys revoke` needs a `--force` flag or confirmation prompt (likely not — revocation is idempotent and reversible by creating a new key)

</decisions>

<specifics>
## Specific Ideas

- The `create` output should feel like `ssh-keygen` — show the key prominently with a clear "save this" warning
- The `list` table should feel like `docker ps` or `kubectl get pods` — clean, aligned columns with a header row
- For `revoke`, accepting display_id (the 8-char prefix shown in `list`) is critical UX — nobody wants to copy/paste UUIDs
- The dual-mode binary pattern is common in Rust (e.g., `diesel` CLI, `sqlx` CLI) — arg parsing before heavy init
- The raw token on `create` should be on its own line for easy copy-paste — no surrounding JSON or formatting that would break clipboard

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements and success criteria
- `.planning/REQUIREMENTS.md` — CLI-01 (create), CLI-02 (list), CLI-03 (revoke)
- `.planning/ROADMAP.md` §Phase 14 — 4 success criteria, especially SC4 (< 1s startup, no model loading)

### Dual-mode binary
- `.planning/REQUIREMENTS.md` §Future — KEY-05: "Dual-mode binary — keys subcommand opens DB only, skips model loading for instant CLI response"

### Prior phase decisions (already built)
- `.planning/phases/11-keyservice-core/11-CONTEXT.md` — D-01 through D-19: KeyService API, token format, create/list/revoke/validate semantics
- `.planning/phases/10-auth-schema-foundation/10-CONTEXT.md` — D-11 (startup log wording references `mnemonic keys create`)

### Existing code
- `src/auth.rs` — `KeyService::create(name, agent_id)`, `KeyService::list()`, `KeyService::revoke(id)` — all already implemented and tested
- `src/auth.rs` — `ApiKey` struct with `id`, `name`, `display_id`, `agent_id`, `created_at`, `revoked_at` fields
- `src/main.rs` — Current server startup pipeline to restructure with early CLI branch
- `src/config.rs` — `load_config()` and `Config` struct with `db_path` field
- `src/db.rs` — `register_sqlite_vec()` and `open(&config)` for database initialization

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `KeyService::create(name, agent_id)` — Returns `(ApiKey, raw_token)`. CLI prints token and metadata.
- `KeyService::list()` — Returns `Vec<ApiKey>`. CLI formats as table.
- `KeyService::revoke(id)` — Returns `()`. CLI prints confirmation.
- `KeyService::new(conn)` — Only needs `Arc<Connection>`, no embedding engine.
- `config::load_config()` — Already handles env vars and TOML file; CLI reuses for `db_path`.
- `db::register_sqlite_vec()` — Must be called before any DB open (existing requirement).
- `db::open(&config)` — Opens and migrates DB. CLI path calls this same function.

### Established Patterns
- `main.rs` does all initialization sequentially — CLI branch inserts at the top, before embedding init
- Services are constructed with `Arc<Connection>` — CLI constructs `KeyService` the same way
- Config is loaded early — CLI reuses the same config path

### Integration Points
- `main.rs` — Major restructure: add clap arg parsing at top, branch to CLI or server path
- `Cargo.toml` — Add `clap` dependency with `derive` feature
- `auth.rs` — No changes needed; `KeyService` already has all required methods
- `config.rs` — May need minor change if `--db` override is implemented (or override can happen in main.rs after config load)

</code_context>

<deferred>
## Deferred Ideas

- `--json` output flag for programmatic use — not in requirements, add if requested
- `mnemonic serve` explicit subcommand — not needed while default behavior is serving
- Interactive confirmation for revoke — revocation is idempotent and a new key can be created; not needed
- `keys info <id>` command to show single key details — not in requirements

</deferred>

---

*Phase: 14-cli-key-management*
*Context gathered: 2026-03-20*
*Mode: auto — all gray areas resolved with recommended defaults*
