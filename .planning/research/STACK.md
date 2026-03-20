# Stack Research

**Domain:** Rust agent memory server (embedded vector search, local ML inference, REST API)
**Researched:** 2026-03-20
**Confidence:** HIGH (all new-addition versions verified against official sources)

---

## Existing Stack (LOCKED — do not re-research)

The following are validated from v1.0 and must not change:

| Technology | Locked Version | Role |
|------------|---------------|------|
| tokio | 1 | Async runtime |
| axum | 0.8 | HTTP server |
| rusqlite | 0.37 (bundled) | SQLite access |
| sqlite-vec | 0.1.7 | Vector KNN extension |
| tokio-rusqlite | 0.7 | Async SQLite wrapper |
| candle-core/nn/transformers | 0.9 | Local ML inference |
| tokenizers | 0.22 | HuggingFace tokenization |
| hf-hub | 0.5 | Model weight download/cache |
| serde + serde_json | 1 | Serialization |
| reqwest | 0.13 | HTTP client (used for OpenAI embedding fallback) |
| zerocopy | 0.8 | Vec<f32>-to-bytes for sqlite-vec |
| tracing + tracing-subscriber | 0.1 / 0.3 | Structured logging |
| thiserror + anyhow | 2 / 1 | Error handling |
| uuid | 1 (v7) | Memory ID generation |
| figment | 0.10 | Config (TOML + env) |
| async-trait | 0.1 | EmbeddingEngine trait |

**Note on reqwest version:** Cargo.toml pins `reqwest = "0.13"`. This is intentional. The existing STACK.md (researched 2026-03-19) incorrectly lists `reqwest 0.12` — the actual binary uses 0.13. This matters for the LLM integration decision (see below).

---

## New Additions for v1.1

The following three capability areas require new stack decisions.

### 1. Vector Similarity Clustering / Dedup

**Recommendation: No new crate. Implement cosine similarity inline.**

**Rationale:**

The all-MiniLM-L6-v2 embeddings stored in `vec_memories` are **not pre-normalized** (confirmed by inspecting the existing inference path in `embedding.rs`). For deduplication at compact time, cosine similarity between embedding pairs is sufficient — no full clustering algorithm is needed for the Tier 1 (algorithmic dedup) use case.

The compaction workflow is:
1. Fetch all embeddings for the scoped agent_id
2. Compute pairwise cosine similarity in-memory
3. Apply greedy threshold clustering (mark pairs above threshold as duplicates)
4. Delete duplicates, insert merged/summarized replacement

This is O(n²) over n memories per agent — acceptable because compaction runs on demand and typical agent scopes are 50–5000 memories, not millions.

**Cosine similarity is four lines of arithmetic.** Adding a crate for this is over-engineering.

```rust
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 { 0.0 } else { dot / (norm_a * norm_b) }
}
```

**Why not hdbscan (0.12.0)?**

Investigated. The `hdbscan` crate's `DistanceMetric` enum does not include cosine similarity — it supports Chebyshev, Cylindrical, Euclidean, Haversine, Manhattan, and Precalculated. The `Precalculated` variant is a workaround (pass in a precomputed distance matrix), but this forces building the full n×n matrix before clustering, then running the algorithm — more complexity and memory than the greedy approach for this use case. HDBSCAN is designed for exploratory clustering of ambiguous data; mnemonic's compaction is threshold-based deduplication with a user-provided similarity cutoff. The simpler tool is correct here.

**Why not linfa-clustering?**

linfa-clustering uses ndarray `Array2<f32>` as its data format, which means converting our `Vec<Vec<f32>>` embeddings into a dense ndarray matrix. linfa's KMeans doesn't support cosine distance (Euclidean only). k-means also requires specifying k upfront, which is inappropriate when the number of duplicate clusters is unknown. Adding ndarray as a dependency for a use case that doesn't need it violates the project's single-binary minimalism.

**Verdict:** Zero new crates for clustering/dedup.

---

### 2. LLM API Integration (Tier 2 Summarization)

**Recommendation: Use reqwest directly. Do not add async-openai.**

**Rationale:**

The project already has `reqwest = "0.13"` in Cargo.toml. `async-openai` 0.33.x depends on `reqwest = "0.12"`. Adding async-openai would pull in **two incompatible versions of reqwest** simultaneously — Cargo resolves this by compiling both, bloating the binary and compile times. This is directly contrary to the single-binary simplicity constraint.

The LLM integration for summarization is a single API call:

```
POST {llm_base_url}/v1/chat/completions
Content-Type: application/json
Authorization: Bearer {api_key}

{
  "model": "{model}",
  "messages": [{"role": "user", "content": "...summarize these memories..."}]
}
```

The response parsing needs one `serde_json` struct (already a dependency). The existing `reqwest` client plus `serde_json` handles this in ~40 lines of Rust. No new crate is justified.

**Following the existing embedding_provider pattern:** The project already implements `EmbeddingEngine` as a trait with local (candle) and remote (OpenAI API via reqwest) backends. The LLM provider should follow the same pattern: a `LlmProvider` trait with a `summarize(memories: &[Memory]) -> Result<String>` method, backed by an HTTP client using the existing reqwest instance.

**OpenAI-compatible endpoint support:** The project's config pattern (following `embedding_provider`) should support:
- `llm_provider = "openai"` (or `"ollama"`, `"anthropic"`, etc.)
- `llm_api_base` — URL override (defaults to `https://api.openai.com`)
- `llm_api_key` — env var or config value
- `llm_model` — model name string

This mirrors how `OPENAI_API_KEY` and `OPENAI_API_BASE` work in async-openai, without the dependency.

**Configuration additions (figment):** No new config crate needed. The existing `figment` setup handles additional keys transparently.

**Verdict:** Zero new crates for LLM integration. Use existing reqwest 0.13 + serde_json.

---

### 3. SQLite Schema Additions for Compaction State

**Recommendation: Two schema additions, applied via `execute_batch` in db.rs.**

No new crates are needed. The existing `rusqlite` + `tokio-rusqlite` handles DDL changes the same way the current schema is managed.

#### Addition 1: `compact_runs` table

Tracks each compaction invocation for auditability and idempotency.

```sql
CREATE TABLE IF NOT EXISTS compact_runs (
    id TEXT PRIMARY KEY,                    -- uuid v7
    agent_id TEXT NOT NULL DEFAULT '',
    started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at DATETIME,
    memories_before INTEGER NOT NULL DEFAULT 0,
    memories_after INTEGER NOT NULL DEFAULT 0,
    clusters_found INTEGER NOT NULL DEFAULT 0,
    llm_used INTEGER NOT NULL DEFAULT 0,    -- boolean: 0/1
    similarity_threshold REAL NOT NULL,
    status TEXT NOT NULL DEFAULT 'running'  -- 'running' | 'complete' | 'failed'
);

CREATE INDEX IF NOT EXISTS idx_compact_runs_agent_id ON compact_runs(agent_id);
```

**Why:** Agents need to know when compaction last ran, how many memories were reduced, and whether LLM summarization was applied. This also enables `GET /memories/compact/status` as a future endpoint without schema changes.

#### Addition 2: `source_ids` column on `memories`

Tracks provenance of merged/summarized memories back to their source memory IDs.

```sql
ALTER TABLE memories ADD COLUMN source_ids TEXT NOT NULL DEFAULT '[]';
```

**Why:** After compaction, the merged/summary memory replaces N originals. `source_ids` is a JSON array of the deleted memory IDs (same format as `tags`). This lets agents understand that a compact memory represents a consolidation, supports future "expand" operations, and provides audit trail. The format follows the existing `tags` column pattern (JSON array as TEXT) — no schema complexity added.

#### No changes needed to `vec_memories`

The `vec_memories` virtual table stores only `(memory_id, embedding float[384])`. The embedding for a merged memory is either:
- The centroid of the cluster embeddings (algorithmic Tier 1: average the vectors), or
- The embedding of the LLM-generated summary (Tier 2)

Either way, it's just a new `INSERT` into `vec_memories` with the merged memory's ID. No structural change required.

#### Migration strategy

The schema uses `CREATE TABLE IF NOT EXISTS` and `ALTER TABLE ... ADD COLUMN IF NOT EXISTS` (SQLite 3.37+, available since 2021 — safe assumption for the bundled SQLite in rusqlite 0.37's `bundled` feature). Apply additions in `db::open()` after the existing `execute_batch`.

---

## Recommended Cargo.toml Changes

```toml
# No new dependencies required for v1.1.
# All three capability areas (clustering, LLM API, schema) are served by
# the existing dependency set.

# Verify existing versions are consistent with these notes:
rusqlite = { version = "0.37", features = ["bundled"] }  # DO NOT upgrade to 0.38/0.39 — sqlite-vec 0.1.7 has known conflict with 0.39's libsqlite3-sys
reqwest = { version = "0.13", features = ["json"] }      # Required for LLM summarization HTTP calls — already present
```

---

## Alternatives Considered

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| Inline cosine similarity (no crate) | hdbscan 0.12.0 | No cosine metric support; Precalculated workaround adds complexity; HDBSCAN over-engineered for threshold dedup |
| Inline cosine similarity (no crate) | linfa-clustering (k-means) | Requires ndarray conversion; k-means needs upfront k; no cosine distance support |
| Raw reqwest + serde_json | async-openai 0.33.x | async-openai uses reqwest 0.12; project uses reqwest 0.13; adding both means two HTTP stacks in binary; ~40 lines of raw HTTP replaces the entire dependency |
| ALTER TABLE ADD COLUMN | New separate table for source_ids | Over-engineering; the column belongs on `memories` since it's an attribute of a memory, not a join table. |

---

## What NOT to Add

| Avoid | Why | What to Use Instead |
|-------|-----|---------------------|
| async-openai | Uses reqwest 0.12; conflicts with existing reqwest 0.13; binary bloat | Raw reqwest 0.13 + serde_json (already in project) |
| hdbscan | No cosine distance support; adds kdtree/rayon transitive deps; over-engineered for use case | Inline cosine similarity function (4 lines) |
| linfa / linfa-clustering | Requires ndarray Array2; k-means wrong algorithm for unknown-cluster-count dedup | Inline cosine similarity + greedy threshold algorithm |
| ndarray | Not needed once linfa is excluded; Vec<Vec<f32>> is sufficient for compaction workload | Rust standard Vec types |
| openai-api-rs / openai-api-rust | Low maintenance, low download count alternatives to async-openai; same dependency issues | Raw reqwest |

---

## Version Compatibility Notes

| Package | Note |
|---------|------|
| rusqlite 0.37 | Must stay at 0.37. sqlite-vec 0.1.7 has a documented conflict with rusqlite 0.39's libsqlite3-sys version. The project already pins this. |
| reqwest 0.13 | Do not add async-openai — it pins reqwest 0.12 and Cargo will compile both. |
| candle 0.9 | All three subcrates (core, nn, transformers) must be identical version. LLM integration does not touch candle — no version risk. |

---

## Sources

- [async-openai docs.rs 0.33.1](https://docs.rs/async-openai/latest/async_openai/) — confirmed reqwest 0.12 dependency, confirmed OpenAIConfig.with_api_base() builder method (HIGH confidence)
- [async-openai Cargo.toml on GitHub](https://github.com/64bit/async-openai/blob/main/async-openai/Cargo.toml) — confirmed `reqwest = "0.12"` dependency (HIGH confidence)
- [hdbscan 0.12.0 docs.rs DistanceMetric enum](https://docs.rs/hdbscan/0.12.0/hdbscan/enum.DistanceMetric.html) — confirmed variants: Chebyshev, Cylindrical, Euclidean, Haversine, Manhattan, Precalculated — no cosine (HIGH confidence)
- [hdbscan 0.12.0 docs.rs Hdbscan struct](https://docs.rs/hdbscan/0.12.0/hdbscan/struct.Hdbscan.html) — confirmed Vec<Vec<f32>> input format (HIGH confidence)
- [reqwest 0.13 breaking changes](https://github.com/openapitools/openapi-generator/issues/22621) — confirmed 0.12→0.13 is a breaking change (query/form now feature-gated; rustls default changed) (MEDIUM confidence)
- [Existing Cargo.toml](../../../Cargo.toml) — confirmed reqwest 0.13 in use, confirmed rusqlite 0.37 pinned (HIGH confidence — source of truth)
- [Existing db.rs](../../../src/db.rs) — confirmed schema structure: memories table, vec_memories virtual table (HIGH confidence — source of truth)
- [arewelearningyet.com clustering](https://www.arewelearningyet.com/clustering/) — surveyed full Rust ML ecosystem for clustering options (MEDIUM confidence)

---
*Stack research for: Mnemonic v1.1 — memory summarization/compaction additions*
*Researched: 2026-03-20*
