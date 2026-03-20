# Project Research Summary

**Project:** Mnemonic v1.1 — Memory Compaction and Summarization
**Domain:** Rust single-binary agent memory server — embedded vector search, local ML inference, REST API
**Researched:** 2026-03-20
**Confidence:** HIGH

## Executive Summary

Mnemonic v1.1 adds memory compaction and LLM-powered summarization to an already-shipped v1.0 baseline. The v1.0 system is a clean 4-layer Rust architecture (axum HTTP -> MemoryService -> EmbeddingEngine + SQLite) with 1,932 lines across 8 files, and the architecture research confirms this foundation accommodates all v1.1 capabilities through additive changes only — no refactoring, no new external dependencies, no structural upheaval. The recommended approach follows a two-tier compaction model: Tier 1 is purely algorithmic greedy pairwise cosine similarity deduplication (zero new crates, 30 lines of Rust), and Tier 2 is optional LLM-powered summarization using the existing reqwest 0.13 + serde_json stack. The new `CompactionService` is a peer of `MemoryService` rather than a method on it, mirroring the `SummarizationEngine` trait off the already-proven `EmbeddingEngine` pattern.

The features research, backed by the SimpleMem paper (arXiv 2601.02553) and AgentZero production patterns, establishes that a default similarity threshold of 0.85 is appropriate, with configurable `recency_bias` for temporal weighting. The competitor analysis confirms that Mnemonic's differentiator is "no LLM required" — Mem0, AgentZero, and SimpleMem all require a language model; Mnemonic's Tier 1 algorithmic path works without one. Stack research ruled out all candidate third-party crates: async-openai conflicts with reqwest 0.13, hdbscan lacks cosine distance support, linfa-clustering requires ndarray. The existing dependency set is sufficient for every v1.1 capability.

The critical risks concentrate in compaction atomicity, namespace isolation, and LLM integration safety. Non-atomic merge — deleting source memories before confirming the replacement is written — is an irreversible data-loss scenario with no in-system recovery. Cross-agent compaction (missing `agent_id` WHERE filter in clustering SQL) is a data contamination scenario. Prompt injection via stored memory content into the LLM summarization prompt is a novel attack surface documented by Palo Alto Unit 42 (2025) specifically for agent memory systems. All three have clear preventions that must be enforced before the compaction endpoint ships.

## Key Findings

### Recommended Stack

The v1.0 dependency set requires zero additions for v1.1. All three capability areas — clustering, LLM integration, and schema migration — are served by existing crates. The only Cargo.toml requirements are to keep `rusqlite = "0.37"` pinned (sqlite-vec 0.1.7 has a documented conflict with rusqlite 0.39's libsqlite3-sys) and to keep `reqwest = "0.13"` (async-openai 0.33.x depends on reqwest 0.12 and would introduce two HTTP stacks in the binary). Two SQL additions are needed: a `compact_runs` table for auditability and a `source_ids` column on `memories` for provenance tracking.

**Core technologies (locked from v1.0, reused for v1.1):**
- **reqwest 0.13 + serde_json**: LLM summarization HTTP client — already present; ~40 lines replaces the entire async-openai dependency
- **rusqlite 0.37 (bundled) + tokio-rusqlite 0.7**: Schema migration via `ALTER TABLE ADD COLUMN IF NOT EXISTS`; atomic write transactions for compaction
- **sqlite-vec 0.1.7**: KNN similarity queries for cluster candidate fetching during compaction
- **candle-core/nn/transformers 0.9**: Re-embedding of compacted memory content using the same local inference path as v1.0
- **async-trait 0.1**: `SummarizationEngine` trait follows the existing `EmbeddingEngine` trait pattern exactly
- **figment 0.10**: `llm_provider`, `llm_api_key`, `llm_base_url` config fields added identically to existing `embedding_provider` pattern

**No new dependencies required.**

### Expected Features

**Must have (table stakes for v1.1 to be coherent):**
- `POST /memories/compact` endpoint — agent-triggered, explicit, no background magic
- Scoped compaction — `agent_id` required, hard WHERE filter in clustering SQL
- Tier 1: Greedy pairwise vector similarity deduplication — works for all users without an LLM
- Configurable `similarity_threshold` (default 0.85, range [0.5, 1.0])
- Metadata merge — merged memory inherits tag union from all source memories, preserves earliest `created_at`
- Compaction response with full stats: `{ memories_before, memories_after, clusters_found, memories_removed, memories_created }`
- Response includes old-to-new ID mapping — agents that cache memory IDs can update stale references

**Should have (high value, low-to-medium cost — include in v1.1):**
- Tier 2: LLM-powered cluster summarization (opt-in via `llm_provider` config)
- Configurable LLM provider following `embedding_provider` pattern (`openai` / `none`)
- `recency_bias` float [0.0, 1.0] for time-weighted similarity using SimpleMem's affinity formula
- `dry_run: bool` request field — preview proposed clusters without committing changes

**Defer (v1.2+):**
- `session_id` scoping for compaction (agent-level only for v1.1)
- Streaming progress events for compactions over 1000 memories
- Compaction history / audit log endpoint (`GET /memories/compact/status`)

**Confirmed anti-features (exclude permanently):**
- Automatic background compaction — violates agent control philosophy; PROJECT.md explicitly excludes
- Hierarchical/parent-child summaries — PROJECT.md explicitly excludes
- Memory decay/TTL — PROJECT.md explicitly excludes
- Cross-agent compaction — violates `agent_id` isolation invariant

### Architecture Approach

The v1.1 architecture adds exactly two new source files (`compaction.rs`, `summarization.rs`) and makes additive edits to six existing files (`main.rs`, `config.rs`, `server.rs`, `db.rs`, `error.rs`, `lib.rs`). `MemoryService` is untouched. `CompactionService` is a new peer struct in `AppState` that reuses the same `Arc<Connection>` clone and the same `Arc<dyn EmbeddingEngine>` as `MemoryService` — no second connection, no new infrastructure. The compaction data flow has four distinct phases: fetch (read-only DB call), cluster (pure Rust computation, no DB lock held), synthesize (optional LLM call, no DB lock held), and write (single atomic SQLite transaction covering INSERT new memory + DELETE source memories in both `memories` and `vec_memories`).

**Major components:**
1. **`CompactionService`** (`src/compaction.rs`) — orchestrates the full compaction pipeline; holds `Arc<Connection>`, `Arc<dyn EmbeddingEngine>`, `Option<Arc<dyn SummarizationEngine>>`; the `Option` provides clean Tier 1 / Tier 2 semantics
2. **`SummarizationEngine` trait + `OpenAiSummarizer`** (`src/summarization.rs`) — single method `async fn summarize(&self, texts: &[String]) -> Result<String>`; mirrors EmbeddingEngine pattern exactly; `MockSummarizer` for tests without network calls
3. **Schema additions** (`db.rs`) — `compact_runs` table for run auditability; `source_ids TEXT DEFAULT '[]'` column on `memories` for provenance of merged memories
4. **Compaction handler** (`server.rs`) — thin axum handler; deserializes `CompactRequest`, calls `CompactionService::compact()`, returns `CompactResponse`
5. **Config extensions** (`config.rs`) — `llm_provider`, `llm_api_key`, `llm_base_url`, `llm_model` fields; `validate_config()` extended with LLM validation block matching embedding_provider pattern

### Critical Pitfalls

1. **Non-atomic merge (delete before insert)** — Irreversible data loss if crash occurs between DELETE and INSERT. Prevention: always insert the merged memory first, then delete sources, entirely within a single `conn.call()` transaction that never crosses an async boundary. Never hold a SQLite write transaction open while awaiting an LLM response.

2. **Cross-namespace compaction** — Missing `agent_id = ?` WHERE filter in clustering SQL merges memories across agents. Prevention: `agent_id` must be required in the `CompactRequest` body; the clustering query must enforce it as a hard filter; multi-agent integration tests are mandatory before ship.

3. **Prompt injection via stored memory content** — Malicious content in a memory (from a previous agent session reading a hostile source) can manipulate the LLM summarizer output. Prevention: always delimit memory content with explicit data-framing tags in the prompt (`<memories>...</memories>`), never use raw string interpolation; set `max_tokens` on every LLM call.

4. **Similarity threshold semantics inversion** — sqlite-vec returns distance (lower = more similar); treating it as similarity or inverting the comparison operator causes compaction to merge the most dissimilar memories instead of the most similar. Prevention: lock threshold semantics before implementation; implement `dry_run` mode for validation before commit.

5. **Runaway LLM cost from unbounded compaction** — A looping agent or large cluster can burn hundreds of API calls. Prevention: enforce max cluster size per LLM call (20 memories / 4000 tokens), max LLM calls per compaction request (10 clusters), and per-agent rate limiting on the compact endpoint.

## Implications for Roadmap

Based on the dependency graph from ARCHITECTURE.md and risk profile from PITFALLS.md, the natural build order is:

### Phase 1: Foundation — Errors, Config, Schema

**Rationale:** Every subsequent component depends on having `CompactionError`/`SummarizationError` in `error.rs`, the `llm_provider` config fields in `config.rs`, and the new schema columns in `db.rs`. These have zero risk (pure additive changes) and unblock all other phases. This phase also establishes the `agent_id` scoping requirement at the schema level before any clustering logic exists.

**Delivers:** A server that starts cleanly on v1.0 databases (idempotent `ALTER TABLE ADD COLUMN IF NOT EXISTS`), exposes new config fields, and has the error type hierarchy ready for downstream use.

**Addresses (from FEATURES.md):** Configurable `llm_provider`, `llm_api_base`, `llm_model` config; `source_ids` provenance column; `compact_runs` audit table.

**Avoids (from PITFALLS.md):** Schema migration bugs; config validation gaps; error type collisions.

---

### Phase 2: SummarizationEngine Trait + OpenAiSummarizer

**Rationale:** The `SummarizationEngine` trait is a dependency of `CompactionService` but has no dependencies of its own beyond `error.rs`. Isolating it as a separate phase makes it unit-testable with `MockSummarizer` before any clustering logic exists. This is also where the prompt injection prevention must be designed and validated — the highest-security-risk component should be reviewable in isolation.

**Delivers:** A tested `SummarizationEngine` trait with `OpenAiSummarizer` (real LLM) and `MockSummarizer` (deterministic, no network). The summarization prompt with explicit data-framing tags. LLM timeout and `max_tokens` enforcement. Token usage returned in the response.

**Implements (from ARCHITECTURE.md):** `SummarizationEngine` trait pattern mirroring `EmbeddingEngine`; `OpenAiSummarizer` using existing reqwest 0.13 client.

**Avoids (from PITFALLS.md):** Prompt injection (data-framing tags in prompt design); runaway LLM cost (max_tokens, per-call timeout); LLM failure leaving inconsistent state (fallback behavior defined here).

---

### Phase 3: Compaction Core Logic

**Rationale:** This is the highest-complexity phase and depends on Phases 1 and 2. All clustering, metadata merge, atomic write, and dry-run logic lives here. The atomicity guarantee (insert-first, delete-second within a single `conn.call()` transaction) must be implemented and verified with fault injection tests before the HTTP endpoint is wired up.

**Delivers:** `CompactionService::compact()` implementing the full 4-phase pipeline: fetch memories + embeddings, compute pairwise cosine similarity with optional `recency_bias` time weighting, greedy cluster formation with centroid validation, atomic write phase. `dry_run` mode returns proposed clusters without committing. Metadata merge (tag union, earliest `created_at`).

**Addresses (from FEATURES.md):** Tier 1 deduplication; Tier 2 LLM summarization (via `Option<Arc<dyn SummarizationEngine>>`); `recency_bias` parameter; `dry_run` mode; compaction stats response; old-to-new ID mapping.

**Avoids (from PITFALLS.md):** Non-atomic merge (fault injection test required); non-transitive cluster instability (centroid validation + determinism test); wrong metadata on merged memory (explicit assertions on `created_at` and tag union); compaction blocking reads/writes (`spawn_blocking` for CPU-intensive clustering); similarity threshold semantics inversion (dry_run tests with known pairs).

---

### Phase 4: HTTP Integration and Wiring

**Rationale:** Wire everything together last, after all lower-layer components are tested in isolation. This phase is deliberately thin — the handler itself should be 15-20 lines. Integration tests at this layer cover the full end-to-end flow including multi-agent namespace isolation.

**Delivers:** `POST /memories/compact` route in `server.rs`; `CompactionService` injected into `AppState` in `main.rs`; full integration tests including multi-agent scenario (compact Agent A, verify Agent B untouched); load test measuring read/write latency during compaction.

**Avoids (from PITFALLS.md):** Cross-namespace compaction (multi-agent integration test mandatory here); breaking agents via ID deletion (integration test verifies response includes old-to-new ID mapping); compaction blocking normal operations (concurrent load test).

---

### Phase Ordering Rationale

- Errors before config before schema matches the dependency graph in ARCHITECTURE.md's Build Order section — each layer is needed by the next.
- SummarizationEngine isolated to Phase 2 keeps the highest-security-risk component (prompt design, LLM timeout, cost controls) reviewable in isolation before clustering logic adds complexity.
- Compaction core before HTTP ensures the most complex and highest-risk logic (atomicity, namespace isolation) is proven by unit/integration tests before the endpoint is exposed.
- All four phases must ship together as v1.1 — none can be deferred. The Tier 2 LLM path is explicitly a v1.1 feature per the features research.

### Research Flags

Phases with standard, well-documented patterns (research-phase likely unnecessary):
- **Phase 1 (Foundation):** Pure Rust additions following existing patterns; all decisions are additive mirrors of v1.0 code.
- **Phase 4 (HTTP Integration):** Axum routing is well-understood in this codebase; the complexity lives in the layers below.

Phases that may benefit from `/gsd:research-phase` if implementation hits uncertainty:
- **Phase 2 (Summarization):** Prompt injection prevention is an evolving field. The XML data-framing approach is current best practice (2025 OWASP + Unit 42 sources), but a research-phase update before implementation is prudent if the specific prompt structure needs refinement for a given LLM provider.
- **Phase 3 (Compaction Core):** Centroid validation for non-transitive cluster stability is the one algorithm decision with meaningful implementation choices. If the greedy + centroid approach produces unexpected results during testing, a research-phase into alternative single-linkage strategies would be warranted.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All crate version decisions verified against official sources (docs.rs, GitHub Cargo.toml). The reqwest 0.12/0.13 conflict is a concrete finding from the actual async-openai Cargo.toml, not inference. |
| Features | MEDIUM-HIGH | Similarity threshold defaults from SimpleMem paper and AgentZero production system. Threshold universality acknowledged as LOW confidence (Jason Liu source) — the `dry_run` mode is the mitigation. Feature scope confirmed against PROJECT.md. |
| Architecture | HIGH | Architecture is additive extension of existing v1.0 codebase inspected directly. Component boundaries derived from the proven EmbeddingEngine pattern. tokio-rusqlite actor model behavior is documented in official docs. |
| Pitfalls | HIGH | Critical pitfalls have official source backing: SQLite atomicity docs, Palo Alto Unit 42 prompt injection research (2025), OWASP LLM01:2025, NeurIPS 2025 InjecMEM paper. |

**Overall confidence:** HIGH

### Gaps to Address

- **Similarity threshold validation:** The 0.85 default is research-backed but not validated against real Mnemonic memory content. The `dry_run` mode must be implemented before the default is finalized; first production users should be encouraged to run dry-run first and tune accordingly.
- **Centroid validation algorithm detail:** The architecture recommends centroid-based cluster validation to handle non-transitivity, but the exact handling of cluster members that fail the secondary centroid check (eject to singleton vs. keep in cluster) is left to implementation. This should be a documented decision in the Phase 3 plan.
- **LLM model default:** Research recommends `gpt-4o-mini` for cost/quality balance, but this is not validated against actual compaction output quality. Users must be able to override with any OpenAI-compatible model string.
- **`max_candidates` limit for large memory sets:** ARCHITECTURE.md recommends a configurable `max_candidates` limit for agents approaching 10K memories, but the exact default and enforcement mechanism are not specified. Define these in the Phase 3 plan.

## Sources

### Primary (HIGH confidence)
- [SimpleMem: Efficient Lifelong Memory for LLM Agents (arXiv 2601.02553)](https://arxiv.org/html/2601.02553v1) — Affinity formula, 0.85 clustering threshold
- [async-openai Cargo.toml on GitHub](https://github.com/64bit/async-openai/blob/main/async-openai/Cargo.toml) — reqwest 0.12 dependency confirmed; conflict with project's reqwest 0.13
- [hdbscan 0.12.0 docs.rs DistanceMetric enum](https://docs.rs/hdbscan/0.12.0/hdbscan/enum.DistanceMetric.html) — no cosine distance support confirmed
- [Unit 42 / Palo Alto: Indirect Prompt Injection Poisons AI Long-Term Memory](https://unit42.paloaltonetworks.com/indirect-prompt-injection-poisons-ai-longterm-memory/) — prompt injection via summarization, persistent memory attack
- [OWASP LLM01:2025 Prompt Injection](https://genai.owasp.org/llmrisk/llm01-prompt-injection/) — current classification and prevention strategies
- [InjecMEM: Memory Injection Attack on LLM Agent Memory Systems (NeurIPS 2025)](https://openreview.net/forum?id=QVX6hcJ2um) — query-only injection into memory banks
- [tokio-rusqlite docs](https://docs.rs/tokio-rusqlite/latest/tokio_rusqlite/) — actor model, Connection clone behavior
- [SQLite WAL mode official docs](https://sqlite.org/wal.html) — reader/writer concurrency model
- [SQLite Atomic Commit](https://sqlite.org/atomiccommit.html) — rollback journal atomicity guarantees
- Existing v1.0 source code (`src/*.rs`) — architecture baseline; confirmed reqwest 0.13, confirmed rusqlite 0.37

### Secondary (MEDIUM confidence)
- [AgentZero Memory Consolidation System (DeepWiki)](https://deepwiki.com/frdel/agent-zero/4.3-memory-consolidation-system) — 0.70/0.90 threshold values, LLM prompt structure
- [NVIDIA NeMo SemDeDup documentation](https://docs.nvidia.com/nemo-framework/user-guide/24.09/datacuration/semdedup.html) — threshold configuration, aggressive vs. conservative tradeoffs
- [PSA: SQLite connection pool write performance — Evan Schwartz](https://emschwartz.me/psa-your-sqlite-connection-pool-might-be-ruining-your-write-performance/) — single-writer performance data
- [Widemem: importance scoring, decay, conflict resolution](https://discuss.huggingface.co/t/widemem-open-source-memory-layer-for-llms-with-importance-scoring-decay-and-conflict-resolution/174269) — recency decay formula
- [Factory.ai: Evaluating Context Compression](https://factory.ai/news/evaluating-compression) — structure preservation, incremental merge patterns
- [Finding near-duplicates with Jaccard similarity and MinHash](https://blog.nelhage.com/post/fuzzy-dedup/fuzzy-dedup/) — non-transitivity of similarity measures

### Tertiary (LOW confidence)
- [Jason Liu: Two Experiments on Agent Compaction](https://jxnl.co/writing/2025/08/30/context-engineering-compaction/) — field lacks empirical consensus on thresholds; single source, acknowledged caveat
- [Memory Optimization Strategies in AI Agents — Medium](https://medium.com/@nirdiamant21/memory-optimization-strategies-in-ai-agents-1f75f8180d54) — compaction frequency patterns; community source

---
*Research completed: 2026-03-20*
*Ready for roadmap: yes*
