# Mnemonic

## What This Is

A single Rust binary that gives any AI agent persistent memory via a simple REST API. Zero external dependencies — download and run. The "Redis of agents" — lightweight, fast, and universally useful for any agent framework or language.

## Core Value

Any AI agent can store and semantically search memories out of the box with zero configuration — just download and run.

## Requirements

### Validated

- Embedded SQLite with sqlite-vec for vector search in a single file — v1.0
- Configuration via env vars or TOML file with validate_config() startup checks — v1.0
- Bundled local embedding model (all-MiniLM-L6-v2 via candle) for zero-config inference — v1.0
- Optional OpenAI API fallback for embeddings (embedding_provider config-driven) — v1.0
- REST API for storing, searching, filtering, and deleting memories (5 endpoints) — v1.0
- Multi-agent support via agent_id namespacing with KNN pre-filtering — v1.0
- Session-scoped retrieval via session_id grouping — v1.0
- Comprehensive README with quickstart, API reference, curl/Python/agent examples — v1.0
- GitHub Actions cross-platform release workflow (linux-x86_64, macos-x86_64, macos-aarch64) — v1.0

### Active

- [ ] Agent-triggered memory compaction via POST /memories/compact endpoint
- [x] Algorithmic deduplication via vector similarity clustering (no LLM required) — Validated in Phase 8: Compaction Core
- [x] Metadata merge for deduplicated memory clusters — Validated in Phase 8: Compaction Core
- [x] LLM-powered summarization of similar memory clusters (opt-in, requires LLM config) — Validated in Phase 7: Summarization Engine
- [x] Configurable LLM provider following existing embedding_provider pattern — Validated in Phase 6: Foundation
- [ ] Time-based weighting parameter for age-aware compaction aggressiveness

### Out of Scope

- Hierarchical summaries (parent-child relationships, traversal) — too complex for v1.1, cluster-and-replace covers 90% of use cases
- Automatic background compaction — agent stays in control, no silent data mutation
- Authentication / API keys — premature for embeddable local tool; run behind reverse proxy
- Pluggable storage backends (Qdrant, Postgres) — single-file SQLite is a feature, not a limitation
- Web UI / dashboard — adds frontend build pipeline, violates single-binary simplicity
- gRPC support — doubles interface surface; REST sufficient for all reviewed use cases
- Memory decay / TTL — surprising behavior that silently loses data
- Multi-node / distributed mode — SQLite not designed for multi-writer distributed use

## Context

Shipped v1.0 with 1,932 lines of Rust code across 5 phases (11 plans).
Phase 6 complete — LLM config fields, schema migrations (source_ids, compact_runs), LlmError types added.
Phase 7 complete — SummarizationEngine trait, OpenAiSummarizer (prompt-injection-resistant via XML delimiters), MockSummarizer, wired in main.rs.
Phase 8 complete — CompactionService with greedy pairwise clustering, metadata merge, atomic SQLite transactions, dry_run mode, Tier 1/Tier 2 content synthesis with LLM fallback, compact_runs audit logging.
Tech stack: Rust, axum, SQLite+sqlite-vec, tokio-rusqlite, candle (all-MiniLM-L6-v2).
35 unit tests + 29 integration tests passing, zero compiler warnings. MIT licensed.
Target users: AI agent developers who need persistent memory across sessions.
Single-binary distribution — no Python, no Docker, no external services required.

## Constraints

- **Language**: Rust — required for single-binary distribution and performance
- **Storage**: SQLite + sqlite-vec — no external databases, everything in one file
- **Async DB**: tokio-rusqlite — async wrapper to avoid blocking tokio runtime
- **Embeddings**: candle — pure Rust inference, not ort (which requires ONNX Runtime)
- **HTTP**: axum — modern, ergonomic Rust HTTP framework
- **Model**: all-MiniLM-L6-v2 — small, fast, good quality for semantic search

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| candle over ort for inference | Pure Rust, no ONNX Runtime dependency, enables true single-binary | Good — v1.0 |
| sqlite-vec over sqlite-vss | sqlite-vss is archived, sqlite-vec is actively maintained | Good — v1.0 |
| tokio-rusqlite for async SQLite | Avoids blocking async runtime under concurrent agent requests | Good — v1.0 |
| axum for HTTP | Modern, ergonomic, good ecosystem support in Rust | Good — v1.0 |
| rusqlite 0.37 (not 0.39) | sqlite-vec 0.1.7 has version conflict with rusqlite 0.39's libsqlite3-sys | Revisit — compatible but version pinned |
| all-MiniLM-L6-v2 as default model | Small (~22MB), fast inference, good semantic similarity quality | Good — v1.0 |
| Arc<Mutex<LocalEngineInner>> for model sharing | BertModel+Tokenizer not Send+Sync; single mutex serializes inference | Good — v1.0 |
| zerocopy::IntoBytes for sqlite-vec MATCH | Converts Vec<f32> to raw bytes for vector parameter binding | Good — v1.0 |
| CTE over-fetch 10x for filtered KNN search | agent_id/session_id filter applied post-KNN; over-fetch ensures enough results | Good — v1.0 |
| validate_config() at startup | Gates startup on valid provider+key combinations; prevents silent surprises | Good — v1.0 |
| MockEmbeddingEngine with deterministic hash vectors | Reproducible API integration tests without 90MB model download | Good — v1.0 |

---

## Current Milestone: v1.1 Memory Summarization / Compaction

**Goal:** Add agent-triggered memory compaction with algorithmic dedup baseline and optional LLM-powered summarization.

**Target features:**
- POST /memories/compact endpoint (agent-triggered, no background magic)
- Tier 1: Vector similarity deduplication + metadata merge (works for everyone)
- Tier 2: LLM cluster-and-consolidate summarization (opt-in when LLM configured)
- Time-based weighting parameter for age-aware compaction

---
*Last updated: 2026-03-20 after Phase 8 (Compaction Core) completed*
