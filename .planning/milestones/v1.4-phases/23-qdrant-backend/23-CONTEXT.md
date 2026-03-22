# Phase 23: Qdrant Backend - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Implement `QdrantBackend` behind the `backend-qdrant` feature flag, satisfying all 7 `StorageBackend` trait methods using the `qdrant-client` gRPC crate. Wire it into the `create_backend()` factory (replacing the current `todo!()` stub). All memory CRUD, semantic search, and compaction operations must work against a Qdrant instance. Multi-agent namespace isolation via payload filtering. Score normalization to match the trait's lower-is-better distance contract. Compaction uses documented non-transactional semantics (upsert-then-delete). The default binary (without `--features backend-qdrant`) must remain unchanged — zero new dependencies.

</domain>

<decisions>
## Implementation Decisions

### Qdrant collection schema
- **D-01:** Single collection named `mnemonic_memories` — all agents share one collection, isolated by `agent_id` payload filter (per QDRT-04)
- **D-02:** Vector config: dimension 384 (matching all-MiniLM-L6-v2 bundled model), cosine distance metric. If the user switches to OpenAI embeddings (1536-dim), they must recreate the collection — document this clearly
- **D-03:** Payload fields stored per point: `id` (String, our UUID v7), `content` (String), `agent_id` (String, indexed), `session_id` (String, indexed), `tags` (String[], keyword-indexed), `embedding_model` (String), `created_at` (String, ISO 8601), `updated_at` (String or null)
- **D-04:** Create payload indexes on `agent_id`, `session_id`, and `tags` at collection creation time for efficient filtering
- **D-05:** Collection auto-creation: `QdrantBackend::new()` checks if collection exists and creates it if not (with correct vector config and indexes). This is idempotent — safe to call on every startup

### Point ID mapping
- **D-06:** Qdrant point IDs use the UUID string format directly — qdrant-client supports string point IDs (UUID format). Our UUID v7 IDs are valid UUIDs so they map 1:1 with no conversion needed
- **D-07:** `get_by_id` and `delete` use point ID lookup by our string ID. No secondary index needed

### Score-to-distance conversion (QDRT-02)
- **D-08:** Qdrant cosine similarity returns scores in [0, 2] range (cosine distance = 1 - cosine_similarity, but Qdrant returns the raw similarity). Convert via `distance = 1.0 - score` to match the trait's lower-is-better contract
- **D-09:** This conversion is applied in the `search()` method before returning results. The threshold comparison in the caller already uses lower-is-better semantics, so no change needed there

### Compaction semantics (QDRT-03)
- **D-10:** `write_compaction_result` uses upsert-first-then-delete order: upsert the merged point, then delete source points. Rationale: if deletion fails after upsert, we have a duplicate (recoverable) rather than data loss (irrecoverable)
- **D-11:** Each step (upsert, delete) is a separate Qdrant API call — Qdrant has no multi-operation transaction. Document this clearly in the method's doc comment
- **D-12:** On partial failure (upsert succeeds, delete fails): return the error. The merged memory exists but sources remain. Next compaction run will see duplicates but won't lose data. Log a warning

### List and pagination
- **D-13:** `list()` uses Qdrant's scroll API with payload filters for agent_id, session_id, tag, and date range
- **D-14:** For `offset`/`limit` pagination: use scroll with `offset` parameter (PointId-based offset). For the first page, no offset. For subsequent pages, the scroll API returns an `offset` token for the next page. Since our trait uses integer offset/limit, implement by scrolling and skipping — acceptable for typical page sizes (20-100)
- **D-15:** Ordering: `list()` returns results ordered by `created_at DESC`. Since Qdrant scroll doesn't natively sort by payload, fetch with scroll, then sort client-side. For typical memory counts per agent (<10K), this is acceptable. Document the performance note

### Search implementation
- **D-16:** `search()` uses Qdrant's search/query API with the pre-computed embedding vector, applying payload filters for agent_id, session_id, tag, and date range
- **D-17:** The SQLite-specific 10x over-fetch CTE pattern is NOT needed for Qdrant — Qdrant applies filters during search natively. Just pass the limit directly
- **D-18:** `threshold` filtering: apply after score-to-distance conversion, same as SqliteBackend

### fetch_candidates implementation
- **D-19:** Use Qdrant scroll with `agent_id` filter and `with_vectors: true` to retrieve candidate points with their embeddings for compaction
- **D-20:** Limit to `max_candidates + 1` (same over-fetch-by-one pattern as SqliteBackend) to detect truncation
- **D-21:** Sort by `created_at DESC` client-side after scroll retrieval (matching SqliteBackend behavior)

### Module structure and feature gating
- **D-22:** New file `src/storage/qdrant.rs` containing `QdrantBackend` struct and `StorageBackend` impl, behind `#[cfg(feature = "backend-qdrant")]`
- **D-23:** `src/storage/mod.rs` conditionally declares `pub mod qdrant;` behind `#[cfg(feature = "backend-qdrant")]` and conditionally re-exports `QdrantBackend`
- **D-24:** `Cargo.toml` feature `backend-qdrant` gains dependency: `qdrant-client = { version = "1", optional = true }` — only pulled when feature is enabled
- **D-25:** Wire `QdrantBackend::new(&config).await?` into the `create_backend()` factory's `"qdrant"` arm, replacing the `todo!()`

### QdrantBackend construction
- **D-26:** `QdrantBackend::new(config: &Config) -> Result<Self, ApiError>` reads `config.qdrant_url` and optional `config.qdrant_api_key`, constructs a `QdrantClient`, verifies connectivity (list_collections or health check), and ensures collection exists with correct schema
- **D-27:** Store the `QdrantClient` in the struct (it's already `Send + Sync`). No Arc wrapping needed — the client handles connection pooling internally

### Testing strategy
- **D-28:** Unit tests behind `#[cfg(test)]` in `qdrant.rs` test individual helper functions (score conversion, payload construction, filter building) without needing a live Qdrant instance
- **D-29:** Integration tests behind `#[cfg(all(test, feature = "backend-qdrant"))]` require a running Qdrant instance. These are NOT run in default `cargo test` — only when explicitly testing the qdrant feature. Document how to run: `docker run -p 6333:6333 -p 6334:6334 qdrant/qdrant` + `cargo test --features backend-qdrant`
- **D-30:** The existing 273 tests must still pass unchanged when built without `--features backend-qdrant`

### Claude's Discretion
- Exact qdrant-client version (latest stable 1.x)
- Internal helper function decomposition within QdrantBackend
- Error message wording for Qdrant connection failures
- Whether to add a `collection_name` config field or hardcode `mnemonic_memories`
- Payload serialization approach (json_value vs typed fields)
- Whether scroll-based list() needs a practical limit warning in docs

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — QDRT-01 through QDRT-04 define the Qdrant backend requirements
- `.planning/ROADMAP.md` Phase 23 section — success criteria and goal

### StorageBackend contract
- `src/storage/mod.rs` — StorageBackend trait definition, all 7 method signatures, distance semantics contract (lower-is-better), shared types (StoreRequest, CandidateRecord, MergedMemoryRequest)
- `src/storage/sqlite.rs` — Reference implementation: SqliteBackend shows exact method behavior, return types, error handling, and edge cases (NotFound, pagination, threshold filtering)

### Factory wiring point
- `src/storage/mod.rs` lines 110-121 — `create_backend()` "qdrant" arm with `todo!()` stub to replace
- `Cargo.toml` lines 14-15 — `backend-qdrant = []` feature declaration to add qdrant-client dependency

### Config fields (from Phase 22)
- `src/config.rs` — `qdrant_url: Option<String>`, `qdrant_api_key: Option<String>` fields; `validate_config()` already checks qdrant_url is present when storage_provider is "qdrant"

### Error types
- `src/error.rs` — ApiError, MnemonicError, ConfigError types used by StorageBackend returns

### Prior decisions
- `.planning/phases/21-storage-trait-and-sqlite-backend/21-CONTEXT.md` — D-02: distance contract, D-06: feature gate strategy, D-07/D-08: compact_runs audit stays in separate SQLite
- `.planning/phases/22-config-extension-backend-factory-and-config-cli/22-CONTEXT.md` — D-12: feature gate error at create_backend time, D-15: factory accepts sqlite_conn but non-sqlite backends ignore it
- `.planning/STATE.md` "Accumulated Context > Decisions" — all cross-phase decisions

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `SqliteBackend` in `src/storage/sqlite.rs`: Reference implementation for all 7 trait methods — QdrantBackend must produce identical output types and semantics
- `create_backend()` in `src/storage/mod.rs`: Factory function with `todo!()` stub ready for QdrantBackend wiring
- `Config` fields in `src/config.rs`: `qdrant_url` and `qdrant_api_key` already validated by `validate_config()`
- Feature flag `backend-qdrant` in `Cargo.toml`: Already declared as empty feature — needs qdrant-client dependency added

### Established Patterns
- **#[async_trait] on StorageBackend**: QdrantBackend must use the same pattern
- **Arc<dyn StorageBackend>**: QdrantBackend is used as `Arc::new(QdrantBackend::new(...).await?)`
- **ApiError returns**: All trait methods return `Result<T, ApiError>` — map qdrant-client errors to ApiError::Internal
- **Score semantics**: Trait contract is lower-is-better distance; Qdrant's cosine similarity is higher-is-better — convert via `1.0 - score`
- **Per-cluster compaction**: `write_compaction_result()` called once per cluster, not all-at-once — matches Qdrant's per-point API naturally

### Integration Points
- `src/storage/mod.rs`: Add `#[cfg(feature = "backend-qdrant")] pub mod qdrant;` and conditional re-export
- `src/storage/mod.rs` create_backend(): Replace `todo!()` with `QdrantBackend::new(config).await`
- `Cargo.toml`: Add `qdrant-client` as optional dependency under `backend-qdrant` feature
- `src/storage/qdrant.rs`: New file — entire QdrantBackend implementation

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. The SqliteBackend in sqlite.rs is the definitive reference for expected behavior. Qdrant-specific patterns (scroll API, payload filtering, score conversion) should follow qdrant-client crate idioms.

</specifics>

<deferred>
## Deferred Ideas

- Collection name as a config field (`qdrant_collection`) — hardcode `mnemonic_memories` for now, can add config later if users need multi-tenant separation
- Vector dimension auto-detection from first stored memory — adds complexity, explicit 384-dim config is clearer
- Qdrant API key rotation / refresh — out of scope for initial backend
- Qdrant cluster mode / distributed deployment docs — single-node is sufficient for v1.4
- Backend health ping in `/health` endpoint (connection status, not just name) — deferred per Phase 22 D-22

</deferred>

---

*Phase: 23-qdrant-backend*
*Context gathered: 2026-03-21*
