# Architecture Research

**Domain:** Rust single-binary agent memory server — v1.1 memory compaction/summarization integration
**Researched:** 2026-03-20
**Confidence:** HIGH (existing system) / MEDIUM (new component integration patterns)

---

## Context: What Already Exists (v1.0)

The v1.0 binary is 1,932 lines of Rust across 8 source files with a strict 4-layer architecture:

```
axum HTTP handlers (server.rs)
        |
        v
MemoryService orchestrator (service.rs)
   |                 |
   v                 v
EmbeddingEngine    SQLite via tokio-rusqlite
(embedding.rs)       (db.rs)
```

**Key facts that constrain v1.1 design:**

- `MemoryService` holds `Arc<Connection>` and `Arc<dyn EmbeddingEngine>` — both are already `Arc`-wrapped and cheaply cloneable.
- `tokio_rusqlite::Connection` is already cloneable (cheap handle clone, single background thread). WAL mode is already enabled. All DB calls go through `.call()` closures — this pattern must be preserved.
- `EmbeddingEngine` is a trait with two concrete impls (`LocalEngine`, `OpenAiEngine`). The `MockEmbeddingEngine` exists in tests. The pattern is proven and extensible.
- `Config` in `config.rs` uses `figment` with TOML + env-var override. Adding `llm_provider` / `llm_api_key` fields follows the exact same pattern as the existing `embedding_provider` / `openai_api_key`.
- All errors flow through `ApiError → MnemonicError` hierarchy in `error.rs` using `thiserror`. New errors for compaction/LLM follow this pattern.

---

## v1.1 System Overview

```
┌──────────────────────────────────────────────────────────────────────┐
│                          HTTP Layer (axum)                            │
│                                                                      │
│  ┌─────────────────┐  ┌──────────────────┐  ┌──────────────────┐    │
│  │ POST /memories  │  │ GET /memories    │  │DELETE /memories  │    │
│  │                 │  │ GET /memories    │  │       /{id}      │    │
│  │                 │  │ /search          │  │                  │    │
│  └────────┬────────┘  └────────┬─────────┘  └────────┬─────────┘    │
│           │                   │                      │              │
│  ┌────────┴────────────────────────────────────────────────────────┐ │
│  │              POST /memories/compact  (NEW v1.1)                 │ │
│  └────────┬───────────────────────────────────────────────────────┘ │
│           │                                                          │
├───────────┴──────────────────────────────────────────────────────────┤
│                         Service Layer                                 │
│                                                                      │
│  ┌──────────────────────────┐   ┌──────────────────────────────────┐ │
│  │     MemoryService        │   │     CompactionService  (NEW)     │ │
│  │  (existing, unchanged)   │   │                                  │ │
│  │  create_memory()         │   │  compact()                       │ │
│  │  search_memories()       │   │    1. fetch all memories by      │ │
│  │  list_memories()         │   │       agent_id (+ filters)       │ │
│  │  delete_memory()         │   │    2. cluster by cosine sim      │ │
│  └──────────────────────────┘   │    3. merge metadata in cluster  │ │
│                                 │    4. (opt) call SummarizationEng│ │
│                                 │    5. write compacted memory     │ │
│                                 │    6. delete source memories     │ │
│                                 └────────────┬─────────────────────┘ │
│                                              │                       │
├──────────────────────────────────────────────┼───────────────────────┤
│               Engine Layer                   │                       │
│                                              │                       │
│  ┌────────────────────────┐  ┌───────────────┴────────────────────┐  │
│  │   EmbeddingEngine      │  │    SummarizationEngine   (NEW)     │  │
│  │   (existing trait)     │  │    (new trait, parallel pattern)   │  │
│  │                        │  │                                    │  │
│  │   LocalEngine          │  │    OpenAiSummarizer                │  │
│  │   OpenAiEngine         │  │    (only impl initially)           │  │
│  │   MockEmbeddingEngine  │  │    MockSummarizer  (for tests)     │  │
│  └─────────────┬──────────┘  └───────────────┬────────────────────┘  │
│                │                             │                       │
├────────────────┴─────────────────────────────┴───────────────────────┤
│                         Storage Layer                                 │
│                                                                      │
│   Arc<tokio_rusqlite::Connection>  (existing, shared via clone)      │
│                                                                      │
│  ┌────────────────────────────┐  ┌─────────────────────────────┐    │
│  │  memories (existing table) │  │  vec_memories (existing vec0)│    │
│  │  + compaction_source_ids   │  │                             │    │
│  │  + compacted_at  (NEW cols)│  │  no schema change needed    │    │
│  └────────────────────────────┘  └─────────────────────────────┘    │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │  compaction_log  (NEW table — optional, for audit/debugging)   │  │
│  └────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────┘
```

---

## Component Responsibilities

### Existing Components (v1.0 — minimal or no change)

| Component | v1.1 Change | Notes |
|-----------|-------------|-------|
| `MemoryService` (service.rs) | None | Compaction is a separate service, not a method on MemoryService |
| `EmbeddingEngine` (embedding.rs) | None | CompactionService reuses the existing `Arc<dyn EmbeddingEngine>` to embed compacted content |
| `db.rs` — schema init | Add new columns + optional table | `compaction_source_ids`, `compacted_at`, `is_compacted` added to `memories`; `compaction_log` optional |
| `config.rs` — Config struct | Add `llm_provider`, `llm_api_key`, `llm_base_url` | Follows existing `embedding_provider` / `openai_api_key` pattern exactly |
| `error.rs` | Add `CompactionError`, `SummarizationError` variants | Added to `MnemonicError`; `ApiError` gets a `CompactionFailed` variant |
| `server.rs` — router | Add `POST /memories/compact` route | One new route entry; handler calls `CompactionService::compact()` |

### New Components (v1.1)

| Component | Responsibility | Location |
|-----------|----------------|----------|
| `CompactionService` | Orchestrate compaction: cluster embeddings, merge metadata, optionally summarize, write result, delete sources — all in a single DB transaction | `src/compaction.rs` |
| `SummarizationEngine` trait | Abstract "call LLM with list of texts → return summary string" | `src/summarization.rs` |
| `OpenAiSummarizer` | Concrete impl: calls OpenAI chat completions endpoint via reqwest | `src/summarization.rs` |
| `MockSummarizer` | Test impl: concatenates texts deterministically, no network | `src/summarization.rs` (behind `#[cfg(test)]`) |
| Compaction handler | Thin axum handler: deserialize `CompactRequest`, call `CompactionService`, return `CompactResponse` | `src/server.rs` (new handler fn) |

---

## Recommended Project Structure (v1.1 delta)

The existing flat module structure (`src/*.rs`) should be preserved — do not restructure into subdirectories for v1.1. Only add new files.

```
src/
├── main.rs              # Add: construct CompactionService, pass to AppState
├── config.rs            # Add: llm_provider, llm_api_key, llm_base_url fields; extend validate_config()
├── server.rs            # Add: POST /memories/compact route + handler
├── service.rs           # No change
├── embedding.rs         # No change
├── db.rs                # Add: schema migration for new columns + optional table
├── error.rs             # Add: CompactionError, SummarizationError
├── lib.rs               # Add: pub mod compaction; pub mod summarization
│
├── compaction.rs        # NEW: CompactionService struct + compact() method
│                        #      Clustering algorithm (pure Rust, no external dep)
│                        #      Metadata merge logic
│                        #      Calls SummarizationEngine (optional, None if not configured)
│                        #      Writes new memory + deletes sources atomically
│
└── summarization.rs     # NEW: SummarizationEngine trait + OpenAiSummarizer + MockSummarizer
```

**Rationale for flat structure:** v1.0 is deliberately flat and already works. The two new modules (`compaction.rs`, `summarization.rs`) are cohesive single files. Forcing them into subdirectories adds navigation friction without benefit at this codebase size.

---

## Architectural Patterns

### Pattern 1: CompactionService as Parallel Peer of MemoryService

**What:** `CompactionService` is a new top-level struct at the same layer as `MemoryService`. It holds `Arc<Connection>`, `Arc<dyn EmbeddingEngine>`, and `Option<Arc<dyn SummarizationEngine>>`. It is injected into `AppState` alongside `MemoryService`.

**When to use:** When an operation needs access to the same resources (DB + embedding) but has fundamentally different orchestration logic (clustering, merging, deleting batches). Putting compaction methods on `MemoryService` would bloat it with logic that does not belong to normal CRUD flows.

**Trade-offs:** Two service structs in `AppState` instead of one. Minimal overhead. Keeps `MemoryService` readable and unchanged.

```rust
// src/compaction.rs
pub struct CompactionService {
    pub db: Arc<Connection>,
    pub embedding: Arc<dyn EmbeddingEngine>,
    pub summarizer: Option<Arc<dyn SummarizationEngine>>,
}

// src/server.rs (AppState extension)
#[derive(Clone)]
pub struct AppState {
    pub service: Arc<MemoryService>,
    pub compaction: Arc<CompactionService>,   // new
}
```

**Important:** `CompactionService` reuses the **same** `Arc<Connection>` clone as `MemoryService`. There is no second connection. tokio-rusqlite's actor model serializes all `.call()` operations on one background thread — this is the correct concurrency model for SQLite.

### Pattern 2: SummarizationEngine Trait (mirrors EmbeddingEngine)

**What:** A new `async_trait`-based trait with a single method: `async fn summarize(&self, texts: &[String]) -> Result<String, SummarizationError>`. `OpenAiSummarizer` calls the chat completions endpoint. `MockSummarizer` deterministically joins texts (for tests).

**When to use:** When an LLM call may or may not be configured. The `Option<Arc<dyn SummarizationEngine>>` in `CompactionService` provides clean presence/absence semantics: `None` means "run Tier 1 dedup only"; `Some(engine)` means "also run Tier 2 LLM summarization."

**Trade-offs:** Requires `async_trait` (already a dependency). Makes `CompactionService` testable without network calls. Adding a new LLM provider (e.g., Anthropic, Ollama) is a new file, not a refactor.

```rust
// src/summarization.rs
#[async_trait]
pub trait SummarizationEngine: Send + Sync {
    async fn summarize(&self, texts: &[String]) -> Result<String, SummarizationError>;
}

pub struct OpenAiSummarizer {
    client: reqwest::Client,
    api_key: String,
    model: String,   // e.g. "gpt-4o-mini"
}
```

**Config integration** follows the `embedding_provider` pattern exactly:

```toml
# mnemonic.toml
llm_provider = "openai"    # or "none" (default)
llm_api_key  = "sk-..."    # or MNEMONIC_LLM_API_KEY env var
```

`validate_config()` is extended: if `llm_provider = "openai"` then `llm_api_key` must be set.

### Pattern 3: Inline Clustering — No External Dependency

**What:** The clustering algorithm runs inside `CompactionService::compact()` in pure Rust with no external crate. Use a threshold-based greedy grouping algorithm over cosine distances:

```
1. Fetch all memories + their embeddings for agent_id (+ optional session filter)
2. Compute pairwise cosine distances (O(n^2) over f32 dot products — feasible for n < 10K)
3. Greedy cluster formation:
   - Sort pairs by similarity descending
   - For each pair above threshold: assign to same cluster if neither is yet assigned,
     or extend an existing cluster
   - Unmatched memories are singletons (no compaction)
4. Apply time weighting: boost similarity score for pairs where both memories are older
   than a configurable age_weight_days parameter
```

**Why not linfa-clustering or hdbscan:** Both require `ndarray` as a dependency, adding ~500KB binary size and significant compilation time. The clustering needed here is simple: fixed 384-dim cosine similarity between a bounded set of vectors (one agent's memories). A hand-rolled O(n^2) greedy algorithm is 30 lines of Rust and has zero new dependencies. The complexity only becomes relevant above ~10K memories per agent, which is outside the stated scope.

**Cosine distance for L2-normalized vectors:** Since all embeddings are already L2-normalized (enforced by `EmbeddingEngine` contract), cosine similarity reduces to a dot product: `similarity = dot(a, b)`. This is a single `zip().map().sum()` iterator over two `Vec<f32>` slices — no external library needed.

```rust
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    // Valid only for L2-normalized vectors (which all Mnemonic embeddings are)
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}
```

**Similarity threshold:** Default `0.92` (configurable via `compact_similarity_threshold` config field). This value is based on semantic deduplication literature: above 0.92 cosine similarity (for L2-normalized vectors from all-MiniLM-L6-v2) texts are near-identical in meaning. Expose as a request parameter so callers can tune aggressiveness per-call.

### Pattern 4: Atomic Compaction Transaction

**What:** The write phase of compaction (insert compacted memory, delete source memories) must be a single SQLite transaction. If any step fails, the original memories are untouched.

**Implementation:** Uses `tokio-rusqlite`'s `.call()` closure with an explicit `c.transaction()`. The entire read-cluster-write pipeline should NOT be one giant transaction — only the final write phase needs atomicity. The clustering computation happens in Rust, outside the DB.

```rust
// Inside CompactionService::compact() — write phase only
let source_ids = cluster.iter().map(|m| m.id.clone()).collect::<Vec<_>>();
let new_memory_id = uuid::Uuid::now_v7().to_string();
let compacted_content = summarized_or_merged_content;
let compacted_embedding: Vec<f32> = self.embedding.embed(&compacted_content).await?;

self.db.call(move |c| {
    let tx = c.transaction()?;
    // Insert new compacted memory row
    tx.execute("INSERT INTO memories (..., compaction_source_ids, compacted_at) VALUES (...)", ...)?;
    // Insert embedding
    tx.execute("INSERT INTO vec_memories (memory_id, embedding) VALUES (?, ?)", ...)?;
    // Delete all source memories (both tables)
    for id in &source_ids {
        tx.execute("DELETE FROM vec_memories WHERE memory_id = ?", [id])?;
        tx.execute("DELETE FROM memories WHERE id = ?", [id])?;
    }
    tx.commit()
}).await?;
```

**Critical:** Do not hold any tokio-rusqlite connection lock across an `await` point — the `.call()` closure must be entirely synchronous. This is identical to v1.0's `create_memory` and `delete_memory` patterns.

---

## SQLite Schema Changes

### Modified: `memories` table

Add three nullable columns. Use `ALTER TABLE ... ADD COLUMN` migration applied at startup (after existing `CREATE TABLE IF NOT EXISTS`):

```sql
ALTER TABLE memories ADD COLUMN IF NOT EXISTS compaction_source_ids TEXT;
-- JSON array of memory IDs that were merged into this row
-- NULL for normal (non-compacted) memories
-- Example: '["uuid-1","uuid-2","uuid-3"]'

ALTER TABLE memories ADD COLUMN IF NOT EXISTS compacted_at DATETIME;
-- NULL for normal memories; set when this row is a compaction result

ALTER TABLE memories ADD COLUMN IF NOT EXISTS is_compacted INTEGER NOT NULL DEFAULT 0;
-- 0 = normal memory; 1 = compaction result
-- Enables filtering compacted vs. original memories in search/list
```

**Migration safety:** SQLite `ADD COLUMN IF NOT EXISTS` is supported from SQLite 3.37.0 onward. The existing codebase compiles SQLite via the `bundled` feature (currently 3.51.1), so this is safe. The `IF NOT EXISTS` guard ensures the startup migration is idempotent — running on an existing v1.0 database does not corrupt data.

**No vec_memories change:** The `vec_memories` (vec0 virtual table) schema does not change. Compacted memories get a new row in `vec_memories` just like any other memory. Deleted source memories have their `vec_memories` rows deleted in the same transaction.

### Optional: `compaction_log` table

Useful for debugging and auditing compaction runs, but not required for the feature to work. Defer to a later plan if desired:

```sql
CREATE TABLE IF NOT EXISTS compaction_log (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    session_id TEXT,
    compacted_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    source_count INTEGER NOT NULL,
    output_count INTEGER NOT NULL,
    similarity_threshold REAL NOT NULL,
    tier TEXT NOT NULL   -- 'algorithmic' or 'llm'
);
```

---

## Data Flow: POST /memories/compact

```
POST /memories/compact
  { agent_id, session_id?, similarity_threshold?, age_weight_days?, dry_run? }
        |
        v
  server.rs: compact_handler()
    - deserialize CompactRequest
    - call state.compaction.compact(req).await?
        |
        v
  compaction.rs: CompactionService::compact()

  Phase 1 — Fetch (read-only DB call)
    - conn.call: SELECT id, content, embedding_model, created_at, tags
                 FROM memories WHERE agent_id = ? AND is_compacted = 0
                 [+ optional session_id filter]
    - conn.call: SELECT memory_id, embedding FROM vec_memories
                 WHERE memory_id IN (id_list)
    → Vec<(Memory, Vec<f32>)>

  Phase 2 — Cluster (pure Rust, no DB, no await)
    - For each pair: cosine_similarity(a.embedding, b.embedding)
    - Apply age weighting: boost pairs where both created_at < now - age_weight_days
    - Greedy cluster formation above similarity_threshold
    - Filter out singleton clusters (nothing to compact)
    → Vec<Cluster> where Cluster = Vec<(Memory, Vec<f32>)>

  Phase 3 — Synthesize per cluster (may involve await for LLM)
    - Merge metadata: union tags, keep earliest created_at
    - If summarizer is Some AND cluster.len() >= 2:
        summarized_content = summarizer.summarize(&texts).await?
    - Else (Tier 1 only):
        merged_content = format_merged_content(&texts)  // bullet list or concat
    - embed(merged_content) → new Vec<f32>
    → Vec<CompactionResult>

  Phase 4 — Write (single atomic DB call per cluster)
    - conn.call: INSERT new memory + embedding; DELETE source memories + embeddings
    → CompactResponse { compacted_clusters: usize, memories_removed: usize, ... }

        |
        v
  Returns 200 OK CompactResponse
```

---

## Concurrency: Compaction + Ongoing Read/Write

**The core question:** What happens when a client calls `POST /memories/compact` while other agents are concurrently calling `POST /memories` or `GET /memories/search`?

**Answer:** tokio-rusqlite's single-background-thread actor model handles this automatically. All `Connection::call()` invocations — from `MemoryService` and `CompactionService` alike — are queued and executed serially on the same background thread. The SQLite WAL mode (already enabled) ensures that reads never block writes and vice versa at the file level.

**Concrete behavior:**

| Scenario | Effect |
|----------|--------|
| search during compaction Phase 1 (read) | Both proceed; WAL allows concurrent readers |
| create during compaction Phase 2 (Rust computation) | Create proceeds normally; no DB lock held |
| create during compaction Phase 4 (write transaction) | Create queued; executes after transaction commits — milliseconds wait |
| Two simultaneous `/compact` calls | Serialized by actor; second completes after first; no data race |
| compaction reads stale embedding | Cannot happen: Phase 1 fetch and Phase 4 write are separate calls; a new memory created between them will be a singleton (not yet in the candidate set) — safe and correct |

**No explicit lock needed.** Do not introduce a `tokio::sync::Mutex` or semaphore around compaction. The tokio-rusqlite actor model is sufficient. This keeps the API non-blocking: multiple concurrent `/compact` calls from different agents are serialized at the DB level but do not block each other at the HTTP handler level.

**One edge case to handle:** A memory created during Phase 2-3 (while clustering is being computed in Rust) will not appear in the source set and will not be deleted. This is correct behavior — only memories present at Phase 1 fetch time are candidates. Document this in API response with `candidates_at_snapshot` field.

---

## Integration Points: New vs. Modified

### New Components

| Component | Type | Integration |
|-----------|------|-------------|
| `CompactionService` | New struct | Added to `AppState`; receives `Arc<Connection>` (clone from existing), `Arc<dyn EmbeddingEngine>` (clone from existing), `Option<Arc<dyn SummarizationEngine>>` |
| `SummarizationEngine` trait | New trait | Defined in `summarization.rs`; used by `CompactionService` |
| `OpenAiSummarizer` | New struct | Implements `SummarizationEngine`; uses existing `reqwest::Client` pattern from `OpenAiEngine` |
| Compaction handler | New fn in server.rs | `POST /memories/compact`; added to `build_router()` |
| `CompactRequest` / `CompactResponse` | New types | Defined in `service.rs` or new `compaction.rs` |

### Modified Components

| Component | Modification | Impact Risk |
|-----------|-------------|-------------|
| `Config` | Add `llm_provider`, `llm_api_key`, `llm_base_url`, `compact_similarity_threshold` | Low — new fields with defaults; existing configs unaffected |
| `validate_config()` | Add LLM provider validation block | Low — additive match arm |
| `db.rs` `open()` | Add `ALTER TABLE ... ADD COLUMN IF NOT EXISTS` stmts | Low — idempotent; safe on v1.0 databases |
| `AppState` | Add `compaction: Arc<CompactionService>` | Low — additive field; existing handlers unmodified |
| `build_router()` | Add `.route("/memories/compact", post(compact_handler))` | Low — one line |
| `error.rs` | Add `CompactionError`, `SummarizationError` | Low — additive |
| `lib.rs` | Add `pub mod compaction; pub mod summarization;` | Trivial |

### Internal Boundaries (updated)

| Boundary | Communication | Notes |
|----------|---------------|-------|
| `server.rs` ↔ `CompactionService` | Direct async call via `Arc<CompactionService>` | Same pattern as `MemoryService` |
| `CompactionService` ↔ `EmbeddingEngine` | Trait object call via `Arc<dyn EmbeddingEngine>` | Same engine used for normal memories and compacted result |
| `CompactionService` ↔ `SummarizationEngine` | Optional trait object call via `Option<Arc<dyn SummarizationEngine>>` | `None` = Tier 1 only; `Some` = Tier 2 enabled |
| `CompactionService` ↔ SQLite | `conn.call()` closures, same pattern as `MemoryService` | Same `Arc<Connection>` clone; serialized by actor |

---

## Build Order (v1.1 phases, considering dependencies)

```
1. error.rs         — Add CompactionError, SummarizationError variants
                      (no new dependencies; other modules depend on this)

2. config.rs        — Add LLM config fields + validate_config() extension
                      (no new dependencies; CompactionService needs Config)

3. db.rs            — Add schema migration for new columns
                      (depends on nothing new; all other compaction work depends
                       on columns existing in the schema)

4. summarization.rs — SummarizationEngine trait + OpenAiSummarizer + MockSummarizer
                      (depends on error.rs; needed by CompactionService)

5. compaction.rs    — CompactionService (clustering + merge + optional LLM call + write)
                      (depends on error.rs, summarization.rs; uses Arc<Connection> +
                       Arc<dyn EmbeddingEngine> — both already exist)

6. server.rs        — Add compact_handler + route
                      (depends on compaction.rs; thin handler, written last)

7. main.rs          — Wire CompactionService into AppState
                      (depends on all above; last change)
```

**Phase recommendation based on dependency graph:**

- **Phase A (foundation):** steps 1-3 together — config + schema + errors. No new services yet. Can be tested by running the server and verifying the DB has new columns.
- **Phase B (summarization engine):** step 4 in isolation. Unit-testable with `MockSummarizer`. The `OpenAiSummarizer` can be integration-tested with a real key or skipped with the mock.
- **Phase C (compaction logic):** step 5. This is the highest-complexity component. The clustering algorithm + merge logic + transaction are all here. Build with `MockSummarizer` so tests do not require an LLM API key.
- **Phase D (HTTP integration):** steps 6-7. Wire everything together, add the endpoint, write integration tests.

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Compaction as a Method on MemoryService

**What:** Adding `MemoryService::compact()` instead of a separate `CompactionService`.

**Why it's wrong:** Compaction has fundamentally different dependencies (`SummarizationEngine`) that normal CRUD does not need. Adding an optional `summarizer: Option<Arc<dyn SummarizationEngine>>` to `MemoryService` conflates two concerns and makes the struct harder to test and extend.

**Do this instead:** `CompactionService` is a peer struct in `AppState`, not a subcomponent of `MemoryService`.

### Anti-Pattern 2: One Giant Transaction for the Entire Compaction Operation

**What:** Opening a transaction in Phase 1 and holding it through Phase 3 (LLM call).

**Why it's wrong:** An LLM API call inside a SQLite transaction holds the database write lock for the full LLM round-trip latency (typically 2-30 seconds). This blocks all other writes on the connection for the duration. tokio-rusqlite's actor model means the background thread is occupied for the entire period.

**Do this instead:** Only Phase 4 (write new memory + delete sources) is transactional. Phases 1-3 (fetch, cluster, LLM call) happen outside any transaction. The worst case of a crash between Phase 3 and Phase 4 is that compaction didn't happen — original memories are untouched.

### Anti-Pattern 3: Fetching Embeddings via a Second DB Connection

**What:** Opening a second `tokio_rusqlite::Connection` for the Phase 1 embedding fetch to avoid queuing behind MemoryService operations.

**Why it's wrong:** SQLite WAL mode allows one writer at a time. Two connections can both read simultaneously but only one can write. The existing single-connection actor model is intentional and already handles this correctly. A second connection adds complexity without benefit and risks `SQLITE_BUSY` errors during the write phase if both connections attempt to write concurrently.

**Do this instead:** Clone the existing `Arc<Connection>` into `CompactionService`. All operations queue through the same actor.

### Anti-Pattern 4: Embedding Fetched Inline During the DB Call Closure

**What:** Calling `self.embedding.embed(text)` inside a `.call()` closure.

**Why it's wrong:** The `.call()` closure runs on the tokio-rusqlite background thread (a blocking `std::thread`). Calling an async function (or spawning an OS thread for local inference) inside this closure is either impossible or causes nested blocking that degrades performance.

**Do this instead:** Fetch all memory content + embeddings from the DB in one `.call()`, return to async context, run embedding there, then return to the DB with another `.call()` for the write. Phase 1 reads embeddings directly from `vec_memories` — this avoids re-embedding entirely for clustering.

### Anti-Pattern 5: Storing cluster_id as a Foreign Key Instead of JSON source_ids

**What:** Creating a `clusters` table with a `cluster_id` column on `memories` for hierarchical tracking.

**Why it's wrong:** The PROJECT.md explicitly rules out hierarchical summaries. The `compaction_source_ids` JSON array on the output memory is sufficient for provenance. A `clusters` table adds schema complexity for a use case that is out of scope.

**Do this instead:** Store `compaction_source_ids TEXT` as a JSON array on the compacted memory row. Queryable via `json_each()` if needed.

---

## Scaling Considerations

| Scale | Compaction Behavior |
|-------|---------------------|
| < 100 memories per agent | O(n^2) clustering: < 1ms computation; single transaction; fast |
| ~1,000 memories per agent | O(n^2) ~ 500K comparisons: ~10-50ms in Rust on CPU; acceptable |
| ~10,000 memories per agent | O(n^2) ~ 50M comparisons: ~1-5s; add a configurable `max_candidates` limit |
| > 10,000 memories per agent | Out of scope for v1.1; recommend multiple compaction calls with session_id scoping |

**Time weighting:** Applying an age-based similarity boost does not change the O(n^2) complexity — it is a scalar multiplier during the comparison step.

**LLM latency:** OpenAI GPT-4o-mini typically responds in 1-10 seconds for short prompts. For a cluster of 5-10 memories, the full compaction request (including LLM) is 3-15 seconds. This is a user-initiated operation (`POST /memories/compact`), not a background task, so the latency is acceptable. Document it in the API reference.

---

## Sources

- [tokio-rusqlite docs — Connection clone + actor model](https://docs.rs/tokio-rusqlite/latest/tokio_rusqlite/) — HIGH confidence (official docs)
- [SQLite WAL mode — reader/writer concurrency](https://sqlite.org/wal.html) — HIGH confidence (official docs)
- [SQLite ALTER TABLE ADD COLUMN](https://sqlite.org/lang_altertable.html) — HIGH confidence (official docs)
- [async-openai crate — chat completions](https://docs.rs/async-openai) — MEDIUM confidence (official crate docs, v0.33.1)
- [Semantic deduplication with cosine clustering — NVIDIA NeMo](https://docs.nvidia.com/nemo-framework/user-guide/24.09/datacuration/semdedup.html) — MEDIUM confidence (official product docs, reference for threshold values)
- [linfa-clustering DBSCAN](https://docs.rs/linfa-clustering/latest/linfa_clustering/) — MEDIUM confidence (evaluated and rejected: ndarray dependency too heavy for this use case)
- [SQLite concurrent writes — "database is locked" patterns](https://tenthousandmeters.com/blog/sqlite-concurrent-writes-and-database-is-locked-errors/) — MEDIUM confidence (community, consistent with SQLite official docs)
- Existing v1.0 source code (`src/*.rs`) — HIGH confidence (direct inspection)

---

*Architecture research for: Mnemonic v1.1 — memory compaction/summarization integration*
*Researched: 2026-03-20*
