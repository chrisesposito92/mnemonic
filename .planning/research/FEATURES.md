# Feature Research

**Domain:** Memory summarization / compaction for agent memory server (v1.1 milestone)
**Researched:** 2026-03-20
**Confidence:** MEDIUM-HIGH (thresholds from multiple sources; LLM prompt patterns from open-source implementations; Rust clustering crates verified on crates.io)

---

## Scope Note

This document covers **only the new features for v1.1**. The v1.0 baseline (5 REST endpoints, local embeddings, agent_id/session_id namespacing, SQLite+sqlite-vec) is already shipped and is treated as a dependency, not a feature.

---

## Feature Landscape

### Table Stakes (Users Expect These)

Features that any agent developer expects from a "memory compaction" feature. Missing these makes the feature feel incomplete or untrustworthy.

| Feature | Why Expected | Complexity | Dependencies on Existing Architecture |
|---------|--------------|------------|---------------------------------------|
| POST /memories/compact endpoint | Every memory system with compaction uses an explicit API call. Agent stays in control — no background magic. This is the industry-standard pattern (OpenAI, Anthropic, AgentZero all surface compaction as explicit action). | LOW | Requires existing `POST /memories` + `DELETE /memories/{id}`. New route added to `server.rs`. |
| Scoped compaction (agent_id required) | Compacting the wrong agent's memories is data loss. Every reviewed system scopes compaction by namespace. | LOW | Requires `agent_id` query param / body field — matches existing `SearchParams` pattern in `service.rs`. |
| Vector similarity deduplication (no LLM) | The "always works" tier. Agents without LLM credentials still need deduplication. Algorithmic, deterministic, zero-cost. | MEDIUM | Uses existing `vec_memories` sqlite-vec virtual table and its cosine distance queries. No new infra. |
| Metadata merge on dedup | When two memories are merged, the surviving memory should inherit tags from both. Agents expect merged memories to be richer, not information-lossy. | LOW | Requires reading `tags` JSON from `memories` table, merging arrays, updating surviving row. |
| Compaction response with stats | Agents need to know what happened: how many memories existed, how many were removed, how many clusters were merged. Without this, the agent cannot make decisions based on compaction results. | LOW | Response struct: `{ memories_before, memories_after, clusters_found, memories_removed, memories_created }`. New type in `service.rs`. |
| Configurable similarity threshold | No single threshold is correct for all use cases. Agents need to tune aggressiveness. Research shows 0.85 is the established default (SimpleMem paper, AgentZero 0.7 for discovery / 0.9 for replace). | LOW | New field in `CompactRequest` body. Default to 0.85 with valid range [0.5, 1.0]. |

### Differentiators (Competitive Advantage)

Features that are not universally expected but differentiate Mnemonic within its "zero-config, single-binary" positioning.

| Feature | Value Proposition | Complexity | Dependencies on Existing Architecture |
|---------|-------------------|------------|---------------------------------------|
| LLM-powered cluster summarization (opt-in) | Pure dedup loses context; summarization synthesizes. Competitors that require LLM (Mem0, Zep) always use it. Mnemonic differentiates by making it **optional** — works without LLM, better with one. | HIGH | Requires new `llm_provider` config following existing `embedding_provider` pattern in `config.rs`. Calls LLM API to generate summary text for a cluster. Writes summary as new memory via existing store path. |
| Configurable LLM provider (same pattern as embeddings) | Users already understand the `embedding_provider` config enum. Reusing the same pattern for LLM is zero learning curve. | MEDIUM | Extend `Config` struct and `validate_config()`. New `LlmEngine` trait parallel to `EmbeddingEngine`. OpenAI-compatible HTTP client (reqwest already in dep tree via axum). |
| Time-based weighting parameter for compaction aggressiveness | Research-backed: the SimpleMem affinity formula `β·cos(vᵢ,vⱼ) + (1-β)·e^(−λ|tᵢ−tⱼ|)` combines semantic similarity with temporal proximity. Older, semantically similar memories should be more aggressively merged than recent ones. Configurable `recency_bias` float [0.0, 1.0] maps to β. | MEDIUM | Requires `created_at` timestamps from `memories` table (already stored). Adjusted affinity score computed in Rust before clustering decision. No new storage. |
| Dry-run / preview mode | Agent can see what compaction would do without committing changes. Builds trust in the feature. Pattern is present in NetApp compaction docs and database vacuum tooling but rare in agent memory systems — genuine differentiator. | LOW | `dry_run: bool` field in request body. If true, return stats without executing DELETE/INSERT. All computation identical, skip writes. |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Automatic background compaction | "Just run it periodically" feels convenient | Silently mutates agent memory without agent consent. Race conditions with ongoing agent writes. Violates Mnemonic's explicit control philosophy. PROJECT.md explicitly excludes this. | Agent calls POST /memories/compact at natural checkpoints (session end, token budget threshold). |
| Hierarchical / parent-child summaries | More structure = richer memory model | Requires traversal logic, schema changes (parent_id FK), recursive queries. Covers a use case that cluster-and-replace already handles at 90% quality. PROJECT.md explicitly excludes. | Flat cluster-and-replace. Summaries become first-class memories searchable like any other. |
| Per-memory importance scoring at write time | "Weight memories by importance" sounds good | Requires LLM call on every single write, adding latency and API key dependency to the baseline path. Breaks the zero-config guarantee. | Importance is implicit in retrieval: frequently-retrieved memories survive compaction because they form their own clusters. |
| Compaction across agent boundaries | "Merge learnings from all agents" | Violates the agent_id isolation invariant that all existing query logic is built around. A bug here creates cross-agent data leakage. | Multi-agent knowledge sharing is a separate feature with different security model. Out of scope for v1.1. |
| Memory decay / TTL on compact | "Age out old memories automatically" | Surprising and irreversible. Data loss without explicit agent action. PROJECT.md explicitly excludes. | Time-based weighting in compaction makes old memories more likely to be clustered, not silently deleted. |
| HDBSCAN / DBSCAN clustering crate | "Proper density-based clustering" sounds rigorous | `petal-clustering` and `hdbscan` crates exist in Rust (verified on crates.io) but add a significant dependency. For typical agent memory sizes (10–500 memories per agent_id), simple greedy pairwise similarity with a threshold outperforms density-based clustering on correctness-per-complexity ratio. DBSCAN requires choosing ε (epsilon) and min_samples, which users cannot tune intuitively. | Greedy single-linkage: sort pairs by similarity, merge above threshold. O(n²) acceptable for 100–500 memories. Produces predictable, debuggable clusters. |

---

## Feature Dependencies

```
[POST /memories/compact endpoint]
    └──requires──> [POST /memories] (to write summary/merged content)
    └──requires──> [DELETE /memories/{id}] (to remove deduplicated originals)
    └──requires──> [agent_id namespacing] (to scope compaction safely)
    └──requires──> [vec_memories sqlite-vec table] (for similarity queries)

[Vector similarity deduplication]
    └──requires──> [vec_memories embeddings] (existing, 384-dim float vectors)
    └──requires──> [memories.created_at] (for time-based weighting)
    └──enables──> [Metadata merge] (once cluster is identified, merge tags)

[Time-based weighting]
    └──requires──> [memories.created_at] (already in schema — no migration needed)
    └──enhances──> [Vector similarity deduplication] (adjusts similarity scores before clustering)

[LLM-powered summarization]
    └──requires──> [Vector similarity deduplication] (clusters must be identified first)
    └──requires──> [LlmEngine trait + config] (new, following embedding_provider pattern)
    └──requires──> [POST /memories] (to store the generated summary)
    └──enhances──> [Metadata merge] (LLM summary replaces merged content, metadata still merged)

[Dry-run mode]
    └──requires──> [all compaction logic above] (identical computation, no writes)
    └──conflicts──> [nothing] (purely additive flag)

[Configurable similarity threshold]
    └──requires──> [CompactRequest body struct] (new field, not a breaking change)
```

### Dependency Notes

- **Dedup requires existing vec_memories:** The KNN query pattern already used in `GET /memories/search` is reused for similarity lookups during compaction. No new vector infrastructure needed.
- **LLM tier requires dedup tier:** Clusters must be identified algorithmically before the LLM is invoked. LLM receives a cluster of N memory contents and returns a consolidated summary. Dedup-without-LLM is Tier 1; Tier 2 adds LLM on top.
- **Time weighting requires no schema migration:** `created_at DATETIME` is already in the `memories` table. The weighting is applied in application code, not stored.
- **LLM provider follows embedding_provider pattern:** Same `validate_config()` gate, same env-var-or-TOML approach. Users who have already configured OpenAI for embeddings can reuse that key for summarization.

---

## MVP Definition

### This Milestone Is v1.1 (not a greenfield MVP)

The question is not "what is minimum to validate the concept" — v1.0 is already shipped and validated. The question is "what is the minimum coherent compaction feature that delivers real value?"

### Ship in v1.1

- [ ] `POST /memories/compact` endpoint — agent-triggered, scoped by `agent_id`, returns stats
- [ ] Tier 1: Greedy pairwise vector similarity deduplication — works for all users, no LLM required
- [ ] Metadata merge — surviving memory inherits tags union from all merged memories
- [ ] Configurable `similarity_threshold` (default 0.85, range [0.5, 1.0])
- [ ] Time-based weighting via `recency_bias` float [0.0, 1.0] (default 0.0 = pure semantic)
- [ ] Compaction response with `{ memories_before, memories_after, clusters_found, memories_removed, memories_created }`
- [ ] Tier 2: LLM-powered summarization (opt-in, requires `llm_provider` config) — produces a consolidated summary memory per cluster instead of just keeping the most-similar one
- [ ] `dry_run: bool` request field — preview without committing

### Add After Validation (v1.2+)

- [ ] `POST /memories/compact` with `session_id` scoping — currently agent_id only; add session scoping after collecting feedback on compaction granularity
- [ ] Streaming progress events for large compaction jobs — add when users report timeouts on large memory sets (>1000 memories)
- [ ] Compaction history / audit log — add when users ask "what was removed and why?"

### Out of Scope (Confirmed by PROJECT.md)

- [ ] Automatic background compaction — explicitly excluded
- [ ] Hierarchical summaries — explicitly excluded
- [ ] Memory decay / TTL — explicitly excluded
- [ ] Cross-agent compaction — not in scope for any milestone

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| POST /memories/compact endpoint + routing | HIGH | LOW | P1 |
| Greedy similarity deduplication (Tier 1) | HIGH | MEDIUM | P1 |
| Metadata tag merge | MEDIUM | LOW | P1 |
| Configurable similarity threshold | MEDIUM | LOW | P1 |
| Compaction response stats | HIGH | LOW | P1 |
| LLM summarization (Tier 2) | HIGH | HIGH | P1 |
| Configurable LLM provider (config + validate) | HIGH | MEDIUM | P1 |
| Time-based weighting (recency_bias) | MEDIUM | LOW | P2 |
| Dry-run mode | MEDIUM | LOW | P2 |

**Priority key:**
- P1: Must have for v1.1 to be a coherent compaction feature
- P2: High value, low cost — include in v1.1 unless schedule pressure forces cut
- P3: Future milestone

---

## Algorithm Detail: Greedy Pairwise Deduplication

This section bridges research findings into the implementation decision.

**Why greedy pairwise, not DBSCAN/HDBSCAN:**
- Agent memory sets are small (typically 10–500 per agent_id) — O(n²) pairwise is 250,000 comparisons max, negligible on modern hardware
- DBSCAN requires ε (distance threshold) and min_samples — not intuitive for users; our single `similarity_threshold` parameter is simpler and maps directly to user intent
- `petal-clustering` and `hdbscan` Rust crates exist but add dependency weight without benefit at this scale
- Greedy single-linkage produces predictable, deterministic clusters that are easy to debug and explain in compaction stats

**Similarity scoring with time weighting:**

Research from SimpleMem (arXiv 2601.02553) defines the affinity score as:

```
affinity(i, j) = β · cos(vᵢ, vⱼ) + (1-β) · e^(−λ·|tᵢ − tⱼ|)
```

Where:
- `β` = `recency_bias` parameter [0.0, 1.0] — weight between pure semantic vs. temporal closeness
- `cos(vᵢ, vⱼ)` = cosine similarity from sqlite-vec distance query (convert distance to similarity)
- `λ` = decay constant (recommend `ln(2) / 7days` so affinity halves every week)
- `|tᵢ − tⱼ|` = absolute time difference in seconds

At `recency_bias = 0.0` (default), this reduces to pure cosine similarity — identical to v1.0 search behavior. Users who don't set `recency_bias` see no behavioral change.

**Threshold guidance (evidence-based):**
- 0.70: Discovery threshold — finds loosely related memories for review (AgentZero default)
- 0.85: Merge threshold — established in SimpleMem paper as cluster formation cutoff
- 0.90: High-confidence replace — used in AgentZero as "safe replacement" validation gate
- Recommend: default `similarity_threshold = 0.85`, document the three zones in API reference

**Cluster merge strategy (Tier 1, no LLM):**
Retain the memory with the earliest `created_at` within the cluster (the "original") and delete the rest, merging their tags into the survivor. This is conservative — oldest memory is most-established information. Alternative (retain newest) risks losing historical context.

**LLM summarization prompt pattern (Tier 2):**
Research from AgentZero's memory consolidation system shows this structure works in production:

```
System: You are a memory consolidator. Given a cluster of related memories,
produce a single consolidated memory that preserves all unique information.
Do not add information not present in the inputs. Be concise.

User: Cluster of {N} related memories from agent '{agent_id}':
[1] {content_1} (stored: {created_at_1})
[2] {content_2} (stored: {created_at_2})
...

Produce a single consolidated memory. Output only the memory text, no preamble.
```

The consolidated summary is stored as a new memory (new UUID, `created_at = now()`, merged tags), then all cluster members are deleted. This is the "replace with summary" strategy used by Mem0, AgentZero, and SimpleMem.

---

## Competitor Compaction Feature Analysis

| Feature | Mem0 | AgentZero | SimpleMem (research) | Mnemonic v1.1 |
|---------|------|-----------|----------------------|----------------|
| Trigger mechanism | Implicit on every write | Explicit call | Asynchronous background | Explicit API call (agent-triggered) |
| Dedup threshold | Not published | 0.70 (discovery), 0.90 (replace) | 0.85 | Configurable, default 0.85 |
| Time-based weighting | No | No | Yes (β decay formula) | Yes (recency_bias param) |
| LLM required | Yes (always) | Yes (always) | Yes (always) | No (Tier 1 is pure algorithmic) |
| Dry-run preview | No | No | No | Yes |
| Single-binary friendly | No (Python) | No (Python) | Research only | Yes (Rust, no new external deps) |
| Cluster strategy | LLM decides | Greedy + LLM | Affinity clustering | Greedy pairwise + optional LLM |
| Response stats | Partial | Metadata only | Not applicable | Full stats (before/after counts) |

---

## Sources

- [SimpleMem: Efficient Lifelong Memory for LLM Agents (arXiv 2601.02553)](https://arxiv.org/html/2601.02553v1) — Affinity formula with time weighting, 0.85 threshold for cluster formation. HIGH confidence.
- [AgentZero Memory Consolidation System (DeepWiki)](https://deepwiki.com/frdel/agent-zero/4.3-memory-consolidation-system) — 0.70 discovery threshold, 0.90 replace safety threshold, five consolidation strategies (SKIP/KEEP_SEPARATE/MERGE/REPLACE/UPDATE), LLM prompt structure. HIGH confidence.
- [Jason Liu: Two Experiments on Agent Compaction](https://jxnl.co/writing/2025/08/30/context-engineering-compaction/) — Compaction is an active research problem; field lacks empirical consensus on thresholds. LOW confidence on universality of any single threshold.
- [Factory.ai: Evaluating Context Compression](https://factory.ai/news/evaluating-compression) — Structure forces preservation; incremental merge outperforms full regeneration. MEDIUM confidence.
- [Supermemory: Infinitely Running Stateful Coding Agents](https://supermemory.ai/blog/infinitely-running-stateful-coding-agents/) — Preemptive compaction at 80% context usage; preserve negative constraints verbatim. MEDIUM confidence.
- [petal-clustering crate (crates.io)](https://crates.io/crates/petal-clustering) — Pure Rust DBSCAN/HDBSCAN available; not recommended at typical memory set sizes. HIGH confidence on existence.
- [hdbscan crate (crates.io)](https://crates.io/crates/hdbscan) — Pure Rust HDBSCAN available. HIGH confidence on existence.
- [NVIDIA SemDedup](https://docs.nvidia.com/nemo-framework/user-guide/25.07/datacuration/semdedup.html) — Semantic deduplication reduces dataset size 20-50%. MEDIUM confidence on applicability to agent memory.
- [Widemem: importance scoring, decay, conflict resolution (Hugging Face Forums)](https://discuss.huggingface.co/t/widemem-open-source-memory-layer-for-llms-with-importance-scoring-decay-and-conflict-resolution/174269) — Exponential decay with half-life parameterization; `recencyScore = exp(-decayRate * ageInHours)`. MEDIUM confidence.

---
*Feature research for: Mnemonic v1.1 memory summarization / compaction milestone*
*Researched: 2026-03-20*
