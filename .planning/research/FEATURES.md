# Feature Research

**Domain:** Pluggable storage backends for a Rust memory-server binary (v1.4 milestone)
**Researched:** 2026-03-21
**Confidence:** HIGH (patterns sourced from Rust trait ecosystem, qdrant-client docs, pgvector-rust, redis/agent-memory-server, production migration case studies, existing mnemonic codebase)

---

## Scope Note

This document covers **only the new features for v1.4**: adding a storage trait abstraction so the existing SQLite backend can sit behind an interface, with Qdrant and Postgres as opt-in alternatives selectable via config. The v1.0–v1.3 baseline (REST API, embeddings, compaction, auth, full CLI) is already shipped and represents the _consumer_ of these backends, not a feature here.

The central question: what does pluggable storage look like when the existing compaction, auth, and semantic search surface must work transparently across backends?

---

## Feature Landscape

### Table Stakes (Users Expect These)

Features that any production-grade pluggable backend system provides. Missing these makes the abstraction feel like a toy.

| Feature | Why Expected | Complexity | Dependencies on Existing Architecture |
|---------|--------------|------------|---------------------------------------|
| **`StorageBackend` trait** covering all memory CRUD operations | Any backend system requires a single interface that every implementation satisfies. Without a trait, "pluggable" is just conditional code paths, not an abstraction. | MEDIUM | Must cover: `store_memory`, `get_memory`, `delete_memory`, `list_memories`, `search_memories` (KNN), `bulk_delete` (for compaction), `store_compact_run`, `get_compact_run`. All methods are currently direct `tokio_rusqlite` calls inside `MemoryService` and `CompactionService`. Both services must be refactored to hold `Arc<dyn StorageBackend>` instead of `Arc<Connection>`. |
| **SQLite remains default with zero config change** | Existing users cannot be broken. Changing the default behavior requires explicit opt-in. Every tool with pluggable backends (n8n, Open Web UI) maintains backward compat: SQLite stays until the user changes `storage_backend` in config. | LOW | SQLite implementation of the trait wraps existing `tokio_rusqlite` code. The `db.rs` open/schema path stays identical. `service.rs` and `compaction.rs` are refactored to use the trait, not the concrete `Connection`. |
| **Config-driven backend selection via TOML/env** | Users expect `storage_backend = "qdrant"` in `mnemonic.toml` or `MNEMONIC_STORAGE_BACKEND=qdrant` env var to switch backends. This is the same pattern already used for `embedding_provider` and `llm_provider`. Config-driven > code changes. | LOW | Extend `Config` struct with `storage_backend: String` (default `"sqlite"`), plus backend-specific fields: `qdrant_url`, `qdrant_api_key`, `qdrant_collection`, `postgres_url`. Follow existing `validate_config()` pattern: gate startup on required fields being present for the selected backend. |
| **`validate_config()` expanded for new backends** | Startup should fail loudly if `storage_backend = "qdrant"` but `qdrant_url` is missing. Users expect the same "gates startup on valid config" behavior already present for embedding/LLM providers. | LOW | Extend the existing `validate_config()` match arm logic. No new mechanism — same pattern already ships. |
| **Qdrant backend implementation** | Qdrant is the most-requested vector-native backend for agent memory tools. It manages embeddings as first-class data (not a bolt-on virtual table), supports advanced filtering, and is the vector database agents teams most commonly reach for. The Rust client (`qdrant-client`) is mature and async-native. | HIGH | Requires adding `qdrant-client` crate. Must map Mnemonic's `Memory` struct to Qdrant Points (payload fields + vector). `agent_id`, `session_id`, `tags`, `created_at`, `id` become payload fields. Vector = the 384-dim embedding. Search uses Qdrant's `query` with payload filters for `agent_id`. Compaction: fetch N candidate points with embeddings via scroll API, run existing clustering logic in Rust, delete old points, upsert new ones — all via Qdrant client. API key auth table: Qdrant is a vector DB, not relational — auth keys must remain in SQLite (or a separate simple file store) even when the memory backend is Qdrant. |
| **Postgres + pgvector backend implementation** | Postgres is the "I already have Postgres" use case. Teams running Postgres in production for their app data want to co-locate agent memory without operating a second service. pgvector-rust crate provides the `Vector` type and operator support. | HIGH | Requires `sqlx` (or `tokio-postgres` + `pgvector`) crates. Schema: `memories` table with a `vector(384)` column, plus `compact_runs`. Similarity search: `ORDER BY embedding <-> $1 LIMIT N` with `WHERE agent_id = $2` filter. Compaction: same pattern as Qdrant — fetch candidates, cluster in Rust, batch delete+insert inside a transaction. API key auth: Postgres can host the `api_keys` table (all relational). This makes Postgres the only backend where auth and memories live in the same DB. |
| **`mnemonic config` subcommand** | Users need to inspect and validate their current backend configuration without starting the server. Same pattern as Heroku CLI's `heroku config`, kubectl's `kubectl config view`. A tool that requires reading the TOML manually to know what backend is active has poor UX. | LOW | New clap subcommand. `mnemonic config show` prints current resolved config (backend, relevant URLs, no secrets). `mnemonic config validate` runs `validate_config()` and exits 0/1. Does NOT mutate config (no `set` subcommand for v1.4 — writing TOML programmatically is scope creep). |
| **Startup error message identifies missing backend dependency** | If `qdrant_url` is set but Qdrant is unreachable, the error should say "cannot connect to Qdrant at http://localhost:6334" — not a generic connection refused. Users deploy mnemonic to production and need actionable error messages. | LOW | Ping / health check the backend at startup before accepting traffic. Qdrant: `client.health_check().await`. Postgres: connection pool creation failure is sufficient. SQLite: file open error (already exists). |

### Differentiators (Competitive Advantage)

Features that are not universally expected but align with Mnemonic's positioning and solve real user pain.

| Feature | Value Proposition | Complexity | Dependencies on Existing Architecture |
|---------|-------------------|------------|---------------------------------------|
| **`mnemonic migrate` subcommand for data portability** | The biggest pain point when switching backends is data loss. Real-world migrations (SQLite→Postgres in n8n, Postgres→Qdrant in OpenWebUI) fail because users change config and discover their data is stranded. A built-in migration command is table stakes for trusted backend switching, but the _implementation_ is a differentiator since most tools leave this to the user. | HIGH | Reads from source backend (any `StorageBackend` impl), writes to target backend. Must preserve: memory IDs, content, agent_id, session_id, tags, created_at, embeddings. Does NOT re-embed (preserves existing vectors to avoid model mismatch). Runs as a one-shot command: `mnemonic migrate --from sqlite --to qdrant`. Progress output to stderr. Atomicity: best-effort (insert-then-verify, not transactional across backends). This is a genuine differentiator — the qdrant/migration tool only handles Qdrant targets. |
| **API key auth remains in SQLite regardless of memory backend** | Auth is relational, not vector-native. Qdrant is the wrong store for `api_keys`. Mixing auth state into the vector backend adds complexity and breaks Qdrant's data model. Keeping auth in a lightweight local SQLite file (even when memories are in Qdrant) is simpler and avoids a second Qdrant collection for non-vector data. | MEDIUM | Introduce a split: `StorageBackend` trait covers memory + compaction. `AuthBackend` (or just a hard-coded SQLite file) covers API keys. Config: `auth_db_path` (default `mnemonic-auth.db` or same `mnemonic.db`). For Postgres backend users, the option to keep auth in Postgres is a v1.5+ consideration. |
| **Compaction works identically across all backends** | Compaction's clustering is done in Rust memory (fetch candidates → cluster → write results). The `StorageBackend` trait provides `fetch_memories_with_embeddings(agent_id, limit)` and `atomic_compact(deletions, insertions)`. This means the compaction algorithm doesn't need to know which backend is active — same logic runs against SQLite, Qdrant, or Postgres. | MEDIUM | `CompactionService` currently holds `Arc<Connection>` and runs SQL directly. Refactor to use `Arc<dyn StorageBackend>` and call trait methods. The clustering logic in `compaction.rs` is pure Rust — no SQL dependency — and moves cleanly. `atomic_compact` semantics differ: SQLite wraps in a transaction, Qdrant does delete+upsert (no cross-point transaction), Postgres wraps in a transaction. Document this semantic difference. |
| **`mnemonic config show --json` for automation** | Shell scripts and CI pipelines that deploy mnemonic need to introspect the active backend without parsing TOML. `mnemonic config show --json` outputs `{"backend":"qdrant","qdrant_url":"http://..."}`. Extends the existing `--json` global flag convention. | LOW | Trivially extends `mnemonic config show` with `--json` serialization of the sanitized config struct (no secrets). Already a project-wide pattern. |
| **Backend-specific health info in `/health` endpoint** | The existing `GET /health` returns `{"status":"ok"}`. With pluggable backends, operators want `{"status":"ok","backend":"qdrant","backend_status":"ok","latency_ms":3}`. This surfaces backend connectivity issues before they manifest as memory operation failures. | LOW | Extend `HealthResponse` struct with `backend` and `backend_latency_ms` fields. Each `StorageBackend` impl exposes a `health_check() -> HealthStatus` method. Minimal implementation effort, high operational value. |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Auto-migrate on config change** | "When I change `storage_backend`, my data should follow automatically." | Silent data migration on startup is dangerous: partial failures leave data in inconsistent state across both backends. Users who change config accidentally could trigger a large migration. The right UX is explicit: run `mnemonic migrate` deliberately. | Detect backend mismatch (e.g., SQLite file exists but config says `qdrant`) and print a warning with the `mnemonic migrate` command. Never auto-migrate. |
| **Simultaneous multi-backend writes** | "Write to SQLite AND Qdrant at the same time for redundancy." | Multi-backend fan-out adds distributed systems complexity: partial write failures, consistency guarantees, write amplification. For agent memory at the scale Mnemonic targets (N<100K memories), single-backend reliability is sufficient. | Choose one backend. Use OS-level backups or Qdrant's own snapshot feature for durability. |
| **ORM-based query builder** | "Use Diesel or SeaORM to abstract SQL differences between SQLite and Postgres." | ORMs add Rust compile time, binary size, and leaky abstraction (not all SQL features map cleanly). Mnemonic already uses raw SQL with `tokio_rusqlite`. For vector operations, ORMs either don't support `<->` operators or require plugins. SeaORM doesn't natively support pgvector. | Use raw SQL for SQLite and Postgres backends (same as current approach). Qdrant uses its own gRPC/REST client — no SQL involved. |
| **Pinecone / Weaviate / Chroma backends** | "Add Pinecone support too!" | Each additional backend multiplies trait implementation surface and test matrix. Pinecone is managed-only (no local dev). Weaviate and Chroma have Rust clients but niche user bases for Mnemonic's target audience. The three backends (SQLite, Qdrant, Postgres) cover: zero-external-dependency, vector-native, and "I already have Postgres" — the actual usage segments. | Document the `StorageBackend` trait so the community can implement additional backends. The trait is the extension point, not the binary. |
| **Hot backend switching without restart** | "Let me change `storage_backend` in the TOML and have mnemonic switch live." | `Arc<dyn StorageBackend>` is initialized at startup and shared via `AppState`. Live-swapping it requires replacing a shared reference while requests may be in flight — complex synchronization. The operational benefit is marginal (most deploys involve restart anyway). | Restart mnemonic after changing the backend config. Document this explicitly. |
| **Per-agent backend routing** | "Route agent A's memories to SQLite and agent B's to Qdrant." | Adds routing logic to every operation, breaks the clean trait abstraction, and creates data spread across multiple backends that `mnemonic migrate` can't handle atomically. | Use a single backend per mnemonic instance. Run two instances if isolation is required. |
| **mnemonic config set <key> <value>** | "Let me change config from the CLI without editing TOML." | Programmatic TOML writing is fragile (comments get stripped, ordering changes, existing formatting lost). The existing env var override path is the right mechanism for scripting. | Use `MNEMONIC_STORAGE_BACKEND=qdrant mnemonic serve` for ephemeral overrides, or edit `mnemonic.toml` for permanent config. |

---

## Feature Dependencies

```
[StorageBackend trait]
    └──required by──> [SQLite backend] (wraps existing tokio_rusqlite code)
    └──required by──> [Qdrant backend] (new — uses qdrant-client)
    └──required by──> [Postgres backend] (new — uses sqlx or tokio-postgres + pgvector)
    └──required by──> [MemoryService refactor] (holds Arc<dyn StorageBackend> not Arc<Connection>)
    └──required by──> [CompactionService refactor] (same — trait-based fetch + atomic_compact)

[Config extension: storage_backend field]
    └──required by──> [validate_config() expansion] (new backend-specific validation branches)
    └──required by──> [startup backend initialization] (factory function: match backend → Box<dyn StorageBackend>)

[MemoryService refactor]
    └──required by──> [Qdrant backend]
    └──required by──> [Postgres backend]
    └──required by──> [mnemonic migrate] (reads and writes via trait, not concrete type)

[CompactionService refactor]
    └──required by──> [Qdrant backend] (compaction must work without SQL transactions)
    └──required by──> [Postgres backend] (compaction in a Postgres transaction)

[mnemonic config subcommand]
    └──requires──> [Config struct] (reads and displays resolved config)
    └──enhances──> [--json flag] (trivially — same global flag pattern)

[mnemonic migrate subcommand]
    └──requires──> [StorageBackend trait] (reads from source impl, writes to target impl)
    └──requires──> [All backend implementations] (both source and target must be available)
    └──requires──> [Config extension] (needs to know source and target backend config)

[Auth key isolation]
    └──requires──> [SQLite backend] (auth stays in SQLite even when memory backend is Qdrant)
    └──conflicts with──> [Qdrant backend for api_keys] (Qdrant is not the right store for relational auth data)

[/health endpoint extension]
    └──requires──> [StorageBackend trait: health_check() method]
    └──enhances──> [existing HealthResponse struct] (adds backend fields)
```

### Dependency Notes

- **`StorageBackend` trait is the load-bearing piece.** Every other feature in this milestone either implements it or consumes it. Phase 1 must define and stabilize the trait before any backend or consumer code is written.
- **Auth key isolation is a design decision, not an implementation detail.** The `api_keys` table exists in SQLite today. When users switch to Qdrant, the auth system must continue to work — either by keeping a local SQLite file just for auth, or by adding an `AuthBackend` trait separately. The cleaner choice is keeping auth in a separate SQLite file, always. This avoids forcing Qdrant to store relational data it is not designed for.
- **CompactionService is the hardest refactor.** It currently fetches embeddings via raw SQL, runs Rust clustering, and does atomic writes via a single SQLite transaction. The Qdrant equivalent has no cross-point transactions — delete and upsert are separate operations. The trait's `atomic_compact` method must document that "atomic" means best-effort for non-transactional backends (Qdrant), and true atomic for transactional backends (SQLite, Postgres).
- **`mnemonic migrate` requires all backends to be implemented first.** It is the last feature to implement, not the first.
- **SQLite backend implementation is just a refactor, not new code.** The current `db.rs` + `service.rs` + `compaction.rs` code becomes the `SqliteBackend` struct that implements `StorageBackend`. No functional change, only structural.

---

## MVP Definition

### Ship in v1.4 (This Milestone)

- [ ] `StorageBackend` trait with all memory + compaction operations defined
- [ ] `SqliteBackend` — wraps existing code behind the trait (zero functional change)
- [ ] `MemoryService` refactored to `Arc<dyn StorageBackend>` (removes `Arc<Connection>` coupling)
- [ ] `CompactionService` refactored to use trait methods for fetch + atomic_compact
- [ ] `QdrantBackend` — full implementation behind the trait
- [ ] `PostgresBackend` — full implementation behind the trait (pgvector)
- [ ] Config extension: `storage_backend`, `qdrant_url`, `qdrant_api_key`, `qdrant_collection`, `postgres_url`
- [ ] `validate_config()` expanded for Qdrant and Postgres required fields
- [ ] Startup backend health check with actionable error messages
- [ ] Auth keys always remain in SQLite (isolated from memory backend)
- [ ] `mnemonic config show` subcommand (read-only, with `--json`)
- [ ] `mnemonic config validate` subcommand (runs validate_config, exits 0/1)
- [ ] `/health` endpoint extended with `backend` and `backend_latency_ms` fields

### Add After Validation (v1.5+)

- [ ] `mnemonic migrate --from <backend> --to <backend>` — high value but requires all backends stable first; the data portability guarantee is most credible when backends are proven
- [ ] `mnemonic config set <key> <value>` — only if users report TOML editing friction at scale
- [ ] Auth keys in Postgres when Postgres is the memory backend — eliminates the split-DB concern for Postgres users
- [ ] Additional community backends (Weaviate, Chroma) — document the trait, let community implement

### Confirmed Out of Scope (v1.4)

- [ ] Auto-migrate on config change — too dangerous
- [ ] Multi-backend fan-out writes — distributed systems complexity without proportionate value
- [ ] ORM-based query builder — binary bloat, no vector support
- [ ] Pinecone / Weaviate / Chroma backends — document trait for community extension instead
- [ ] Hot backend switching without restart — requires complex shared-ref swap
- [ ] Per-agent backend routing — breaks trait abstraction, creates multi-backend data spread
- [ ] `mnemonic config set` — TOML writing is fragile; env vars cover the scripting use case

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| `StorageBackend` trait definition | HIGH | MEDIUM | P1 — everything else is blocked on this |
| SQLite backend (refactor existing code) | HIGH | LOW | P1 — preserves existing users |
| `MemoryService` + `CompactionService` refactor | HIGH | MEDIUM | P1 — consumers of the trait |
| Config extension + validate_config expansion | HIGH | LOW | P1 — config-driven switching is the UX |
| `QdrantBackend` implementation | HIGH | HIGH | P1 — primary new backend |
| `PostgresBackend` implementation | HIGH | HIGH | P1 — "I already have Postgres" use case |
| Auth key SQLite isolation design | HIGH | MEDIUM | P1 — correctness concern, not a feature |
| Startup backend health check | MEDIUM | LOW | P1 — operational necessity |
| `mnemonic config show` + `validate` subcommands | MEDIUM | LOW | P1 — UX without this is "read the TOML" |
| `/health` endpoint extension | MEDIUM | LOW | P2 |
| `mnemonic migrate` subcommand | HIGH | HIGH | P2 (v1.5) — needs all backends stable first |
| `mnemonic config show --json` | LOW | LOW | P2 — easy extension of existing pattern |

**Priority key:**
- P1: Must have for v1.4 to be a working backend abstraction
- P2: High value, add when possible (v1.4 end or v1.5)
- P3: Nice to have, future consideration

---

## Cross-Cutting Concerns: Existing Features vs. Backend Abstraction

The existing features (v1.0–v1.3) are consumers of the storage backend. Each has specific requirements the `StorageBackend` trait must satisfy.

### Compaction (v1.1)

Compaction requires: (1) fetching candidate memories with their embeddings for a given `agent_id`, (2) writing a compact run audit log entry, (3) an atomic operation that deletes source memories and inserts merged memories in one logical transaction.

For SQLite: this is a single SQL transaction. For Postgres: same. For Qdrant: delete and upsert are separate gRPC calls — "atomic" means sequential with error rollback, not true ACID. The trait must document this semantic difference.

The `StorageBackend` trait method `atomic_compact(run_id, agent_id, deletions: Vec<String>, insertions: Vec<MemoryWithEmbedding>) -> Result<()>` encapsulates this complexity. Each backend implements whatever "atomic" means for its model.

### Auth (v1.2)

The `api_keys` table is relational: lookup by key hash, join with `agent_id` scopes, revocation. This is a poor fit for Qdrant (a vector DB) or for a generic `StorageBackend` trait focused on memories. Auth must be a parallel concern, not a `StorageBackend` method.

Decision: `api_keys` always lives in a SQLite file (default: same `mnemonic.db`, or a separate `mnemonic-auth.db` if the memory backend is non-SQLite). The `AuthService` holds its own `Arc<tokio_rusqlite::Connection>` independently of the memory backend. This is a split that the `AppState` must reflect: `memory_backend: Arc<dyn StorageBackend>` and `auth_db: Arc<tokio_rusqlite::Connection>`.

### CLI Subcommands (v1.3)

The CLI subcommands (`remember`, `recall`, `search`, `compact`) all currently initialize the DB via `db::open()`. After refactoring, they must initialize the appropriate backend via the factory function. The tiered init pattern (DB-only for `recall`/`keys`, full for `remember`/`search`) still applies — but "DB-only" now means "lightweight backend init" (SQLite opens the file; Qdrant creates a client connection; Postgres creates a connection pool). The fast path is maintained by not loading the embedding model, not by skipping backend init.

---

## Competitor / Reference Analysis

| Feature | redis/agent-memory-server | Open Web UI | Mnemonic v1.4 |
|---------|--------------------------|-------------|----------------|
| Pluggable backend mechanism | Factory pattern (Python) | Env var `DATABASE_URL` | Rust trait + config |
| Default backend | Redis | SQLite | SQLite |
| Postgres support | No (Redis-centric) | Yes (via SQLAlchemy) | Yes (via pgvector-rust) |
| Qdrant support | Yes (primary vector DB) | Yes (separate collection) | Yes (via qdrant-client) |
| Migration tool | No | No | `mnemonic migrate` (v1.5) |
| Auth backend isolation | N/A | SQLite always for auth | SQLite always for auth |
| Backend health in /health | No | No | Yes (latency + status) |
| Config subcommand | No | No | `mnemonic config show/validate` |

---

## Sources

- [qdrant-client Rust docs](https://docs.rs/qdrant-client/latest/qdrant_client/index.html) — `Qdrant::from_url`, `create_collection`, `upsert_points`, `query`, `delete_points` APIs. HIGH confidence (official docs).
- [pgvector-rust GitHub](https://github.com/pgvector/pgvector-rust) — `Vector` type, `<->` operator support for tokio-postgres and sqlx. HIGH confidence (official repo).
- [redis/agent-memory-server GitHub](https://github.com/redis/agent-memory-server) — Pluggable memory vector database factory pattern reference. MEDIUM confidence (README-level).
- [Migrating Vector Embeddings from PostgreSQL to Qdrant (Medium)](https://0xhagen.medium.com/migrating-vector-embeddings-from-postgresql-to-qdrant-challenges-learnings-and-insights-f101f42f78f5) — Real-world migration pitfalls: type mismatches, scroll API patterns, delete-after-insert workflow. MEDIUM confidence (practitioner post).
- [Qdrant migration tool](https://qdrant.tech/blog/beta-database-migration-tool/) — What Qdrant's own migration tool covers (and what it doesn't: no SQLite source). MEDIUM confidence (official blog).
- [n8n SQLite→Postgres migration](https://community.n8n.io/t/how-to-migrate-from-sqlite-to-postgres/97414) — User pain: "changing DATABASE_URL does not migrate data." Validates `mnemonic migrate` as a real need. MEDIUM confidence (community forum).
- [pgvector vs Qdrant comparison (TigerData)](https://www.tigerdata.com/blog/pgvector-vs-qdrant) — Backend selection tradeoffs: Postgres for teams already on Postgres, Qdrant for vector-native workloads. HIGH confidence (detailed technical comparison).
- [Working with data storages in Rust (Medium)](https://medium.com/@disserman/working-with-data-storages-in-rust-a1428fd9ba2c) — Rust storage trait abstraction patterns, async_trait usage. MEDIUM confidence.
- [async-fn in trait stabilized (Rust Blog)](https://blog.rust-lang.org/inside-rust/2022/11/17/async-fn-in-trait-nightly/) — Rust 1.75 stabilized async fn in traits; `async_trait` macro still needed for object-safe dyn traits with async methods. HIGH confidence (official Rust blog).
- Mnemonic `src/service.rs` — `MemoryService` struct, `Memory`, `SearchResultItem`, `ListParams`, `SearchParams` types. HIGH confidence (primary source).
- Mnemonic `src/compaction.rs` — `CompactionService`, clustering logic, `atomic_compact` semantics needed from trait. HIGH confidence (primary source).
- Mnemonic `src/config.rs` — `Config` struct, `validate_config()` pattern, `load_config()`. HIGH confidence (primary source).
- Mnemonic `src/auth.rs` — API key table design, auth middleware. HIGH confidence (primary source — auth isolation decision).

---
*Feature research for: Mnemonic v1.4 Pluggable Storage Backends*
*Researched: 2026-03-21*
