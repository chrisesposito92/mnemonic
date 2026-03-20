# Phase 8: Compaction Core - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

CompactionService implements the full compaction pipeline — fetch an agent's memories, cluster by vector similarity, synthesize merged content (Tier 1 algorithmic or Tier 2 LLM), and atomic write (insert merged + delete sources in a single transaction). dry_run mode returns proposed clusters without modifying data. The HTTP endpoint is Phase 9 scope.

</domain>

<decisions>
## Implementation Decisions

### Clustering algorithm
- Greedy pairwise: single-pass, iterate all pairs, group by cosine similarity above threshold (default 0.85)
- First-match assignment: once a memory joins a cluster, it is not reconsidered for other clusters
- No centroid secondary validation — the threshold check is sufficient at N<500
- Cluster centroid computed as average of member embedding vectors (used only for pairing new candidates against existing clusters during the single pass)
- Pairs sorted by descending similarity before processing — most similar pairs merge first

### Content synthesis
- Tier 1 (no LLM): chronological concatenation of source memory content, separated by newlines, ordered by created_at ascending
- No source attribution in merged content — source_ids column tracks provenance
- Tier 2 (LLM configured): pass cluster texts to SummarizationEngine; on any LlmError, fall back to Tier 1 concatenation silently (log at warn level)
- Merged memory embedding: re-computed from the merged content via EmbeddingEngine (not averaged from sources) — more accurate for search

### Metadata merge rules
- Tags: union of all source memory tags (deduplicated)
- created_at: earliest created_at from source memories (preserves original creation time)
- agent_id: inherited from sources (all same — clustering is agent-scoped)
- session_id: empty string (merged memories span sessions)
- embedding_model: current model name (re-embedded with current engine)
- source_ids: JSON array of all source memory IDs

### Atomic write
- Single SQLite transaction: INSERT new merged memory + INSERT vec_memories embedding + DELETE source memories + DELETE source vec_memories entries
- If any step fails, entire transaction rolls back — no orphans, no data loss
- compact_runs table updated: status='completed', completed_at=now, counts populated
- On transaction failure: compact_runs status='failed'

### CompactionService struct design
- New file: `src/compaction.rs` (mirrors embedding.rs, summarization.rs separation)
- Struct holds: `Arc<Connection>`, `Arc<dyn EmbeddingEngine>`, `Option<Arc<dyn SummarizationEngine>>`
- Constructor: `CompactionService::new(db, embedding, summarization)`
- Main method: `async fn compact(&self, req: CompactRequest) -> Result<CompactResponse, ApiError>`
- Internal pipeline steps: fetch_candidates → compute_pairs → cluster → synthesize → write (or preview for dry_run)

### CompactRequest / CompactResponse types
- CompactRequest: `agent_id: String` (required), `threshold: Option<f32>` (default 0.85), `max_candidates: Option<u32>` (default 100), `dry_run: Option<bool>` (default false)
- CompactResponse: `run_id: String`, `clusters_found: u32`, `memories_merged: u32`, `memories_created: u32`, `id_mapping: Vec<ClusterMapping>`, `truncated: bool`
- ClusterMapping: `source_ids: Vec<String>`, `new_id: Option<String>` (None in dry_run)

### dry_run mode
- Runs full pipeline: fetch, cluster, synthesize content — but skips the atomic write transaction
- Returns the same CompactResponse shape, with new_id = None in each ClusterMapping
- compact_runs row still created with dry_run=1 for audit logging

### Performance limits (max_candidates)
- Default: 100 candidates (4,950 pairs — fast even on modest hardware)
- Candidates selected by most recent created_at first (ORDER BY created_at DESC LIMIT max_candidates)
- When limit applies: truncated=true in response, all candidates within limit are still fully processed
- Config-level default threshold (0.85) with per-request override via CompactRequest.threshold

### Claude's Discretion
- Exact cosine similarity computation (dot product of L2-normalized vectors, or 1-distance from sqlite-vec)
- Internal helper function organization within compaction.rs
- Error message wording for edge cases (no memories found, all memories already compacted)
- Whether to add tracing::info for compaction pipeline stages
- CompactionError enum design (new error type or reuse existing)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — DEDUP-01 through DEDUP-04 define clustering, metadata merge, atomic write, and max_candidates requirements

### Architecture patterns to mirror
- `src/service.rs` — MemoryService pattern: struct with `Arc<Connection>` + engine deps, async methods using `db.call(move |c| { ... })`
- `src/embedding.rs` — EmbeddingEngine trait, LocalEngine/OpenAiEngine: the engine pattern CompactionService consumes
- `src/summarization.rs` — SummarizationEngine trait, MockSummarizer: used by CompactionService for Tier 2
- `src/db.rs` — Schema DDL, vec_memories virtual table, transaction pattern
- `src/error.rs` — Error hierarchy: DbError, LlmError, ApiError with thiserror + From impls
- `src/server.rs` — AppState struct (CompactionService will be added here in Phase 9)
- `src/main.rs` lines 72-92 — LLM engine init; lines 94-112 — MemoryService + AppState construction

### Prior phase context
- `.planning/phases/06-foundation/06-CONTEXT.md` — Schema decisions (source_ids, compact_runs), config validation
- `.planning/phases/07-summarization-engine/07-CONTEXT.md` — SummarizationEngine API, fallback responsibility assignment, MockSummarizer

### Project decisions
- `.planning/PROJECT.md` §Key Decisions — rusqlite 0.37 pin, tokio-rusqlite async pattern, zerocopy for vec binding
- `.planning/STATE.md` §Accumulated Context — CompactionService is peer of MemoryService, LlmError conversion chain, XML delimiters

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `MemoryService` (service.rs): transaction pattern (`db.call(move |c| { let tx = c.transaction()?; ... tx.commit()?; })`) — copy for atomic merge write
- `EmbeddingEngine` (embedding.rs): `embed(&str) -> Vec<f32>` — call to re-embed merged content
- `SummarizationEngine` (summarization.rs): `summarize(&[String]) -> Result<String, LlmError>` — call for Tier 2 synthesis
- `MockSummarizer` (summarization.rs): deterministic output for unit/integration tests
- `Memory` struct (service.rs): existing serializable memory type with all needed fields
- `compact_runs` table (db.rs): audit log schema already created
- `source_ids` column (db.rs): JSON array column already on memories table
- `vec_memories` virtual table (db.rs): 384-dim float vector store with KNN MATCH support

### Established Patterns
- `Arc<Connection>` shared across services — same conn for CompactionService
- `db.call(move |c| ...)` for all SQLite access — never direct rusqlite from async
- Transaction: `let tx = c.transaction()?; ... tx.commit()?;` inside db.call closure
- JSON-in-TEXT columns: `serde_json::to_string` / `serde_json::from_str` for tags, source_ids
- `uuid::Uuid::now_v7().to_string()` for ID generation
- `zerocopy::IntoBytes` for converting `Vec<f32>` to bytes for sqlite-vec MATCH binding
- Query parameters: `rusqlite::params![...]` macro
- Error conversion: domain error → MnemonicError → ApiError chain

### Integration Points
- `src/main.rs` line 100-107: MemoryService construction — CompactionService will be constructed similarly, sharing `db_arc` and `embedding`
- `src/main.rs` line 73: `_llm_engine` variable — currently unused, CompactionService will consume it
- `src/server.rs` AppState: currently holds only `Arc<MemoryService>` — Phase 9 adds `Arc<CompactionService>`
- `src/lib.rs`: needs `pub mod compaction;` added
- `Cargo.toml`: no new dependencies expected — all needed crates already present

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. Follow existing codebase patterns exactly. The MemoryService transaction pattern is the blueprint for atomic merge writes.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 08-compaction-core*
*Context gathered: 2026-03-20*
