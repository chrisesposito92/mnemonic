# Feature Research

**Domain:** API key authentication for developer tools / infrastructure APIs (v1.2 milestone)
**Researched:** 2026-03-20
**Confidence:** HIGH (patterns sourced from Stripe, GitHub, Paddle, prefix.dev, OWASP; axum middleware patterns verified against current docs)

---

## Scope Note

This document covers **only the new features for v1.2**. The v1.0/v1.1 baseline (6 REST endpoints, local embeddings, agent_id/session_id namespacing, SQLite+sqlite-vec, memory compaction) is already shipped and treated as a dependency, not a feature.

The central question: what does "good" API key authentication look like for a developer infrastructure tool that starts as a local single-binary and can be optionally deployed on a network?

---

## Feature Landscape

### Table Stakes (Users Expect These)

Features any developer expects from an authenticated API. Missing these makes the auth system feel half-baked or insecure.

| Feature | Why Expected | Complexity | Dependencies on Existing Architecture |
|---------|--------------|------------|---------------------------------------|
| Bearer token via `Authorization` header | RFC 7235 standard. Every developer tool from Stripe to OpenAI uses `Authorization: Bearer <token>`. Any other transport (header name, query param) is a surprise and an antipattern. | LOW | New axum `from_request` extractor or `tower::Service` middleware layer in `server.rs`. |
| `mnk_` prefix on generated keys | Stripe (sk_live_), GitHub (ghp_, ghu_), PyPI (pypi-), prefix.dev (pfx_) all use prefixes so keys are identifiable in logs, env dumps, and grep output. Without a prefix, a leaked key cannot be traced back to the service. Stripe invented this pattern in 2012; it is now universal. | LOW | Pure format convention. Enforced in the key generator function, checked in the auth middleware. |
| Keys stored as hashes only, never plaintext | OWASP requirement. Stripe does not store plaintext keys. Leaked DB does not expose credentials. The raw key is shown exactly once at creation. | MEDIUM | New `api_keys` table in SQLite (schema migration in `db.rs`). Store `SHA-256(key)` or `BLAKE3(key)` — fast lookup, not password hashing. See "Hash Storage" note below. |
| Key shown once at creation, never again | Universal pattern: Stripe, GitHub, Paddle all show the full key once and never again. Users expect this — it enforces discipline to store keys immediately. | LOW | `POST /keys` response includes full key. No `GET /keys/:id` returns the secret. List endpoint returns only metadata (id, description, agent_id, created_at, last_used). |
| 401 on invalid/missing key when auth is active | Standard HTTP semantics. Missing auth = 401 Unauthorized. Wrong key = 401. Authenticated but wrong agent_id = 403 Forbidden. These are different errors and must not be conflated. | LOW | Axum middleware extracts Bearer token, looks up hash in DB, returns 401/403 via existing `ApiError` type in `error.rs`. |
| Open mode (no auth when no keys exist) | Mnemonic's target for local dev is zero-config. If auth is mandatory from install, it breaks the quickstart. The established pattern (used by Ollama, Home Assistant, and other local-first tools) is: no keys configured = open access. First key created = auth enforced. | LOW | Auth middleware checks key count at startup (or via cached flag). If zero keys exist, pass all requests. If any key exists, enforce. |
| CLI key management (create / list / revoke) | Developer tools that have API keys always have a matching CLI sub-command. Heroku: `heroku authorizations:create`. GitHub CLI: `gh auth token`. Stripe CLI: key management commands. Without CLI management, users must craft raw HTTP to manage credentials — unacceptable DX. | MEDIUM | New `KeysCommand` subcommand in `main.rs` / CLI arg parsing. Calls the keys API endpoints. Must be able to run against a configured server address. |
| Key revocation (immediate effect) | Once revoked, a key must stop working immediately. Delayed revocation after compromise is unacceptable. Implies no long-lived in-process cache of valid keys without invalidation support. | LOW | `DELETE /keys/:id` marks key as revoked in DB. Middleware checks revocation status on every request (SQLite lookup is fast; no need for distributed invalidation given single-process architecture). |

### Differentiators (Competitive Advantage)

Features that are not universally expected but align with Mnemonic's positioning as a scoped, agent-aware memory tool.

| Feature | Value Proposition | Complexity | Dependencies on Existing Architecture |
|---------|-------------------|------------|---------------------------------------|
| Keys scoped to specific `agent_id` | Most infrastructure APIs offer global keys. Mnemonic's core concept is multi-agent namespacing — scoping a key to exactly one agent_id means a compromised agent key cannot read another agent's memories. This is a genuine differentiator because the threat model (multiple AI agents sharing one server) is specific to Mnemonic. | MEDIUM | `api_keys` table needs `agent_id TEXT` column (nullable for admin keys). Middleware checks that the `agent_id` in the request query/body matches the key's `agent_id`. Uses existing `agent_id` column pattern from `memories` table. |
| Global admin key (unscoped) | Operators need to manage the server itself — list all agents, run compaction across namespaces, generate per-agent keys. A single unscoped admin key serves this role. Pattern: `agent_id IS NULL` on the key row = admin access. | LOW | `agent_id` on key row: `NULL` = admin (all namespaces), non-null = scoped. No separate key type needed; the data model carries the distinction. |
| `last_used_at` timestamp on each key | Shows operators which keys are still active. Standard feature: GitHub, Stripe both surface this. Helps answer "can I safely revoke this old key?" | LOW | `last_used_at DATETIME` column on `api_keys` table. Updated on successful auth in the middleware. SQLite write on every authenticated request — acceptable for single-process server. |
| Machine-readable key metadata (description field) | Per-key `description` field lets operators label keys by deployment ("prod agent", "staging agent", "CI pipeline"). Stripe and GitHub both have this. Without it, the key list is an undifferentiated set of prefixed random strings. | LOW | `description TEXT` column on `api_keys` table. Optional, set at creation time via CLI flag `--description`. |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Argon2/bcrypt for API key hashing | "Treat keys like passwords" is a reasonable instinct. prefix.dev uses Argon2 for their API keys. | API keys are 32+ bytes of cryptographically random data, not user-chosen passwords. Brute force is infeasible regardless of hash speed. Argon2 adds ~100ms of latency to every authenticated request in a single-process server. For a local infrastructure tool, this is noticeable. Password hash functions are for protecting weak secrets — random tokens are already strong. Use SHA-256 or BLAKE3 (fast, collision-resistant, purpose-appropriate). | Store `BLAKE3(key)` or `SHA-256(key)`. Fast lookup, no brute-force risk, no request latency penalty. |
| Key rotation with overlap / grace period | "Enterprises need zero-downtime rotation" is a real concern for SaaS. Overlap periods (old key valid for 7 days after new key issued) prevent outages during rotation. | For a single-binary local/small-network tool, this adds significant complexity (two active keys per slot, expiry tracking, overlap logic). The user base (individual developers, small teams) can tolerate a brief rotation window — revoke old key, start using new key, done. | Simple revoke-and-recreate. Document the pattern: create new key, test it, revoke old key. CLI makes this a 2-command operation. |
| Rate limiting per key | "Prevent one agent from hammering the server" sounds important for multi-tenant SaaS. | Mnemonic is a single-binary local/network tool for agent developers who own all the keys. Rate limiting adds complexity (sliding window counters, storage), latency, and a confusing 429 response that breaks agents unexpectedly. The actual threat model is a buggy agent loop — which is better addressed by the agent's own circuit breaker. | Document the concern. Add to v1.3+ consideration if user feedback confirms the need. |
| JWT tokens instead of static keys | "JWTs are stateless and scale better" is true for distributed systems. | JWT adds library dependencies (JWT parsing/validation), introduces expiry semantics (agents must handle token refresh), and provides no benefit in a single-process server that can do a DB lookup in microseconds. Static API keys with a fast hash lookup are simpler, more transparent, and sufficient for this use case. | Static `mnk_` prefixed keys stored as hashed tokens in SQLite. Stateful, revocable immediately, zero library overhead. |
| Per-endpoint permission scopes | "Keys should have fine-grained permissions (read vs. write vs. compact)" is a reasonable RBAC ask. | For Mnemonic's current feature set (one memory namespace per agent), the relevant permission boundary is already agent_id scoping. Adding endpoint-level scopes (e.g., "this key can search but not compact") adds UI/CLI complexity for a permission model that most users will never need. | Agent-scoped keys (agent_id on the key row) provide the meaningful isolation. Endpoint scopes are a v2+ feature if multi-tenant production deployments emerge. |
| OAuth 2.0 / OIDC integration | "Use the org's identity provider" is expected for enterprise tooling. | OAuth adds an authorization server dependency, browser redirect flows, and token exchange logic — directly violating Mnemonic's zero-external-dependency principle. The target user is a developer running a local binary, not an enterprise SSO deployment. | Static API keys managed via CLI. Document that keys can be stored in `.env` files or secret managers. |
| Web UI for key management | "A dashboard would be nice" comes up for any tool with credentials. | Adds a frontend build pipeline, static file serving, and session management. Violates the single-binary simplicity invariant. PROJECT.md explicitly excludes "Web UI / dashboard." | CLI commands: `mnemonic keys create`, `mnemonic keys list`, `mnemonic keys revoke <id>`. REST API for programmatic access. |

---

## Feature Dependencies

```
[Auth middleware (Bearer token check)]
    └──requires──> [api_keys table in SQLite] (key hash storage)
    └──requires──> [open-mode detection] (zero-keys = pass-through)
    └──requires──> [agent_id scoping logic] (compare key.agent_id to request agent_id)

[api_keys SQLite table]
    └──requires──> [schema migration in db.rs] (new table, added to existing open() function)
    └──requires──> [key hash function] (BLAKE3 or SHA-256 — new util)

[CLI key management (mnemonic keys create/list/revoke)]
    └──requires──> [REST key management endpoints] (POST/GET/DELETE /keys)
    └──requires──> [server address config] (keys CLI must know where the server is)

[REST key management endpoints (POST/GET/DELETE /keys)]
    └──requires──> [api_keys table in SQLite]
    └──requires──> [auth middleware] (key management endpoints must themselves be authenticated
                    when auth is active — prevents unauthenticated key creation after first key)

[Key scoping to agent_id]
    └──requires──> [api_keys.agent_id column] (nullable: NULL = admin, text = scoped)
    └──requires──> [agent_id extraction from request] (existing pattern from memories endpoints)
    └──enhances──> [existing multi-agent namespacing] (no changes to namespace logic, just enforces it)

[Open mode fallback]
    └──requires──> [key count query at startup OR per-request check]
    └──conflicts──> [nothing] (disabling auth is not the same as bypassing it)
```

### Dependency Notes

- **Auth middleware depends on api_keys table:** The table must exist before the middleware can function. Schema migration in `db.rs::open()` is the natural location — Mnemonic already uses `execute_batch` for idempotent migrations.
- **Key management endpoints must be self-protecting:** After the first key is created, `POST /keys` and `DELETE /keys/:id` must require an admin key. Before the first key exists (open mode), they are accessible. This circular dependency is resolved by checking key count per-request in the middleware, not at startup.
- **CLI requires server address:** The `mnemonic keys` subcommand sends HTTP requests to the running server. It needs a `--server` flag or reads from config (port from `MNEMONIC_PORT` or `mnemonic.toml`). This reuses the existing `Config` struct.
- **Agent-scoped keys do not change the memory API:** The `agent_id` in memory requests is already a parameter. Scoping enforcement is additive — the middleware compares the key's `agent_id` to the request's `agent_id` and returns 403 if they differ. No changes to `service.rs` or `compaction.rs`.

---

## MVP Definition

### This Milestone Is v1.2 (adding auth to a shipped product)

The goal is not to build a full auth platform. It is to make Mnemonic safe for network deployment without breaking the local-first zero-config experience.

### Ship in v1.2

- [ ] `api_keys` table with columns: `id`, `key_hash`, `description`, `agent_id` (nullable), `created_at`, `last_used_at`, `revoked_at`
- [ ] Key generation: `mnk_` prefix + 32 bytes cryptographically secure random (base58 or base62 encoded)
- [ ] Key storage: `BLAKE3(key)` or `SHA-256(key)` — never plaintext
- [ ] Open mode: auth is bypassed when `api_keys` table has zero non-revoked rows
- [ ] Axum middleware: extracts `Authorization: Bearer mnk_...` header, hashes it, looks up in DB, enforces agent_id scope
- [ ] `POST /keys` — create a new key (returns full key once, never again)
- [ ] `GET /keys` — list keys (metadata only: id, description, agent_id, created_at, last_used_at)
- [ ] `DELETE /keys/:id` — revoke a key immediately
- [ ] `mnemonic keys create [--agent-id <id>] [--description <text>]` CLI subcommand
- [ ] `mnemonic keys list` CLI subcommand
- [ ] `mnemonic keys revoke <id>` CLI subcommand

### Add After Validation (v1.3+)

- [ ] Key rotation helper: `mnemonic keys rotate <id>` — creates new key for same agent_id, then revokes old — when users report needing zero-downtime key transitions
- [ ] Rate limiting per key — add when user feedback confirms runaway-agent protection is needed
- [ ] Per-endpoint scope flags (read/write/compact) — add when multi-tenant production deployments emerge
- [ ] Expiry date on keys (`expires_at` column) — add for enterprise deployment contexts

### Confirmed Out of Scope (v1.2)

- [ ] JWT / OAuth / OIDC — violates zero-external-dependency principle
- [ ] Web UI for key management — violates single-binary principle (PROJECT.md)
- [ ] Rate limiting — premature for target user base
- [ ] Argon2/bcrypt for key hashing — wrong tool for random-token use case
- [ ] Key rotation grace periods — unnecessary complexity for target scale

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| `api_keys` table + schema migration | HIGH | LOW | P1 |
| `mnk_` key generation + BLAKE3 hash storage | HIGH | LOW | P1 |
| Open mode (zero-keys = pass-through) | HIGH | LOW | P1 |
| Axum auth middleware (Bearer + agent_id scope) | HIGH | MEDIUM | P1 |
| `POST /keys`, `GET /keys`, `DELETE /keys/:id` REST endpoints | HIGH | LOW | P1 |
| CLI `mnemonic keys create/list/revoke` subcommands | HIGH | MEDIUM | P1 |
| Admin key (agent_id = NULL) | MEDIUM | LOW | P1 |
| `last_used_at` timestamp update on auth | MEDIUM | LOW | P1 |
| `description` field on key | MEDIUM | LOW | P1 |
| Key rotation helper CLI command | LOW | LOW | P3 |
| Rate limiting | LOW | HIGH | P3 |
| Per-endpoint scopes | LOW | HIGH | P3 |

**Priority key:**
- P1: Must have for v1.2 to be a coherent, deployable auth system
- P2: High value, low cost — include if schedule permits
- P3: Future milestone

---

## Implementation Notes

### Hash Algorithm: BLAKE3 vs SHA-256

API keys are 32+ bytes of cryptographically random data. The threat model is a leaked database — not a brute-force attack on weak secrets. Therefore:

- **Use BLAKE3 or SHA-256** — both are fast (microseconds), collision-resistant, and purpose-appropriate for authenticating random tokens
- **Do not use Argon2/bcrypt/scrypt** — these are designed to slow down brute force on weak (human-chosen) passwords. A random 32-byte token has 256 bits of entropy — brute force is computationally infeasible regardless of hash speed. Adding Argon2 only adds ~100ms of latency to every request with no security benefit.
- **BLAKE3** is available in Rust via the `blake3` crate (MIT licensed, no unsafe, pure Rust or with AVX2 acceleration). SHA-256 via the `sha2` crate. Either is fine — BLAKE3 is faster but both are adequate.

Confidence: HIGH (OWASP Password Storage Cheat Sheet distinguishes "password hashing" from "token hashing"; the recommendation to use fast hashes for random tokens is standard).

### Key Format

```
mnk_[base62-encoded 32 random bytes]
```

Example: `mnk_7xK9mP2nQvR4sT6uW8yZ0aB1cD3eF5g`

- `mnk_` prefix: 4 characters, identifies the service in logs/grep/secret scanners
- 32 random bytes = 256 bits entropy via `rand::thread_rng().fill_bytes()`
- Base62 encoding (a-z, A-Z, 0-9): URL-safe, no special characters, easy to copy-paste
- Total length: ~47 characters — comparable to GitHub's 40-char tokens

### Open Mode Logic

The simplest correct implementation:

```rust
// In auth middleware:
let active_key_count: i64 = db.query_row(
    "SELECT COUNT(*) FROM api_keys WHERE revoked_at IS NULL", [], |r| r.get(0)
)?;
if active_key_count == 0 {
    return next.call(req).await; // open mode
}
// ... proceed with Bearer token check
```

Caveat: this query runs on every request. For a single-process SQLite server handling 100s of req/s, this is negligible (SQLite COUNT(*) on a small indexed table is microseconds). An `Arc<AtomicBool>` flag updated on key create/revoke is a valid optimization if profiling shows it matters — but is premature for v1.2.

### Middleware Placement in axum

The auth middleware should wrap the entire router except `GET /health`. Pattern:

```rust
Router::new()
    .route("/health", get(health_handler))  // unauthenticated
    .nest("/", authenticated_routes())
    .layer(AuthMiddlewareLayer::new(db_arc))
```

Alternatively, use axum's `from_request` extractor on a `ValidatedKey` type — this is idiomatic in axum and avoids the need for a separate middleware layer. The extractor approach is preferred in the axum ecosystem for per-request auth because it composes cleanly with handler signatures.

---

## Competitor / Reference Analysis

| Feature | Stripe | GitHub | Ollama | Mnemonic v1.2 |
|---------|--------|--------|--------|----------------|
| Key prefix | `sk_live_`, `sk_test_`, `rk_` | `ghp_`, `ghu_`, `gha_` | No auth | `mnk_` |
| Hash storage | Yes (SHA-256 equivalent) | Yes | N/A | Yes (BLAKE3) |
| Show once | Yes | Yes | N/A | Yes |
| Scoping | Restricted key permissions | Repository/org scopes | N/A | agent_id namespace |
| Open mode | No (always required) | No (always required) | Yes (default open) | Yes (default open, auth on first key) |
| CLI management | Stripe CLI | GitHub CLI | N/A | `mnemonic keys` |
| Key metadata | Name, created, last used | Note, created, last used | N/A | Description, agent_id, created, last used |
| Rate limiting | Yes (plan-based) | Yes (GitHub limits) | No | No (v1.2) |
| Key rotation | Create + revoke | Create + revoke | N/A | Create + revoke (v1.2), rotate helper (v1.3+) |

Ollama's "open by default" pattern is the most relevant precedent for Mnemonic — Ollama is a local inference server that recently added optional auth via `OLLAMA_API_KEY` env var. When not set, it runs open. This is exactly the UX model Mnemonic should follow.

---

## Sources

- [API Key Management Best Practices (OneUptime, 2026)](https://oneuptime.com/blog/post/2026-02-20-api-key-management-best-practices/view) — prefix conventions, hash storage, key generation. HIGH confidence.
- [How prefix.dev implemented API keys](https://prefix.dev/blog/how_we_implented_api_keys) — Argon2 for keys (noted but not recommended here — see Hash Algorithm note), pfx_ prefix pattern, show-once UX. MEDIUM confidence on Argon2 recommendation (overridden by OWASP reasoning).
- [Best practices for building secure API Keys (freeCodeCamp)](https://www.freecodecamp.org/news/best-practices-for-building-api-keys-97c26eabfea9/) — 32-byte minimum, prefix for identification, scope restrictions. HIGH confidence.
- [Stripe API Keys Documentation](https://docs.stripe.com/keys) — sk_live_ / sk_test_ / rk_ prefix conventions, restricted keys, show-once pattern. HIGH confidence (authoritative source).
- [On API Keys Best Practices (Mergify)](https://articles.mergify.com/api-keys-best-practice/) — prefix design rationale, scoping approach. MEDIUM confidence.
- [Rotate API keys (Paddle Developer Docs)](https://developer.paddle.com/api-reference/about/rotate-api-keys) — grace period overlap pattern (noted as anti-feature for Mnemonic's scale). HIGH confidence on the pattern.
- [OWASP Password Storage Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html) — distinction between password hashing and token hashing. HIGH confidence.
- [Axum authentication with Bearer tokens (rust-classes.com)](https://rust-classes.com/chapter_7_4) — axum extractor pattern for Bearer auth. MEDIUM confidence.
- [Simple API Key Auth in Axum (ruststepbystep.com)](https://www.ruststepbystep.com/simple-api-key-authentication-in-axum-step-by-step-guide/) — middleware layer pattern for axum. MEDIUM confidence.
- [API Authentication Best Practices 2026 (DEV Community)](https://dev.to/apiverve/api-authentication-best-practices-in-2026-3k4a) — current landscape. MEDIUM confidence.

---
*Feature research for: Mnemonic v1.2 API key authentication milestone*
*Researched: 2026-03-20*
