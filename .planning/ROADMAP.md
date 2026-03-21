# Roadmap: Mnemonic

## Milestones

- ✅ **v1.0 MVP** — Phases 1-5 (shipped 2026-03-20)
- ✅ **v1.1 Memory Compaction** — Phases 6-9 (shipped 2026-03-20)
- 🚧 **v1.2 Authentication / API Keys** — Phases 10-14 (in progress)

## Phases

<details>
<summary>✅ v1.0 MVP (Phases 1-5) — SHIPPED 2026-03-20</summary>

- [x] Phase 1: Foundation (3/3 plans) — completed 2026-03-19
- [x] Phase 2: Embedding (2/2 plans) — completed 2026-03-19
- [x] Phase 3: Service and API (3/3 plans) — completed 2026-03-19
- [x] Phase 4: Distribution (2/2 plans) — completed 2026-03-19
- [x] Phase 5: Config & Embedding Provider Cleanup (1/1 plan) — completed 2026-03-20

</details>

<details>
<summary>✅ v1.1 Memory Compaction (Phases 6-9) — SHIPPED 2026-03-20</summary>

- [x] Phase 6: Foundation (2/2 plans) — completed 2026-03-20
- [x] Phase 7: Summarization Engine (1/1 plan) — completed 2026-03-20
- [x] Phase 8: Compaction Core (2/2 plans) — completed 2026-03-20
- [x] Phase 9: HTTP Integration (1/1 plan) — completed 2026-03-20

</details>

### 🚧 v1.2 Authentication / API Keys (In Progress)

**Milestone Goal:** Add optional API key authentication so mnemonic can be safely deployed on a network — scoped to agent namespaces, enforced in middleware, off by default for local dev.

- [x] **Phase 10: Auth Schema Foundation** - DB table, error variant, and module skeleton that all auth logic builds on (completed 2026-03-20)
- [ ] **Phase 11: KeyService Core** - Business logic for key creation, listing, revocation, validation, and secure hashing
- [ ] **Phase 12: Auth Middleware** - Axum middleware that enforces authentication and injects AuthContext into requests
- [ ] **Phase 13: HTTP Wiring and REST Key Endpoints** - Attach middleware to router, add key management REST endpoints, enforce scope at handler layer
- [ ] **Phase 14: CLI Key Management** - `mnemonic keys` subcommand for creating, listing, and revoking keys from the terminal

## Phase Details

### Phase 10: Auth Schema Foundation
**Goal**: The DB schema and error infrastructure that all auth work depends on exists and is safe to upgrade over
**Depends on**: Phase 9
**Requirements**: INFRA-01, INFRA-03
**Success Criteria** (what must be TRUE):
  1. Server starts cleanly on an existing v1.1 database with no migration errors (CREATE TABLE IF NOT EXISTS pattern)
  2. Server startup log prints whether it is running in open mode or authenticated mode
  3. An HTTP response for an unauthorized request returns 401 with a structured JSON body (not a generic 500)
  4. `pub mod auth` is declared in `lib.rs` and the project compiles with zero warnings
**Plans:** 2/2 plans complete
Plans:
- [x] 10-01-PLAN.md — Schema DDL, Unauthorized error variant, auth module skeleton
- [x] 10-02-PLAN.md — AppState wiring, startup auth-mode log, integration tests

### Phase 11: KeyService Core
**Goal**: Admin can create, list, and revoke API keys with secure hashing — and keys can be validated without exposing the raw token
**Depends on**: Phase 10
**Requirements**: KEY-01, KEY-02, KEY-03, KEY-04, INFRA-02
**Success Criteria** (what must be TRUE):
  1. Admin creates a key and receives a `mnk_...` prefixed raw token exactly once — it is not stored in the DB and cannot be retrieved again
  2. Admin lists keys and sees name, prefix, scope, and creation date — never the raw token
  3. Admin revokes a key by ID; subsequent validation of that key returns an error
  4. A key scoped to `agent-x` cannot be validated against memories for `agent-y` (scope is stored on the key record)
  5. Key comparison uses constant-time BLAKE3 hash comparison — `==` is never used on raw or hashed key values
**Plans:** 1 plan
Plans:
- [ ] 11-01-PLAN.md — Implement create/list/revoke/validate with BLAKE3 hashing, constant-time comparison, and unit tests

### Phase 12: Auth Middleware
**Goal**: Every matched route checks authentication via the middleware, with open mode and health-check exemption working correctly
**Depends on**: Phase 11
**Requirements**: AUTH-01, AUTH-02, AUTH-03, AUTH-05
**Success Criteria** (what must be TRUE):
  1. A valid Bearer token allows the request through and injects `AuthContext` into request extensions
  2. An invalid or revoked Bearer token returns 401 regardless of which endpoint is called
  3. When zero active keys exist in the DB, all requests are allowed through (open mode — no restart needed)
  4. `GET /health` returns 200 even when auth is active and no Authorization header is present
  5. A request with a malformed Authorization header (not `Bearer <token>`) returns 400, not a panic or 500
**Plans**: TBD

### Phase 13: HTTP Wiring and REST Key Endpoints
**Goal**: Key management is accessible via REST, auth middleware is attached to all protected routes, and scoped keys enforce namespace isolation at the handler layer
**Depends on**: Phase 12
**Requirements**: AUTH-04, INFRA-03
**Success Criteria** (what must be TRUE):
  1. A scoped key for `agent-x` used with a request body specifying `agent_id: "agent-y"` returns 403 — the handler uses `AuthContext.allowed_agent_id`, not the request body
  2. `POST /keys` creates a key and returns the raw token (shown once)
  3. `GET /keys` returns all key metadata with no raw token values
  4. `DELETE /keys/:id` revokes a key and subsequent requests with that key return 401
  5. Server startup log (first request or startup hook) confirms open or authenticated mode
**Plans**: TBD

### Phase 14: CLI Key Management
**Goal**: Admin can manage API keys from the terminal without starting the full server or loading the embedding model
**Depends on**: Phase 13
**Requirements**: CLI-01, CLI-02, CLI-03
**Success Criteria** (what must be TRUE):
  1. `mnemonic keys create` prints the raw `mnk_...` token with a "copy now — not shown again" warning, then exits
  2. `mnemonic keys list` prints a table of key metadata (name, prefix, scope, created date) with no raw tokens
  3. `mnemonic keys revoke <id>` revokes the key and confirms revocation; the server rejects that key on the next request
  4. The `keys` subcommand starts in under 1 second — the embedding model is never loaded on the CLI path
**Plans**: TBD

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation | v1.0 | 3/3 | Complete | 2026-03-19 |
| 2. Embedding | v1.0 | 2/2 | Complete | 2026-03-19 |
| 3. Service and API | v1.0 | 3/3 | Complete | 2026-03-19 |
| 4. Distribution | v1.0 | 2/2 | Complete | 2026-03-19 |
| 5. Config Cleanup | v1.0 | 1/1 | Complete | 2026-03-20 |
| 6. Foundation | v1.1 | 2/2 | Complete | 2026-03-20 |
| 7. Summarization Engine | v1.1 | 1/1 | Complete | 2026-03-20 |
| 8. Compaction Core | v1.1 | 2/2 | Complete | 2026-03-20 |
| 9. HTTP Integration | v1.1 | 1/1 | Complete | 2026-03-20 |
| 10. Auth Schema Foundation | v1.2 | 2/2 | Complete    | 2026-03-20 |
| 11. KeyService Core | v1.2 | 0/1 | Not started | - |
| 12. Auth Middleware | v1.2 | 0/? | Not started | - |
| 13. HTTP Wiring and REST Key Endpoints | v1.2 | 0/? | Not started | - |
| 14. CLI Key Management | v1.2 | 0/? | Not started | - |
