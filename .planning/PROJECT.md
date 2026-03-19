# Mnemonic

## What This Is

A single Rust binary that gives any AI agent persistent memory via a simple REST API. Zero external dependencies — download and run. Designed to be the "Redis of agents" — lightweight, fast, and universally useful for any agent framework or language.

## Core Value

Any AI agent can store and semantically search memories out of the box with zero configuration — just download and run.

## Requirements

### Validated

- [x] Embedded SQLite with sqlite-vec for vector search in a single file — Validated in Phase 1: Foundation
- [x] Configuration via env vars or TOML file — Validated in Phase 1: Foundation
- [x] Bundled local embedding model (all-MiniLM-L6-v2 via candle) for zero-config inference — Validated in Phase 2: Embedding
- [x] Optional OpenAI API fallback for embeddings — Validated in Phase 2: Embedding

### Active

- [ ] Clean README with quickstart, API reference, and examples

### Validated (cont.)

- [x] REST API for storing, searching, filtering, and deleting memories — Validated in Phase 3: Service and API
- [x] Multi-agent support via agent_id namespacing — Validated in Phase 3: Service and API
- [x] Session-scoped retrieval via session_id grouping — Validated in Phase 3: Service and API

### Out of Scope

- Memory summarization / compaction — future milestone
- Authentication / API keys — future milestone
- Pluggable storage backends (Qdrant, Postgres, etc.) — future milestone
- Web UI / dashboard — future milestone
- gRPC support — future milestone

## Context

- Target users are AI agent developers who need persistent memory across sessions
- Must be a true single-binary distribution — no Python, no Docker, no external services
- Embedding model runs locally via candle (pure Rust) for zero-dependency inference
- sqlite-vec is the actively maintained SQLite vector extension (sqlite-vss is archived)
- tokio-rusqlite provides async SQLite access without blocking the async runtime
- axum is the HTTP framework for the REST API layer

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
| candle over ort for inference | Pure Rust, no ONNX Runtime dependency, enables true single-binary | Validated Phase 2 |
| sqlite-vec over sqlite-vss | sqlite-vss is archived, sqlite-vec is actively maintained | Validated Phase 1 |
| tokio-rusqlite for async SQLite | Avoids blocking async runtime under concurrent agent requests | Validated Phase 1 |
| axum for HTTP | Modern, ergonomic, good ecosystem support in Rust | Validated Phase 1 |
| rusqlite 0.37 (not 0.39) | sqlite-vec 0.1.7 has version conflict with rusqlite 0.39's libsqlite3-sys | Decided Phase 1 |
| all-MiniLM-L6-v2 as default model | Small (~22MB), fast inference, good semantic similarity quality | Validated Phase 2 |

---
## Current State

Phase 3 complete — Full REST API operational: POST /memories (201), GET /memories/search (KNN with agent_id pre-filter via CTE over-fetch + JOIN), GET /memories (offset/limit pagination), DELETE /memories/:id, GET /health. MemoryService orchestrates embedding + dual-table transactional writes. ApiError with IntoResponse for consistent JSON error formatting. UUID v7 for time-ordered IDs. 21 tests passing (11 new API integration tests via MockEmbeddingEngine + tower oneshot). Next: Phase 4 (Distribution).

---
*Last updated: 2026-03-19 after Phase 3 completion*
