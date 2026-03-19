# Roadmap: Mnemonic

## Overview

Mnemonic is built in four phases that follow the dependency graph of the architecture: the database foundation and configuration must exist before embedding vectors can be stored, the embedding pipeline must be validated before search correctness can be verified, and the REST API integrates both layers before the binary is packaged and documented for distribution. Each phase delivers a complete, independently testable capability.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Foundation** - Project skeleton, SQLite+sqlite-vec storage, and configuration wired up (completed 2026-03-19)
- [ ] **Phase 2: Embedding** - Local all-MiniLM-L6-v2 model via candle with OpenAI fallback
- [ ] **Phase 3: Service and API** - MemoryService orchestrator + axum REST endpoints + multi-agent namespacing
- [ ] **Phase 4: Distribution** - Binary packaging, README, quickstart, and API reference

## Phase Details

### Phase 1: Foundation
**Goal**: A compiling Rust binary that initializes a SQLite database with the correct schema on startup, applies WAL mode, loads the sqlite-vec extension, and reads configuration from environment variables or a TOML file
**Depends on**: Nothing (first phase)
**Requirements**: STOR-01, STOR-02, STOR-03, STOR-04, CONF-01, CONF-02, CONF-03
**Success Criteria** (what must be TRUE):
  1. Running `./mnemonic` starts the server with zero arguments and prints a startup message confirming port, storage path, and embedding provider
  2. The SQLite file on disk contains a `memories` table with `agent_id`, `session_id`, `embedding_model`, and `created_at` columns after first run
  3. All database operations execute via tokio-rusqlite async closures — no blocking calls on the tokio thread pool
  4. Setting `MNEMONIC_PORT=9090` in the environment causes the server to bind to port 9090; an optional TOML file can override all settings
**Plans:** 3/3 plans complete

Plans:
- [ ] 01-01-PLAN.md — Project skeleton, error types, and layered configuration (CONF-01, CONF-02, CONF-03)
- [ ] 01-02-PLAN.md — Database module with sqlite-vec, axum server, and main.rs wiring (STOR-01, STOR-02, STOR-03, STOR-04)
- [ ] 01-03-PLAN.md — Integration tests and example config file (all requirements verified)

### Phase 2: Embedding
**Goal**: An `EmbeddingEngine` trait with a working `LocalEngine` (candle BERT, masked mean pooling, L2 normalization) that produces semantically valid vectors, loaded once at startup and shared across requests, with an optional `OpenAiEngine` fallback selectable via environment variable
**Depends on**: Phase 1
**Requirements**: EMBD-01, EMBD-02, EMBD-03, EMBD-04, EMBD-05
**Success Criteria** (what must be TRUE):
  1. The server starts without an OpenAI API key and produces embeddings using the bundled all-MiniLM-L6-v2 model downloaded on first run to `~/.cache/huggingface/`
  2. Cosine similarity between embeddings of semantically related sentences (e.g., "dog" and "puppy") is measurably higher than for unrelated sentences (e.g., "dog" and "database") — validates correct pooling and normalization
  3. Setting `OPENAI_API_KEY` causes the server to use OpenAI text-embedding-3-small instead of the local model; the code path is the same trait call
  4. The model is initialized once at startup and reused across all requests — not reloaded per call
**Plans**: TBD

### Phase 3: Service and API
**Goal**: A fully working HTTP API where agents can store, search, list, and delete memories, with namespacing by agent_id and session_id, returning correct JSON responses and HTTP status codes for all success and error cases
**Depends on**: Phase 2
**Requirements**: API-01, API-02, API-03, API-04, API-05, API-06, AGNT-01, AGNT-02, AGNT-03
**Success Criteria** (what must be TRUE):
  1. `POST /memories` with content, agent_id, session_id, and tags stores a memory and returns the assigned ID; the memory persists across server restarts
  2. `GET /memories/search?q=...&agent_id=foo` returns only memories belonging to agent "foo" ranked by semantic similarity — a different agent's memories do not appear
  3. `GET /memories` with filter parameters returns a filtered list; `DELETE /memories/:id` removes the specified memory and returns 404 for a subsequent request on that ID
  4. `GET /health` returns `{"status":"ok"}` with HTTP 200; all endpoints return structured JSON error bodies with appropriate HTTP status codes on failure
  5. Two agents storing memories with the same content but different agent_ids retrieve only their own memories when searching
**Plans**: TBD

### Phase 4: Distribution
**Goal**: A shippable binary artifact with documentation that enables any developer to go from download to first stored memory in under 3 commands, with a complete API reference covering every endpoint
**Depends on**: Phase 3
**Requirements**: DOCS-01, DOCS-02, DOCS-03
**Success Criteria** (what must be TRUE):
  1. A user following only the README quickstart can download the binary, start the server, and store a memory via curl in 3 commands or fewer
  2. The README API reference documents every endpoint with request parameters, response schema, and a copy-paste curl example
  3. The README includes working usage examples in curl, Python, and at least one agent framework showing realistic agent memory patterns
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation | 3/3 | Complete   | 2026-03-19 |
| 2. Embedding | 0/? | Not started | - |
| 3. Service and API | 0/? | Not started | - |
| 4. Distribution | 0/? | Not started | - |
