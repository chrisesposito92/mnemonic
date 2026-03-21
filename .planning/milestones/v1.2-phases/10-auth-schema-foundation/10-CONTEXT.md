# Phase 10: Auth Schema Foundation - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Create the DB table (`api_keys`), error variant (`Unauthorized`), module skeleton (`auth.rs`), and startup auth-mode log that all subsequent auth phases build on. No middleware, no key generation logic, no HTTP wiring — just the foundation.

</domain>

<decisions>
## Implementation Decisions

### Schema Columns
- **D-01:** Use BLAKE3 (not SHA-256) for key hashing — `blake3` crate, 32-byte output, hex-encoded for storage. Faster, pure Rust, no OpenSSL.
- **D-02:** Use `constant_time_eq` crate (not `subtle`) for hash comparison — `constant_time_eq_32()` on `[u8; 32]`.
- **D-03:** Display ID column stores the first 8 hex characters of BLAKE3(raw_key) — NOT a prefix of the raw key itself. Prevents enumeration (Auth Pitfall 7).
- **D-04:** `agent_id TEXT` with NULL = wildcard (any agent). No sentinel strings.
- **D-05:** Soft delete via `revoked_at DATETIME` — NULL = active. Preserves audit trail. Idempotent if revoked twice.
- **D-06:** Schema uses `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT EXISTS` — idempotent migration, safe on existing v1.1 databases.

### 401 Error Response
- **D-07:** Add `ApiError::Unauthorized(String)` variant — separate from BadRequest/NotFound/Internal. Maps to HTTP 401.
- **D-08:** 401 response body is detailed: `{ "error": "unauthorized", "auth_mode": "active", "hint": "Provide Authorization: Bearer mnk_..." }`. Helps agent developers debug.
- **D-09:** No trace/request ID in error responses — keep it simple, consistent with existing patterns.
- **D-10:** Only add `Unauthorized` variant now. `Forbidden` (403) deferred to Phase 13 when scope enforcement is wired.

### Startup Auth Log
- **D-11:** Log auth mode at startup (INFO level) with actionable hint: `"Auth: OPEN (no keys) — run 'mnemonic keys create' to enable"` or `"Auth: ACTIVE (3 keys)"`.
- **D-12:** Check active key count during server init via DB query — log once at startup.
- **D-13:** INFO level for both open and active modes — consistent with existing startup log messages (bind address, etc.).

### Module Skeleton Scope
- **D-14:** `auth.rs` contains `AuthContext`, `ApiKey` structs with real fields, and `KeyService` struct with method signatures returning `todo!()`. Enough scaffolding for Phase 11 to fill in.
- **D-15:** Crypto helpers (BLAKE3 hashing, token generation) deferred to Phase 11 — not part of foundation.
- **D-16:** Add `key_service: Arc<KeyService>` to `AppState` in Phase 10. Avoids breaking AppState construction in later phases.
- **D-17:** Implement real `KeyService::count_active_keys()` function — queries DB, used by startup log. First real auth function, tested against empty DB returning 0.

### Claude's Discretion
- Exact column ordering in DDL
- Whether to add `last_used_at DATETIME` column now or defer to Phase 11
- Index strategy beyond `hashed_key` (e.g., whether to index `agent_id` on api_keys)
- Exact wording of startup log messages

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Auth schema and pitfalls
- `.planning/research/PITFALLS.md` §Auth Pitfall 1-10 — Security pitfalls for auth implementation, especially Pitfall 7 (display ID), Pitfall 8 (migration), Pitfall 10 (open mode + invalid token)
- `.planning/research/ARCHITECTURE.md` §SQLite Schema: `api_keys` Table — Reference DDL (NOTE: override `prefix` with hash-derived `display_id`, and SHA-256 with BLAKE3 per decisions D-01 and D-03)
- `.planning/research/ARCHITECTURE.md` §Build Order — Phase dependency chain

### Requirements
- `.planning/REQUIREMENTS.md` — INFRA-01 (idempotent migration), INFRA-03 (startup log)
- `.planning/ROADMAP.md` §Phase 10 — Success criteria (4 items)

### Existing patterns
- `src/db.rs` — Existing idempotent migration pattern (CREATE TABLE IF NOT EXISTS, ALTER TABLE error-swallowing)
- `src/error.rs` — Existing ApiError enum and IntoResponse impl
- `src/server.rs` — Existing AppState struct and build_router()
- `src/lib.rs` — Module declaration pattern

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `db.rs::open()` — Idempotent DDL pattern with `execute_batch` + `ALTER TABLE` error-swallowing for v1.1 columns. New `api_keys` table follows the same `CREATE TABLE IF NOT EXISTS` approach.
- `error.rs::ApiError` — Existing enum with `IntoResponse` impl. Adding `Unauthorized(String)` variant follows the exact same pattern as `BadRequest(String)`.
- `server.rs::AppState` — Existing struct with `Arc<MemoryService>` and `Arc<CompactionService>`. Adding `Arc<KeyService>` is additive.

### Established Patterns
- All DB operations go through `conn.call(|c| ...)` — never direct rusqlite from async context.
- Error types use `thiserror::Error` derive macro.
- Services are constructed in `main.rs` and passed as `Arc<T>` to `AppState`.
- `lib.rs` declares all modules with `pub mod name;`.

### Integration Points
- `db.rs::open()` — Add `api_keys` DDL block inside the existing `execute_batch` call.
- `error.rs` — Add `Unauthorized` variant to `ApiError` enum and update `IntoResponse` impl.
- `lib.rs` — Add `pub mod auth;`.
- `server.rs::AppState` — Add `key_service` field.
- `main.rs` — Construct `KeyService` and pass to `AppState`. Add startup auth-mode log after DB open.

</code_context>

<specifics>
## Specific Ideas

- The `api_keys` DDL should be added to the same `execute_batch` call as existing tables — single atomic schema init.
- The architecture research schema needs adaptation: rename `prefix` → `display_id`, change hash from SHA-256 to BLAKE3, keep all other columns.
- The 401 response body format follows the JSON pattern already used by `BadRequest`: `(status, Json(json!({"error": ...})))`.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 10-auth-schema-foundation*
*Context gathered: 2026-03-20*
