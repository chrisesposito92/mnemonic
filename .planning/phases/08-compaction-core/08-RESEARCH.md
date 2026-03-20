# Phase 8: Compaction Core - Research

**Researched:** 2026-03-20
**Domain:** Rust async service implementation — greedy-pairwise vector clustering, SQLite transactional merge, cosine similarity computation
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Clustering algorithm**
- Greedy pairwise: single-pass, iterate all pairs, group by cosine similarity above threshold (default 0.85)
- First-match assignment: once a memory joins a cluster, it is not reconsidered for other clusters
- No centroid secondary validation — the threshold check is sufficient at N<500
- Cluster centroid computed as average of member embedding vectors (used only for pairing new candidates against existing clusters during the single pass)
- Pairs sorted by descending similarity before processing — most similar pairs merge first

**Content synthesis**
- Tier 1 (no LLM): chronological concatenation of source memory content, separated by newlines, ordered by created_at ascending
- No source attribution in merged content — source_ids column tracks provenance
- Tier 2 (LLM configured): pass cluster texts to SummarizationEngine; on any LlmError, fall back to Tier 1 concatenation silently (log at warn level)
- Merged memory embedding: re-computed from the merged content via EmbeddingEngine (not averaged from sources) — more accurate for search

**Metadata merge rules**
- Tags: union of all source memory tags (deduplicated)
- created_at: earliest created_at from source memories (preserves original creation time)
- agent_id: inherited from sources (all same — clustering is agent-scoped)
- session_id: empty string (merged memories span sessions)
- embedding_model: current model name (re-embedded with current engine)
- source_ids: JSON array of all source memory IDs

**Atomic write**
- Single SQLite transaction: INSERT new merged memory + INSERT vec_memories embedding + DELETE source memories + DELETE source vec_memories entries
- If any step fails, entire transaction rolls back — no orphans, no data loss
- compact_runs table updated: status='completed', completed_at=now, counts populated
- On transaction failure: compact_runs status='failed'

**CompactionService struct design**
- New file: `src/compaction.rs` (mirrors embedding.rs, summarization.rs separation)
- Struct holds: `Arc<Connection>`, `Arc<dyn EmbeddingEngine>`, `Option<Arc<dyn SummarizationEngine>>`
- Constructor: `CompactionService::new(db, embedding, summarization)`
- Main method: `async fn compact(&self, req: CompactRequest) -> Result<CompactResponse, ApiError>`
- Internal pipeline steps: fetch_candidates → compute_pairs → cluster → synthesize → write (or preview for dry_run)

**CompactRequest / CompactResponse types**
- CompactRequest: `agent_id: String` (required), `threshold: Option<f32>` (default 0.85), `max_candidates: Option<u32>` (default 100), `dry_run: Option<bool>` (default false)
- CompactResponse: `run_id: String`, `clusters_found: u32`, `memories_merged: u32`, `memories_created: u32`, `id_mapping: Vec<ClusterMapping>`, `truncated: bool`
- ClusterMapping: `source_ids: Vec<String>`, `new_id: Option<String>` (None in dry_run)

**dry_run mode**
- Runs full pipeline: fetch, cluster, synthesize content — but skips the atomic write transaction
- Returns the same CompactResponse shape, with new_id = None in each ClusterMapping
- compact_runs row still created with dry_run=1 for audit logging

**Performance limits (max_candidates)**
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

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| DEDUP-01 | System clusters memories by vector cosine similarity using configurable threshold (default 0.85) | Greedy pairwise algorithm with dot-product cosine sim on L2-normalized vectors; all vectors already L2-normalized by EmbeddingEngine |
| DEDUP-02 | System merges metadata for deduplicated clusters (tags union, earliest timestamp, combined content) | Tags/source_ids are JSON TEXT columns via serde_json; created_at is TEXT DATETIME; union logic is pure Rust |
| DEDUP-03 | Merge operation is atomic — new memory inserted before source memories deleted, within single transaction | `c.transaction()` pattern already proven in MemoryService.delete_memory; same pattern applies |
| DEDUP-04 | System enforces max candidates limit to prevent O(n²) on large memory sets | ORDER BY created_at DESC LIMIT ?N in candidate fetch query; truncated flag in response |
</phase_requirements>

---

## Summary

Phase 8 builds `CompactionService` — a pure Rust service that takes a set of agent memories, finds clusters of highly similar ones via a greedy pairwise algorithm, synthesizes merged content (Tier 1 text concat or Tier 2 LLM), and atomically replaces the source memories with a single merged memory. Every pattern needed already exists in the codebase: the `db.call(move |c| { tx = c.transaction()? ... tx.commit()? })` pattern from `MemoryService`, the `Arc<dyn EmbeddingEngine>` + `Arc<dyn SummarizationEngine>` consumption pattern, and the JSON-in-TEXT column serialization.

The core algorithmic challenge is the greedy pairwise clustering step: fetch up to `max_candidates` memories (with their 384-dim embeddings from vec_memories), compute all N*(N-1)/2 pairs sorted by descending cosine similarity, then apply first-match cluster assignment. Since all embeddings are already L2-normalized by the embedding engines, cosine similarity reduces to a simple dot product. At the default limit of 100 candidates, this is 4,950 pairs — trivially fast in Rust.

The atomic write is a direct extension of the delete pattern already in `MemoryService.delete_memory`: open a transaction, INSERT merged memory + INSERT vec embedding + DELETE source memories + DELETE source vec entries, then commit. The `compact_runs` table and `source_ids` column are already in the schema (added in Phase 6), so no DDL migrations are needed.

**Primary recommendation:** Mirror `MemoryService` structure exactly — `CompactionService` in `src/compaction.rs` with `Arc<Connection>` + engine deps, all DB access via `db.call()`, one main `compact()` async method that orchestrates internal sync helper functions.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio-rusqlite | 0.7 | Async SQLite bridge — `db.call()` for blocking ops | Already in use; required pattern |
| rusqlite | 0.37 | SQLite driver (bundled) — `c.transaction()`, params![] | Pinned at 0.37 (sqlite-vec 0.1.7 conflict with 0.39) |
| serde_json | 1 | JSON encode/decode for tags, source_ids TEXT columns | Already in use |
| uuid | 1 (v7) | ID generation — `Uuid::now_v7().to_string()` | Already in use for memory IDs |
| zerocopy | 0.8 | `Vec<f32>` → `&[u8]` for vec_memories MATCH binding | Already in use; required for sqlite-vec |
| async-trait | 0.1 | Trait objects with async methods | Already in use |
| tracing | 0.1 | Structured logging for pipeline stages | Already in use |

**No new dependencies.** All required crates are present in Cargo.toml.

### Architecture Modules

| Module | File | Role |
|--------|------|------|
| CompactionService | src/compaction.rs | New — core logic |
| Memory (reuse) | src/service.rs | Reused struct for candidate rows |
| EmbeddingEngine | src/embedding.rs | Consumed — re-embed merged content |
| SummarizationEngine | src/summarization.rs | Consumed — optional Tier 2 synthesis |
| compact_runs table | db.rs (already created) | Audit log — no schema change needed |
| source_ids column | db.rs (already added) | Provenance tracking — already in schema |

---

## Architecture Patterns

### Recommended Project Structure

```
src/
├── compaction.rs     # CompactionService — NEW (mirrors service.rs, embedding.rs pattern)
├── service.rs        # MemoryService (unchanged — reuse Memory struct)
├── embedding.rs      # EmbeddingEngine (unchanged — consumed by CompactionService)
├── summarization.rs  # SummarizationEngine (unchanged — consumed by CompactionService)
├── db.rs             # Schema (unchanged — compact_runs + source_ids already exist)
├── error.rs          # Error types (unchanged or minor addition)
├── server.rs         # AppState (unchanged in Phase 8 — Phase 9 adds compaction field)
├── lib.rs            # Add: pub mod compaction;
└── main.rs           # Wire CompactionService (Phase 9 does HTTP endpoint; Phase 8 does construction)
```

### Pattern 1: CompactionService Struct

```rust
// src/compaction.rs
use std::sync::Arc;
use tokio_rusqlite::Connection;
use crate::embedding::EmbeddingEngine;
use crate::summarization::SummarizationEngine;
use crate::error::ApiError;

pub struct CompactionService {
    db: Arc<Connection>,
    embedding: Arc<dyn EmbeddingEngine>,
    summarization: Option<Arc<dyn SummarizationEngine>>,
    embedding_model: String,
}

impl CompactionService {
    pub fn new(
        db: Arc<Connection>,
        embedding: Arc<dyn EmbeddingEngine>,
        summarization: Option<Arc<dyn SummarizationEngine>>,
        embedding_model: String,
    ) -> Self {
        Self { db, embedding, summarization, embedding_model }
    }

    pub async fn compact(&self, req: CompactRequest) -> Result<CompactResponse, ApiError> {
        // 1. fetch_candidates — query memories + vec embeddings for agent_id
        // 2. compute_pairs — cosine similarity for all N*(N-1)/2 pairs
        // 3. cluster — greedy first-match assignment
        // 4. synthesize — Tier 1 concat or Tier 2 LLM with fallback
        // 5. write (or preview for dry_run)
        todo!()
    }
}
```

### Pattern 2: Candidate Fetch with Embedding Retrieval

The critical query fetches both the Memory metadata AND the embedding bytes from vec_memories in one pass — avoids N+1 queries:

```rust
// Inside db.call closure:
let sql = "SELECT m.id, m.content, m.agent_id, m.session_id, m.tags,
                  m.embedding_model, m.created_at, v.embedding
           FROM memories m
           JOIN vec_memories v ON v.memory_id = m.id
           WHERE m.agent_id = ?1
           ORDER BY m.created_at DESC
           LIMIT ?2";
```

This JOIN on vec_memories (a virtual table) works because sqlite-vec supports standard SQL joins against the virtual table's primary key.

### Pattern 3: Cosine Similarity (Pure Rust)

Since all embeddings are L2-normalized by EmbeddingEngine, cosine similarity = dot product:

```rust
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}
```

This is already validated in tests/integration.rs (`cosine_similarity` helper, lines 404-412). The result is in [0.0, 1.0] for normalized vectors where 1.0 = identical.

### Pattern 4: Greedy Pairwise Clustering

```rust
// Step 1: compute all pairs with similarity
struct Pair { i: usize, j: usize, similarity: f32 }

let mut pairs: Vec<Pair> = Vec::new();
for i in 0..candidates.len() {
    for j in (i+1)..candidates.len() {
        let sim = cosine_similarity(&candidates[i].embedding, &candidates[j].embedding);
        if sim >= threshold {
            pairs.push(Pair { i, j, similarity: sim });
        }
    }
}
// Sort descending: most similar pairs merged first
pairs.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));

// Step 2: greedy first-match cluster assignment
let mut cluster_id: Vec<Option<usize>> = vec![None; candidates.len()];
let mut clusters: Vec<Vec<usize>> = Vec::new();

for pair in &pairs {
    match (cluster_id[pair.i], cluster_id[pair.j]) {
        (None, None) => {
            // New cluster
            let id = clusters.len();
            clusters.push(vec![pair.i, pair.j]);
            cluster_id[pair.i] = Some(id);
            cluster_id[pair.j] = Some(id);
        }
        (Some(id), None) => {
            clusters[id].push(pair.j);
            cluster_id[pair.j] = Some(id);
        }
        (None, Some(id)) => {
            clusters[id].push(pair.i);
            cluster_id[pair.i] = Some(id);
        }
        (Some(_), Some(_)) => {
            // Both already assigned — skip (first-match wins)
        }
    }
}
```

Note: The CONTEXT.md says "once a memory joins a cluster, it is not reconsidered for other clusters" — this is the `(Some(_), Some(_))` skip case above.

### Pattern 5: Atomic Write Transaction

Direct extension of the delete_memory pattern (service.rs lines 295-332):

```rust
// Inside db.call closure — mirrors MemoryService transaction pattern
let tx = c.transaction()?;

// INSERT merged memory
tx.execute(
    "INSERT INTO memories (id, content, agent_id, session_id, tags,
                           embedding_model, created_at, source_ids)
     VALUES (?1, ?2, ?3, '', ?4, ?5, ?6, ?7)",
    rusqlite::params![
        new_id, merged_content, agent_id,
        tags_json, embedding_model, earliest_created_at, source_ids_json
    ],
)?;

// INSERT vec embedding for merged memory
tx.execute(
    "INSERT INTO vec_memories (memory_id, embedding) VALUES (?1, ?2)",
    rusqlite::params![new_id, embedding_bytes],
)?;

// DELETE source vec entries first (foreign-key-safe ordering)
for src_id in &source_ids {
    tx.execute("DELETE FROM vec_memories WHERE memory_id = ?1", rusqlite::params![src_id])?;
}

// DELETE source memories
for src_id in &source_ids {
    tx.execute("DELETE FROM memories WHERE id = ?1", rusqlite::params![src_id])?;
}

tx.commit()?;
```

### Pattern 6: compact_runs Audit Record

```rust
// Create run record BEFORE pipeline (status='running')
let run_id = uuid::Uuid::now_v7().to_string();
db.call(move |c| {
    c.execute(
        "INSERT INTO compact_runs (id, agent_id, threshold, dry_run, status)
         VALUES (?1, ?2, ?3, ?4, 'running')",
        rusqlite::params![run_id, agent_id, threshold, dry_run as i64],
    )
}).await?;

// After completion — update with counts
db.call(move |c| {
    c.execute(
        "UPDATE compact_runs
         SET status='completed', completed_at=datetime('now'),
             clusters_found=?2, memories_merged=?3, memories_created=?4
         WHERE id=?1",
        rusqlite::params![run_id, clusters_found, memories_merged, memories_created],
    )
}).await?;
```

### Pattern 7: Tier 2 LLM Fallback

```rust
// In synthesize step
let merged_content = match &self.summarization {
    Some(engine) => {
        let texts: Vec<String> = cluster_memories.iter()
            .map(|m| m.content.clone())
            .collect();
        match engine.summarize(&texts).await {
            Ok(summary) => summary,
            Err(e) => {
                tracing::warn!(error = %e, "LLM summarization failed, falling back to Tier 1");
                tier1_concat(&cluster_memories)  // chronological concat
            }
        }
    }
    None => tier1_concat(&cluster_memories),
};
```

### Pattern 8: Tags Union

```rust
fn union_tags(clusters: &[&Memory]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for m in clusters {
        for tag in &m.tags {
            if seen.insert(tag.clone()) {
                result.push(tag.clone());
            }
        }
    }
    result
}
```

Tags are already `Vec<String>` on the Memory struct (deserialized from JSON TEXT in the DB fetch).

### Pattern 9: lib.rs Addition

```rust
// src/lib.rs — add one line
pub mod compaction;
```

### Pattern 10: main.rs Integration

Phase 8 wires CompactionService construction (Phase 9 adds it to AppState and HTTP routing):

```rust
// After MemoryService construction (line ~100 in main.rs)
let compaction_service = std::sync::Arc::new(
    compaction::CompactionService::new(
        db_arc.clone(),
        embedding.clone(),
        _llm_engine,   // moves Option<Arc<dyn SummarizationEngine>>
        embedding_model.clone(),
    )
);
// _llm_engine consumed here — no longer prefixed with _ after Phase 8
```

### Anti-Patterns to Avoid

- **Fetch embeddings separately from memories:** Never do N+1 queries (one per memory). Use a JOIN on vec_memories in the candidate fetch query.
- **Average embeddings for merged memory:** CONTEXT.md locked decision is to RE-EMBED from merged content — not average source vectors. Averaging loses semantic meaning.
- **Multiple db.call() calls inside one transaction:** tokio-rusqlite requires each db.call() to be its own closure. The entire transaction (INSERT merged + DELETE sources) must be inside a single db.call() closure.
- **Borrowing c after creating tx:** The MemoryService pattern shows the fix — create and drop any statements BEFORE calling `c.transaction()`. The statement borrow must be released first.
- **Using `?` on a Vec<f32> from raw bytes incorrectly:** vec_memories stores embeddings as BLOB bytes. Reading them back requires `row.get::<_, Vec<u8>>(col)?` then casting bytes to `Vec<f32>` via safe transmute or zerocopy.
- **Skipping compact_runs on transaction failure:** The CONTEXT.md specifies `status='failed'` must be written. Use a separate db.call() in the error path.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cosine similarity | Custom distance function | Dot product on already-L2-normalized embeddings | EmbeddingEngine guarantees L2 norm ≈ 1.0; dot product IS cosine sim |
| JSON serialization | Manual string building | `serde_json::to_string(&vec)` / `from_str` | Already proven pattern for tags column |
| Transaction rollback | Manual ROLLBACK SQL | rusqlite `Transaction::rollback()` (auto on drop if not committed) | rusqlite Transaction rolls back on drop — no explicit call needed on error path |
| UUID generation | Custom ID scheme | `uuid::Uuid::now_v7().to_string()` | Already proven, sortable, collision-free |
| Embedding bytes for sqlite-vec | Manual `unsafe` cast | `zerocopy::IntoBytes::as_bytes()` | Already proven pattern in create_memory |

**Key insight:** Reading embeddings back from vec_memories (as Vec<u8> BLOB) and converting to Vec<f32> for in-memory similarity computation is the one potentially tricky operation. The safest approach is `bytemuck` or direct unsafe transmute — but zerocopy works for write; for read, use `unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const f32, bytes.len() / 4).to_vec() }` which is safe given sqlite-vec always writes aligned 4-byte floats.

---

## Common Pitfalls

### Pitfall 1: Borrow-After-Transaction on rusqlite Connection

**What goes wrong:** Compiler error — cannot borrow `c` as mutable after creating a `Statement` (which borrows `c` immutably).

**Why it happens:** `c.prepare()` returns a `Statement<'_>` that holds an immutable borrow of `c`. You cannot call `c.transaction()` (mutable borrow) while the statement is alive.

**How to avoid:** The pattern from `delete_memory` (service.rs lines 296-320) shows the fix — wrap the prepare+query in a nested block `{ let mut stmt = c.prepare(...)?; ... }` so the statement is dropped before `c.transaction()` is called.

**Warning signs:** Compiler error about conflicting borrows on `c` inside a `db.call()` closure.

### Pitfall 2: Embedding Read-Back from BLOB

**What goes wrong:** Fetching embedding bytes from vec_memories returns a `Vec<u8>`, but cosine similarity needs `Vec<f32>`. Naive iteration over bytes gives garbage.

**Why it happens:** sqlite-vec stores 384 f32 values as 384*4 = 1536 bytes (little-endian IEEE 754). Each f32 must be reconstructed from 4 bytes.

**How to avoid:** Use `bytemuck::cast_slice::<u8, f32>(&bytes).to_vec()` or the unsafe slice reinterpret. Bytemuck is not in Cargo.toml — use unsafe transmute pattern:
```rust
let embedding: Vec<f32> = unsafe {
    let ptr = bytes.as_ptr() as *const f32;
    std::slice::from_raw_parts(ptr, bytes.len() / 4).to_vec()
};
```
This is safe because sqlite-vec always writes properly-aligned 4-byte floats and the byte count is always a multiple of 4.

**Warning signs:** Cosine similarity values outside [0.0, 1.0] or NaN — indicates byte reinterpretation is wrong.

### Pitfall 3: O(n²) Pair Generation Without Candidate Cap

**What goes wrong:** Without max_candidates, a user with 5,000 memories generates 12.5 million pairs — visible CPU spike, seconds of latency.

**Why it happens:** The algorithm is inherently O(n²) in candidates.

**How to avoid:** The candidate fetch SQL includes `LIMIT ?max_candidates` and `ORDER BY created_at DESC`. The `truncated` flag in the response signals when the limit was applied. At default 100 candidates = 4,950 pairs — imperceptible.

**Warning signs:** Any code path that fetches memories without LIMIT for the compaction candidate set.

### Pitfall 4: LlmError → ApiError Conversion Chain

**What goes wrong:** Returning `LlmError` from a method that returns `ApiError` fails to compile — there is no direct `From<LlmError> for ApiError` impl.

**Why it happens:** STATE.md explicitly notes: "LlmError has no direct From impl for ApiError — conversion chain is LlmError -> MnemonicError::Llm -> ApiError::Internal".

**How to avoid:** Convert explicitly:
```rust
.map_err(|e| ApiError::Internal(MnemonicError::Llm(e)))?
```
Or use the error hierarchy: `MnemonicError::from(llm_err)` then `ApiError::from(mnemonic_err)`.

**Warning signs:** Compiler error `the trait From<LlmError> is not implemented for ApiError`.

### Pitfall 5: Singleton Cluster Handling

**What goes wrong:** The algorithm finds no pairs above threshold for a memory — it forms no cluster. But the code tries to access `clusters[0]` for a singleton, panicking.

**Why it happens:** Greedy pairwise only creates clusters when two memories exceed the threshold. Singletons are intentionally NOT merged.

**How to avoid:** After clustering, filter: only process `clusters` with 2+ members. Singletons are left untouched. The response `clusters_found` counts only multi-member clusters.

**Warning signs:** Off-by-one in cluster count; merging single memories into "merged" memories with the same content.

### Pitfall 6: dry_run compact_runs Row Still Required

**What goes wrong:** Skipping the compact_runs INSERT in dry_run mode, losing audit history.

**Why it happens:** dry_run sounds like "nothing is written" — but the CONTEXT.md explicitly states: "compact_runs row still created with dry_run=1 for audit logging."

**How to avoid:** Always create the compact_runs row. Gate only the atomic memory write transaction on `!dry_run`.

**Warning signs:** dry_run calls produce no compact_runs rows when queried later.

### Pitfall 7: Move Semantics in db.call Closures

**What goes wrong:** Capture error like "closure may outlive the current function, but it borrows `x`".

**Why it happens:** `db.call(move |c| ...)` requires all captured variables to be moved into the closure (it's `Send + 'static`). References to local variables don't work.

**How to avoid:** Clone all data needed inside the closure before calling db.call:
```rust
let run_id_clone = run_id.clone();
let agent_id_clone = agent_id.clone();
db.call(move |c| {
    c.execute(..., rusqlite::params![run_id_clone, agent_id_clone, ...])?;
    Ok(())
}).await?;
```
This is the established pattern throughout service.rs.

---

## Code Examples

### Embedding Retrieval from vec_memories (candidate fetch)

```rust
// Source: service.rs search_memories pattern + vec_memories schema in db.rs
// Adapted for compaction: JOIN to get both metadata and embedding bytes
let candidates = self.db.call(move |c| -> Result<Vec<CandidateMemory>, rusqlite::Error> {
    let mut stmt = c.prepare(
        "SELECT m.id, m.content, m.agent_id, m.session_id, m.tags,
                m.embedding_model, m.created_at, v.embedding
         FROM memories m
         JOIN vec_memories v ON v.memory_id = m.id
         WHERE m.agent_id = ?1
         ORDER BY m.created_at DESC
         LIMIT ?2"
    )?;
    let rows = stmt.query_map(
        rusqlite::params![agent_id, max_candidates as i64],
        |row| {
            let tags_str: String = row.get(4)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            let emb_bytes: Vec<u8> = row.get(7)?;
            // SAFETY: sqlite-vec writes aligned 4-byte IEEE 754 floats
            let embedding: Vec<f32> = unsafe {
                let ptr = emb_bytes.as_ptr() as *const f32;
                std::slice::from_raw_parts(ptr, emb_bytes.len() / 4).to_vec()
            };
            Ok(CandidateMemory {
                id: row.get(0)?,
                content: row.get(1)?,
                tags,
                created_at: row.get(6)?,
                embedding,
            })
        },
    )?;
    rows.collect::<Result<Vec<_>, _>>()
}).await?;
```

### Transaction Delete Pattern (from service.rs)

```rust
// Source: service.rs delete_memory (lines 295-332) — proven pattern
// Adapted for compaction: delete MULTIPLE source memories in one transaction
let tx = c.transaction()?;
// DELETE vecs first (no FK constraint but cleaner ordering)
for src_id in &source_ids {
    tx.execute(
        "DELETE FROM vec_memories WHERE memory_id = ?1",
        rusqlite::params![src_id],
    )?;
}
for src_id in &source_ids {
    tx.execute(
        "DELETE FROM memories WHERE id = ?1",
        rusqlite::params![src_id],
    )?;
}
tx.commit()?;
```

### Tier 1 Chronological Concatenation

```rust
fn tier1_concat(memories: &[&CandidateMemory]) -> String {
    // Already sorted by created_at DESC from DB fetch; reverse for ascending
    let mut sorted: Vec<&&CandidateMemory> = memories.iter().collect();
    sorted.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    sorted.iter().map(|m| m.content.as_str()).collect::<Vec<_>>().join("\n")
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| DBSCAN / HDBSCAN clustering | Greedy pairwise with similarity threshold | Phase 8 design (N<500 scope) | Simpler, no hyperparameter tuning, sufficient for N<500 |
| Averaged embeddings for merged memory | Re-embed merged content | Phase 8 design | More accurate semantic representation of actual merged text |
| Separate embedding INSERT after memory INSERT | Single transaction for both | Phase 6/7 precedent in create_memory | Atomic consistency between memories and vec_memories |

**No deprecated patterns:** All libraries (rusqlite, tokio-rusqlite, sqlite-vec, zerocopy) are at their pinned stable versions with no known deprecations affecting this phase.

---

## Open Questions

1. **Embedding bytes → Vec<f32> casting approach**
   - What we know: zerocopy works for Vec<f32> → bytes (write path). For read path, there is no bytemuck in Cargo.toml.
   - What's unclear: Whether to use unsafe transmute/slice reinterpret or add bytemuck to Cargo.toml.
   - Recommendation: Use the unsafe slice approach (1 line, zero new deps). Add a `// SAFETY:` comment documenting the sqlite-vec invariant. If planner prefers bytemuck, it's a one-line Cargo.toml addition — but CONTEXT.md says "no new dependencies expected."

2. **CompactionError vs. reusing ApiError directly**
   - What we know: Claude's discretion — no locked decision. EmbeddingEngine errors go through EmbeddingError → MnemonicError → ApiError. LlmError goes LlmError → MnemonicError → ApiError.
   - What's unclear: Whether a dedicated CompactionError type adds clarity or just adds boilerplate.
   - Recommendation: Reuse existing error types directly — no new CompactionError enum. The pipeline can return ApiError where needed, using `.map_err()` to convert domain errors via existing chains. This keeps error.rs unchanged.

3. **tracing instrumentation granularity**
   - What we know: Claude's discretion. Existing code uses tracing::info and tracing::debug in embedding.rs and summarization.rs.
   - What's unclear: Whether per-cluster info logging (one log per merged cluster) is too verbose.
   - Recommendation: `tracing::info!` at pipeline entry/exit with aggregate counts; `tracing::debug!` for per-cluster operations. Matches existing pattern in summarization.rs.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test + tokio::test (via tokio 1.x) |
| Config file | none (Cargo.toml dev-dependencies configure tower, http-body-util) |
| Quick run command | `cargo test --lib -- compaction` |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DEDUP-01 | Memories above similarity threshold are grouped into clusters | unit | `cargo test --lib -- compaction::tests::test_clustering` | ❌ Wave 0 |
| DEDUP-01 | Memories below threshold are NOT clustered | unit | `cargo test --lib -- compaction::tests::test_below_threshold_no_cluster` | ❌ Wave 0 |
| DEDUP-01 | First-match: memory already in cluster is not reassigned | unit | `cargo test --lib -- compaction::tests::test_first_match_assignment` | ❌ Wave 0 |
| DEDUP-02 | Merged memory has tag union of all sources | unit | `cargo test --lib -- compaction::tests::test_metadata_merge_tags` | ❌ Wave 0 |
| DEDUP-02 | Merged memory has earliest created_at from sources | unit | `cargo test --lib -- compaction::tests::test_metadata_merge_created_at` | ❌ Wave 0 |
| DEDUP-02 | Merged content is chronological concat of source texts | unit | `cargo test --lib -- compaction::tests::test_tier1_concat` | ❌ Wave 0 |
| DEDUP-03 | Atomic write: merged memory present + sources deleted in one tx | integration | `cargo test -- test_compact_atomic_write` | ❌ Wave 0 |
| DEDUP-03 | dry_run: no memories written, clusters returned | integration | `cargo test -- test_compact_dry_run` | ❌ Wave 0 |
| DEDUP-03 | Simulated tx failure: no orphans, no data loss | unit | `cargo test --lib -- compaction::tests::test_transaction_rollback_consistency` | ❌ Wave 0 |
| DEDUP-04 | max_candidates caps candidate set, truncated=true in response | unit | `cargo test --lib -- compaction::tests::test_max_candidates_truncation` | ❌ Wave 0 |
| DEDUP-04 | Candidates selected by created_at DESC ordering | unit | `cargo test --lib -- compaction::tests::test_candidate_ordering` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test --lib -- compaction`
- **Per wave merge:** `cargo test`
- **Phase gate:** Full suite green (currently 25 lib + integration) before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `src/compaction.rs` — CompactionService struct, all helper functions, unit test module
- [ ] `tests/integration.rs` additions — `test_compact_atomic_write`, `test_compact_dry_run` (append to existing file)

*(No new test files needed — unit tests go in `src/compaction.rs` `#[cfg(test)]` module, integration tests append to the existing `tests/integration.rs` following established patterns)*

---

## Sources

### Primary (HIGH confidence)

- Direct code inspection: `src/service.rs`, `src/embedding.rs`, `src/summarization.rs`, `src/error.rs`, `src/db.rs`, `src/main.rs`, `src/server.rs`, `src/lib.rs` — all patterns verified in current codebase
- `tests/integration.rs` — `cosine_similarity` helper function (lines 404-412) confirms dot-product approach already in use
- `.planning/phases/08-compaction-core/08-CONTEXT.md` — all locked decisions
- `Cargo.toml` — confirmed no new dependencies needed
- `.planning/STATE.md` — LlmError conversion chain, rusqlite pin rationale

### Secondary (MEDIUM confidence)

- rusqlite 0.37 docs: Transaction drop behavior (auto-rollback) — consistent with service.rs pattern and standard Rust RAII
- tokio-rusqlite 0.7: `db.call(move |c| ...)` signature requires `Send + 'static` closure — consistent with all existing uses

### Tertiary (LOW confidence)

- sqlite-vec BLOB read byte layout — inferred from write path (`zerocopy::IntoBytes` in service.rs) + sqlite-vec documentation that stores float[384] as packed 4-byte IEEE 754. Not directly verified via Context7 for the read path.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — zero new dependencies; all crates directly verified in Cargo.toml and existing code
- Architecture: HIGH — CompactionService mirrors existing patterns exactly; all sub-patterns (transaction, JSON, UUID, zerocopy, trait consumption) are proven in the codebase
- Pitfalls: HIGH — borrow conflict, LlmError chain, dry_run audit row are all directly stated in STATE.md and verified in error.rs; embedding byte reinterpretation is MEDIUM (inferred from write path)
- Algorithm: HIGH — greedy pairwise is simple, well-understood, explicitly described in CONTEXT.md with exact first-match semantics

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable — no external dependencies changing)
