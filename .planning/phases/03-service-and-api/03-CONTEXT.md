# Phase 3: Service and API - Context

**Gathered:** 2026-03-19
**Status:** Ready for planning

<domain>
## Phase Boundary

A fully working HTTP API where agents can store, search, list, and delete memories, with namespacing by agent_id and session_id, returning correct JSON responses and HTTP status codes for all success and error cases. A MemoryService orchestrator coordinates database writes and embedding generation. No binary packaging, no README documentation, no authentication — those are Phase 4 or future milestones.

</domain>

<decisions>
## Implementation Decisions

### Search query design
- `GET /memories/search` with query parameters: `q` (required search text), `agent_id`, `session_id`, `tag`, `limit`, `after`, `before`
- Default top-K: 10 results, max 100 via `limit` parameter
- No distance threshold by default — return top-K regardless; optional `threshold` param for callers who want relevance cutoff
- Agent pre-filter: JOIN between `vec_memories` KNN results and `memories` table filtered by `agent_id` (sqlite-vec doesn't support WHERE clauses inside KNN queries — researcher must validate exact JOIN syntax)
- Over-fetch from KNN when agent_id filter is present (request more than `limit` from vec_memories, then filter down to `limit` after JOIN) to ensure enough results after filtering

### Response format
- Omit embedding vectors from all responses — large, not useful to callers
- `POST /memories` returns 201 Created with the full memory object including generated `id`, `created_at`, and `embedding_model`
- `GET /memories` returns `{ "memories": [...], "total": N }` with offset/limit pagination via query params (`offset` default 0, `limit` default 20, max 100)
- `GET /memories/search` returns `{ "memories": [...] }` where each memory includes a `distance` float field for relevance ranking (lower = more similar)
- `DELETE /memories/:id` returns 200 with the deleted memory object (confirms what was removed)
- Memory object shape: `{ id, content, agent_id, session_id, tags, embedding_model, created_at, updated_at }`
- Tags returned as a JSON array (matching storage format)

### Error responses
- JSON error body: `{ "error": "Human-readable message" }` — simple, no error codes in v1
- Status code mapping:
  - 201: successful create
  - 200: successful read/search/delete
  - 400: validation errors (missing required `content`, missing required `q` for search, invalid `limit`/`offset`)
  - 404: memory not found (DELETE or future GET by ID)
  - 500: internal errors (database failures, embedding failures)
- Embedding failures during POST (e.g., OpenAI API down, empty content) return 400 or 500 depending on cause
- axum `IntoResponse` implementation on a unified API error type for consistent error formatting

### Service layer architecture
- New `src/service.rs` module with `MemoryService` struct
- MemoryService holds `Arc<Connection>` and `Arc<dyn EmbeddingEngine>` (not the full AppState — just what it needs)
- Orchestration flow for POST: validate input → embed content → generate UUID v7 → insert into `memories` table + `vec_memories` virtual table in a single `conn.call()` closure → return created memory
- Thin axum handlers in `server.rs`: extract query/body params, call MemoryService method, format response
- MemoryService methods return `Result<T, MnemonicError>` — handlers convert to HTTP responses
- `server.rs` gains new routes: POST /memories, GET /memories, GET /memories/search, DELETE /memories/:id
- UUID v7 generation via `uuid` crate with `v7` feature (time-ordered, globally unique)

### Claude's Discretion
- Exact sqlite-vec KNN query syntax and JOIN pattern (research needed)
- Over-fetch multiplier for agent_id-filtered KNN queries
- Request/response serde struct naming and field ordering
- Whether to use axum extractors (Query, Json, Path) or manual extraction
- Test structure for API integration tests (in-memory DB vs temp file)
- Whether MemoryService methods are async or use conn.call internally

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project specifications
- `.planning/REQUIREMENTS.md` — Phase 3 covers API-01 through API-06 and AGNT-01 through AGNT-03; see §API and §Multi-Agent sections
- `.planning/ROADMAP.md` §Phase 3 — Success criteria (5 criteria covering CRUD, search, filtering, multi-agent isolation, error responses)
- `.planning/PROJECT.md` — Constraints (axum for HTTP, tokio-rusqlite for async DB, single-binary distribution)

### Prior phase context
- `.planning/phases/01-foundation/01-CONTEXT.md` — Schema decisions (UUID v7, tags as JSON array, column layout), module structure, error handling conventions
- `.planning/phases/02-embedding/02-CONTEXT.md` — EmbeddingEngine trait shape, 384-dimension vectors, input rejection policy, AppState structure

### Technical blockers from STATE.md
- sqlite-vec KNN query syntax with agent_id pre-filter join pattern needs validation (STATE.md §Blockers/Concerns) — researcher MUST investigate this before planning

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/server.rs:20-24` — AppState struct with db, config, embedding fields; `build_router()` function ready for new routes
- `src/server.rs:39-48` — `serve()` function binds TCP listener and starts axum
- `src/db.rs:27-67` — `open()` function with schema already created (memories table, vec_memories virtual table, indexes)
- `src/embedding.rs:9` — EmbeddingEngine trait with `async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>`
- `src/error.rs` — MnemonicError, DbError, EmbeddingError enums with thiserror (add API error variants)
- `src/config.rs:9-14` — Config struct with port, db_path, embedding_provider, openai_api_key

### Established Patterns
- thiserror for domain errors, anyhow for main.rs propagation
- Flat module structure: one file per domain (db.rs, config.rs, server.rs, error.rs, embedding.rs)
- Arc wrapping for shared state in AppState
- `conn.call(|c| { ... })` pattern for all database operations (tokio-rusqlite async closure)
- tracing::info for structured logging

### Integration Points
- `src/server.rs:27-31` — `build_router()`: add POST/GET/DELETE routes here
- `src/server.rs:20-24` — AppState: MemoryService can be constructed from these fields
- `src/lib.rs` — Add `pub mod service` re-export
- `src/error.rs` — Add API-specific error variants or a new ApiError enum
- `src/main.rs:65-69` — AppState construction: MemoryService can be built here and added to state, or constructed from state in handlers

</code_context>

<specifics>
## Specific Ideas

No specific requirements — auto mode selected recommended defaults across all areas. Prior phases indicate a preference for idiomatic Rust conventions, standard REST patterns, and ecosystem-standard tools.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 03-service-and-api*
*Context gathered: 2026-03-19*
