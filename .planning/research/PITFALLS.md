# Pitfalls Research

**Domain:** Rust pluggable storage backend abstraction — adding a trait layer over an existing single-backend (SQLite+sqlite-vec) system
**Researched:** 2026-03-21 (v1.4 pluggable backends milestone)
**Confidence:** HIGH (critical pitfalls verified against official Rust docs, async-trait crate docs, sqlite-vec API reference, pgvector GitHub issues, Qdrant documentation, and direct codebase inspection)

---

## Critical Pitfalls

### Pitfall 1: Async fn in Traits Is Not dyn-Compatible — `async_trait` Must Stay

**What goes wrong:**
Rust 1.75 stabilized `async fn` in traits, which tempts developers to remove `#[async_trait]` and write native async trait methods. The EmbeddingEngine and SummarizationEngine traits already use `#[async_trait]` and work as `Arc<dyn EmbeddingEngine>`. If a new storage trait is written with native `async fn` instead of `#[async_trait]`, it will compile for generic monomorphic code but fail the moment you try `Box<dyn StorageBackend>` or `Arc<dyn StorageBackend>`. The error message is "the trait `StorageBackend` is not dyn compatible."

**Why it happens:**
Rust 1.75's stabilization announcement says "async fn in traits" and developers assume this includes dyn compatibility. It does not. Native stabilized async fn in traits produces opaque `impl Future` return types that cannot be placed in a vtable. The `async_trait` crate works specifically because it desugars to `Pin<Box<dyn Future + Send + 'async_trait>>`, which is dyn-compatible.

**How to avoid:**
Continue using `#[async_trait]` on the storage backend trait, exactly as EmbeddingEngine does today. The pattern is already proven in the codebase. Do not attempt to use native `async fn` in the storage trait expecting it to be dyn-compatible — it will not be until the Rust team stabilizes dyn-compatible async fn (not yet done as of early 2026).

**Warning signs:**
- Compiler error: "the trait `X` is not dyn compatible" when writing `Box<dyn StorageBackend>`.
- Compiler error: "cannot be made into an object" on a trait with `async fn`.
- Confusion when `impl StorageBackend for SqliteBackend` compiles but `Arc<dyn StorageBackend>` does not.

**Phase to address:** Storage trait definition phase (the very first phase of v1.4). Get this right before writing any backend implementations.

---

### Pitfall 2: The Send Bound Problem With `async_trait`

**What goes wrong:**
`#[async_trait]` by default adds `Send` bounds to all futures returned by trait methods: `Pin<Box<dyn Future<Output = ...> + Send + 'async_trait>>`. This is correct for axum handlers and tokio multi-thread runtime. However, if an implementation uses any non-`Send` type internally (e.g., `Rc<T>`, `RefCell<T>`, `MutexGuard<T>` held across an await point), the impl will fail to compile with confusing error messages about lifetime or Send bounds. Conversely, using `#[async_trait(?Send)]` on the trait removes the Send requirement and makes `Arc<dyn StorageBackend>` fail to satisfy the `Send + Sync` bounds required by axum State.

**Why it happens:**
The Qdrant Rust client and the tokio-postgres client are async and Send-safe. The SQLite backend wraps `tokio_rusqlite::Connection` which is also Send-safe. The pitfall occurs when developers write a test stub or in-memory backend using `RefCell<Vec<Memory>>` for simplicity — this causes the entire `Arc<dyn StorageBackend>` to become non-Send, which breaks the axum State constraint at compile time.

**How to avoid:**
Use `#[async_trait]` (with Send, the default) on both the trait definition and all impl blocks. For in-memory test backends, use `tokio::sync::Mutex<Vec<Memory>>` instead of `RefCell`. Establish a compile-time test at the trait definition site: `fn _assert_send_sync<T: Send + Sync>() {}` called with the trait object type. This will surface violations immediately.

**Warning signs:**
- Impl compiles individually but `Arc<dyn StorageBackend>` fails in AppState.
- Error: "the trait bound `dyn StorageBackend: Send` is not satisfied."
- Error: "cannot be shared between threads safely."

**Phase to address:** Storage trait definition phase. Add Send+Sync compile-time assertions to `lib.rs` before any backend implementations are written.

---

### Pitfall 3: Threshold Semantics Inversion — Lower-Is-Better vs Higher-Is-Better

**What goes wrong:**
This is the most dangerous silent correctness bug in the v1.4 milestone. The current codebase has a deep semantic dependency that is easy to get wrong:

- **Search threshold (service.rs line 221):** `distance <= t` — lower distance means more similar. The `distance` field in `SearchResultItem` is a value returned by sqlite-vec's `vec0` MATCH query. sqlite-vec defaults to **L2 Euclidean distance** for `vec0` tables (cosine is opt-in via `distance_metric=cosine`). The current `vec_memories` table does NOT specify `distance_metric=cosine`, so it uses L2. Lower L2 = more similar.

- **Compaction threshold (compaction.rs line 96):** `sim >= threshold` with default 0.85 — the compaction service fetches raw embeddings from the database and computes cosine similarity directly via dot product (works because embeddings are L2-normalized). Higher similarity = more similar. The threshold logic is `>=`.

These two uses of "threshold" have **opposite directions**: search threshold is a maximum distance (lower-is-better), compaction threshold is a minimum similarity (higher-is-better). If a new backend's search implementation returns a similarity score (higher-is-better) instead of a distance score (lower-is-better), the search threshold filter will invert — very similar results will be filtered out, very dissimilar results will pass through. No test currently catches this inversion because tests set `threshold` to very permissive or very tight values that behave identically under both interpretations when extreme.

Additionally:
- **pgvector's `<=>` operator** returns cosine distance (0-2 range, lower is better, same direction as search threshold).
- **Qdrant's search API** returns a `score` where **higher is better** for cosine collections (opposite direction from search threshold).

**How to avoid:**
The storage trait must standardize on one semantic for its return type. The safest choice is to adopt what the public API surface already exposes: a `distance` field that is lower-is-better. Every backend must convert its native output to this semantic:
- SQLite vec0 with default L2: native output already lower-is-better (pass through).
- pgvector `<=>`: native output is cosine distance, lower-is-better (pass through).
- Qdrant: native output is `score` where higher-is-better (must convert: `distance = 1.0 - score` for cosine).

The compaction service fetches raw embeddings and computes similarity itself — this logic can remain backend-independent as long as the backend returns `Vec<f32>` embeddings alongside memories. The compaction threshold (similarity >= 0.85) must NOT be conflated with the search threshold.

**Warning signs:**
- New backend returns all results when threshold is set, or returns nothing when no threshold is set.
- Search results appear reversed (distant results come first).
- CLI `--threshold 0.9` returns zero results even for semantically identical content on a non-SQLite backend.
- Compaction finds no clusters after backend migration even with low threshold.

**Phase to address:** Storage trait design phase (define the return type semantics explicitly in the trait docs) and each backend implementation phase (verify with a threshold boundary test for each backend).

---

### Pitfall 4: SQLite-Specific Behavior Leaking Through the Trait (Leaky Abstraction)

**What goes wrong:**
The current MemoryService has several SQLite-specific behaviors that will leak through the trait boundary if the trait is not designed carefully:

1. **Dual-table write atomicity:** Every `create_memory` write touches both `memories` (relational) and `vec_memories` (virtual vec0 table) in a single SQLite transaction. Backends like Qdrant store the vector and metadata in a single point — there is no dual-table split. A trait method that exposes `write_to_memories_table()` and `write_to_vec_table()` as separate operations forces all backends to fake this split.

2. **The 10x KNN over-fetch pattern:** `service.rs` line 159 computes `k = limit * 10` when agent_id/session_id filters are present, because sqlite-vec KNN returns unfiltered candidates that are then filtered post-hoc in SQL. Qdrant performs filter-aware HNSW natively — it doesn't need over-fetching. A trait that exposes `k` as a required parameter leaks the SQLite workaround into all backends.

3. **Tags stored as JSON string:** SQLite stores tags as `TEXT NOT NULL DEFAULT '[]'` (JSON serialized). The LIKE '%tag%' filter on line 186 works for SQLite but is not portable. Other backends have native array or set semantics for metadata filtering.

4. **compact_runs audit table:** The compaction service writes audit records to `compact_runs` after every compaction. This table is entirely SQLite-specific. If the storage trait includes a `record_compaction_run()` method, all non-SQLite backends must implement this for an audit concern they don't natively have.

5. **Idempotent schema migration via error swallowing:** `db.rs` line 95 catches `extended_code == 1` to detect "duplicate column name." This is purely a SQLite behavior and must not appear in the storage trait.

**How to avoid:**
Design the storage trait from the consumer's perspective, not the SQLite implementation's perspective. The right interface level is: `store(memory, embedding)`, `search_semantic(embedding, limit, filters)`, `list(filters)`, `get(id)`, `delete(id)`. Keep compaction logic (fetch all + cluster in Rust) above the trait, taking raw embeddings from the backend. Keep the `compact_runs` audit table as a SQLite-only concern hidden in the SQLite implementation, or extract it to a separate optional trait. The over-fetch factor should be decided per-backend, not passed in from the service layer.

**Warning signs:**
- The trait method signature mentions `vec_memories`, `compact_runs`, `rusqlite`, or SQLite-specific types.
- A non-SQLite backend implementation has `unimplemented!()` stubs for methods that make no sense for it.
- The trait requires a `Connection` or transaction object to be passed in from outside.
- Adding a new backend requires changing the trait definition rather than just adding a new impl.

**Phase to address:** Storage trait design phase. Audit every trait method against "would Qdrant implement this differently from Postgres?" If yes, the method is exposing the wrong level of abstraction.

---

### Pitfall 5: The 239 Existing Tests All Assume Concrete SQLite Types

**What goes wrong:**
The test suite currently directly calls `mnemonic::db::open(&config)` and works with `Arc<tokio_rusqlite::Connection>`. Tests in `integration.rs` use `conn.call(|c| ...)` to seed data directly into SQLite. The auth tests in `auth.rs` access `ks.conn` directly to verify stored hash values. The compaction tests use `PRAGMA table_info(memories)` and inspect SQLite-specific column structures.

After abstracting behind a trait, all of this breaks:
- `Arc<Connection>` is no longer the concrete type in `MemoryService` — it becomes `Arc<dyn StorageBackend>`.
- Direct DB seeding via `conn.call()` no longer exists for tests against non-SQLite backends.
- Schema verification tests (`test_schema_created`) are SQLite-specific and cannot run against Qdrant.
- Tests that access `ks.conn` (a private field of `KeyService` currently typed to `Arc<Connection>`) will not compile.

This is not a minor issue. 239 tests passing is the quality bar. An abstraction that breaks 100 of them to pass 139 is not acceptable — the refactor strategy must keep them green throughout.

**How to avoid:**
Use the "branch by abstraction" pattern:
1. First, introduce the trait without changing any concrete types. The SQLite implementation becomes `SqliteBackend` implementing the trait. `MemoryService` keeps its `Arc<Connection>` internally but delegates to `SqliteBackend` behind the trait at the service boundary.
2. Make all existing tests pass against `SqliteBackend`. Do not touch test files during the trait definition phase.
3. Only after all existing tests pass, wire `MemoryService` to use `Arc<dyn StorageBackend>` and move the seeding helpers to a test-utility module that creates `SqliteBackend` directly.
4. Schema-level tests (PRAGMA, table names) belong in the `SqliteBackend` unit tests, not in service-level integration tests. Move them during the refactor, do not delete them.

The `seed_memory()` fast-path pattern used in recall/keys tests seeds via direct rusqlite — this will need a `SqliteBackend::seed_for_test()` associated function, not a trait method.

**Warning signs:**
- More than 10 tests fail after introducing the storage trait.
- Tests that previously seeded data via `conn.call()` panic with "method not found."
- `auth.rs` tests fail to access `ks.conn` after it changes type.
- Test compile time increases dramatically because trait objects require different monomorphization.

**Phase to address:** A dedicated "keep tests green" sub-phase within the storage trait introduction phase. Set a hard rule: no phase is complete until `cargo test` shows the same 239 passing tests.

---

### Pitfall 6: Auth Keys Stored in SQLite — Backend Selection Breaks the Auth Assumption

**What goes wrong:**
The `api_keys` table lives in SQLite alongside the memories tables. The `KeyService` holds an `Arc<tokio_rusqlite::Connection>` and reads/writes API keys via SQL. If a user switches to Qdrant or Postgres for memory storage, there are two dangerous assumptions that break:

1. **Auth keys will not move:** If the pluggable backend applies only to memory storage, auth keys stay in SQLite. This means even a user with `backend=qdrant` in config still needs a SQLite file for auth. This must be communicated explicitly to users — it is not a bug but it will be unexpected.

2. **Auth keys could accidentally move:** If the storage backend abstraction is too broad (e.g., includes auth key operations in the `StorageBackend` trait), a misconfigured Qdrant backend could receive auth key lookups. Qdrant has no concept of `hashed_key`-based auth tables — `unimplemented!()` in the trait impl would cause a runtime panic on every auth request.

3. **The per-request `count_active_keys()` call in auth_middleware:** This runs on every protected request. If auth keys were ever moved to a remote backend (Qdrant or Postgres over network), this becomes a per-request network call. At 100 req/s, that is 100 auth-only network round trips per second that didn't exist before.

**How to avoid:**
Keep auth key storage permanently in SQLite, independent of the memory storage backend. The `StorageBackend` trait should cover only memory operations. `KeyService` keeps its `Arc<tokio_rusqlite::Connection>` and is explicitly not part of the pluggable backend. Document this decision clearly: "auth keys always use local SQLite; only memory storage is pluggable." The config system should have `backend=` only configure the memory storage, not auth.

**Warning signs:**
- The `StorageBackend` trait has a `create_api_key()` or `validate_api_key()` method.
- `KeyService` is refactored to hold `Arc<dyn StorageBackend>` instead of `Arc<Connection>`.
- Config accepts `backend=qdrant` but user can no longer create API keys after switching.

**Phase to address:** Storage trait design phase. Explicitly exclude auth operations from the trait scope with a code comment explaining the decision.

---

### Pitfall 7: Compaction's Embedding Fetch Assumes Stored Raw Bytes in vec_memories

**What goes wrong:**
`compaction.rs` line 187-216 fetches candidate embeddings as raw bytes from `vec_memories` and reinterprets them via unsafe pointer casting (`std::slice::from_raw_parts` as `*const f32`). This works because sqlite-vec stores float32 embeddings as raw IEEE-754 little-endian bytes.

If compaction is moved behind the storage trait, this coupling breaks in two ways:
1. Backends that store embeddings as `Vec<f32>` directly (Qdrant stores vectors natively, Postgres with pgvector stores vectors in a typed column) cannot return raw bytes — the deserialization logic is backend-specific.
2. If the trait exposes embeddings as `Vec<f32>` uniformly, the unsafe byte cast in `fetch_candidates` must be removed. The current code is the only place in the codebase with `unsafe` beyond the sqlite-vec extension registration.

There is also a subtle endianness assumption: the cast `std::slice::from_raw_parts(bytes.as_ptr() as *const f32, bytes.len() / 4)` assumes the system is little-endian (x86/ARM). This will produce wrong similarity values on a big-endian architecture even with the same SQLite backend.

**How to avoid:**
The storage trait's "fetch candidates for compaction" method should return `Vec<(Memory, Vec<f32>)>` — memories with their embeddings already deserialized to `Vec<f32>`. Each backend is responsible for its own embedding deserialization. The unsafe byte cast stays inside `SqliteBackend::fetch_candidates_with_embeddings()` and is not exposed through the trait. The compaction service remains backend-agnostic: it receives `Vec<(Memory, Vec<f32>)>` and runs the Rust-side clustering algorithm unchanged.

**Warning signs:**
- The storage trait has a `fetch_raw_embedding_bytes()` method returning `Vec<u8>`.
- The unsafe byte cast appears anywhere except inside `SqliteBackend`.
- Compaction produces wildly wrong similarity values after switching backends.
- Tests pass on CI (x86) but produce wrong clusters on ARM if endianness handling is wrong (unlikely — ARM is also little-endian, but log the assumption explicitly).

**Phase to address:** Storage trait design phase, and explicitly in the compaction refactor phase (separate from the basic CRUD trait).

---

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Feature-flag the new trait behind `#[cfg(feature = "pluggable-backends")]` | Keeps existing code unchanged | Two code paths to maintain; test matrix doubles | Never — use branch-by-abstraction instead |
| Add a `backend: String` field to Config and match/dispatch in MemoryService | No trait required, simple | Match arms grow with every new backend; logic scattered everywhere | Never — this is exactly what traits prevent |
| Store auth keys in the same backend as memories | Unified storage | Auth fails when backend is misconfigured; security-critical path exposed to new code | Never |
| Make the storage trait take `&self` (immutable) for all methods including writes | Cleaner API, easy to implement | Cannot use interior mutability; write contention impossible to reason about | Only for read-only backends |
| Include `compact_runs` in the storage trait | Audit log preserved across backends | All backends must implement a table that only SQLite needs | Never — keep audit in SQLite-only impl |
| Return `serde_json::Value` from trait methods instead of typed structs | Maximum flexibility | Defeats Rust's type safety; runtime panics replace compile errors | Never |
| Put all backends in the same crate | Simpler module structure | Qdrant client becomes a required dependency even when not used | Acceptable for v1.4, revisit if binary size grows |

---

## Integration Gotchas

Common mistakes when connecting to external services.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Qdrant | Create collection with `Distance::Cosine` but compare scores as distances (lower-is-better) | Qdrant cosine collections return scores where higher=better; convert: `distance = 1.0 - score` before storing in `SearchResultItem.distance` |
| pgvector | Use `<->` operator (L2) instead of `<=>` (cosine) for normalized embeddings | Use `<=>` (cosine distance) for all-MiniLM-L6-v2 normalized embeddings; L2 and cosine give different threshold semantics |
| pgvector | Assume `<=>` returns range 0-1 | `<=>` returns 0-2 (cosine distance); threshold values that work for similarity scores (0-1) will silently pass everything through |
| Qdrant | Use REST client and assume payload filtering is free | Qdrant's payload filtering uses indexed fields; create payload indexes for `agent_id` and `session_id` before filtering at scale |
| Qdrant | Send 384-dim embeddings to a collection created with 768-dim | Vector dimension mismatch returns a Qdrant API error, not a Rust compile error; validate dim at startup |
| tokio-postgres | Use a single connection for both reads and writes from multiple async tasks | Use a connection pool (deadpool-postgres or tokio-postgres built-in pool); unlike SQLite, Postgres handles concurrent writers safely |
| All backends | Assume `agent_id = ""` (empty string, the default) filters correctly | Some backends treat empty string and null/absent differently; test explicitly with empty-string agent_id after each backend implementation |

---

## Performance Traps

Patterns that work at small scale but fail as usage grows.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Per-request auth `count_active_keys()` on remote backend | Auth adds 5-20ms latency per request when backend is Qdrant/Postgres | Keep auth keys in SQLite regardless of backend; never route auth through the pluggable storage trait | First request after switching auth to remote backend |
| KNN over-fetch (10x) on Qdrant | Fetching 10x candidates from Qdrant at network cost | Use Qdrant's native payload filtering HNSW instead of post-filtering; pass filters into the Qdrant query directly | N > 100 candidates or >10 req/s with filters |
| Single Postgres connection for writes | `connection pool exhausted` errors under concurrent store operations | Use deadpool-postgres with pool_size ≥ 10; unlike SQLite, Postgres benefits from multiple connections | > 5 concurrent agent write sessions |
| Embedding vector stored in both Postgres memories table and pgvector | Double storage, sync problems on delete | Use pgvector as the single source: store embedding in a vector column on the memories table, not a separate table | Day 1 — architectural decision |
| Re-embedding every candidate during compaction on a remote backend | Compaction fetches N memories + N embeddings from remote, then re-embeds | Fetch embeddings from backend (avoid re-embedding); only re-embed the merged output | N > 50 memories, compaction latency becomes seconds |

---

## Security Mistakes

Domain-specific security issues beyond general web security.

| Mistake | Risk | Prevention |
|---------|------|------------|
| Storing raw API key tokens in Qdrant or Postgres payload | Key exposure in DB dump or log | Auth keys must stay in SQLite with BLAKE3 hashed storage; never route auth through the pluggable backend |
| Using Qdrant's gRPC API without TLS in production | Token/key interception in transit | Configure TLS on the Qdrant client; or use Qdrant Cloud which enforces TLS |
| Passing `agent_id` from user-controlled input directly as a Qdrant filter without validation | Cross-agent data access if validation is missing | Validate agent_id scope in the service layer before passing to backend, same as current `enforce_scope()` in server.rs |
| Logging raw Qdrant API responses | API keys or vector data in logs | Ensure tracing spans don't log full response bodies from external storage backends |

---

## UX Pitfalls

Common user experience mistakes in this domain.

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Switching backends silently loses all existing memories | User loses all history without warning | Require explicit `--migrate` flag or document that backend switch is not a migration; memories stay where they are |
| Config error (wrong Qdrant URL) shows cryptic Rust error at request time | User doesn't know backend failed | Validate backend connectivity at startup in `validate_config()`, same as current embedding provider validation |
| `mnemonic config` shows backend=qdrant even when Qdrant is unreachable | Confuses user about system state | `mnemonic config` should probe the configured backend and show connectivity status |
| Different search results between SQLite and Qdrant for the same query | User confusion — "why did search get worse?" | Document that L2 distance (SQLite default) and cosine distance (Qdrant) rank results slightly differently for non-normalized vectors |

---

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces.

- [ ] **Storage trait compiled:** Verify `Arc<dyn StorageBackend + Send + Sync>` compiles and can be passed to axum State — not just that `impl StorageBackend for SqliteBackend` compiles.
- [ ] **SQLite backend parity:** Every existing test (239) passes with the SQLite backend behind the trait, not just the new backend-specific tests.
- [ ] **Threshold semantics documented:** The `StorageBackend::search_semantic()` method must have a doc comment explicitly stating whether the returned distance is lower-is-better or higher-is-better.
- [ ] **Qdrant score converted:** Qdrant returns `score` (higher-is-better for cosine); verify conversion to `distance` (lower-is-better) is applied before the threshold filter in service.rs.
- [ ] **Auth keys stay in SQLite:** Run `mnemonic keys create` after switching to Qdrant backend; verify keys are still persisted and auth middleware still works.
- [ ] **Compaction works across backends:** Run `POST /memories/compact` against Qdrant backend and verify clusters are found; this requires the embedding-fetch path to work through the trait.
- [ ] **Empty agent_id tested:** Test that `agent_id = ""` (default namespace) works correctly for all backends, not just `agent_id = "some-agent"`.
- [ ] **validate_config() extended:** Backend URL and credentials are validated at startup, not at first request.
- [ ] **binary size regression checked:** Adding Qdrant and tokio-postgres client crates will increase binary size significantly; measure before and after and document for users.

---

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Trait is not dyn-compatible | LOW | Add `#[async_trait]` to trait and all impl blocks; remove native `async fn` from trait |
| Threshold semantics inverted on one backend | MEDIUM | Add a `score_is_higher_is_better: bool` flag in the backend's search response and normalize in the service layer; re-run all search integration tests |
| 239 tests broken by trait refactor | HIGH | Revert trait changes; apply branch-by-abstraction incrementally; never introduce the trait and change service types in the same commit |
| Auth keys accidentally moved to non-SQLite backend | HIGH | Roll back config change; restore SQLite file from backup; add explicit guard in `validate_config()` that rejects `backend=X` for auth key operations |
| Compaction produces wrong clusters on new backend | MEDIUM | Add a canary test: insert two identical memories, run compact, assert exactly 1 cluster found; run this test for each backend |
| Binary too large after adding all backend crates | LOW | Use Cargo features to gate each backend behind a feature flag; `default = ["sqlite"]`; `qdrant` and `postgres` are opt-in features that add their crates only when enabled |

---

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Async fn not dyn-compatible | Storage trait definition (Phase 1) | `Arc<dyn StorageBackend + Send + Sync>` compiles as AppState field |
| Send bound breaks AppState | Storage trait definition (Phase 1) | In-memory test backend using `tokio::sync::Mutex` compiles in AppState |
| Threshold semantics inversion | Storage trait design + each backend impl | Threshold boundary test: insert 2 identical memories, search with threshold=0.01, expect 2 results; insert 2 unrelated, search with threshold=0.01, expect 0 results |
| Leaky SQLite abstraction | Storage trait design (Phase 1) | Zero mentions of `rusqlite`, `vec_memories`, `compact_runs` in the trait definition |
| 239 tests broken | SQLite backend implementation (Phase 2) | `cargo test` after Phase 2 = 239 passing, 0 failing |
| Auth key backend coupling | Storage trait design (Phase 1) | `KeyService` type unchanged; `StorageBackend` trait has no auth methods |
| Compaction embedding fetch | Storage trait design (Phase 1) + compaction refactor | Compaction returns correct clusters on a fresh test backend with two identical mock embeddings |
| Qdrant score direction | Qdrant backend implementation | Search for identical content with `threshold=0.1`, expect results; search for opposite content, expect empty |
| pgvector operator choice | Postgres backend implementation | Confirm `<=>` operator is used, `<->` is not; check query plan uses HNSW index |
| Config validation | Config extension phase | Starting with invalid Qdrant URL exits with clear error, does not silently degrade |

---

## Sources

- [Rust async-trait crate documentation](https://docs.rs/async-trait/0.1.83/async_trait/index.html)
- [Rust Blog: Announcing async fn and RPIT in traits (1.75)](https://blog.rust-lang.org/2023/12/21/async-fn-rpit-in-traits/)
- [sqlite-vec KNN query documentation](https://alexgarcia.xyz/sqlite-vec/features/knn.html)
- [sqlite-vec API reference — distance metrics](https://alexgarcia.xyz/sqlite-vec/api-reference.html)
- [pgvector GitHub issue: cosine distance vs cosine similarity](https://github.com/pgvector/pgvector/issues/72)
- [Supabase issue: `<=>` is cosine distance, not cosine similarity](https://github.com/supabase/supabase/issues/12244)
- [Qdrant distance metrics documentation](https://qdrant.tech/course/essentials/day-1/distance-metrics/)
- [Qdrant vector search filtering guide](https://qdrant.tech/articles/vector-search-filtering/)
- [Rust Forum: sharing database transactions across trait methods](https://users.rust-lang.org/t/how-do-i-share-a-database-transaction-across-trait-methods/101606)
- [Rust Forum: async trait dyn object safety](https://users.rust-lang.org/t/resolving-not-object-safe-error-with-trait-having-async-methods/105175)
- Direct codebase inspection: `src/service.rs`, `src/compaction.rs`, `src/auth.rs`, `src/db.rs`, `src/embedding.rs`

---
*Pitfalls research for: v1.4 Pluggable Storage Backends — Mnemonic*
*Researched: 2026-03-21*
