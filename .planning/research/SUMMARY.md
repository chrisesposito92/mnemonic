# Project Research Summary

**Project:** Mnemonic
**Domain:** Rust single-binary agent memory server (embedded vector search + local ML inference + REST API)
**Researched:** 2026-03-19
**Confidence:** HIGH

## Executive Summary

Mnemonic is a self-contained agent memory server that competes on a single differentiating axis: it is the only tool in its category that ships as a true single Rust binary with bundled local ML inference, no Python dependency, no external database, and no API key required to get started. Competitors (Mem0, Zep, Redis Agent Memory Server) all require Python runtimes, external services, or cloud accounts. The recommended approach is a layered Rust service — axum for HTTP, tokio-rusqlite + sqlite-vec for storage, and candle (HuggingFace's pure-Rust inference) for local embedding — wired together as a 4-layer architecture: HTTP handlers, a MemoryService orchestrator, a trait-based EmbeddingEngine, and a MemoryRepository over SQLite. Model weights are downloaded on first run via hf-hub and cached to `~/.cache/mnemonic/`; they are not bundled in the binary.

The feature surface for v1 is deliberately narrow: CRUD operations on memories (`POST`, `GET search`, `GET list`, `DELETE`), agent_id + session_id namespacing on every operation, a health endpoint, and the bundled all-MiniLM-L6-v2 model with an optional OpenAI fallback. Features that would destroy the "zero-config, single binary" story — web UI, auth, pluggable storage backends, memory summarization — are explicitly deferred to v2+. The schema must be designed correctly from day one: adding `agent_id`, `session_id`, `created_at`, and `embedding_model` columns retroactively requires a migration and re-embedding pass.

The top risks are all correctness traps that produce silent failures: wrong embedding pooling (CLS token instead of masked mean pooling) produces semantically useless vectors; storing un-normalized vectors then using inner-product distance gives wrong rankings; mixing embeddings from different models corrupts search results permanently. The second category of risk is concurrency: blocking rusqlite calls on the tokio thread pool will starve the executor under load, and naive multi-writer SQLite connection pools produce `SQLITE_BUSY` errors. All of these must be addressed in the foundation and embedding phases before any user-facing features are built.

## Key Findings

### Recommended Stack

The stack is fully determined by two constraints: single Rust binary and zero external dependencies. These constraints rule out ONNX Runtime (system C library), sqlx (cannot load sqlite-vec extension), diesel (same), actix-rt or async-std (wrong runtime for the ecosystem), and fastembed-rs (default ONNX backend). The resulting stack has no ambiguity: tokio + axum for async HTTP, rusqlite + tokio-rusqlite + sqlite-vec for storage, candle + tokenizers + hf-hub for inference. All versions are confirmed against official sources as of 2026-03-19.

**Core technologies:**
- tokio 1.50.0: async runtime — required by axum, tokio-rusqlite, and the entire ecosystem; no real alternative
- axum 0.8.8: HTTP layer — tokio-rs maintained, Tower-native, best DX in Rust HTTP today
- rusqlite 0.38.0 (bundled feature): SQLite access — `bundled` feature compiles SQLite 3.51.1 into the binary; required because sqlite-vec needs `load_extension` which sqlx does not expose
- tokio-rusqlite 0.7.0: async SQLite wrapper — prevents blocking rusqlite calls from starving tokio worker threads; uses mpsc/oneshot actor pattern internally
- sqlite-vec 0.1.7: vector search extension — Mozilla Builders-maintained, actively developed (last release March 17 2026), replaces archived sqlite-vss; pure C, zero external dependencies
- candle-core + candle-nn + candle-transformers 0.9.2: pure-Rust ML inference — the only way to bundle a neural network in a Rust binary without a C runtime dependency; HuggingFace-maintained
- tokenizers 0.22.2: HuggingFace tokenization — required alongside candle for sentence-transformer models
- hf-hub 0.3.x: model download and caching — downloads safetensors weights on first run to `~/.cache/huggingface/`; keeps model out of the binary
- thiserror 2.x: error types — structured errors that map to HTTP status codes; anyhow loses this structure and is wrong for REST APIs

**Critical version constraints:**
- All candle subcrates (core, nn, transformers) must be identical version 0.9.2; mismatches cause linker errors
- axum 0.8.x requires tower-http 0.6.x; keep them aligned
- tokio-rusqlite 0.7 is compatible with rusqlite 0.38; verify if upgrading either

### Expected Features

The feature set is well-documented against multiple competitor reference implementations. The MVP is narrow and unambiguous.

**Must have (table stakes):**
- `POST /memories` — store content + metadata (agent_id, session_id, arbitrary tags)
- `GET /memories/search` — semantic search with agent_id/session_id filters and limit
- `GET /memories` — list memories with filter params
- `DELETE /memories/{id}` — delete a single memory
- `GET /health` — liveness check; required for any process manager or container orchestration
- agent_id + session_id namespacing on all operations — must be in schema from day one
- Persistence across restarts — SQLite file on disk covers this
- Plain JSON API — no SDK required

**Should have (competitive differentiators):**
- Bundled local embedding model (all-MiniLM-L6-v2 via candle) — the defining differentiator; no competitor ships this; makes zero-config possible
- Single Rust binary — enables `cargo install` and GitHub release distribution
- SQLite single-file storage — trivial to back up, inspect, or wipe
- Zero-config startup with sensible defaults
- Optional OpenAI embedding fallback (env var, no code change required)
- Metadata filtering on search (agent_id + session_id + time range)

**Defer (v2+):**
- Memory summarization / compaction — requires LLM call per write; adds latency and API key dependency
- Authentication / API keys — premature for local tool; run behind a reverse proxy
- Web UI / dashboard — violates single-binary simplicity story
- gRPC interface — REST is sufficient; double the interface surface is not worth it
- Pluggable storage backends — the single-file story is a feature, not a limitation
- Memory decay / TTL expiration — surprising silent data loss; let users delete explicitly

### Architecture Approach

The architecture is a clean 4-layer design with strict separation of concerns. HTTP handlers are thin wrappers that call the service layer; the MemoryService is the only place with business logic; the EmbeddingEngine is a trait that abstracts local vs. OpenAI providers; the MemoryRepository owns all SQL. Shared state flows via `Arc<AppState>` attached to the axum Router. The component build order matters: models and error types first, then DB and embedding layers in parallel, then the service layer that integrates them, then the HTTP layer on top.

**Major components:**
1. HTTP Layer (axum Router + handlers in `api/`) — parse/route requests, serialize responses; no business logic; thin wrappers over service calls
2. AppState (`Arc<AppState>`) — holds `Arc<MemoryService>` and `Arc<Config>`; shared across all handler invocations via axum `.with_state()`
3. MemoryService (`service/memory.rs`) — orchestrates store/search/delete flows; calls EmbeddingEngine then MemoryRepository; only place with business rules
4. EmbeddingEngine trait (`embedding/`) — `async fn embed(&self, text: &str) -> Vec<f32>`; `LocalEngine` (candle BERT) and `OpenAiEngine` are concrete impls; provider selected at startup from config
5. MemoryRepository (`db/memory.rs`) — all SQL via tokio-rusqlite `.call()` closures; insert, knn_search with agent_id pre-filter, delete
6. SQLite storage — two tables: `memories` (metadata) and `vec_memories` (vec0 virtual table with embeddings); joined at query time

### Critical Pitfalls

1. **Wrong embedding pooling (silent semantic failure)** — all-MiniLM-L6-v2 requires mean pooling with attention mask weighting, not CLS token extraction and not simple average. Validate against Python sentence-transformers output for known sentence pairs before shipping. Cosine similarity for related pairs should be >0.85.

2. **Mutex held across `.await` (deadlock under concurrent load)** — `std::sync::Mutex` guard held across an await point makes the future `!Send` and causes deadlocks under concurrent traffic. Use tokio-rusqlite's actor pattern (`.call()` closures) for all SQLite state; use `tokio::sync::Mutex` only when async-held guards are unavoidable.

3. **SQLite multi-connection write pool (`SQLITE_BUSY` errors)** — SQLite serializes writes; multiple concurrent writers cause `SQLITE_BUSY` and up to 20x throughput degradation. Use a single write connection backed by a tokio channel queue, plus a separate read pool. Enable WAL mode (`PRAGMA journal_mode = WAL`) so readers never block writers.

4. **Embedding model mismatch after provider change (silent wrong results)** — switching between candle and OpenAI providers mixes vectors from different spaces; similarity scores become meaningless. Store `embedding_model VARCHAR NOT NULL` in the memories schema from day one; warn on startup if configured provider differs from stored embeddings.

5. **sqlite-vec brute-force scale ceiling** — sqlite-vec has no ANN index (tracked, not yet implemented); at ~100K total vectors KNN latency reaches ~50ms, at 1M vectors ~500ms. Mitigation: always pre-filter by `agent_id` to reduce the scanned vector set; document the scale limit; design schema with this filter from the start.

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: Foundation
**Rationale:** Domain models, error types, configuration, and the SQLite connection setup are dependencies for every other component. Getting the DB layer right here (WAL mode, single write connection, sqlite-vec extension registration, correct schema with all required columns) prevents the most expensive retrofit work. Architecture research explicitly states the build order starts here.
**Delivers:** Compiling project skeleton; typed `Config`; `AppError` with `IntoResponse`; working SQLite connection with WAL mode, sqlite-vec loaded, and initial schema applied on startup; `memories` table with `agent_id`, `session_id`, `embedding_model`, `created_at` columns
**Addresses:** agent_id + session_id namespacing (schema), persistence (SQLite file), zero-config startup (defaults)
**Avoids:** Missing `embedding_model` column (pitfall 7), no WAL mode (pitfall 2), sqlite-vec extension not registered (integration gotcha)

### Phase 2: Embedding Pipeline
**Rationale:** The embedding layer must be built and validated in isolation before the service layer integrates it. Wrong pooling is a silent failure — it must be caught here with explicit validation against Python sentence-transformers before any search features are built on top. The trait abstraction must be established here so the OpenAI fallback can be added without refactoring the service.
**Delivers:** `EmbeddingEngine` trait; `LocalEngine` (candle BERT with correct masked mean pooling + L2 normalization); model loaded once at startup via hf-hub; startup progress message for model download; `OpenAiEngine` stub or full implementation; validation test against known sentence pairs
**Addresses:** Bundled local embedding model (core differentiator), optional OpenAI fallback, zero-config startup (model auto-download)
**Avoids:** Wrong embedding pooling (pitfall 3), missing L2 normalization (pitfall 4), model loaded per request (performance trap), binary bloat from `include_bytes!` (pitfall 5)

### Phase 3: Storage Layer
**Rationale:** With the schema in place (Phase 1) and embedding vectors available (Phase 2), the repository layer can be built and tested with real vectors. This is the phase where KNN query patterns with `agent_id` pre-filtering are implemented and verified. The single-writer connection pattern must be in place before any write throughput testing.
**Delivers:** `MemoryRepository` with insert, knn_search (with `agent_id` pre-filter), list/filter, and delete operations; all SQL via tokio-rusqlite `.call()` closures; integration tests confirming correct KNN results; write serialization confirmed with concurrent test
**Addresses:** SQLite single-file storage, metadata filtering, namespace isolation
**Avoids:** Unscoped vector queries (anti-pattern 3), blocking rusqlite in tokio runtime (anti-pattern 1), multi-connection write pool (pitfall 2)

### Phase 4: Service and API
**Rationale:** The service layer can only be built once both the embedding and storage layers have working implementations. This is the integration point per the architecture research build order. HTTP handlers are deliberately thin — all logic stays in MemoryService.
**Delivers:** `MemoryService` (store, search, delete); axum Router with all v1 endpoints (`POST /memories`, `GET /memories/search`, `GET /memories`, `DELETE /memories/{id}`, `GET /health`); `AppState` wiring; structured JSON error responses; input validation (max content length, required fields)
**Addresses:** All table-stakes features, health endpoint, plain JSON API, structured error codes
**Avoids:** No input length limits (security mistake), generic 500 errors for embedding failures (UX pitfall), `DELETE` returning 200 for missing IDs (UX pitfall)

### Phase 5: Distribution and Hardening
**Rationale:** A working server that cannot be distributed or run reliably in production is not a product. This phase turns the working binary into a shippable artifact with proper documentation, tested distribution paths, and the operational characteristics that make it credible for production agent use.
**Delivers:** cargo-dist GitHub release artifacts (macOS arm64/x86_64, Linux musl); `RUSTFLAGS="-C target-cpu=native"` guidance for performance; startup self-check (sqlite-vec version, WAL mode confirmed, model loaded); rate limiting on search endpoint; README with quickstart in under 3 commands, full API reference, curl + Python + LangChain examples; documented scale limits
**Addresses:** Single Rust binary distribution, Unix-friendly output, zero-config startup documentation
**Avoids:** No rate limiting on `/search` (security mistake), silent model download (UX pitfall), no `total_count` in search responses (UX pitfall)

### Phase Ordering Rationale

- Foundation before everything because `agent_id`, `embedding_model`, and WAL mode must be in the schema from day one; retrofitting these requires a migration and full re-embedding pass
- Embedding before storage because the repository tests need real embedding vectors to verify KNN correctness; validating pooling in isolation (Phase 2) catches the most expensive silent failure before it can contaminate stored data
- Storage before service because the service layer calls both; building service against stubs delays the discovery of integration bugs
- Service and API together because axum handlers are thin wrappers; they do not justify a separate phase
- Distribution last because it requires a working binary but does not block feature development; hardening work (rate limiting, self-checks) is included here rather than added as an afterthought

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 2 (Embedding Pipeline):** The candle BERT + masked mean pooling + L2 normalization implementation is the highest-risk code in the project. Research the exact candle API for batch embedding, attention mask shapes, and the sentence-transformers pooling algorithm before writing production code. The community article cited in ARCHITECTURE.md (MEDIUM confidence) should be cross-referenced against the candle BERT example in the official repo.
- **Phase 3 (Storage Layer):** sqlite-vec's Rust API for vec0 virtual tables and KNN query syntax (MATCH + distance ORDER BY) should be verified against the official sqlite-vec Rust integration guide and demo.rs example. The pre-filter join pattern with `agent_id` is not explicitly documented in sqlite-vec — validate the query plan.

Phases with standard patterns (skip research-phase):
- **Phase 1 (Foundation):** axum, rusqlite, config crate, and thiserror are well-documented with standard patterns. No research needed.
- **Phase 4 (Service and API):** axum handler patterns and `Arc<AppState>` wiring are standard. No research needed.
- **Phase 5 (Distribution):** cargo-dist is well-documented. musl static builds are standard. No research needed.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All versions verified against official crates.io, docs.rs, and GitHub releases as of 2026-03-19. One MEDIUM caveat: uuid v1.22.0 confirmed via crates.io (requires JS to verify) |
| Features | HIGH | Cross-referenced against 4 competitor implementations (Mem0, Zep, Redis Agent Memory Server, Hindsight) plus 3 ecosystem survey sources. Feature table stakes and differentiators are consistent across all sources |
| Architecture | HIGH | Component boundaries and patterns sourced from official axum, tokio-rusqlite, sqlite-vec, and candle docs. Community articles cross-checked against official sources. Build order is internally consistent with dependency graph |
| Pitfalls | HIGH | Critical pitfalls sourced from official bug trackers (candle #2418, sqlite-vec #25), SQLite official WAL docs, tokio shared state tutorial, and a verified benchmark (20x throughput claim). Pooling and normalization pitfalls sourced from Milvus AI reference docs |

**Overall confidence:** HIGH

### Gaps to Address

- **sqlite-vec ANN index availability:** sqlite-vec's ANN index is tracked in issue #25 but not yet implemented as of the research date. Check the sqlite-vec release notes before the Phase 3 planning session — if ANN has shipped, the scale ceiling concern changes significantly.
- **candle BERT batch embedding API:** The architecture research assumes batch embedding is supported, but the exact API (tensor shapes for batched tokenizer output + attention mask) is not spelled out in the research files. Validate during Phase 2 planning before writing embedding code.
- **all-MiniLM-L6-v2 vs. nomic-embed-text-v1.5 model choice:** STACK.md flags that all-MiniLM-L6-v2 is aging (512 token limit, lower MTEB scores in 2025 benchmarks) and recommends nomic-embed-text-v1.5 or BGE-small-en-v1.5 as upgrades. The model identifier should be in config so users can swap it — but the initial default model choice should be validated against the target use case (short agent memory snippets vs. long documents) before committing it to the schema.
- **OpenAI text-embedding-3-small token limit handling:** PITFALLS.md notes a hard 8191 token input limit for the OpenAI API. The chunking strategy for inputs that exceed this limit is not designed. Needs a decision during Phase 4 (reject with 400, or silently truncate, or chunk-and-average).

## Sources

### Primary (HIGH confidence)
- [axum docs.rs 0.8.8](https://docs.rs/axum/latest/axum/) — handler patterns, State extractor, Router
- [tokio GitHub releases](https://github.com/tokio-rs/tokio/releases) — v1.50.0 confirmed
- [sqlite-vec GitHub releases](https://github.com/asg017/sqlite-vec/releases) — v0.1.7 confirmed (March 17 2026)
- [sqlite-vec Rust integration guide](https://alexgarcia.xyz/sqlite-vec/rust.html) — extension loading, KNN query patterns
- [sqlite-vec demo.rs](https://github.com/asg017/sqlite-vec/blob/main/examples/simple-rust/demo.rs) — official Rust example
- [candle GitHub](https://github.com/huggingface/candle/blob/main/Cargo.toml) — v0.9.2 confirmed, BERT implementation
- [tokenizers docs.rs](https://docs.rs/tokenizers/latest/tokenizers/) — v0.22.2 confirmed
- [tokio-rusqlite docs.rs](https://docs.rs/tokio-rusqlite/latest/tokio_rusqlite/) — v0.7.0, actor pattern
- [rusqlite docs.rs](https://docs.rs/crate/rusqlite/latest) — v0.38.0, bundled SQLite 3.51.1
- [tower-http docs.rs](https://docs.rs/tower-http/latest/tower_http/) — v0.6.8 confirmed
- [config docs.rs](https://docs.rs/config/latest/config/) — v0.15.22 confirmed
- [SQLite WAL mode official docs](https://sqlite.org/wal.html) — concurrent reader/single writer model
- [Tokio shared state docs](https://tokio.rs/tokio/tutorial/shared-state) — Mutex-across-await deadlock mechanics
- [sqlite-vec stable release blog](https://alexgarcia.xyz/blog/2024/sqlite-vec-stable-release/index.html) — brute-force scale benchmarks
- [ANN index tracking — sqlite-vec #25](https://github.com/asg017/sqlite-vec/issues/25) — ANN not yet implemented
- [candle issue #2418](https://github.com/huggingface/candle/issues/2418) — GELU performance root cause

### Secondary (MEDIUM confidence)
- [Mem0 GitHub](https://github.com/mem0ai/mem0) — competitor feature set
- [Zep Agent Memory Product Page](https://www.getzep.com/product/agent-memory/) — competitor features and positioning
- [Redis Agent Memory Server GitHub](https://github.com/redis/agent-memory-server) — competitor API design and namespace patterns
- [Milvus FAQ: sentence transformer mistakes](https://milvus.io/ai-quick-reference/what-are-common-mistakes-that-could-lead-to-poor-results-when-using-sentence-transformer-embeddings-for-semantic-similarity-tasks) — pooling and normalization pitfalls
- [SQLite write pool benchmark — Evan Schwartz](https://emschwartz.me/psa-your-sqlite-connection-pool-might-be-ruining-your-write-performance/) — 20x throughput difference
- [Building Sentence Transformers in Rust with Candle](https://dev.to/mayu2008/building-sentence-transformers-in-rust-a-practical-guide-with-burn-onnx-runtime-and-candle-281k) — candle BERT pooling patterns
- [include_bytes! compile time issue — rust-lang #65818](https://github.com/rust-lang/rust/issues/65818) — large blob compile time cost
- [Mem0 Research Paper (arXiv 2504.19413)](https://arxiv.org/abs/2504.19413) — memory architecture analysis
- [Zep Temporal KG Architecture (arXiv 2501.13956)](https://arxiv.org/abs/2501.13956) — Zep feature set

### Tertiary (MEDIUM-LOW confidence)
- [HN: Don't use all-MiniLM-L6-v2](https://news.ycombinator.com/item?id=46081800) — model aging context; community discussion, useful directional signal
- [5 AI Agent Memory Systems Compared (DEV Community)](https://dev.to/varun_pratapbhardwaj_b13/5-ai-agent-memory-systems-compared-mem0-zep-letta-supermemory-superlocalmemory-2026-benchmark-59p3) — benchmark data and differentiation analysis

---
*Research completed: 2026-03-19*
*Ready for roadmap: yes*
