# Project Research Summary

**Project:** Mnemonic v1.4 — Pluggable Storage Backends
**Domain:** Rust async trait abstraction over storage backends (SQLite, Qdrant, Postgres)
**Researched:** 2026-03-21
**Confidence:** HIGH

## Executive Summary

Mnemonic v1.4 introduces a `StorageBackend` trait that decouples the memory server's business logic from its SQLite-only storage dependency. The approach follows a pattern already proven in the codebase: `EmbeddingEngine` and `SummarizationEngine` are both `#[async_trait]` object-safe traits held as `Arc<dyn Trait>` and selected at startup via config. The storage backend follows the identical pattern, with three concrete implementations — `SqliteBackend` (zero-config default), `QdrantBackend` (vector-native scale), and `PostgresBackend` (teams already running Postgres). The entire existing feature surface (compaction, auth, CLI subcommands) must continue to work transparently through the trait.

The recommended implementation strategy is "branch by abstraction": introduce the `StorageBackend` trait and `SqliteBackend` first without changing any service types, keep all 239 existing tests green, then wire `MemoryService` and `CompactionService` to use `Arc<dyn StorageBackend>`, and only after that implement Qdrant and Postgres backends. This order prevents the most expensive pitfall: a trait refactor that breaks the full test suite simultaneously with backend additions. The order is non-negotiable — the trait is the load-bearing piece that gates everything else.

The dominant risks are two subtle correctness bugs baked into the existing codebase. First, search threshold semantics are directional: SQLite's sqlite-vec returns L2 distance (lower-is-better), but Qdrant's cosine search returns `score` (higher-is-better). Without explicit conversion, the threshold filter silently inverts — distant results pass through and similar results are filtered out. Second, auth keys must remain in local SQLite regardless of the memory backend, because API key lookups are a per-request relational operation and routing them through a remote backend adds latency that compounds at scale. Both risks are avoidable if addressed at the trait design stage before any backend code is written.

## Key Findings

### Recommended Stack

The existing stack (axum 0.8, tokio 1, rusqlite 0.37+sqlite-vec 0.1.7, async-trait 0.1) is locked and must not change. Three new optional dependencies are added via Cargo feature flags: `qdrant-client 1.17` (official Qdrant gRPC SDK), `sqlx 0.8.6` with `runtime-tokio` and `tls-native-tls` features (matches existing reqwest TLS backend), and `pgvector 0.4.1` with the `sqlx` feature for `Vector` type binding. All three are declared `optional = true` and gated behind `backend-qdrant` and `backend-postgres` feature flags so the default binary carries zero new dependencies.

**Core technologies:**
- `qdrant-client 1.17`: Official Qdrant Rust SDK — the only maintained client; gRPC via tonic 0.12.3 (transitive, do not add tonic directly to avoid version conflicts)
- `sqlx 0.8.6`: Async-native Postgres with built-in `PgPool`; `tls-native-tls` matches existing reqwest TLS to avoid a second TLS stack in the binary
- `pgvector 0.4.1`: Provides `Vector` type for f32 arrays; must enable the `sqlx` feature to get the `Type` impl for sqlx query binding
- `async-trait 0.1`: Already present; mandatory for `Arc<dyn StorageBackend>` because native async fn in traits (Rust 1.75) is not dyn-compatible as of early 2026

**Critical version constraint:** Do not upgrade `rusqlite` past 0.37 — sqlite-vec 0.1.7 has a known conflict with rusqlite 0.39 via `libsqlite3-sys`.

### Expected Features

The `StorageBackend` trait is the load-bearing abstraction that every other feature depends on. The MVP for v1.4 is: the trait, SQLite wrapped behind it (zero user-visible change), MemoryService and CompactionService refactored to use `Arc<dyn StorageBackend>`, Qdrant and Postgres backends implemented, config extended with `storage_provider` and per-backend credential fields, `validate_config()` expanded to gate startup on required fields, startup connectivity health checks, auth key SQLite isolation, and `mnemonic config show/validate` subcommands.

**Must have (table stakes):**
- `StorageBackend` trait covering all memory CRUD + compaction operations — every other feature is blocked on this
- `SqliteBackend` wrapping existing tokio_rusqlite code — zero functional change for existing users
- `MemoryService` + `CompactionService` refactored to `Arc<dyn StorageBackend>` — removes `Arc<Connection>` coupling from service layer
- Config extension: `storage_provider`, `qdrant_url`, `qdrant_api_key`, `qdrant_collection`, `postgres_url`
- `validate_config()` expansion — fail loud at startup with actionable messages when required backend config is missing
- `QdrantBackend` implementation behind `backend-qdrant` feature flag
- `PostgresBackend` implementation behind `backend-postgres` feature flag (uses `<=>` cosine distance operator, not `<->` L2)
- Auth keys always remain in local SQLite, independent of memory backend selection
- `mnemonic config show` + `mnemonic config validate` subcommands

**Should have (competitive):**
- `/health` endpoint extended with `backend` and `backend_latency_ms` fields — surfaces backend connectivity issues before they manifest as request failures
- `mnemonic config show --json` for CI/CD pipeline introspection

**Defer (v1.5+):**
- `mnemonic migrate --from <backend> --to <backend>` — genuine differentiator (no competing tool handles SQLite as a migration source) but requires all backends proven stable first
- Auth keys in Postgres when Postgres is the memory backend — eliminates the split-DB concern for Postgres users; deferred to avoid auth complexity in v1.4
- Additional community backends (Weaviate, Chroma) — document the trait as the extension point; let community implement

**Confirmed out of scope:**
- Auto-migrate on config change — dangerous, silent data movement at startup is unacceptable UX
- Multi-backend fan-out writes — distributed systems complexity without proportionate value at Mnemonic's target scale
- ORM-based query builder — SeaORM does not natively support pgvector; Mnemonic already uses raw SQL patterns throughout
- Hot backend switching without restart — requires complex shared-ref swap under live traffic
- Per-agent backend routing — breaks the clean trait abstraction and creates data spread across multiple backends

### Architecture Approach

The v1.4 architecture inserts a `StorageBackend` trait layer between the service layer and concrete storage implementations. `MemoryService` and `CompactionService` replace their `Arc<tokio_rusqlite::Connection>` fields with `Arc<dyn StorageBackend>`. `KeyService` is explicitly excluded — auth key operations are not part of the pluggable backend and `KeyService` keeps its direct `Arc<Connection>` unchanged. Backend selection happens in `main.rs` via a match on `config.storage_provider`, mirroring the existing embedding engine factory pattern. Each backend lives in its own module (`src/storage/sqlite.rs`, `src/storage/qdrant.rs`, `src/storage/postgres.rs`) gated by cfg feature flags.

**Major components:**
1. `StorageBackend` trait (`src/storage/mod.rs`) — defines `initialize`, `insert_memory`, `get_memory`, `get_memory_agent_id`, `delete_memory`, `list_memories`, `search_memories`, `fetch_compaction_candidates`, `apply_compaction`, `create_compact_run`, `finish_compact_run`; all methods are `#[async_trait]` to enable `Arc<dyn StorageBackend>`
2. `SqliteBackend` (`src/storage/sqlite.rs`) — wraps existing `tokio_rusqlite::Connection` + sqlite-vec MATCH queries; the unsafe byte cast for embedding deserialization from raw little-endian IEEE-754 bytes stays here and is never exposed through the trait
3. `QdrantBackend` (`src/storage/qdrant.rs`, cfg-gated) — gRPC upsert/query via `qdrant-client`; converts Qdrant `score` (higher-is-better for cosine) to `distance` (lower-is-better) before returning search results; compact_run audit records written to a companion SQLite file
4. `PostgresBackend` (`src/storage/postgres.rs`, cfg-gated) — `sqlx::PgPool` + `pgvector::Vector`; uses `<=>` cosine distance operator; `apply_compaction` wraps delete+insert in a real Postgres transaction
5. Backend factory in `main.rs` — constructs the appropriate `Arc<dyn StorageBackend>` at startup via match on `config.storage_provider`, calls `initialize()` before accepting traffic

### Critical Pitfalls

1. **Native async fn in traits is not dyn-compatible** — Rust 1.75 stabilized async fn in traits but this does NOT support `Arc<dyn StorageBackend>`. Use `#[async_trait]` on both the trait definition and all impl blocks, identical to the existing `EmbeddingEngine` pattern. The compiler error ("the trait X is not dyn compatible") will appear only when writing `Arc<dyn StorageBackend>`, not when writing `impl StorageBackend for SqliteBackend`.

2. **Threshold semantics inversion (silent correctness bug)** — The existing search threshold is lower-is-better (L2 distance from sqlite-vec); Qdrant's cosine collections return `score` where higher-is-better — the opposite direction. Every backend must normalize to the same semantic before returning results: `distance = 1.0 - score` for Qdrant cosine, pass-through for pgvector `<=>` cosine distance. Document the expected semantic explicitly in the `search_memories` doc comment on the trait.

3. **239 existing tests all assume concrete SQLite types** — Tests seed via `conn.call()`, access `ks.conn` directly, and inspect SQLite schema via `PRAGMA`. Introduce the trait and change service types in the same commit and all tests break simultaneously. Use branch-by-abstraction: introduce `SqliteBackend` first while services still hold `Arc<Connection>` internally, reach 239 green, then migrate service field types.

4. **Leaky SQLite abstraction in the trait design** — Existing code has SQLite-specific behaviors (10x KNN over-fetch for post-filter, dual-table atomic write, JSON string tags, `compact_runs` audit table) that must not appear in the trait interface. Design from the consumer perspective: `search_memories(embedding, params)` not `knn_with_overselect(embedding, k * 10, params)`. The over-fetch factor is a `SqliteBackend` implementation detail.

5. **Auth key backend coupling** — If the `StorageBackend` trait accidentally includes auth operations (e.g., `create_api_key`), switching to Qdrant triggers `unimplemented!()` panics on every protected request. Explicitly exclude auth operations from the trait. `KeyService` keeps its direct `Arc<Connection>` unchanged across the entire v1.4 milestone.

## Implications for Roadmap

Based on research, the dependency graph mandates a strict implementation order. The trait is the critical path — everything else is blocked on it or gated behind it.

### Phase 1: Storage Trait Definition and SQLite Backend

**Rationale:** The `StorageBackend` trait is the load-bearing piece that gates all subsequent work. Defining it incorrectly is expensive to fix after backends are implemented. The SQLite backend is a refactor of existing code, proving the trait works against real operations without introducing new failure modes. No subsequent phase can start until this phase completes with all 239 tests green.

**Delivers:** `StorageBackend` trait with documented semantics (including explicit distance direction in `search_memories` doc comment), `SqliteBackend` struct, `MemoryService` and `CompactionService` refactored to `Arc<dyn StorageBackend>`, full test suite at 239 passing.

**Addresses:** StorageBackend trait (table stakes), SQLite backward compat, MemoryService + CompactionService refactor

**Avoids:** Pitfalls 1 (async dyn compatibility), 3 (test suite breakage), 4 (leaky abstraction), 5 (auth coupling), 7 (compaction embedding fetch unsafe bytes surfacing through trait)

**Research flag:** Standard patterns — the existing `EmbeddingEngine` trait is the direct template. No additional research needed.

### Phase 2: Config Extension and Backend Factory

**Rationale:** Config and runtime wiring must exist before any new backend can be exercised end-to-end. `validate_config()` expansion, startup health checks, and the `mnemonic config show/validate` subcommands are all config-layer concerns with no backend-specific implementation complexity. Done before Qdrant/Postgres so the factory wiring exists when those backends are plugged in.

**Delivers:** `storage_provider` config field with `qdrant_url`, `qdrant_api_key`, `qdrant_collection`, `postgres_url` optional fields; expanded `validate_config()` with actionable startup errors; backend factory in `main.rs`; `mnemonic config show` and `mnemonic config validate` subcommands.

**Addresses:** Config-driven backend selection (table stakes), validate_config expansion, startup error messages, config subcommands

**Avoids:** UX pitfall of cryptic errors at request time; auth key accidentally routed through new backend before guard is in place

**Research flag:** Standard patterns — mirrors existing `embedding_provider` config pattern exactly. No research needed.

### Phase 3: Qdrant Backend Implementation

**Rationale:** Qdrant is the primary new backend and the most technically novel — gRPC API, score-vs-distance semantic inversion, payload indexing requirement for filtering performance, and non-transactional `apply_compaction`. Implementing Qdrant before Postgres forces resolution of the harder architectural questions (score direction conversion, compact_run audit location) that inform trait documentation. Postgres is simpler once Qdrant is working.

**Delivers:** `QdrantBackend` implementing all `StorageBackend` methods; collection creation with vector dimension validation at startup; payload indexes on `agent_id` and `session_id`; score-to-distance conversion (`1.0 - score`); compaction via scroll+delete+upsert; compact_run audit in companion SQLite file.

**Uses:** `qdrant-client 1.17` (gRPC, builder API), `backend-qdrant` Cargo feature flag

**Addresses:** QdrantBackend (FEATURES P1), auth key isolation confirmed working end-to-end

**Avoids:** Threshold semantics inversion (Pitfall 2), Qdrant payload filter indexing gotcha, vector dimension mismatch at startup

**Research flag:** Likely needs phase research — Qdrant's scroll API pagination pattern for compaction candidate fetch and payload index creation syntax are niche and worth checking against current qdrant-client 1.17 docs before implementation.

### Phase 4: Postgres Backend Implementation

**Rationale:** Postgres backend is architecturally simpler than Qdrant — SQL transactions make `apply_compaction` truly atomic, pgvector `<=>` operator is well-documented, and sqlx+pgvector combination has clear examples. Implement after Qdrant so the trait's `apply_compaction` semantics are already proven. This phase completes the backend surface and enables cross-backend integration testing.

**Delivers:** `PostgresBackend` implementing all `StorageBackend` methods; schema with `vector(384)` column via pgvector; `<=>` cosine distance queries; `PgPool` connection pooling; full compaction in a Postgres transaction; `compact_runs` table in Postgres.

**Uses:** `sqlx 0.8.6`, `pgvector 0.4.1`, `backend-postgres` Cargo feature flag

**Addresses:** PostgresBackend (FEATURES P1), "I already have Postgres" use case

**Avoids:** Integration gotcha (use `<=>` not `<->` for normalized embeddings), pgvector range gotcha (`<=>` returns 0-2 not 0-1), performance trap (single connection vs PgPool)

**Research flag:** Standard patterns — pgvector + sqlx is well-documented at pgvector-rust GitHub and sqlx docs.

### Phase 5: Health Endpoint Extension and Release Polish

**Rationale:** Backend observability is a force multiplier — operators need to know if Qdrant or Postgres is unreachable before it surfaces as failed memory operations. `/health` extension is low implementation effort and high operational value. This phase also catches any binary size regressions from the new feature-flagged crates and runs final cross-backend integration tests.

**Delivers:** `/health` endpoint extended with `backend` and `backend_latency_ms` fields; binary size measurement for default, `backend-qdrant`, and `backend-postgres` builds; `mnemonic config show --json`; cross-backend integration test suite confirming compaction, auth, and search all work for each backend configuration.

**Addresses:** /health extension (FEATURES P2), `--json` flag extension, release documentation

**Avoids:** UX pitfall of config showing `backend=qdrant` when Qdrant is unreachable; performance trap of per-request auth on remote backend confirmed absent

**Research flag:** Standard patterns — extends existing `HealthResponse` struct.

### Phase Ordering Rationale

- Phases 1-2 are hard prerequisites for all subsequent work and must run in strict order. Phase 1 establishes the trait contract; Phase 2 establishes the runtime wiring that exercises it.
- Phases 3-4 depend on Phases 1 and 2. Qdrant precedes Postgres because Qdrant's harder semantic questions (score inversion, non-transactional compaction) should be resolved first to inform trait documentation.
- Phase 5 is a cap that verifies the complete system and adds operational tooling. It must run last because it exercises all backends end-to-end.
- `mnemonic migrate` is explicitly deferred to v1.5 — it requires all backends proven stable and the risk profile of data migration tooling warrants a separate milestone.
- The branch-by-abstraction pattern (mandated by PITFALLS) creates internal sequencing within Phase 1: introduce `SqliteBackend` implementing the trait before wiring services to `Arc<dyn StorageBackend>`, so the test suite stays green throughout the refactor.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 3 (Qdrant):** Scroll API pagination for compaction candidate fetch, payload index creation syntax for `agent_id`/`session_id`, and exact delete-then-upsert pattern for non-transactional `apply_compaction` in qdrant-client 1.17
- **Phase 1 (Trait definition):** Compact_run audit log design for non-relational backends — whether Qdrant delegates audit records to a companion SQLite file or the audit methods are `no-op` with a doc comment is an open design decision to settle before any backend code is written

Phases with standard patterns (skip research-phase):
- **Phase 2 (Config):** Mirrors existing `embedding_provider` config pattern exactly
- **Phase 4 (Postgres):** pgvector + sqlx combination is well-documented with official examples
- **Phase 5 (Health):** Extends existing `HealthResponse` struct; minimal new surface

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All crate versions verified against docs.rs, GitHub Cargo.toml, and feature flag docs. Version compatibility matrix includes known conflict (rusqlite 0.37 lock). |
| Features | HIGH | Sourced from direct codebase inspection (primary source), competitor analysis (agent-memory-server, OpenWebUI), and real-world migration case studies. Feature scope is opinionated and complete. |
| Architecture | HIGH | Direct code inspection of v1.3 (22,198 lines across 12 source files). Trait shape derived from existing method signatures in `service.rs` and `compaction.rs`. AppState change is mechanical. |
| Pitfalls | HIGH | Seven critical pitfalls verified against official Rust docs, direct codebase inspection, and Qdrant/pgvector documentation. Threshold semantics bug identified via direct inspection of `service.rs` line 221 and `compaction.rs` line 96. |

**Overall confidence:** HIGH

### Gaps to Address

- **Compact_run audit log for non-relational backends:** Where do compaction audit records go when the memory backend is Qdrant? The architecture research suggests a companion SQLite file for Qdrant users, but this means the Qdrant backend silently has a SQLite dependency. This design decision should be settled in Phase 1 (trait design) before any backend code is written, not deferred to Phase 3 implementation.

- **Feature flag discoverability for `cargo install` users:** The research recommends `default = []` with opt-in backend features, meaning `cargo install mnemonic` produces a SQLite-only binary. The documentation and release artifacts must make the `--features backend-qdrant` installation path discoverable. Address this during Phase 2 when the config subcommand and its documentation are built.

- **Default Qdrant collection name:** The architecture shows `config.qdrant_collection.clone().unwrap_or("mnemonic".to_string())` as the default. Once released, this default becomes a compatibility constraint. Confirm the collection name convention before Phase 3 ships so it does not need to change in a future version.

## Sources

### Primary (HIGH confidence)
- [qdrant-client docs.rs 1.17](https://docs.rs/qdrant-client/latest/qdrant_client/index.html) — API overview, tonic 0.12.3, tokio 1.40+ compatibility confirmed
- [qdrant/rust-client Cargo.toml (master)](https://raw.githubusercontent.com/qdrant/rust-client/master/Cargo.toml) — Exact deps and feature list
- [qdrant/rust-client README](https://github.com/qdrant/rust-client/blob/master/README.md) — Upsert and search API examples
- [pgvector 0.4.1 docs.rs](https://docs.rs/pgvector/latest/pgvector/) — Feature flags, sqlx integration, Vector types
- [sqlx 0.8.6 docs.rs](https://docs.rs/sqlx/latest/sqlx/) — Runtime features, PgPool, Postgres support
- [Rust Blog: async fn in traits (Dec 2023)](https://blog.rust-lang.org/2023/12/21/async-fn-rpit-in-traits/) — Confirms native async fn not object-safe for dyn dispatch
- [async-trait crate docs](https://docs.rs/async-trait/0.1.83/async_trait/index.html) — Pin<Box<dyn Future>> desugaring, Send bounds
- [Qdrant distance metrics documentation](https://qdrant.tech/course/essentials/day-1/distance-metrics/) — Score direction for cosine collections (higher-is-better)
- Direct codebase inspection — `src/service.rs`, `src/compaction.rs`, `src/auth.rs`, `src/db.rs`, `src/embedding.rs` — primary source for trait shape, test count, and pitfall identification

### Secondary (MEDIUM confidence)
- [pgvector GitHub issue #72: cosine distance vs similarity](https://github.com/pgvector/pgvector/issues/72) — `<=>` returns 0-2 range, not 0-1
- [Supabase issue: `<=>` is cosine distance, not cosine similarity](https://github.com/supabase/supabase/issues/12244) — Confirms range and direction
- [Migrating Vector Embeddings from PostgreSQL to Qdrant (Medium)](https://0xhagen.medium.com/migrating-vector-embeddings-from-postgresql-to-qdrant-challenges-learnings-and-insights-f101f42f78f5) — Real-world migration pitfalls and scroll API patterns
- [redis/agent-memory-server GitHub](https://github.com/redis/agent-memory-server) — Pluggable backend factory pattern reference
- [n8n SQLite→Postgres migration community forum](https://community.n8n.io/t/how-to-migrate-from-sqlite-to-postgres/97414) — Validates `mnemonic migrate` as a real user need
- [pgvector vs Qdrant comparison (TigerData)](https://www.tigerdata.com/blog/pgvector-vs-qdrant) — Backend selection tradeoffs
- [baby steps: dyn async traits part 10 (Mar 2025)](https://smallcultfollowing.com/babysteps/blog/2025/03/24/box-box-box/) — Latest state of native dyn async trait support
- [rust-lang/impl-trait-utils#34](https://github.com/rust-lang/impl-trait-utils/issues/34) — dyn async trait still in active development as of early 2026
- [axum sqlx-postgres example Cargo.toml](https://github.com/tokio-rs/axum/blob/main/examples/sqlx-postgres/Cargo.toml) — Confirmed sqlx 0.8 feature pattern

---
*Research completed: 2026-03-21*
*Ready for roadmap: yes*
