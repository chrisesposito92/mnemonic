# Phase 11: KeyService Core - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Fill in the `todo!()` stubs in `auth.rs` — `create`, `list`, `revoke`, and `validate` methods on `KeyService`. Add BLAKE3 hashing, constant-time comparison, and secure token generation. No middleware, no HTTP endpoints, no CLI — just the service-layer business logic.

</domain>

<decisions>
## Implementation Decisions

### Token Generation
- **D-01:** Raw token format: `mnk_<64 hex chars>` — 32 random bytes hex-encoded (256-bit entropy)
- **D-02:** Random source: `rand` crate with `OsRng` (cryptographically secure, standard Rust practice)
- **D-03:** Token is returned exactly once from `create()` and never stored — only the BLAKE3 hash is persisted

### Key Hashing (carried from Phase 10)
- **D-04:** BLAKE3 via `blake3` crate — 32-byte output, hex-encoded for storage in `hashed_key TEXT` column
- **D-05:** Constant-time comparison via `constant_time_eq::constant_time_eq_32()` on `[u8; 32]` — never `==` on hash values
- **D-06:** Display ID = first 8 hex chars of BLAKE3(raw_key) — not a prefix of the raw key itself (Auth Pitfall 7)

### Validation
- **D-07:** `validate()` hashes the incoming raw token with BLAKE3, queries DB for matching `hashed_key`, returns `AuthContext` on success
- **D-08:** Error granularity: descriptive messages within `DbError` — "key not found" vs "key revoked" — but both map to 401 at the API layer. No separate error variants needed.
- **D-09:** Scope enforcement (checking agent_id match) deferred to Phase 13 handler layer — `validate()` returns `AuthContext { key_id, allowed_agent_id }` and the handler decides
- **D-10:** Revoked keys: `validate()` checks `revoked_at IS NULL` in the query — a revoked key returns an error, never an AuthContext

### List Behavior
- **D-11:** `list()` returns all keys (active AND revoked) — preserves audit trail, matches D-05 soft delete decision from Phase 10
- **D-12:** Results ordered by `created_at DESC` (newest first)
- **D-13:** Never returns raw token or hashed_key — only ApiKey fields (id, name, display_id, agent_id, created_at, revoked_at)

### Revocation
- **D-14:** `revoke()` sets `revoked_at = CURRENT_TIMESTAMP` via UPDATE — does not DELETE the row
- **D-15:** Idempotent — revoking a non-existent or already-revoked key returns `Ok(())`, not an error
- **D-16:** No confirmation step — immediate effect, subsequent `validate()` calls reject the key

### Crate Dependencies
- **D-17:** Add `blake3` to Cargo.toml (pure Rust, fast, no OpenSSL)
- **D-18:** Add `constant_time_eq` to Cargo.toml (single-purpose crate for `constant_time_eq_32`)
- **D-19:** Add `rand` to Cargo.toml with `std` + `std_rng` features for `OsRng` + random byte generation

### Claude's Discretion
- Internal helper function organization (e.g., whether to extract `hash_token()` and `generate_token()` as private functions or module-level functions)
- Whether to add `#[cfg(test)]` unit tests inline in `auth.rs` or in a separate `tests/` file
- Exact SQL query structure (single query vs multiple for validation)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Auth security and pitfalls
- `.planning/research/PITFALLS.md` §Auth Pitfall 1 — Constant-time comparison requirement (CVE-2025-59425 reference)
- `.planning/research/PITFALLS.md` §Auth Pitfall 2 — Hashed storage requirement (never plaintext)
- `.planning/research/PITFALLS.md` §Auth Pitfall 7 — Display ID must be hash-derived, not key prefix
- `.planning/research/PITFALLS.md` §Auth Pitfall 9 — No in-memory cache; always hit DB for validation

### Architecture and integration points
- `.planning/research/ARCHITECTURE.md` §v1.2 System Overview — KeyService position in architecture
- `.planning/research/ARCHITECTURE.md` §Context: What Already Exists — Service construction pattern

### Requirements and success criteria
- `.planning/REQUIREMENTS.md` — KEY-01 through KEY-04, INFRA-02
- `.planning/ROADMAP.md` §Phase 11 — 5 success criteria

### Phase 10 foundation (already built)
- `.planning/phases/10-auth-schema-foundation/10-CONTEXT.md` — All D-01 through D-17 decisions

### Existing code patterns
- `src/auth.rs` — Current struct definitions with `todo!()` stubs to fill in
- `src/db.rs` — `api_keys` table DDL, `conn.call(|c| ...)` pattern for DB operations
- `src/error.rs` — `DbError` enum, `ApiError::Unauthorized` variant
- `src/server.rs` — `AppState` with `key_service: Arc<KeyService>` already wired

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `auth.rs::KeyService` — Struct with `conn: Arc<Connection>` and `count_active_keys()` already implemented. Four `todo!()` methods to fill in: `create`, `list`, `revoke`, `validate`
- `auth.rs::ApiKey` — Row struct with all fields matching `api_keys` table columns
- `auth.rs::AuthContext` — Result struct for successful validation (`key_id`, `allowed_agent_id`)
- `error.rs::DbError` — Existing error type that `KeyService` methods return
- `uuid::Uuid::now_v7()` — Already in Cargo.toml, used for primary key generation (same pattern as memory IDs)

### Established Patterns
- All DB operations go through `self.conn.call(|c| ...)` — never direct rusqlite from async context
- Services return `Result<T, DbError>` — handlers convert to `ApiError`
- Primary keys use UUID v7 (time-ordered) via `uuid::Uuid::now_v7().to_string()`
- `#[allow(dead_code)]` annotations on Phase 10 stubs — remove these as methods are implemented

### Integration Points
- `KeyService::new(conn)` — Already constructed in `main.rs` and passed to `AppState`
- `count_active_keys()` — Already used by startup auth-mode log; new methods are additive
- No changes needed to `AppState`, `main.rs`, `server.rs`, or `build_router()` in this phase

</code_context>

<specifics>
## Specific Ideas

- The `create()` method should generate the token, hash it, insert the row, and return `(ApiKey, raw_token)` — the raw token string is the caller's responsibility to display once
- The `validate()` method should do a single SQL query: `SELECT ... FROM api_keys WHERE hashed_key = ? AND revoked_at IS NULL` — one round-trip, indexed lookup
- Remove the `#[allow(dead_code)]` annotations from the four method signatures once they have real implementations
- The `blake3` and `constant_time_eq` crates are lightweight — no risk to binary size or compile time

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 11-keyservice-core*
*Context gathered: 2026-03-20*
