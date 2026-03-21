# Mnemonic

## What This Is

A single Rust binary that gives any AI agent persistent memory via a simple REST API — including agent-triggered memory compaction with optional LLM summarization. Zero external dependencies — download and run. The "Redis of agents" — lightweight, fast, and universally useful for any agent framework or language.

## Core Value

Any AI agent can store and semantically search memories out of the box with zero configuration — just download and run.

## Current Milestone: v1.3 CLI

**Goal:** Turn the single binary into a full CLI tool with subcommands for every operation — serve the API, store/recall/search memories, compact, and manage keys, all from the terminal.

**Target features:**
- `mnemonic serve` — start the HTTP server (current default behavior)
- `mnemonic remember` — store a memory directly from CLI
- `mnemonic recall` — retrieve memories by ID or filter
- `mnemonic search` — semantic search from CLI
- `mnemonic compact` — trigger compaction from CLI
- `mnemonic keys` — existing key management (already shipped)

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
- Agent-triggered memory compaction via POST /memories/compact endpoint — v1.1
- Algorithmic deduplication via vector similarity clustering (no LLM required) — v1.1
- Metadata merge for deduplicated memory clusters (tags union, earliest timestamp, combined content) — v1.1
- LLM-powered summarization of similar memory clusters (opt-in, requires LLM config) — v1.1
- Configurable LLM provider following existing embedding_provider pattern — v1.1
- Prompt-injection-resistant LLM summarization with structured delimiters — v1.1
- LLM fallback to algorithmic merge on failure — v1.1
- Atomic compaction writes (insert new + delete sources in single transaction) — v1.1
- Max candidates limit to prevent O(n^2) on large memory sets — v1.1
- Dry-run compaction preview (no data mutation) — v1.1
- Compaction response with old-to-new ID mapping for stale cache updates — v1.1
- Multi-agent namespace isolation during compaction — v1.1
- API key authentication via Authorization: Bearer mnk_... headers — v1.2
- API keys scoped to specific agent_ids for enforced namespace isolation — v1.2
- CLI key management commands (mnemonic keys create/list/revoke) — v1.2
- Optional auth — open mode by default, auth activates when keys exist — v1.2
- Axum auth middleware enforcement across all protected endpoints — v1.2

### Active

- `mnemonic compact` subcommand for triggering compaction from CLI — v1.3 (Phase 19, next)

### Recently Validated

- `mnemonic search` subcommand for semantic search from CLI with ranked results and filtering flags — Validated in Phase 18: search-subcommand
- `mnemonic serve` subcommand with backward-compatible bare invocation — v1.3 (Phase 15)
- `mnemonic recall` subcommand with list, get-by-id, and filter flags (DB-only, no model loading) — Validated in Phase 16: recall-subcommand
- `mnemonic remember` subcommand with positional/stdin content, agent/session/tag metadata, medium-init helper — Validated in Phase 17: remember-subcommand

### Out of Scope

- Hierarchical summaries (parent-child relationships, traversal) — too complex for v1.1, cluster-and-replace covers 90% of use cases
- Automatic background compaction — agent stays in control, no silent data mutation
- ~~Authentication / API keys~~ → **shipped in v1.2** (promoted from out-of-scope after compaction raised destructive-operation stakes)
- Pluggable storage backends (Qdrant, Postgres) — single-file SQLite is a feature, not a limitation
- Web UI / dashboard — adds frontend build pipeline, violates single-binary simplicity
- gRPC support — doubles interface surface; REST sufficient for all reviewed use cases
- Memory decay / TTL — surprising behavior that silently loses data
- Multi-node / distributed mode — SQLite not designed for multi-writer distributed use
- Session-scoped compaction — agent_id scoping sufficient; session_id adds complexity
- DBSCAN/HDBSCAN clustering — overkill for N<500; greedy pairwise with single threshold is simpler and sufficient

## Context

v1.2 shipped with 14 phases (22 plans total: 11 v1.0 + 6 v1.1 + 5 v1.2).
Tech stack: Rust, axum, SQLite+sqlite-vec, tokio-rusqlite, candle (all-MiniLM-L6-v2), reqwest (LLM HTTP), blake3 + constant_time_eq (auth), clap (CLI).
5,925 lines of Rust. 57 unit + 53 integration tests passing (194 total), zero compiler warnings. MIT licensed.
9 REST endpoints: POST/GET/DELETE /memories, GET /memories/search, POST /memories/compact, POST/GET /keys, DELETE /keys/{id}, GET /health.
CLI: `mnemonic serve` starts HTTP server (also default with no args), `mnemonic keys create/list/revoke` and `mnemonic recall` — fast path (DB only, no model loading). `mnemonic remember` and `mnemonic search` — medium-init path (DB + embedding, no server) for storing and searching memories from CLI.
Auth middleware enforces Bearer token authentication on all /memories and /keys endpoints with open mode bypass. Scoped keys enforce namespace isolation at the handler layer.
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
| reqwest for LLM HTTP (not async-openai) | async-openai conflicts with reqwest 0.13; raw HTTP simpler for single endpoint | Good — v1.1 |
| SummarizationEngine mirrors EmbeddingEngine trait pattern | Consistent architecture; MockSummarizer enables deterministic tests | Good — v1.1 |
| XML delimiters for prompt injection prevention | System message contains only instructions; user data wrapped in <memory> tags | Good — v1.1 |
| CompactionService as peer of MemoryService in AppState | No nested hierarchy; shares db_arc and embedding via Arc | Good — v1.1 |
| Greedy pairwise clustering (not DBSCAN) | Simple, predictable, O(n*max_candidates) with cap; sufficient for N<500 | Good — v1.1 |
| Cosine similarity = dot product (pre-normalized) | EmbeddingEngine guarantees L2 norm; avoids redundant normalization | Good — v1.1 |
| SQLite error-swallowing for idempotent migration | ALTER TABLE ADD COLUMN IF NOT EXISTS unsupported; catch extended_code==1 | Good — v1.1 |
| POST /memories/compact returns 200 (not 201) | Compaction mutates data but does not create a new addressable resource | Good — v1.1 |
| route_layer() not layer() for auth middleware | Prevents 401 on unmatched routes; only matched protected routes hit middleware | Good — v1.2 |
| Per-request COUNT for open mode (not startup flag) | Auth activates/deactivates live when keys are created/revoked — no restart needed | Good — v1.2 |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd:transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-03-21 after Phase 18 (search subcommand) complete*
