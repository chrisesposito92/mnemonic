# Requirements: Mnemonic

**Defined:** 2026-03-21
**Core Value:** Any AI agent can store and semantically search memories out of the box with zero configuration — just download and run

## v1.4 Requirements

Requirements for pluggable storage backends. Each maps to roadmap phases.

### Storage Abstraction

- [ ] **STOR-01**: StorageBackend async trait defines store, get, list, search, delete, and compact operations with normalized distance semantics
- [ ] **STOR-02**: SqliteBackend implements StorageBackend by wrapping existing SQLite+sqlite-vec code with zero behavior change
- [ ] **STOR-03**: MemoryService holds Arc<dyn StorageBackend> instead of direct tokio-rusqlite connection
- [ ] **STOR-04**: CompactionService uses StorageBackend trait methods instead of direct SQLite queries
- [ ] **STOR-05**: All 239 existing tests pass unchanged after trait refactor

### Qdrant Backend

- [ ] **QDRT-01**: QdrantBackend implements StorageBackend using qdrant-client gRPC, feature-gated behind backend-qdrant
- [ ] **QDRT-02**: Qdrant score (higher=better) is normalized to distance (lower=better) matching StorageBackend contract
- [ ] **QDRT-03**: Compaction works on Qdrant with documented non-transactional semantics (separate delete+upsert)
- [ ] **QDRT-04**: Multi-agent namespace isolation via Qdrant payload filtering on agent_id

### Postgres Backend

- [ ] **PGVR-01**: PostgresBackend implements StorageBackend using sqlx + pgvector, feature-gated behind backend-postgres
- [ ] **PGVR-02**: Vector search uses pgvector cosine distance operator with proper indexing
- [ ] **PGVR-03**: Compaction uses Postgres transactions for atomic delete+insert
- [ ] **PGVR-04**: Multi-agent namespace isolation via SQL WHERE filtering on agent_id

### Configuration & CLI

- [ ] **CONF-01**: storage_provider config field (sqlite/qdrant/postgres) in TOML and env vars with startup validation
- [ ] **CONF-02**: Backend-specific config fields (qdrant_url, qdrant_api_key, postgres_url) with validate_config() checks
- [ ] **CONF-03**: mnemonic config show subcommand displays current configuration with secret redaction
- [ ] **CONF-04**: GET /health reports active storage backend name and connection status

## Future Requirements

### Migration

- **MIGR-01**: mnemonic migrate subcommand exports memories from one backend and imports to another
- **MIGR-02**: Migration preserves memory IDs, timestamps, tags, and embeddings

## Out of Scope

| Feature | Reason |
|---------|--------|
| Cross-backend migration (v1.4) | All backends must be stable first; deferred to v1.5 |
| Auto-migration on config change | Surprising behavior that silently moves data — explicit migration better |
| Multi-backend fan-out (write to multiple) | Complexity explosion; single active backend is simpler and sufficient |
| Backend-specific query syntax | Leaky abstraction; trait must normalize all operations |
| Auth keys in remote backends | Auth must stay in local SQLite — no network round-trip per request |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| STOR-01 | Phase 21 | Pending |
| STOR-02 | Phase 21 | Pending |
| STOR-03 | Phase 21 | Pending |
| STOR-04 | Phase 21 | Pending |
| STOR-05 | Phase 21 | Pending |
| CONF-01 | Phase 22 | Pending |
| CONF-02 | Phase 22 | Pending |
| CONF-03 | Phase 22 | Pending |
| CONF-04 | Phase 22 | Pending |
| QDRT-01 | Phase 23 | Pending |
| QDRT-02 | Phase 23 | Pending |
| QDRT-03 | Phase 23 | Pending |
| QDRT-04 | Phase 23 | Pending |
| PGVR-01 | Phase 24 | Pending |
| PGVR-02 | Phase 24 | Pending |
| PGVR-03 | Phase 24 | Pending |
| PGVR-04 | Phase 24 | Pending |

**Coverage:**
- v1.4 requirements: 17 total
- Mapped to phases: 17
- Unmapped: 0

---
*Requirements defined: 2026-03-21*
*Last updated: 2026-03-21 — traceability mapped after roadmap creation*
