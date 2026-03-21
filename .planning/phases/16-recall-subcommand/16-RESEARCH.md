# Phase 16: recall subcommand - Research

**Researched:** 2026-03-21
**Domain:** Rust CLI subcommand implementation — DB-only fast path, clap Args struct, tabular output
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Data access pattern**
- D-01: Recall uses the DB-only fast path — same init as `keys` (register_sqlite_vec → load_config → apply --db → open DB). No MemoryService, no embedding, no LLM, no server.
- D-02: Recall handlers take `Arc<Connection>` directly and run raw SQL queries, matching the `KeyService` pattern where the service wraps `Arc<Connection>` without needing embedding.
- D-03: A `get_memory(id)` query must be implemented — `get_memory_agent_id()` exists in MemoryService but returns only agent_id, not the full memory. The recall handler needs a full-memory fetch by ID.

**DB init deduplication**
- D-04: Extract a shared DB init helper (`init_db` or similar) that encapsulates: register_sqlite_vec → load_config → apply --db override → open DB → return `(Arc<Connection>, Config)`. Both Keys and Recall match arms call this instead of duplicating 10 lines.
- D-05: The helper lives in `cli.rs` or a new utility — NOT in main.rs. Keep main.rs as pure dispatch.

**CLI args structure**
- D-06: `Recall` variant in `Commands` enum wraps a `RecallArgs` struct with flat optional flags (not nested subcommands like Keys):
  - `--id <UUID>` — fetch a single memory (mutually exclusive with filter flags)
  - `--agent-id <ID>` — filter by agent
  - `--session-id <ID>` — filter by session
  - `--limit <N>` — max results (default 20)
- D-07: Bare `mnemonic recall` (no flags) lists the 20 most recent memories across all agents.
- D-08: No `--offset` or `--page` flag — keep it simple, `--limit` is enough for v1.3.

**List output format**
- D-09: Table columns: `ID` (first 8 chars of UUID), `CONTENT` (truncated to 60 chars), `AGENT` (truncated to 15 chars), `CREATED` (datetime truncated to 19 chars)
- D-10: Footer line: `"Showing {count} of {total} memories"` — gives user awareness of how many are hidden by the limit
- D-11: Empty state: `"No memories found."` — simple, no further suggestion
- D-12: Reuse the `truncate()` helper already in `cli.rs` (line 154)

**Single-memory display (--id)**
- D-13: Full key-value detail format:
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
- D-14: If `--id` is provided and memory not found, print `"No memory found with ID <id>"` to stderr and exit with code 1.

**Error handling and exit codes**
- D-15: Exit code 0 on success, exit code 1 on any error (DB failure, not found) — matches Phase 20 requirements (OUT-03) early.
- D-16: Errors print to stderr, data prints to stdout — matches Phase 20 requirements (OUT-04) early.

**Dispatch entry point**
- D-17: Add `run_recall(args: RecallArgs, conn: Arc<Connection>)` function in `cli.rs`, parallel to `run_keys()`.
- D-18: main.rs gets a new match arm: `Some(Commands::Recall(recall_args))` → calls shared DB init helper → calls `run_recall()`.

### Claude's Discretion
- Exact SQL queries (can mirror `list_memories` SQL from service.rs or simplify)
- Whether `RecallArgs` uses clap groups to enforce `--id` mutual exclusivity with filters, or just runtime check
- Test structure and whether to extract DB-query functions into testable units
- Column widths and exact spacing in table output

### Deferred Ideas (OUT OF SCOPE)
- `--json` flag for machine-readable output — Phase 20 (OUT-02) handles this across all subcommands
- `--tag` filter flag — could be added but not in RCL-03 requirements; keep scope tight
- `--offset` / `--page` for pagination — v1.4 concern
- Content search/grep within recall results — that's what `search` is for (Phase 18)
- Color-coded output — v1.4 (CLR-01 in future requirements)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| RCL-01 | `mnemonic recall` lists recent memories (DB-only, no model load) | DB-only fast path verified in main.rs; list SQL in service.rs list_memories() is the reference pattern |
| RCL-02 | `mnemonic recall --id <uuid>` retrieves a specific memory | Full-memory fetch SQL confirmed absent from MemoryService public API — must be written inline; delete_memory() contains identical SELECT as reference |
| RCL-03 | `mnemonic recall` accepts `--agent-id`, `--session-id`, `--limit` filters | clap `Args` struct with Option<String>/Option<u32> fields; SQL WHERE clause filter pattern exists in list_memories() |
</phase_requirements>

---

## Summary

Phase 16 adds the `recall` subcommand to the `mnemonic` CLI. The implementation follows the established "fast path" pattern from the `keys` subcommand: only DB init (no embedding model, no LLM, no tracing, no server bind) and a direct `Arc<Connection>` passed to handler functions. The entire feature is a pure Rust extension of `src/cli.rs` and `src/main.rs` — no new crates, no new files strictly required.

The two primary implementation surfaces are: (1) the `RecallArgs` clap struct with four optional flags (`--id`, `--agent-id`, `--session-id`, `--limit`) plus dispatch logic in `run_recall()`, and (2) two raw SQL queries written inline in the handler — one for listing (mirroring `service.rs:list_memories` SQL but without offset) and one for fetching a single full memory (mirroring the SELECT in `service.rs:delete_memory`). The only missing piece identified from prior research is `get_memory(id)` returning the full `Memory` struct, which is confirmed absent from MemoryService's public API and must be added inline in cli.rs.

An additional deliverable locked by D-04/D-05 is the `init_db` helper that deduplicates the 10-line DB init block currently duplicated between the Keys arm and the new Recall arm in main.rs. This helper must live in `cli.rs` or a new utility module — not in main.rs.

**Primary recommendation:** Implement `RecallArgs`, `run_recall()`, two inline SQL query functions, and the `init_db` helper entirely within `src/cli.rs` and `src/main.rs`, following existing patterns exactly. Zero new dependencies required.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4.x (derive feature) | CLI argument parsing for `RecallArgs` struct | Already in Cargo.toml; derive macro matches all other Args structs |
| tokio-rusqlite | 0.7 | Async SQLite via `Connection::call()` closure | Already in Cargo.toml; all existing DB access uses this pattern |
| rusqlite | 0.37 (bundled) | SQL query execution, `OptionalExtension` for nullable row | Already in Cargo.toml; `OptionalExtension` used in `get_memory_agent_id()` |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| std::sync::Arc | stdlib | Shared connection ownership across async boundary | Always — matches KeyService and MemoryService pattern |
| serde_json | 1.x | Deserialize `tags` JSON column into `Vec<String>` | Tags are stored as JSON strings; `serde_json::from_str()` used identically in list_memories() and delete_memory() |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Inline SQL in cli.rs | Extend MemoryService with new methods | MemoryService requires Arc<dyn EmbeddingEngine> — instantiating it for recall would force embedding init, violating the fast-path constraint |
| Runtime mutual-exclusivity check for `--id` vs filters | clap `ArgGroup` with `conflicts_with` | Both valid; runtime check is simpler and consistent with how other subcommands handle validation. See discretion note. |

**Installation:**
No new dependencies. All required crates are already in Cargo.toml.

---

## Architecture Patterns

### Recommended Project Structure
No new files are strictly required. Changes concentrate in two existing files:

```
src/
├── cli.rs          # Add: RecallArgs struct, run_recall(), cmd_list_memories(),
│                   #      cmd_get_memory(), init_db() helper
└── main.rs         # Add: Recall match arm, call init_db() from both Keys and Recall arms
```

Optionally extract DB init to a new module:

```
src/
└── cli_utils.rs    # Alternative location for init_db() — only if cli.rs exceeds ~400 lines
```

### Pattern 1: DB-Only Fast Path (matches `keys` arm exactly)

**What:** Minimal init sequence that opens SQLite without loading any ML model, without tracing init, without network services.

**When to use:** Any subcommand that only reads/writes the SQLite database.

**Example (current Keys arm, to be replaced by init_db call):**
```rust
// Source: src/main.rs lines 27-43 (verified)
db::register_sqlite_vec();
let mut config = config::load_config()
    .map_err(|e| anyhow::anyhow!(e))?;
if let Some(ref db_path) = db_override {
    config.db_path = db_path.clone();
}
let conn = db::open(&config).await
    .map_err(|e| anyhow::anyhow!(e))?;
let conn_arc = std::sync::Arc::new(conn);
```

**Proposed init_db helper signature:**
```rust
// In cli.rs — encapsulates the above 10 lines
pub async fn init_db(db_override: Option<String>)
    -> anyhow::Result<(std::sync::Arc<tokio_rusqlite::Connection>, crate::config::Config)>
```

### Pattern 2: RecallArgs Struct (clap derive, flat flags)

**What:** A simple Args struct with all optional fields — no nested Subcommand, unlike `KeysArgs`.

**When to use:** Subcommands whose behaviors are differentiated by presence/absence of flags rather than named sub-verbs.

**Example:**
```rust
// Source: CONTEXT.md D-06; clap derive docs (verified)
#[derive(Args)]
pub struct RecallArgs {
    /// Fetch a single memory by full UUID
    #[arg(long, value_name = "UUID")]
    pub id: Option<String>,

    /// Filter by agent_id
    #[arg(long, value_name = "ID")]
    pub agent_id: Option<String>,

    /// Filter by session_id
    #[arg(long, value_name = "ID")]
    pub session_id: Option<String>,

    /// Maximum number of results (default: 20)
    #[arg(long, value_name = "N", default_value_t = 20)]
    pub limit: u32,
}
```

### Pattern 3: List SQL Query (mirror of list_memories)

**What:** Two SQL queries — a COUNT for total and a SELECT for rows — using the same filter clause pattern as `service.rs:list_memories`. No offset (D-08).

**When to use:** `cmd_list_memories()` handler.

**Example:**
```rust
// Source: src/service.rs lines 245-289 (verified — adapted for recall, no offset)
let filter_clause = "WHERE (?1 IS NULL OR agent_id = ?1)
      AND (?2 IS NULL OR session_id = ?2)";

let count_sql = format!("SELECT COUNT(*) FROM memories {}", filter_clause);
let total: u64 = c.query_row(&count_sql,
    rusqlite::params![agent_id_c, session_id_c], |row| row.get(0))?;

let results_sql = format!(
    "SELECT id, content, agent_id, session_id, tags, embedding_model, created_at, updated_at
     FROM memories {}
     ORDER BY created_at DESC
     LIMIT ?3",
    filter_clause
);
```

### Pattern 4: Full Memory Fetch by ID

**What:** SELECT all columns for a single memory, returning `None` if not found via `OptionalExtension`.

**When to use:** `cmd_get_memory()` handler when `--id` is provided.

**Example:**
```rust
// Source: src/service.rs lines 316-332 (delete_memory — SELECT portion verified)
let mut stmt = c.prepare(
    "SELECT id, content, agent_id, session_id, tags, embedding_model, created_at, updated_at
     FROM memories WHERE id = ?1"
)?;
let memory = stmt.query_row(rusqlite::params![id], |row| {
    let tags_str: String = row.get(4)?;
    let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
    Ok(Memory {
        id: row.get(0)?,
        content: row.get(1)?,
        agent_id: row.get(2)?,
        session_id: row.get(3)?,
        tags,
        embedding_model: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}).optional()?;
```

### Pattern 5: Table Output (mirror of cmd_list)

**What:** Fixed-width columns with `format!("{:<width}")`, header + dashes separator, truncated fields.

**When to use:** `cmd_list_memories()` when results are non-empty.

**Example:**
```rust
// Source: src/cli.rs lines 115-143 (cmd_list verified — adapt for memory columns)
// D-09 specifies: ID(8), CONTENT(60), AGENT(15), CREATED(19)
let header = format!("{:<8}  {:<60}  {:<15}  {}", "ID", "CONTENT", "AGENT", "CREATED");
println!("{}", header);
println!("{}", "-".repeat(header.len()));
for mem in &memories {
    let id_short = if mem.id.len() >= 8 { &mem.id[..8] } else { &mem.id };
    let content = truncate(&mem.content, 60);
    let agent = if mem.agent_id.is_empty() { "(none)".to_string() }
                else { truncate(&mem.agent_id, 15) };
    let created = if mem.created_at.len() >= 19 { &mem.created_at[..19] }
                  else { &mem.created_at };
    println!("{:<8}  {:<60}  {:<15}  {}", id_short, content, agent, created);
}
// D-10 footer:
println!("Showing {} of {} memories", memories.len(), total);
```

### Pattern 6: Key-Value Detail Output (--id path)

**What:** Multi-line labeled output, no table, full content without truncation.

**When to use:** `cmd_get_memory()` when memory is found.

**Example:**
```rust
// Source: CONTEXT.md D-13
println!("ID:       {}", mem.id);
println!("Content:  {}", mem.content);
println!("Agent:    {}", if mem.agent_id.is_empty() { "(none)" } else { &mem.agent_id });
println!("Session:  {}", if mem.session_id.is_empty() { "(none)" } else { &mem.session_id });
let tags_display = if mem.tags.is_empty() { "(none)".to_string() }
                   else { mem.tags.join(", ") };
println!("Tags:     {}", tags_display);
println!("Model:    {}", mem.embedding_model);
println!("Created:  {}", mem.created_at);
println!("Updated:  {}", mem.updated_at.as_deref().unwrap_or("(never)"));
```

### Anti-Patterns to Avoid

- **Instantiating MemoryService for recall:** MemoryService requires `Arc<dyn EmbeddingEngine>`, which forces embedding model load (~2-3s). Run SQL directly from `Arc<Connection>` instead.
- **Putting init_db in main.rs:** main.rs is pure dispatch. Helper logic in cli.rs or a utility module keeps main.rs readable and testable.
- **Using positional args for `--id`:** D-06 explicitly chose flags over positional. The default behavior is listing; a positional would wrongly imply that providing an ID is the primary use case.
- **Calling validate_config in recall init:** validate_config rejects missing OPENAI_API_KEY even for DB-only commands. Keys arm deliberately skips it; recall must do the same.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Argument parsing with optional flags | Manual argc/argv parsing | clap derive with `Option<String>` fields | clap handles `--flag value`, help text, type coercion, and error messages with zero code |
| String truncation with ellipsis | Custom truncate fn | `truncate()` at cli.rs:154 | Already exists, tested, and used by cmd_list |
| UUID prefix display (first 8 chars) | Custom format fn | Inline `&mem.id[..8]` with bounds check | Simple enough inline; no helper needed |
| Async SQLite access | Spawning threads manually | `conn.call(move |c| { ... }).await` | tokio-rusqlite's `call()` correctly handles blocking SQLite on the dedicated thread pool |
| JSON tags deserialization | Manual string splitting | `serde_json::from_str::<Vec<String>>(&tags_str)` | Already used identically in list_memories() and delete_memory() |

**Key insight:** All infrastructure (async SQLite, clap, string helpers, DB init) already exists in the codebase. Phase 16 is entirely additive — new functions calling existing primitives.

---

## Common Pitfalls

### Pitfall 1: Partial Move of cli_args into Keys Arm Prevents db_override Access
**What goes wrong:** `cli_args.command` moves into the `Keys` match arm, making `cli_args.db` inaccessible afterwards unless extracted first.
**Why it happens:** Rust's ownership rules prevent using a field after any field of the struct has been moved.
**How to avoid:** Extract `let db_override = cli_args.db;` before the match — this is already done in main.rs (line 21). The Recall arm must use the same pre-extracted `db_override` variable.
**Warning signs:** `use of partially moved value` compiler error.

### Pitfall 2: validate_config Rejects Valid DB-Only Configs
**What goes wrong:** If the user has `embedding_provider = "openai"` in their config but no `MNEMONIC_OPENAI_API_KEY` set (because they run the server in Docker with secrets), calling `validate_config` in the recall path fails with a config error even though recall never touches embeddings.
**Why it happens:** `validate_config` enforces embedding constraints unconditionally.
**How to avoid:** Call only `load_config()` in the fast-path init — never `validate_config()`. Identical to the Keys arm (main.rs lines 29-33, comment confirms this: "skip validate_config").
**Warning signs:** Error `embedding_provider is "openai" but MNEMONIC_OPENAI_API_KEY is not set` when running `mnemonic recall`.

### Pitfall 3: Statement Borrow Held Across Transaction
**What goes wrong:** `rusqlite::Statement` borrows `&Connection`. If you hold a statement open while trying to begin a transaction, the compiler rejects it.
**Why it happens:** `Statement` is `!Send` and holds a mutable borrow on the connection.
**How to avoid:** For recall, this is only a risk in cmd_get_memory if it were to do a write after a read (it won't). For read-only queries, drop the `stmt` before any transaction — or use block scoping as in delete_memory() (lines 315-335). Recall is read-only so this pitfall is low probability but worth knowing.
**Warning signs:** `cannot borrow as mutable because it is also borrowed as immutable` compiler error involving `Connection`.

### Pitfall 4: Empty String vs NULL for agent_id/session_id
**What goes wrong:** The `memories` table stores agent_id and session_id as `TEXT NOT NULL DEFAULT ''` (empty string, not NULL). Filter queries using `?1 IS NULL OR agent_id = ?1` will correctly pass `None` as SQL NULL, but developers may mistakenly store empty string and then filter by `None` expecting to get those rows — they won't.
**Why it happens:** The schema uses empty string as the "no value" sentinel, not NULL.
**How to avoid:** In the display output, check `mem.agent_id.is_empty()` to show `(none)` — do not check for NULL. The SQL filter pattern from list_memories() is correct as-is.
**Warning signs:** `recall --agent-id ""` might match differently from bare `recall`. Test with real empty agent_id rows.

### Pitfall 5: truncate() Panics on Non-UTF-8 Boundary
**What goes wrong:** `&s[..max_len - 3]` in the truncate helper slices by bytes, not chars. If a multi-byte UTF-8 character spans the slice boundary, the slice panics.
**Why it happens:** Rust's string slicing is byte-indexed; slicing at a non-char boundary is a panic.
**How to avoid:** The existing `truncate()` has this same potential issue. For now it's an accepted limitation (matches existing behavior). If memory content contains multi-byte chars at exactly the truncation boundary, it will panic. Use `s.chars().take(max_len - 3).collect::<String>()` for a Unicode-safe alternative in the planner's discretion.
**Warning signs:** Panic at runtime with `byte index X is not a char boundary`.

### Pitfall 6: The `init_db` Helper Needs `register_sqlite_vec` Called Before It Returns
**What goes wrong:** `db::open()` internally opens a Connection, which triggers sqlite-vec auto-extension loading. If `register_sqlite_vec()` was not called first, the extension is not registered and vec_memories virtual table creation fails.
**Why it happens:** `register_sqlite_vec()` calls `sqlite3_auto_extension()` which registers the extension for all subsequent Connection opens. It must happen exactly once before any `Connection::open`.
**How to avoid:** The `init_db` helper must call `db::register_sqlite_vec()` as its first operation, identical to the existing Keys arm (main.rs line 27). The `Once` guard in `db::register_sqlite_vec()` makes duplicate calls safe.
**Warning signs:** Errors about `vec_memories` virtual table not existing, or sqlite3_vec extension not loaded.

---

## Code Examples

Verified patterns from existing codebase:

### Commands Enum Extension
```rust
// Source: src/cli.rs lines 21-27 (verified)
// Add Recall variant after Serve:
#[derive(Subcommand)]
pub enum Commands {
    Serve,
    Keys(KeysArgs),
    Recall(RecallArgs),  // NEW
}
```

### main.rs Recall Arm
```rust
// Source: src/main.rs lines 24-53 (verified — Keys arm pattern)
Some(cli::Commands::Recall(recall_args)) => {
    let (conn_arc, _config) = cli::init_db(db_override).await?;
    cli::run_recall(recall_args, conn_arc).await;
    return Ok(());
}
```

### run_recall Dispatch
```rust
// Source: CONTEXT.md D-17; mirrors run_keys() at cli.rs:57 (verified)
pub async fn run_recall(args: RecallArgs, conn: std::sync::Arc<tokio_rusqlite::Connection>) {
    if let Some(id) = args.id {
        cmd_get_memory(conn, id).await;
    } else {
        cmd_list_memories(conn, args.agent_id, args.session_id, args.limit).await;
    }
}
```

### OptionalExtension Usage for Nullable Row
```rust
// Source: src/service.rs lines 300-307 (get_memory_agent_id — verified)
use rusqlite::OptionalExtension;
stmt.query_row(rusqlite::params![id], |row| { ... })
    .optional()   // converts Err(QueryReturnedNoRows) to Ok(None)
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Direct rusqlite Connection in main thread | tokio-rusqlite `Connection::call()` | v1.0 | All DB access must use the `call()` closure API — no direct rusqlite calls from async context |
| MemoryService wraps all DB access | Direct `Arc<Connection>` for CLI fast paths | v1.2 | Keys subcommand established this pattern; recall extends it |
| Single `if let` dispatch for Commands | `match cli_args.command` with multiple arms | Phase 15 | Phase 16 adds a third arm to the match |
| Duplicate DB init per match arm | Shared `init_db` helper in cli.rs | Phase 16 (D-04) | Deduplicates 10-line init block; plan must include this extraction |

**Deprecated/outdated:**
- `if let Some(Commands::Keys(...))` early return: Phase 15 already converted this to a `match`. Phase 16 extends that match — don't revert to `if let`.

---

## Open Questions

1. **Mutual exclusivity enforcement for `--id` vs filter flags**
   - What we know: D-06 notes `--id` is "mutually exclusive with filter flags" but leaves enforcement to Claude's discretion.
   - What's unclear: Whether to use clap `ArgGroup` with `conflicts_with` or a runtime check.
   - Recommendation: Use a runtime check with `eprintln!("error: --id cannot be combined with --agent-id, --session-id, or --limit")` + exit(1). This matches the existing error handling style (eprintln + process::exit) and avoids clap group complexity. The planner should decide.

2. **Visibility of `truncate()` helper**
   - What we know: `truncate()` is currently `fn truncate` (private, no `pub`) in cli.rs at line 154.
   - What's unclear: Whether the recall handlers (added in the same file) can call it as-is, or need it to be `pub(crate)`.
   - Recommendation: Private `fn` is accessible within the same module (`cli.rs`). No visibility change needed since recall handlers live in the same file. Confirmed by existing test module usage at cli.rs line 317-331.

3. **init_db placement: cli.rs vs new module**
   - What we know: D-05 says "NOT in main.rs" but doesn't specify cli.rs vs new file.
   - What's unclear: Whether cli.rs will become too long once recall + init_db are added.
   - Recommendation: cli.rs currently has 332 lines. Adding RecallArgs (~10 lines), run_recall (~10 lines), cmd_list_memories (~35 lines), cmd_get_memory (~25 lines), init_db (~15 lines), and tests (~50 lines) brings it to ~477 lines — manageable in one file. No new module needed.

---

## Validation Architecture

> `nyquist_validation: true` in `.planning/config.json` — section included.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in (cargo test + tokio::test for async) |
| Config file | None — Cargo.toml `[dev-dependencies]` is the config |
| Quick run command | `cargo test -p mnemonic --lib -- cli 2>&1 | tail -20` |
| Full suite command | `cargo test -p mnemonic 2>&1 | tail -40` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| RCL-01 | `mnemonic recall` lists 20 most recent memories in table format | integration (binary) | `cargo test -p mnemonic --test cli_integration test_recall` | ❌ Wave 0 |
| RCL-01 | Empty state prints "No memories found." | integration (binary) | `cargo test -p mnemonic --test cli_integration test_recall_empty` | ❌ Wave 0 |
| RCL-01 | Footer shows "Showing X of Y memories" | integration (binary) | `cargo test -p mnemonic --test cli_integration test_recall_footer` | ❌ Wave 0 |
| RCL-02 | `mnemonic recall --id <uuid>` prints key-value detail format | integration (binary) | `cargo test -p mnemonic --test cli_integration test_recall_by_id` | ❌ Wave 0 |
| RCL-02 | `--id <nonexistent>` exits 1 with stderr message | integration (binary) | `cargo test -p mnemonic --test cli_integration test_recall_by_id_notfound` | ❌ Wave 0 |
| RCL-03 | `--agent-id` filters return only matching rows | integration (binary) | `cargo test -p mnemonic --test cli_integration test_recall_filter_agent` | ❌ Wave 0 |
| RCL-03 | `--session-id` filter works | integration (binary) | `cargo test -p mnemonic --test cli_integration test_recall_filter_session` | ❌ Wave 0 |
| RCL-03 | `--limit` limits row count | integration (binary) | `cargo test -p mnemonic --test cli_integration test_recall_limit` | ❌ Wave 0 |
| RCL-01 | Unit: cmd_list_memories returns correct Memory structs from in-memory DB | unit (lib) | `cargo test -p mnemonic --lib -- cli::tests::test_cmd_list_memories` | ❌ Wave 0 |
| RCL-02 | Unit: cmd_get_memory returns full Memory or exits on missing | unit (lib) | `cargo test -p mnemonic --lib -- cli::tests::test_cmd_get_memory` | ❌ Wave 0 |

**Note on integration test strategy:** The existing `tests/cli_integration.rs` already has the infrastructure pattern — `TempDb` struct, `binary()` helper, `std::process::Command` invocations. All recall integration tests must use this same infrastructure. However, recall tests need a way to pre-populate memories in the temp DB before running the binary. Since the `remember` subcommand (Phase 17) doesn't exist yet, tests should insert rows directly via SQLite (rusqlite in-process) before invoking the binary CLI.

### Sampling Rate
- **Per task commit:** `cargo test -p mnemonic --lib -- cli 2>&1 | tail -20`
- **Per wave merge:** `cargo test -p mnemonic 2>&1 | tail -40`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `tests/cli_integration.rs` — extend with recall integration tests (file exists, add new test functions)
- [ ] `src/cli.rs` — extend `#[cfg(test)] mod tests` with unit tests for `cmd_list_memories`, `cmd_get_memory`, and `init_db`
- [ ] No new test infrastructure file needed — `TempDb` and `binary()` helpers already exist in `tests/cli_integration.rs`
- [ ] Test helper to pre-seed memories in TempDb for recall integration tests (direct rusqlite insert, not via CLI)

---

## Sources

### Primary (HIGH confidence)
- `src/cli.rs` (verified in full) — Commands enum, KeysArgs pattern, run_keys(), truncate(), existing unit tests
- `src/main.rs` (verified in full) — Keys fast-path init, match dispatch, db_override extraction pattern
- `src/service.rs` (verified in full) — list_memories() SQL at lines 229-293, Memory struct at line 58, ListParams at line 48, get_memory_agent_id() at line 298, delete_memory() full-fetch SELECT at lines 316-332
- `src/db.rs` (verified in full) — register_sqlite_vec(), db::open(), schema with memories table columns
- `src/config.rs` (verified in full) — load_config(), validate_config(), Config struct
- `Cargo.toml` (verified) — confirmed no new dependencies needed; clap 4.x derive already present
- `.planning/phases/16-recall-subcommand/16-CONTEXT.md` (verified) — all locked decisions D-01 through D-18
- `tests/cli_integration.rs` (verified) — TempDb pattern, binary() helper, existing test structure

### Secondary (MEDIUM confidence)
- `.planning/REQUIREMENTS.md` — RCL-01, RCL-02, RCL-03 definitions confirmed
- `.planning/STATE.md` — prior decisions: "recall is minimal init (DB only, ~50ms)" confirmed

### Tertiary (LOW confidence)
- None — all claims are verified against the codebase directly.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries already in Cargo.toml, versions verified in Cargo.toml
- Architecture: HIGH — DB-only fast path and SQL patterns verified directly in service.rs and main.rs
- Pitfalls: HIGH — each pitfall verified against actual code (partial-move issue in existing main.rs comment, validate_config skip confirmed in Keys arm comment, OptionalExtension usage verified in get_memory_agent_id)

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable codebase — no external dependencies changing)
