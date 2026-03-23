# Mnemonic

## What This Is

A single Rust binary that gives any AI agent persistent memory via REST API and gRPC — including agent-triggered memory compaction with optional LLM summarization, API key authentication, pluggable storage backends (SQLite, Qdrant, Postgres), terminal-native subcommands for every operation, and an embedded operational dashboard for visual memory exploration and compaction management. Zero external dependencies — download and run. The "Redis of agents" — lightweight, fast, and universally useful for any agent framework or language.

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
- `mnemonic serve` subcommand with backward-compatible bare invocation — v1.3
- `mnemonic recall` subcommand with list, get-by-id, and filter flags (DB-only, no model loading) — v1.3
- `mnemonic remember` subcommand with positional/stdin content, agent/session/tag metadata — v1.3
- `mnemonic search` subcommand with semantic search, ranked results, and filtering flags — v1.3
- `mnemonic compact` subcommand with dry-run, agent scoping, and threshold control — v1.3
- Global `--json` flag for machine-readable output across all subcommands — v1.3
- Consistent exit codes (0 success, 1 error) and stdout/stderr separation — v1.3
- StorageBackend async trait with store, get_by_id, list, search, delete, fetch_candidates, write_compaction_result — v1.4
- SqliteBackend wraps existing SQLite+sqlite-vec code with zero behavior change — v1.4
- MemoryService and CompactionService hold Arc<dyn StorageBackend> instead of direct connections — v1.4
- CompactionService dual-connection design (backend + audit_db) for cross-backend audit logging — v1.4
- Config struct extended with storage_provider, qdrant_url, qdrant_api_key, postgres_url fields — v1.4
- create_backend() factory function returns correct StorageBackend based on config — v1.4
- `mnemonic config show` CLI subcommand with human-readable and JSON output, all secrets redacted — v1.4
- GET /health reports active storage backend name — v1.4
- QdrantBackend implements all 7 StorageBackend methods using qdrant-client gRPC, feature-gated behind backend-qdrant — v1.4
- Qdrant cosine score normalized to lower-is-better distance via 1.0 - score — v1.4
- Compaction on Qdrant uses upsert-first-then-delete with documented non-transactional semantics — v1.4
- Multi-agent namespace isolation via Qdrant payload filtering on agent_id — v1.4
- PostgresBackend implements all 7 StorageBackend methods using sqlx + pgvector, feature-gated behind backend-postgres — v1.4
- pgvector cosine distance via `<=>` operator with HNSW index for search — v1.4
- Postgres transactions for atomic compaction (BEGIN/INSERT/DELETE/COMMIT) — v1.4
- Multi-agent namespace isolation via SQL WHERE agent_id filtering — v1.4
- MnemonicService proto contract (4 RPCs) with feature-gated tonic 0.13 / prost 0.13 build pipeline — v1.5
- Dual-port REST+gRPC startup via tokio::try_join! with configurable grpc_port (default 50051) — v1.5
- Async Tower auth layer for gRPC with open-mode bypass, bearer token validation, scope enforcement — v1.5
- StoreMemory, SearchMemories, ListMemories, DeleteMemory gRPC handlers with status code mapping — v1.5
- tonic-health standard health service and tonic-reflection for grpcurl discoverability — v1.5
- CI release workflow updated with protoc installation for all build targets — v1.5
- Recall CLI routed through StorageBackend trait (all backends) instead of raw SQLite — v1.5

- Dashboard Cargo feature gate with rust-embed + axum-embed serving embedded SPA at /ui — v1.6
- Preact + TypeScript + Tailwind v4 + Vite frontend compiled to single-file dist/index.html — v1.6
- CI release workflow produces dual artifacts (slim + dashboard) with regression gate — v1.6
- StorageBackend::stats() returns per-agent memory counts and last-active timestamps across all 3 backends — v1.6
- GET /stats endpoint behind auth middleware with scope-aware filtering for scoped API keys — v1.6
- GET /health includes auth_enabled boolean field for frontend auth detection — v1.6
- CSP header middleware on all /ui/ responses (default-src 'self'; script-src 'unsafe-inline'; style-src 'unsafe-inline') — v1.6
- Dashboard SPA with auth gate (auth_enabled detection), hash routing, 4 tabs (Memories/Agents/Search/Compact) — v1.6
- Paginated memory table with filter controls (agent from /stats, session/tag from response), expandable rows — v1.6
- Semantic search tab with clamped distance bars and per-agent breakdown table — v1.6
- GET /memories/{id} endpoint with auth scope enforcement for single-memory fetch — v1.6
- Compact tab with two-step dry-run flow: agent selector, threshold input, cluster tree preview, confirm/discard controls — v1.6
- Typed API client wrappers (compactMemories, fetchMemoryById) with CompactParams/CompactResponse/ClusterMapping types — v1.6

### Active

(No active milestone — planning next)

### Out of Scope

- Hierarchical summaries (parent-child relationships, traversal) — cluster-and-replace covers 90% of use cases
- Automatic background compaction — agent stays in control, no silent data mutation
- gRPC support for compaction/keys — hot-path only; compaction and key management stay REST-only to limit interface surface
- Memory decay / TTL — surprising behavior that silently loses data
- Multi-node / distributed mode — SQLite not designed for multi-writer distributed use
- Session-scoped compaction — agent_id scoping sufficient; session_id adds complexity
- DBSCAN/HDBSCAN clustering — overkill for N<500; greedy pairwise with single threshold is simpler and sufficient
- Interactive REPL mode — model cold start makes REPL startup same as individual invocations; server already IS the persistent process
- Background daemon mode (--daemon) — platform-specific complexity; systemd/launchd handle this better
- Multi-format output (--format csv/table/json) — --json + jq covers all machine formats; two modes (human/JSON) not three
- Automatic model download — model is bundled in binary; clear error better than silent download
- Cross-backend migration — all backends must be stable first; deferred to future milestone
- Auto-migration on config change — surprising behavior that silently moves data; explicit migration better
- Multi-backend fan-out (write to multiple) — complexity explosion; single active backend is simpler and sufficient
- Backend-specific query syntax — leaky abstraction; trait must normalize all operations
- Auth keys in remote backends — auth must stay in local SQLite; no network round-trip per request

## Context

v1.6 shipped with 32 phases across 7 milestones (60 plans total: 11 v1.0 + 6 v1.1 + 8 v1.2 + 11 v1.3 + 9 v1.4 + 7 v1.5 + 8 v1.6).
Tech stack: Rust, axum, SQLite+sqlite-vec, tokio-rusqlite, candle (all-MiniLM-L6-v2), reqwest (LLM HTTP), blake3 + constant_time_eq (auth), clap (CLI), serde_json (--json output), qdrant-client (optional), sqlx + pgvector (optional), tonic + prost (optional, interface-grpc), rust-embed + axum-embed (optional, dashboard), Preact + TypeScript + Tailwind v4 + Vite (dashboard frontend).
~7,500 lines of Rust + ~2,100 lines of TypeScript. 292+ tests passing, 1 ignored, zero compiler warnings. MIT licensed.
Dual-protocol server: REST (axum) on configurable port (default 8080) + gRPC (tonic) on configurable grpc_port (default 50051), started simultaneously via tokio::try_join!.
11 REST endpoints: POST/GET/DELETE /memories, GET /memories/{id}, GET /memories/search, POST /memories/compact, POST/GET /keys, DELETE /keys/{id}, GET /health, GET /stats.
4 gRPC RPCs: StoreMemory, SearchMemories, ListMemories, DeleteMemory — same semantics as REST, with tonic-health and tonic-reflection for discoverability.
CLI: 7 subcommands — `serve`, `remember`, `recall`, `search`, `compact`, `keys`, `config` — all with `--json` flag, consistent exit codes, stdout/stderr separation.
Pluggable storage via StorageBackend trait — SQLite (default), Qdrant, Postgres+pgvector. All backends behind feature flags.
Embedded dashboard at /ui behind `dashboard` feature flag — Preact SPA with auth gate, memory browsing/search, agent breakdown, and compaction panel.
Init tiers: DB-only (~50ms) for keys/recall/config, medium (DB+embedding, ~2-3s) for remember/search, full (DB+embedding+LLM) for compact.
Target users: AI agent developers who need persistent memory across sessions.
Single-binary distribution — no Python, no Docker, no external services required (SQLite default).

## Constraints

- **Language**: Rust — required for single-binary distribution and performance
- **Default Storage**: SQLite + sqlite-vec — zero-config default, everything in one file
- **Optional Storage**: Qdrant (gRPC, feature-gated) and Postgres+pgvector (SQL, feature-gated) — opt-in
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
| Tiered init helpers (init_db / init_db_and_embedding / init_compaction) | Minimal resource loading per subcommand; recall stays fast at ~50ms | Good — v1.3 |
| Early validation before model load | Empty content/query rejected before 2-3s embedding model load | Good — v1.3 |
| Direct rusqlite seeding for fast-path tests | Avoids embedding model overhead in recall/keys tests; seed_memory() pattern | Good — v1.3 |
| Global --json on Cli struct (not per-subcommand) | One extraction point in main.rs; consistent across all handlers | Good — v1.3 |
| stderr audit trail regardless of --json mode | Operators always see progress; scripts parse stdout only | Good — v1.3 |
| #[async_trait] for StorageBackend (not native async fn) | Native async fn in traits is not dyn-compatible as of early 2026 | Good — v1.4 |
| KeyService stays on direct Arc<Connection> | Auth must not route through a potentially remote StorageBackend | Good — v1.4 |
| StorageBackend distance contract is lower-is-better | Qdrant scores (higher-is-better) must convert via 1.0 - score; consistent for all backends | Good — v1.4 |
| Feature-gated backends (backend-qdrant, backend-postgres) | Default binary carries zero new dependencies; opt-in only | Good — v1.4 |
| CompactionService dual-connection design | backend for memory ops, audit_db for compact_runs — audit is SQLite-specific infrastructure | Good — v1.4 |
| Per-cluster write_compaction_result() atomicity | Replaces all-clusters-in-one-transaction; necessary for backend abstraction | Good — v1.4 |
| Feature gate errors at create_backend() not validate_config() | Keeps config portable across builds with different feature sets | Good — v1.4 |
| sqlx default-features=false | Prevents libsqlite3-sys version conflict between sqlx-sqlite and rusqlite 0.37 | Good — v1.4 |
| tonic 0.13 / prost 0.13 for interface-grpc | Compatible with qdrant-client prost ^0.13.3 anchor; tonic 0.14 would cause prost version conflict | Good — v1.5 |
| tonic-build non-optional in [build-dependencies] | Build scripts compile as standalone binaries; optional = true causes unresolved module errors on default builds; CARGO_FEATURE env var provides runtime gate | Good — v1.5 |
| arduino/setup-protoc@v3 unconditional in CI | Free in CI time cost; prevents cryptic missing-file errors when interface-grpc is enabled | Good — v1.5 |
| Dual-port via tokio::try_join! (not same-port multiplexing) | Documented body-type mismatch bugs (tonic #1964, axum #2825) make same-port unreliable | Good — v1.5 |
| Async Tower Layer for gRPC auth (not sync interceptor) | KeyService is async; block_on() inside tokio runtime panics | Good — v1.5 |
| Duplicate enforce_scope in grpc/mod.rs | Avoids importing axum-specific types into gRPC module; isolated concern | Good — v1.5 |
| init_recall() fast-path (no validate_config, no embedding) | Recall only needs DB + backend for list/get_by_id; keeps ~50ms startup | Good — v1.5 |
| postgres_url treated as secret (redacted in config show) | Credentials may be embedded in connection string; consistent with other secrets | Good — v1.4 |
| rust-embed 8.11 + axum-embed 0.1 for dashboard | Compile-time asset embedding; both optional deps behind dashboard feature | Good — v1.6 |
| vite-plugin-singlefile for single index.html | All JS+CSS inlined; base:/ui/ fallback if singlefile fails | Good — v1.6 |
| Hash routing (#/path) over history routing | Avoids SPA hard-reload 404s at zero cost | Good — v1.6 |
| Dashboard router merged at top level (not inside protected) | Prevents auth middleware from blocking asset loads | Good — v1.6 |

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
*Last updated: 2026-03-23 after v1.6 milestone*
