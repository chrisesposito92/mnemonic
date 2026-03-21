# Architecture Research

**Domain:** Rust single-binary agent memory server — v1.3 CLI subcommands integration
**Researched:** 2026-03-21
**Confidence:** HIGH (direct code inspection of shipped v1.2 codebase, well-established Rust CLI patterns)

---

## Context: What Already Exists (v1.2)

The v1.2 binary is 5,925 lines of Rust across 12 source files. The dispatch pattern is already established and working:

```
main.rs
  └── clap parse
        ├── Some(Commands::Keys(args)) → minimal init: db only, no model load
        │     └── cli::run_keys(args, key_service)  → print, exit
        └── None (no subcommand)       → full server init: load model, bind port
              └── server::serve(config, AppState)
```

The critical architectural precedent from v1.2: **CLI commands go direct-to-DB, not through HTTP**. The `keys` subcommand opens only the DB, skips embedding model load (~2s), constructs only `KeyService`, and exits. This is the pattern that `remember`, `recall`, `search`, and `compact` must follow.

**v1.2 AppState (server path only):**
```
AppState {
    service:    Arc<MemoryService>,     // needs: Arc<Connection>, Arc<EmbeddingEngine>
    compaction: Arc<CompactionService>, // needs: Arc<Connection>, Arc<EmbeddingEngine>, Option<Arc<SummarizationEngine>>
    key_service: Arc<KeyService>,       // needs: Arc<Connection>
}
```

---

## v1.3 System Overview

```
┌────────────────────────────────────────────────────────────────────────────┐
│                           Entry Point (main.rs)                             │
│                                                                             │
│   clap parse → Commands enum                                                │
│                                                                             │
│   Serve     → full init → server::serve()  [existing behavior, unchanged]  │
│   Remember  → medium init (db + embedding) → cli::run_remember() → exit    │
│   Recall    → minimal init (db only)       → cli::run_recall()   → exit    │
│   Search    → medium init (db + embedding) → cli::run_search()   → exit    │
│   Compact   → medium init (db + embedding) → cli::run_compact()  → exit    │
│   Keys      → minimal init (db only)       → cli::run_keys()     → exit    │
│   (none)    → full init → server::serve()  [default, backward compat]      │
└────────────────────────────────────────────────────────────────────────────┘

Init tiers (determines startup cost):
  Minimal  (db only)       : ~50ms  — recall, keys
  Medium   (db + embedding): ~2-3s  — remember, search, compact
  Full     (db + embed + LLM + server bind): ongoing — serve
```

### Component Responsibilities

| Component | v1.3 Change | Notes |
|-----------|-------------|-------|
| `cli.rs` | MAJOR CHANGE: add 5 new subcommands + handlers | Pattern already established by `run_keys`; same module |
| `main.rs` | MODIFIED: add 5 new `Commands` variants + dispatch branches | Each branch selects its init tier |
| `MemoryService` (service.rs) | NO CHANGE | CLI callers use it directly, same methods as HTTP handlers |
| `CompactionService` (compaction.rs) | NO CHANGE | CLI `compact` calls `compaction.compact(req)` directly |
| `KeyService` (auth.rs) | NO CHANGE | Already CLI-ready |
| `EmbeddingEngine` (embedding.rs) | NO CHANGE | CLI `remember` and `search` need it; existing `LocalEngine` |
| `AppState` (server.rs) | NO CHANGE | Server path unchanged |
| All other modules | NO CHANGE | db.rs, config.rs, error.rs, server.rs unaffected |

---

## v1.3 Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                          CLI Layer (cli.rs)                          │
│                                                                      │
│  run_remember() | run_recall() | run_search() | run_compact()        │
│  run_keys()  (existing)                                              │
│                                                                      │
│  Output: human-readable tables/text to stdout                        │
│  Errors: descriptive message to stderr + exit(1)                     │
└──────────────────────┬─────────────────────┬────────────────────────┘
                       │                     │
           ┌───────────▼──────────┐  ┌───────▼──────────────┐
           │  Service Layer        │  │  Service Layer        │
           │                       │  │                       │
           │  MemoryService        │  │  CompactionService    │
           │  create_memory()      │  │  compact()            │
           │  list_memories()      │  │                       │
           │  search_memories()    │  │  (Arc<EmbeddingEngine>│
           │  delete_memory()      │  │   inside — CLI must   │
           │                       │  │   load embedding too) │
           └───────────┬───────────┘  └───────────────────────┘
                       │
           ┌───────────▼──────────────────────────────────────┐
           │  Storage Layer                                     │
           │  Arc<tokio_rusqlite::Connection>                   │
           │  memories + vec_memories + compact_runs + api_keys│
           └────────────────────────────────────────────────────┘
```

---

## Recommended Project Structure (v1.3 delta)

No new files are needed. All changes are additive within existing modules.

```
src/
├── main.rs      # MODIFIED: add 5 new Commands variants + dispatch branches
├── cli.rs       # MAJOR CHANGE: add run_remember, run_recall, run_search,
│                #   run_compact, with argument structs and output formatting
├── service.rs   # No change
├── compaction.rs # No change
├── embedding.rs # No change
├── auth.rs      # No change
├── server.rs    # No change
├── db.rs        # No change
├── config.rs    # No change
├── error.rs     # No change
├── summarization.rs # No change
└── lib.rs       # No change
```

**Rationale:** The v1.2 `cli.rs` module was built explicitly to hold all CLI handlers. Adding the five new handlers there keeps the pattern consistent. No new module, no new file, no structural change to the rest of the codebase.

---

## Architectural Patterns

### Pattern 1: Tiered Init — Match Init Cost to Subcommand Needs

**What:** main.rs dispatch selects the minimum initialization required for each subcommand. There are three tiers:

- **Minimal** (db only, ~50ms): subcommands that only touch the key or memory table without needing embeddings. `recall` (list/get by ID/filter), `keys` (all key ops).
- **Medium** (db + embedding engine, ~2-3s): subcommands that embed text. `remember` (embeds content before INSERT), `search` (embeds query for KNN), `compact` (re-embeds during clustering).
- **Full** (db + embedding + LLM + server bind): `serve`.

**When to use:** Always for CLI commands in a binary that also contains expensive-to-load components (models, network clients).

**Trade-offs:** Medium init loads the embedding model even for `compact --dry-run` where no new memories are written. Accept this — dry-run still needs embeddings to compute similarities. The ~2s model load is acceptable for a CLI operation that takes 5-10s total on any real dataset.

**Example dispatch in main.rs:**

```rust
match cli_args.command {
    // Minimal init: DB only
    Some(Commands::Keys(keys_args)) => {
        db::register_sqlite_vec();
        let mut config = config::load_config().map_err(|e| anyhow::anyhow!(e))?;
        if let Some(db_override) = cli_args.db { config.db_path = db_override; }
        let conn = Arc::new(db::open(&config).await.map_err(|e| anyhow::anyhow!(e))?);
        let key_service = auth::KeyService::new(conn);
        cli::run_keys(keys_args.subcommand, key_service).await;
    }
    Some(Commands::Recall(recall_args)) => {
        db::register_sqlite_vec();
        let mut config = config::load_config().map_err(|e| anyhow::anyhow!(e))?;
        if let Some(db_override) = cli_args.db { config.db_path = db_override; }
        let conn = Arc::new(db::open(&config).await.map_err(|e| anyhow::anyhow!(e))?);
        let service = service::MemoryService::new(conn, /* ... */);
        cli::run_recall(recall_args, service).await;
    }
    // Medium init: DB + embedding
    Some(Commands::Remember(remember_args)) => {
        db::register_sqlite_vec();
        let mut config = config::load_config().map_err(|e| anyhow::anyhow!(e))?;
        if let Some(db_override) = cli_args.db { config.db_path = db_override; }
        let (conn, embedding) = init_db_and_embedding(&config).await?;
        let service = service::MemoryService::new(conn, embedding, embedding_model(&config));
        cli::run_remember(remember_args, service).await;
    }
    // ... Search, Compact similarly (medium init)
    // Full init: existing server path
    Some(Commands::Serve) | None => {
        // existing main.rs server init — unchanged
    }
}
```

Extract `init_db_and_embedding(&config)` as a private helper in main.rs to avoid duplicating the model-load block across three branches.

### Pattern 2: CLI Handlers Call Service Layer Directly (Not HTTP)

**What:** CLI handlers receive service objects and call their methods directly. No HTTP client, no `reqwest`, no JSON serialization round-trip.

**When to use:** Always, for this binary. The alternative (HTTP client calling `localhost:8080`) requires the server to be running, creates a process dependency, adds network stack overhead, and complicates error handling (what if the server is not running?).

**Trade-offs:**
- Direct DB access means CLI commands work whether the server is running or not.
- A concurrent `mnemonic remember` while `mnemonic serve` is running will share the same SQLite file. SQLite WAL mode handles this safely — concurrent readers/one writer. The CLI write will serialize with any in-flight server writes transparently.
- No duplication of business logic: `run_remember` calls `service.create_memory()` which contains the same embed-then-insert logic the HTTP handler uses.

**Example `run_remember`:**

```rust
pub async fn run_remember(args: RememberArgs, service: MemoryService) {
    let req = CreateMemoryRequest {
        content: args.content,
        agent_id: args.agent_id,
        session_id: args.session_id,
        tags: args.tags,
    };
    match service.create_memory(req).await {
        Ok(memory) => {
            println!("{}", memory.id);  // ID on its own line — pipeable
            println!("Content: {}", memory.content);
            println!("Agent:   {}", memory.agent_id);
            println!("Created: {}", memory.created_at);
        }
        Err(e) => {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
    }
}
```

### Pattern 3: `serve` as Named Subcommand with Default Fallback

**What:** The v1.3 `Commands` enum gains a `Serve` variant. `mnemonic serve` is explicit. `mnemonic` (no subcommand) also runs the server via `None` match arm. Both arms execute identical code.

**When to use:** When backward compatibility with `mnemonic` (no args) must be preserved while also supporting `mnemonic serve` for clarity in scripts and docs.

**Trade-offs:** The `serve` subcommand is redundant with `mnemonic` (no args), but makes intent explicit in shell scripts and process supervisors (e.g., `CMD ["mnemonic", "serve"]` in a Dockerfile). Preserving the no-subcommand default means no existing deployments break.

**clap definition:**

```rust
#[derive(Subcommand)]
pub enum Commands {
    /// Start the HTTP server (also the default with no subcommand)
    Serve,
    /// Store a memory from the command line
    Remember(RememberArgs),
    /// Retrieve memories by ID or filters
    Recall(RecallArgs),
    /// Semantic search across memories
    Search(SearchArgs),
    /// Trigger memory compaction
    Compact(CompactArgs),
    /// Manage API keys
    Keys(KeysArgs),  // existing
}
```

The `None` arm in main.rs dispatch simply runs the same server init as `Some(Commands::Serve)`. No duplication risk — extract `run_server(config)` as a named async fn.

### Pattern 4: Output Design for Pipeable CLI

**What:** CLI handlers print machine-friendly output to stdout and human-readable errors/warnings to stderr. The first significant value (ID, count, or result line) is on its own line to enable piping.

**When to use:** All CLI handlers. This mirrors the existing `run_keys create` behavior where the raw token is line 1 of stdout.

**Trade-offs:** Tabular output (like `keys list`) is less pipeable but more readable for humans. The correct split:
- `remember` → print ID on line 1 (pipeable: `ID=$(mnemonic remember "text")`)
- `recall` → print table rows
- `search` → print table rows with distance
- `compact` → print summary counts

**`--json` flag (optional, deferred):** A `--json` flag on each subcommand that emits JSON instead of tabular output would make scripting trivial. Do not build this in v1.3 — the tabular format is sufficient and `--json` can be added without any service-layer changes.

### Pattern 5: `recall` Uses list_memories, Not search_memories

**What:** `mnemonic recall` is a filter-based retrieval (by agent_id, session_id, tag, ID) using `MemoryService::list_memories()` or `get_memory_by_id()`. It does NOT do semantic search. `mnemonic search` is the semantic search command.

**When to use:** Distinguish commands by intent — `recall` = structured filter, `search` = semantic similarity.

**Trade-offs:** Users may conflate the two. Solve with clear help text: `recall` is "list memories matching filters" and `search` is "find memories semantically similar to a query string."

**`recall --id <UUID>` fast path:** If `--id` is given, bypass `list_memories` and fetch the single memory by primary key. `MemoryService` already has the DB query pattern — add a `get_memory()` method or inline the query in `run_recall`. One new method on `MemoryService` is acceptable.

---

## Data Flow: New CLI Subcommands

### remember flow

```
mnemonic remember "The capital of France is Paris" --agent-id agent1 --tag geography
  |
  v
main.rs: medium init → Arc<Connection>, Arc<EmbeddingEngine>
  |
  v
MemoryService::new(conn, embedding, embedding_model)
  |
  v
cli::run_remember(args, service)
  |
  v
service.create_memory(CreateMemoryRequest { content, agent_id, session_id, tags })
  ├── embedding.embed(content)        → Vec<f32>  (local model or OpenAI)
  └── db INSERT memories + vec_memories (atomic transaction)
  |
  v
stdout: memory ID on line 1
        content, agent_id, created_at summary
```

### recall flow

```
mnemonic recall --agent-id agent1 --limit 5
  |
  v
main.rs: minimal init → Arc<Connection> only (no embedding needed)
  |
  v
cli::run_recall(args, service)
  |
  v
service.list_memories(ListParams { agent_id, session_id, tag, after, before, limit, offset })
  └── db SELECT memories WHERE filters ORDER BY created_at DESC LIMIT N
  |
  v
stdout: formatted table rows (ID, content truncated, agent, created)
```

```
mnemonic recall --id 019541a0-...
  |
  v
minimal init
  |
  v
service.get_memory(id)   [new method or direct query in run_recall]
  └── db SELECT memories WHERE id = ?
  |
  v
stdout: full memory details
```

### search flow

```
mnemonic search "Paris landmarks" --agent-id agent1 --limit 5
  |
  v
main.rs: medium init (embedding needed to embed query)
  |
  v
cli::run_search(args, service)
  |
  v
service.search_memories(SearchParams { q: "Paris landmarks", agent_id, limit, threshold, ... })
  ├── embedding.embed("Paris landmarks") → Vec<f32>
  └── db KNN via sqlite-vec MATCH + filter JOIN
  |
  v
stdout: table with columns: SCORE, ID, CONTENT (truncated), AGENT
```

### compact flow

```
mnemonic compact --agent-id agent1 --dry-run
  |
  v
main.rs: medium init (embedding needed for similarity clustering)
  |
  v
cli::run_compact(args, compaction_service)
  |
  v
compaction.compact(CompactRequest { agent_id, threshold, max_candidates, dry_run: true })
  ├── loads all memories for agent → embed each (or use stored vectors)
  ├── pairwise similarity clustering
  └── (dry-run: returns plan, no writes)
  |
  v
stdout: "Found N clusters, would merge M memories into K"
        id_mapping table if --verbose
```

---

## Integration Points

### New vs. Modified

| Component | Status | What Changes |
|-----------|--------|--------------|
| `cli.rs` | MODIFIED (major) | Add `RememberArgs`, `RecallArgs`, `SearchArgs`, `CompactArgs` structs; add `run_remember()`, `run_recall()`, `run_search()`, `run_compact()` functions |
| `main.rs` | MODIFIED | Add `Commands::Serve`, `Commands::Remember`, `Commands::Recall`, `Commands::Search`, `Commands::Compact` variants; add dispatch branches; extract `init_db_and_embedding()` helper; extract `run_server()` helper |
| `service.rs` | POSSIBLY MODIFIED | May need `get_memory(id: &str)` for `recall --id`. All other existing methods are already usable from CLI without change |
| Everything else | NO CHANGE | server.rs, AppState, auth.rs, db.rs, compaction.rs, embedding.rs, error.rs, config.rs, summarization.rs all untouched |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| `cli::run_remember` ↔ `MemoryService` | Direct async fn call (same binary, same process) | No HTTP, no serialization overhead |
| `cli::run_search` ↔ `MemoryService` | Direct async fn call | Same `search_memories()` method the HTTP handler uses |
| `cli::run_compact` ↔ `CompactionService` | Direct async fn call | Same `compact()` method the HTTP handler uses |
| `cli::run_recall` ↔ `MemoryService` | Direct async fn call | Same `list_memories()` method the HTTP handler uses |
| CLI path ↔ SQLite | Concurrent access via WAL mode | Safe: CLI writes serialize with server writes at the SQLite layer; no application-level locking needed |
| `main.rs` medium init ↔ `LocalEngine` | `tokio::task::spawn_blocking(|| LocalEngine::new())` | Same pattern as server path; model downloaded to `~/.cache/huggingface/` and cached |

### External Dependencies (unchanged from v1.2)

| Dependency | Version | Used by v1.3 CLI |
|------------|---------|-----------------|
| clap (derive) | 4.x | arg structs for new subcommands |
| tokio-rusqlite | existing | same `conn.call()` pattern |
| candle / hf-hub | existing | medium-init model load |
| serde_json | existing | compact response pretty-print optional |

No new Cargo.toml entries required for v1.3.

---

## Anti-Patterns

### Anti-Pattern 1: CLI Commands Going Through HTTP

**What people do:** Implement `mnemonic remember` as an HTTP client that calls `POST /memories` on `localhost:8080`.

**Why it's wrong:** Requires the server to be running. Fails silently if the server is down. Adds network stack overhead. Makes CLI commands useless as standalone tools (e.g., scripts that run before starting the server). Duplicates auth logic (CLI would need to supply an API key to talk to the server).

**Do this instead:** CLI commands call service methods directly. The binary contains both the server and the service implementations — use them directly. SQLite WAL mode handles concurrent access between a running server and a CLI invocation transparently.

### Anti-Pattern 2: Loading the Embedding Model for recall

**What people do:** Use the same "medium init" path for all non-serve subcommands.

**Why it's wrong:** `mnemonic recall` does not embed anything — it does structured filter queries. Loading the embedding model adds ~2s to a command that should complete in <100ms.

**Do this instead:** `recall` uses minimal init (DB only). Only `remember`, `search`, and `compact` need the embedding model.

### Anti-Pattern 3: Adding a New Module for CLI Handlers

**What people do:** Create `src/commands/remember.rs`, `src/commands/recall.rs`, etc.

**Why it's wrong:** Over-engineering. The `cli.rs` module was designed to hold all CLI handlers. The total new code is ~200-300 lines. Adding a module hierarchy now would make the codebase harder to navigate without benefit.

**Do this instead:** Add all five new handler functions and their arg structs to `cli.rs`. If `cli.rs` grows beyond ~800 lines, re-evaluate. At v1.3 it will not.

### Anti-Pattern 4: Duplicating Init Logic Across Each Dispatch Branch

**What people do:** Copy-paste the DB open + embedding load block into each of the `Remember`, `Search`, and `Compact` dispatch branches.

**Why it's wrong:** Three identical blocks that must stay in sync. When config fields change (e.g., adding `--model` flag), all three blocks must be updated.

**Do this instead:** Extract `init_db_and_embedding(config: &Config) -> Result<(Arc<Connection>, Arc<dyn EmbeddingEngine>)>` as a private async fn in main.rs. All medium-init branches call it.

### Anti-Pattern 5: Reimplementing Business Logic in CLI Handlers

**What people do:** Write a `run_search` that manually constructs and executes a SQL query instead of calling `service.search_memories()`.

**Why it's wrong:** Duplicates logic that already exists, is already tested, and already handles edge cases (KNN over-fetch ratio, threshold filtering, etc.). Any future changes to search must be made in two places.

**Do this instead:** CLI handlers are thin wrappers around service calls. They handle argument parsing, call the service, and format the output. Nothing more.

### Anti-Pattern 6: Blocking the Tokio Runtime During Model Load in CLI Path

**What people do:** Call `LocalEngine::new()` directly in an async fn.

**Why it's wrong:** `LocalEngine::new()` is blocking (HF Hub I/O, model weight parsing). Calling it directly in an async context blocks the tokio executor.

**Do this instead:** Same pattern as the server path: `tokio::task::spawn_blocking(|| LocalEngine::new()).await??`. This is already in main.rs for the server path — copy the pattern to the medium-init helper.

---

## Build Order (v1.3 phases, considering dependencies)

```
Phase A — Extend Commands enum + serve subcommand
  1. cli.rs: add ServeArgs (empty, or maybe --port override)
  2. main.rs: add Commands::Serve variant; extract run_server() helper
     Result: `mnemonic serve` works; `mnemonic` (no args) still works
     Test: existing integration tests pass unchanged

Phase B — recall (minimal init, no embedding)
  1. service.rs: add get_memory(id: &str) method if needed for --id fast path
  2. cli.rs: add RecallArgs struct (--id, --agent-id, --session-id, --tag,
     --limit, --after, --before) + run_recall() handler with table output
  3. main.rs: add Commands::Recall dispatch branch (minimal init)
     Result: `mnemonic recall` works, fast (<100ms)
     Test: unit test run_recall with test DB

Phase C — remember (medium init, embedding required)
  1. main.rs: extract init_db_and_embedding() helper (needed by C, D, E)
  2. cli.rs: add RememberArgs struct (content, --agent-id, --session-id,
     --tag) + run_remember() handler
  3. main.rs: add Commands::Remember dispatch branch (medium init)
     Result: `mnemonic remember "text"` stores a memory, prints ID
     Test: unit test run_remember with test DB + mock embedding

Phase D — search (medium init, embedding required)
  1. cli.rs: add SearchArgs struct (query, --agent-id, --limit, --threshold,
     --tag) + run_search() handler with table output including score
  2. main.rs: add Commands::Search dispatch branch (medium init)
     Result: `mnemonic search "query"` returns ranked results
     Test: unit test run_search with test DB + mock embedding

Phase E — compact (medium init, embedding required)
  1. cli.rs: add CompactArgs struct (--agent-id, --threshold,
     --max-candidates, --dry-run) + run_compact() handler with summary output
  2. main.rs: add Commands::Compact dispatch branch (medium init,
     constructs CompactionService instead of MemoryService)
     Result: `mnemonic compact --agent-id X` triggers compaction
     Test: unit test run_compact with test DB + mock embedding
```

**Rationale for this ordering:**
- `serve` first because it's a zero-risk rename of existing behavior; confirms the Commands expansion doesn't break anything.
- `recall` second because it uses minimal init — fastest to implement, no embedding complexity, validates the new dispatch infrastructure.
- `remember` third because it introduces the medium-init helper that `search` and `compact` will reuse.
- `search` fourth because it follows the same medium-init pattern with a different service call.
- `compact` last because `CompactionService` construction is more complex (needs optional LLM engine) and `compact --dry-run` is the highest-value test case.

---

## Concurrent Access: CLI + Running Server

SQLite WAL (Write-Ahead Logging) mode is already enabled by `db::open()`. This makes the concurrent-access scenario safe by design:

```
mnemonic serve  (running)
  + concurrent:
mnemonic remember "..."  (CLI)

→ Both open the same .db file
→ WAL allows concurrent readers + one writer
→ CLI write acquires write lock, inserts memory, releases
→ Server reads proceed concurrently (not blocked by CLI write)
→ No data corruption, no application-level locking needed
```

The only edge case: if both the server and a CLI command attempt to write at the exact same microsecond, one will wait for the other's WAL lock. This is handled transparently by SQLite. For a tool at this scale (personal agent memory server), this is not a problem.

---

## Scaling Considerations

| Scale | CLI Architecture Behavior |
|-------|--------------------------|
| Single user, local | Default. No issues. CLI and server share the DB file safely. |
| Multiple agents, same host | CLI `--agent-id` filtering keeps namespaces isolated. No change needed. |
| Remote deployment | CLI subcommands work by pointing to the remote DB file via `--db /path/to/remote.db` or `MNEMONIC_DB_PATH`. SSH + direct DB access or just use the REST API from remote. |

The CLI is intentionally a local-only interface. Remote programmatic access uses the REST API. This is the correct split.

---

## Sources

- Existing v1.2 source code (`src/main.rs`, `src/cli.rs`, `src/service.rs`, `src/compaction.rs`) — HIGH confidence (direct inspection of shipped code)
- clap documentation — subcommand patterns: https://docs.rs/clap/latest/clap/_derive/_tutorial/chapter_0/index.html — HIGH confidence (official docs)
- SQLite WAL mode documentation — concurrent access guarantees: https://www.sqlite.org/wal.html — HIGH confidence (official docs)
- v1.2 ARCHITECTURE.md (existing research) — Pattern 4 (Dual-Mode Binary with clap) directly informs v1.3 extension — HIGH confidence

---

*Architecture research for: Mnemonic v1.3 — CLI subcommands integration*
*Researched: 2026-03-21*
