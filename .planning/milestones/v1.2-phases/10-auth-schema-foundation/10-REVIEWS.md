---
phase: 10
reviewers: [gemini, codex]
reviewed_at: 2026-03-20T00:00:00Z
plans_reviewed: [10-01-PLAN.md, 10-02-PLAN.md]
---

# Cross-AI Plan Review — Phase 10

## Gemini Review

### 1. Summary
Overall, the plans for Phase 10 are well-structured, safely scoped, and adhere closely to the architectural decisions outlined in the context. The division of labor between foundational schema/stubs (10-01) and system wiring/testing (10-02) is logical and minimizes merge conflicts. However, there is a significant Rust module resolution issue between the two plans, as well as a minor redundancy regarding SQLite index creation, that need to be addressed before implementation.

### 2. Strengths
* **Strict Scope Management:** Excellent adherence to the "deferred" constraints (e.g., deferring crypto helpers, the `Forbidden` variant, and the `last_used_at` column to later phases).
* **Robust Testing Strategy:** Using `PRAGMA table_info` to verify the schema and explicitly testing the idempotent nature of the migration ensures the "safe to upgrade" requirement is mathematically proven in CI.
* **Graceful Degradation:** Implementing the `todo!()` stubs with concrete return types (`Result<T, DbError>`) instead of `-> !` ensures the application will compile cleanly without cascading type errors in Phase 11.
* **Clear State Management:** Wiring the `Arc<KeyService>` into the existing `AppState` guarantees that database connections and auth logic will be thread-safe and easily accessible to axum handlers.

### 3. Concerns
* **[HIGH] Double Module Declaration:** Plan 10-01 adds `pub mod auth;` to `lib.rs`, but Plan 10-02 explicitly states to add `mod auth;` to `main.rs`. In Rust, declaring a module in both the library root and binary root causes the compiler to build the module twice as two distinct crates. This will lead to confusing type mismatch errors (e.g., `main.rs`'s `KeyService` will not match `lib.rs`'s `KeyService`).
* **[MEDIUM] Redundant SQLite Index:** Plan 10-01 specifies adding `idx_api_keys_hashed_key`. Because the `hashed_key` column is defined as `TEXT NOT NULL UNIQUE`, SQLite automatically creates a unique B-tree index behind the scenes. Manually creating another index on this column is redundant, wastes storage, and slows down writes.
* **[LOW] Incomplete DDL Details in Plan Text:** While the context mandates `CREATE TABLE IF NOT EXISTS` (D-06), Plan 10-01's text omits explicitly mentioning this clause. If an engineer strictly follows the bullet point, they might write a hard `CREATE TABLE` that crashes on a v1.1 database.

### 4. Suggestions
* **Fix the Module Tree:** Remove the instruction to add `mod auth;` to `main.rs` in Plan 10-02. Instead, instruct `main.rs` to import the module exposed by the library crate (e.g., `use mnemonic::auth::KeyService;` or `use crate::auth::KeyService;` depending on your workspace setup).
* **Drop the Redundant Index:** Update Plan 10-01 to only create the index for `agent_id` (`idx_api_keys_agent_id`). Rely on the `UNIQUE` constraint to automatically index `hashed_key`.
* **Explicit `IF NOT EXISTS`:** Explicitly state in Plan 10-01 Task 1 that both the table and the `agent_id` index must be created using the `IF NOT EXISTS` syntax to guarantee idempotency.
* **Specify the Count Query:** Ensure the implementation of `count_active_keys()` in Plan 10-01 specifically uses the query `SELECT COUNT(*) FROM api_keys WHERE revoked_at IS NULL`, directly honoring the soft-delete logic defined in D-05.

### 5. Risk Assessment
**Risk Level: MEDIUM**

*Justification:* While the logic and database designs are sound, the double module declaration (`mod auth;` in both `lib.rs` and `main.rs`) is a classic Rust pitfall that will cause immediate compilation failure during Phase 10-02. Once the module declaration issue is fixed, the risk drops to **LOW**, as the schema changes are additive, non-breaking, and thoroughly covered by the proposed integration tests.

---

## Codex Review

### PLAN 10-01 Review

#### Summary
This plan is close to the right scope for a schema-foundation phase: it adds the migration, error variant, and auth module skeleton without prematurely implementing full auth behavior. The main risks are around a few design details that could create avoidable friction later: a likely redundant index on `hashed_key`, uncertainty around the proposed `KeyService` connection type, and whether `auth.rs` should suppress dead-code warnings at the module level instead of shaping the stubs to compile cleanly naturally.

#### Strengths
- Keeps scope aligned to the phase goal: schema, error infrastructure, and module foundation only.
- Uses `CREATE TABLE IF NOT EXISTS`, which matches the upgrade-safety requirement.
- Defers crypto and validation behavior to Phase 11 instead of leaking implementation into the schema phase.
- Adds a dedicated `Unauthorized` variant now without prematurely introducing `Forbidden`.
- Explicitly gives stub methods concrete signatures, which reduces churn in later phases.
- Includes a real `count_active_keys()` implementation, which directly supports the startup auth-mode requirement.

#### Concerns
- `HIGH`: `hashed_key TEXT NOT NULL UNIQUE` plus a separate `idx_api_keys_hashed_key` is probably redundant in SQLite. A `UNIQUE` constraint already creates an index. Keeping both adds write overhead and schema noise for no benefit.
- `HIGH`: `KeyService struct: conn (Arc<Connection>)` may not match the project's real DB abstraction. If the app currently uses a wrapper, pool, or `tokio_rusqlite::Connection` ownership pattern, locking the service to `Arc<Connection>` in this phase can force a refactor in Phase 10-02.
- `MEDIUM`: `#![allow(dead_code)]` at the module level is a broad suppression. It solves the warning issue, but it can also hide genuinely unused or drifting code later.
- `MEDIUM`: The plan says to insert DDL "after compact_runs index" inside an existing `execute_batch` call. That is fine only if the migration model is strictly bootstrap DDL. If the project is already sequencing schema changes carefully, placement in a monolithic batch may make future migrations harder to reason about.
- `LOW`: Storing `name TEXT NOT NULL DEFAULT ''` is pragmatic, but it bakes in empty-string semantics. If "name omitted" and "name intentionally blank" should ever differ, this becomes awkward.

#### Suggestions
- Drop `idx_api_keys_hashed_key` unless there is a demonstrated reason to keep a second index beyond the `UNIQUE` constraint.
- Confirm the exact DB handle type already used in the codebase, and make `KeyService` depend on that abstraction rather than introducing `Arc<Connection>` speculatively.
- Prefer narrower warning suppression: put `#[allow(dead_code)]` only on the stub structs/methods that need it, or make the stubs public and referenced in tests.
- Be explicit about the `count_active_keys()` query shape: it should count only rows where `revoked_at IS NULL`.
- Consider whether `display_id` should also be `UNIQUE`. If it is derived from 8 hex chars of a hash, collisions are possible; if collisions are acceptable, say that explicitly now.

#### Risk Assessment
**MEDIUM**. The overall phase shape is solid, but the redundant index and uncertain `KeyService` DB type are real design risks. If those are corrected up front, the rest of the plan is low-risk.

---

### PLAN 10-02 Review

#### Summary
This plan covers the missing runtime wiring and verification needed to make the foundation usable, but it has a few gaps in how the success criteria are actually proven. The startup auth-mode log and `AppState` wiring are appropriate, but `main.rs` adding `mod auth;` looks suspicious, and the proposed unauthorized-response integration test does not clearly have a real execution path yet unless some handler or middleware already emits `ApiError::Unauthorized`.

#### Strengths
- Correctly treats startup auth-mode logging as runtime behavior, not just schema work.
- Updates test state builders, which is necessary once `AppState` changes shape.
- Includes idempotent migration testing, which directly maps to the phase success criteria.
- Verifies `count_active_keys()` on an empty DB, which is the minimum needed for open-mode startup behavior.
- Keeps most verification in integration tests, which is appropriate for DB bootstrap and server wiring.

#### Concerns
- `HIGH`: `main.rs` adding `mod auth;` is likely the wrong move if `lib.rs` already exports `pub mod auth;`. In a typical Rust bin+lib layout, the binary should use the library module path rather than redeclaring the module. Doing both can create duplicate module structure or compile confusion.
- `HIGH`: The plan does not clearly prove success criterion 2 as written. It logs on startup, but there is no explicit test or verification strategy for the actual log output.
- `HIGH`: `test_unauthorized_response_shape` may not be testable through the real HTTP stack in Phase 10 unless a route or middleware already returns `ApiError::Unauthorized`. If nothing emits that variant yet, the test either becomes artificial or forces scope creep.
- `MEDIUM`: Logging `Err = warning` for `count_active_keys()` may violate the roadmap intent. If auth mode cannot be determined at startup, that is not merely informational. The server may be starting in an ambiguous state.
- `MEDIUM`: The plan updates `tests/integration.rs` only. If there are unit tests or helper builders elsewhere constructing `AppState`, this can create churn and follow-up fixes.
- `MEDIUM`: The auth-mode log criteria mention "with actionable hint." The plan summary says `"Auth: OPEN"` / `"Auth: ACTIVE"` but does not specify the operator guidance text.
- `LOW`: Verifying exact column count via `PRAGMA table_info` is brittle if the schema intentionally grows in a later patch. For this phase it is acceptable, but column-name assertions may age better.

#### Suggestions
- Do not add `mod auth;` to `main.rs` if the crate already exposes `src/lib.rs`. Use the existing library module path consistently.
- Tighten the startup behavior on DB query failure: either fail startup or explicitly justify why degraded startup is acceptable.
- Change the unauthorized test strategy: unit test `ApiError::Unauthorized(...).into_response()` directly, or add a tiny test-only handler.
- Add one verification step for the startup log itself.
- Make the expected log lines explicit now, including the actionable hint, so later phases do not change operator-facing wording accidentally.
- Audit all `AppState` constructors across the repo, not just the two named helpers.

#### Risk Assessment
**MEDIUM-HIGH**. The runtime wiring is necessary, but the `main.rs` module declaration looks incorrect, and the plan does not cleanly demonstrate all success criteria yet, especially the startup log and 401 response behavior. It is a good plan once those verification gaps are corrected.

---

## Consensus Summary

### Agreed Strengths
- **Scope discipline is excellent** — Both reviewers praised the strict adherence to deferred constraints (no crypto, no Forbidden variant, no last_used_at). Phase 10 does only foundation work.
- **Plan split is logical** — 10-01 (static artifacts) vs 10-02 (runtime wiring + tests) minimizes conflicts and follows a natural dependency order.
- **Concrete stub signatures** — Using `Result<T, DbError>` return types with `todo!()` bodies instead of `-> !` reduces Phase 11 churn. Both reviewers called this out as a strength.
- **Testing strategy is thorough** — PRAGMA-based schema verification, idempotent migration testing, and empty-DB count testing directly map to success criteria.

### Agreed Concerns
1. **[HIGH] `mod auth;` in main.rs is incorrect** — Both Gemini and Codex flagged this as the highest-risk issue. Adding `mod auth;` to `main.rs` when `lib.rs` already declares `pub mod auth;` creates duplicate module compilation and type mismatches. The binary should use the library's module path instead.
2. **[HIGH] Redundant `idx_api_keys_hashed_key` index** — Both reviewers independently identified that `hashed_key TEXT NOT NULL UNIQUE` already creates an implicit SQLite index. The explicit `CREATE INDEX` is wasteful and adds write overhead.
3. **[MEDIUM] `#![allow(dead_code)]` is too broad** — Both reviewers suggested narrower suppression (per-item `#[allow(dead_code)]` or referencing stubs in tests) over module-level suppression.

### Divergent Views
- **KeyService DB handle type** — Codex raised `Arc<Connection>` as a HIGH concern, questioning whether it matches the project's real DB abstraction. Gemini did not flag this. *Note: The existing codebase uses exactly `Arc<tokio_rusqlite::Connection>` for MemoryService and CompactionService, so this concern is addressed by existing patterns.*
- **Unauthorized response test validity** — Codex flagged `test_unauthorized_response_shape` as potentially untestable through the HTTP stack (no handler emits it yet). Gemini did not raise this. *Note: The plan's test code directly calls `.into_response()` on the error variant — it's a unit-style test, not an HTTP integration test, so this concern is mitigated.*
- **Startup log verification** — Codex wanted an explicit test for startup log output. Gemini did not flag this. Testing log output is typically impractical in Rust (requires capturing tracing subscriber output); manual verification via `cargo run` is the standard approach.
- **display_id uniqueness** — Codex asked whether `display_id` should be UNIQUE given 8-hex-char collision possibility. Gemini did not raise this. Collisions are mathematically possible but acceptable (display_id is for human identification, not lookups).
