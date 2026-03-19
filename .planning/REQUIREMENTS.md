# Requirements: Mnemonic

**Defined:** 2026-03-19
**Core Value:** Any AI agent can store and semantically search memories out of the box with zero configuration

## v1 Requirements

### Storage

- [x] **STOR-01**: Server persists memories in a single SQLite database file with sqlite-vec for vector search
- [x] **STOR-02**: Server starts with WAL mode enabled and single-writer connection to prevent SQLITE_BUSY errors under concurrent agent load
- [x] **STOR-03**: All database access uses tokio-rusqlite async closures to avoid blocking the tokio runtime
- [x] **STOR-04**: Schema tracks `embedding_model` per memory row to prevent vector space mismatch when switching embedding providers

### Embedding

- [x] **EMBD-01**: Server bundles all-MiniLM-L6-v2 via candle for zero-config local embedding inference (no external API key required)
- [x] **EMBD-02**: Embedding pipeline uses attention-mask-weighted mean pooling and L2 normalization (not CLS token)
- [x] **EMBD-03**: Embedding model loads once at startup and is shared across requests via Arc
- [x] **EMBD-04**: User can optionally set `OPENAI_API_KEY` env var to use OpenAI embeddings instead of local model
- [x] **EMBD-05**: Embedding provider is abstracted behind a trait with local (candle) and OpenAI implementations

### API

- [x] **API-01**: `POST /memories` stores a memory with content, optional agent_id, session_id, and arbitrary tags
- [x] **API-02**: `GET /memories/search` performs semantic search via vector similarity with optional agent_id, session_id, tag, and time range filters
- [x] **API-03**: `GET /memories` lists memories with structured filtering by agent_id, session_id, tags, and time range
- [x] **API-04**: `DELETE /memories/:id` deletes a specific memory
- [x] **API-05**: `GET /health` returns server readiness status
- [x] **API-06**: All endpoints return JSON responses with appropriate HTTP status codes and error messages

### Multi-Agent

- [x] **AGNT-01**: Memories are namespaced by agent_id so multiple agents can share a single mnemonic instance without collisions
- [x] **AGNT-02**: Memories can be grouped by session_id for conversation-scoped retrieval
- [x] **AGNT-03**: Semantic search pre-filters by agent_id before KNN to scope results and maintain performance

### Configuration

- [x] **CONF-01**: Server runs with zero configuration using sensible defaults (port 8080, local embeddings, ./mnemonic.db)
- [x] **CONF-02**: User can override settings via environment variables (port, storage path, embedding provider, OpenAI API key)
- [x] **CONF-03**: User can optionally provide a TOML configuration file for all settings

### Documentation

- [x] **DOCS-01**: README includes quickstart guide that gets a user from download to first stored memory in under 3 commands
- [ ] **DOCS-02**: README includes full API reference with request/response examples for every endpoint
- [ ] **DOCS-03**: README includes usage examples for curl, Python, and at least one agent framework

## v2 Requirements

### Enhanced API

- **EAPI-01**: `PUT /memories/:id` updates memory content and triggers re-embedding
- **EAPI-02**: Batch write endpoint (`POST /memories/batch`) for high-throughput ingestion
- **EAPI-03**: Hybrid search combining vector similarity with BM25 keyword matching

### Observability

- **OBSV-01**: OpenTelemetry tracing for request lifecycle
- **OBSV-02**: Structured logging with configurable log levels

### Security

- **SECR-01**: Optional API key authentication
- **SECR-02**: Rate limiting per client

## Out of Scope

| Feature | Reason |
|---------|--------|
| Memory summarization / compaction | Requires LLM calls per write; conflicts with zero-config, offline-capable value proposition |
| Web UI / dashboard | Adds frontend build pipeline and static asset serving; violates single-binary simplicity |
| gRPC support | Doubles interface surface; REST sufficient for all reviewed use cases |
| Pluggable storage backends (Qdrant, Postgres) | Single-file SQLite is a feature, not a limitation; massively increases abstraction complexity |
| Knowledge graph / entity extraction | Requires LLM calls per write; out of scope for local zero-config tool |
| Memory decay / TTL expiration | Surprising behavior that can silently lose data; let users delete explicitly |
| Multi-node / distributed mode | SQLite not designed for multi-writer distributed use |
| Authentication / API keys | Premature for embeddable tool used locally; run behind reverse proxy instead |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| STOR-01 | Phase 1 | Complete |
| STOR-02 | Phase 1 | Complete |
| STOR-03 | Phase 1 | Complete |
| STOR-04 | Phase 1 | Complete |
| CONF-01 | Phase 1 | Complete |
| CONF-02 | Phase 1 | Complete |
| CONF-03 | Phase 1 | Complete |
| EMBD-01 | Phase 2 | Complete |
| EMBD-02 | Phase 2 | Complete |
| EMBD-03 | Phase 2 | Complete |
| EMBD-04 | Phase 2 | Complete |
| EMBD-05 | Phase 2 | Complete |
| API-01 | Phase 3 | Complete |
| API-02 | Phase 3 | Complete |
| API-03 | Phase 3 | Complete |
| API-04 | Phase 3 | Complete |
| API-05 | Phase 3 | Complete |
| API-06 | Phase 3 | Complete |
| AGNT-01 | Phase 3 | Complete |
| AGNT-02 | Phase 3 | Complete |
| AGNT-03 | Phase 3 | Complete |
| DOCS-01 | Phase 4 | Complete |
| DOCS-02 | Phase 4 | Pending |
| DOCS-03 | Phase 4 | Pending |

**Coverage:**
- v1 requirements: 24 total
- Mapped to phases: 24
- Unmapped: 0

---
*Requirements defined: 2026-03-19*
*Last updated: 2026-03-19 after roadmap creation — all requirements mapped*
