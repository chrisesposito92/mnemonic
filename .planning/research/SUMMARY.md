# Project Research Summary

**Project:** Mnemonic — Rust agent memory server
**Domain:** Embedded vector search + local ML inference + REST API with optional authentication (v1.2)
**Researched:** 2026-03-20
**Confidence:** HIGH

## Executive Summary

Mnemonic is a single-binary Rust server providing persistent, searchable memory for AI agents via a REST API. The v1.1 milestone (memory compaction) is already shipped, and this research covers v1.2: optional API key authentication that keeps the binary zero-config for local development while making it safe to expose on a network. The established pattern for this class of tool — shared with Ollama, Home Assistant, and similar local-first infrastructure — is "open by default, auth activates on first key creation." Research confirms that all authentication infrastructure can be built on the existing dependency set with five targeted additions (rand_core, blake3, hex, constant_time_eq, clap), with no architectural rework required.

The recommended approach follows the four-layer architecture already established (HTTP handlers -> service layer -> DB/embedding engines -> SQLite storage). Auth adds a single new source file (`auth.rs`) containing `KeyService`, `AuthContext`, and the axum middleware function, plus one new `api_keys` table in SQLite. Scoped keys (each key constrained to one `agent_id`) provide genuine multi-agent namespace isolation without any changes to the existing memory service or compaction service. The dual-mode binary pattern (clap subcommand dispatch) keeps CLI key management and server startup in one binary by guarding the embedding model load behind the server code path only.

The dominant risks are security correctness issues specific to auth implementation: timing attacks from non-constant-time key comparison (a real CVE filed against vLLM in 2025 for exactly this pattern, rated High), horizontal privilege escalation via `agent_id` override (the middleware validates the key but handlers must use the authorized scope from extensions — not the request body), and health endpoint lockout when middleware is applied globally. Recovery from these mistakes is costly (rotate all keys, audit access logs, notify affected users), so they must be verified before any production deployment. Each has a well-defined prevention strategy documented in PITFALLS.md.

## Key Findings

### Recommended Stack

The existing stack is locked and needs no changes for authentication logic. All auth infrastructure is covered by five new dependencies. The binary already uses `reqwest 0.13` for LLM embedding fallback — this constraint blocked `async-openai` (which pins reqwest 0.12) and confirms the no-new-HTTP-client policy for v1.1. The same constraint continues to apply. The `rusqlite 0.37` pin must not be upgraded; sqlite-vec 0.1.7 has a documented conflict with rusqlite 0.39's libsqlite3-sys.

**New dependencies for v1.2:**
- `rand_core 0.9` (os_rng feature): OS-provided cryptographic entropy for key generation — minimal crate; full `rand` adds PRNGs and distributions this use case does not need
- `blake3 1.8`: Key hashing at rest — fast (appropriate for high-entropy tokens, unlike Argon2/bcrypt designed for low-entropy passwords), pure Rust, zero C dependencies, verified at 1.8.3 on docs.rs
- `hex 0.4`: Encode/decode BLAKE3 hashes for SQLite TEXT storage — round-trip via `hex::encode()` / `hex::decode()`
- `constant_time_eq 0.4`: Constant-time 32-byte comparison (`constant_time_eq_32()`) — simpler API than `subtle` for this specific use case
- `clap 4.6` (derive feature): CLI subcommand dispatch for `mnemonic keys create/list/revoke`

**Locked existing stack (no changes needed):**
- tokio 1 / axum 0.8: async runtime and HTTP server; axum's `middleware::from_fn_with_state` is the auth middleware mechanism
- rusqlite 0.37 (bundled) + sqlite-vec 0.1.7: vector KNN search and all schema work
- candle 0.9 + tokenizers 0.22 + hf-hub 0.5: local ML inference (untouched by auth)
- reqwest 0.13 + serde_json 1: HTTP client and serialization (reused for LLM provider)
- figment 0.10: config (TOML + env vars, extended transparently for any new keys)

**No changes to existing dependency versions required.**

### Expected Features

**Must have (v1.2 table stakes):**
- Bearer token via `Authorization` header — RFC 7235 standard; any other transport is an anti-pattern
- `mnk_` prefix on generated keys — identifiable in logs, grep output, and secret scanners (Stripe, GitHub, PyPI all use this pattern for the same reason)
- Keys stored as BLAKE3 hashes only, never plaintext — OWASP requirement; leaked DB must not expose credentials
- Key shown once at creation, never retrievable again — universal pattern (Stripe, GitHub, Anthropic)
- 401 on invalid/missing key when auth is active; 403 for valid key accessing wrong agent namespace
- Open mode (no auth when zero active keys exist) — preserves zero-config local dev experience; Ollama uses the same model
- CLI key management: `mnemonic keys create/list/revoke` — without CLI, users must craft raw HTTP to manage credentials; this is an unacceptable DX gap
- Immediate key revocation — `DELETE /keys/:id` must take effect on the next request with no cache lag
- `agent_id`-scoped keys — genuine differentiator; a compromised agent key cannot read another agent's memories
- Admin/wildcard key (`agent_id IS NULL`) — needed for orchestrators managing multiple agents

**Should have (differentiators already defined in scope):**
- `last_used_at` timestamp per key — lets operators identify active vs. stale keys
- `description` field per key — human label for `keys list` output (users cannot tell keys apart from metadata alone)

**Defer to v1.3+:**
- Key rotation helper (`mnemonic keys rotate`) — create + revoke covers v1.2 needs
- Rate limiting per key — premature for target user base (individual developers, small teams)
- Per-endpoint permission scopes — unnecessary complexity at current scale
- Expiry dates on keys — enterprise deployment context only

**Confirmed out of scope:**
- JWT / OAuth / OIDC — violates zero-external-dependency principle
- Web UI for key management — violates single-binary principle (PROJECT.md)
- Argon2/bcrypt for key hashing — wrong tool for high-entropy random token use case (adds 100ms+ latency per request for no security benefit)

### Architecture Approach

The v1.2 architecture is additive: one new source file, five modified files, one new DB table. The existing 4-layer architecture (HTTP handlers -> service layer -> embedding/summarization engines -> SQLite) is preserved intact. Auth enforcement lives entirely at the handler boundary via `AuthContext` injected by middleware — neither `MemoryService` nor `CompactionService` need to know about authentication. The dual-mode binary pattern branches in `main()` via clap: the `keys` subcommand path opens only the DB (skipping the ~2-second embedding model load), while the server path follows v1.1 startup exactly.

**Major components (v1.2 additions):**
1. `src/auth.rs` (new) — `KeyService` (create/list/revoke/validate/has_active_keys), `ApiKey` struct, `AuthContext` struct, `auth_middleware` fn, `generate_raw_token()`, `blake3_hex()`
2. `api_keys` table (new in `db.rs`) — `id`, `name`, `prefix`, `hashed_key`, `agent_id` (nullable = wildcard), `created_at`, `revoked_at`; index on `hashed_key` (auth middleware does this lookup on every request)
3. `main.rs` (modified) — clap CLI parse, dual-mode dispatch (server vs. key management), embedding model load guarded behind server path
4. `server.rs` (modified) — `AppState` gains `key_service: Arc<KeyService>`, router gains `route_layer` for auth middleware with `/health` excluded via split-router pattern
5. `error.rs` (modified) — additive `Unauthorized` variant on `ApiError`

**Key pattern — `route_layer` not `layer`:** Using `layer()` runs auth on all requests including unmatched routes, returning 401 instead of 404 (leaks auth configuration, breaks routing semantics). Using `route_layer()` runs auth only on matched routes.

**Key pattern — `AuthContext` in request extensions, not `AppState`:** Per-request state must not go into shared `AppState` (which is shared across concurrent requests). Request extensions are per-request.

**Key pattern — scope enforcement at handler boundary, not service layer:** Handlers extract `agent_id` from `Extension<AuthContext>`, not from the request body. If the key is scoped to `agent-x`, the handler forces `agent_id = "agent-x"` regardless of what the client sends. Services remain unaware of auth — no changes to `MemoryService` or `CompactionService`.

### Critical Pitfalls

**v1.2 auth-specific (highest priority — security mistakes with costly recovery):**

1. **Timing attack via non-constant-time key comparison** — CVE GHSA-wr9h-g72x-mwhm was filed against vLLM in 2025 for exactly this pattern (rated High severity). The `==` operator short-circuits on the first mismatched byte; over a local network an attacker can reconstruct the key character by character using latency measurements. Use `constant_time_eq::constant_time_eq_32()` on BLAKE3 hash bytes; never use `==` or `String::eq()` on key values.

2. **Horizontal privilege escalation (scope enforcement gap)** — Key for agent-A accepts `agent_id: "agent-B"` in the request body, granting cross-namespace read/write access. This is an IDOR (Insecure Direct Object Reference) pattern. Prevention: middleware injects authorized `agent_id` into request extensions after key lookup; handlers must extract `agent_id` from `Extension<AuthContext>`, never from `Query<>` or `Json<>`. Test: key for agent-A + request body `agent_id: "agent-B"` must return 403.

3. **Health endpoint behind auth breaks monitoring** — Applying auth via `layer()` (not `route_layer()`) intercepts `/health`, causing Docker/K8s health checks to return 401 and trigger container restart loops. Prevention: split router pattern — `protected` Router wrapped with `route_layer(auth_middleware)` merged with a `public` Router containing only `/health`.

4. **Migration cliff on upgrade** — Auth enabled by config flag or binary version immediately locks out existing deployments the moment they upgrade. Prevention: auto-activate auth on first key creation only (check `SELECT COUNT(*) FROM api_keys WHERE revoked_at IS NULL`); use `CREATE TABLE IF NOT EXISTS` for migration; log startup auth mode prominently.

5. **Plaintext key storage in SQLite** — A single DB file read (backup, misconfigured permissions, path traversal) exposes all credentials. Prevention: store `blake3::hash(key)` as hex string; raw key printed once via `println!` only, never via `tracing::*!`, never stored anywhere.

**Pre-existing critical pitfalls (v1.0/v1.1, must remain addressed):**

6. **Non-atomic compaction merge** — Deleting source memories before confirming merged memory write causes permanent data loss on crash. Pattern: compute embedding and LLM summary outside any transaction; then insert + delete inside a single `conn.call(|c| { tx })` that never crosses an async boundary.

7. **Cross-namespace compaction** — Clustering without `WHERE agent_id = ?` filter merges memories from different agents silently. Compaction must require `agent_id` and enforce it as a hard filter in all clustering SQL.

8. **Wrong embedding pooling** — all-MiniLM-L6-v2 requires mean pooling with attention mask weighting. CLS token or simple mean produces silent semantic search quality degradation with no error signal.

## Implications for Roadmap

Based on component dependencies identified in ARCHITECTURE.md and the risk profile from PITFALLS.md, v1.2 has a clear 5-phase build order. The single architectural constraint driving order: the `api_keys` table and `Unauthorized` error variant must exist before any auth logic can be written or tested.

### Phase 1: Auth Schema Foundation
**Rationale:** All other v1.2 work depends on the DB table existing. Schema migration is the zero-risk starting point — pure DDL additions following existing patterns in `db.rs::open()`, immediately verifiable with a startup test on a v1.1 database.
**Delivers:** `api_keys` table in SQLite with correct indexes; `Unauthorized` variant in `ApiError` with 401 response; `pub mod auth` in `lib.rs`
**Uses:** Existing `rusqlite`, `tokio-rusqlite`, `thiserror` — zero new dependencies in this phase
**Avoids:** Migration cliff pitfall (use `CREATE TABLE IF NOT EXISTS`; integration test that starts server on existing v1.1 DB); missing hash index (index on `hashed_key` present from day one)
**Research flag:** Standard patterns — no additional phase research needed

### Phase 2: KeyService Core
**Rationale:** Business logic for key CRUD is pure Rust + SQLite with no HTTP surface, making it independently unit-testable before middleware exists. All new dependencies enter the project here.
**Delivers:** `KeyService::create()`, `list()`, `revoke()`, `validate()`, `has_active_keys()`; `ApiKey` and `AuthContext` structs; `generate_raw_token()` and `blake3_hex()` utilities; BLAKE3 hashing; constant-time hash comparison
**Uses:** `rand_core 0.9` (os_rng), `blake3 1.8`, `hex 0.4`, `constant_time_eq 0.4` — all new additions enter here
**Avoids:** Plaintext key storage (only `blake3_hex(key)` ever reaches the DB); timing attack (constant_time_eq_32 used in `validate()`); key prefix leakage in `list` output (display ID is hash-derived, not a substring of the raw key)
**Research flag:** Standard patterns — all crate choices verified at HIGH confidence

### Phase 3: Auth Middleware
**Rationale:** Middleware depends on `KeyService` (Phase 2) and `ApiError::Unauthorized` (Phase 1) but has no HTTP wiring yet — testable in isolation with a minimal test router and test keys in a test DB.
**Delivers:** `auth_middleware` fn using `from_fn_with_state`; open-mode detection logic (COUNT query per request — no cache needed at this scale); `AuthContext` insertion into request extensions; correct handling of open-mode + invalid token (401, not passthrough); correct handling of malformed Authorization header (400, not passthrough)
**Uses:** Existing `axum 0.8` `middleware::from_fn_with_state`; `constant_time_eq` for hash comparison
**Avoids:** `layer()` vs `route_layer()` confusion (design router split in this phase); stale cache after revocation (skip caching entirely — SQLite indexed lookup is <1ms); open-mode passthrough of invalid tokens (Auth Pitfall 10)
**Research flag:** Standard patterns — axum middleware docs verified at HIGH confidence

### Phase 4: HTTP Wiring and REST Key Endpoints
**Rationale:** Attaches middleware to the router and adds `POST /keys`, `GET /keys`, `DELETE /keys/:id` endpoints. Depends on middleware being proven in Phase 3.
**Delivers:** Auth-protected routes via split router pattern; `/health` excluded; `AppState.key_service: Arc<KeyService>`; REST key management API; `Extension<AuthContext>` extraction in existing memory and compaction handlers for scope enforcement
**Uses:** Existing axum router and `AppState` pattern
**Avoids:** Scope enforcement gap (handlers must use `Extension<AuthContext>.allowed_agent_id`, not request body `agent_id`); self-protecting key endpoints (key management routes must require auth once any key exists — circular dependency resolved by per-request COUNT check, not startup flag); health endpoint behind auth
**Research flag:** Standard patterns — scope enforcement pattern is explicit in architecture research; integration tests for all scope-related scenarios must be written in this phase

### Phase 5: CLI Key Management
**Rationale:** Last phase because it depends on all server-side infrastructure. CLI commands call `KeyService` directly (no HTTP) via minimal startup (DB only — no model load).
**Delivers:** `mnemonic keys create [--name <n>] [--agent-id <id>]`; `mnemonic keys list`; `mnemonic keys revoke <id>`; dual-mode `main()` with clap dispatch; `println!` output with "copy now — not shown again" warning
**Uses:** `clap 4.6` (derive feature); existing `KeyService`; existing `Config`
**Avoids:** Key logged to tracing (only `println!` to stdout — verified by grep of all `tracing::*!` calls in creation path); shell history exposure (no `--value <key>` flag; key is always server-generated); embedding model load on CLI path (`Commands::Keys` branch opens only DB, never initializes `EmbeddingEngine`)
**Research flag:** Standard patterns — clap derive API is well-documented; dual-mode binary pattern is covered in architecture research with explicit code example

### Phase Ordering Rationale

- Schema-first mirrors the existing codebase convention: `db.rs::open()` initializes all tables before any service starts, so the `api_keys` DDL belongs in Phase 1.
- `KeyService` before middleware because the middleware calls `key_service.has_active_keys()` and `key_service.validate()` — these must exist and be tested before the middleware can be written.
- Middleware before HTTP wiring because wiring is straightforward once middleware is proven correct; the security-critical logic is in the middleware, not the router configuration.
- CLI last because it is a consumer of all server-side infrastructure. Placing it last ensures integration tests for server paths are green before CLI complexity is introduced, and the dual-mode dispatch change to `main.rs` is the riskiest change to the entry point.
- All five phases ship together as v1.2 — none can be deferred. The feature set is designed as a coherent auth system; partial auth (keys without scope enforcement, or middleware without CLI) is not usable.

### Research Flags

Phases needing deeper research during planning:
- None identified. All five phases use established Rust/axum/SQLite/clap patterns verified at HIGH confidence against official documentation. The auth security requirements (constant-time comparison, hashed storage, scope enforcement) are documented in security literature and PITFALLS.md contains per-pitfall prevention checklists with explicit implementation guidance.

Phases with standard patterns (skip research-phase):
- **All five phases** — stack choices, axum middleware patterns, SQLite migration patterns, and clap CLI patterns are all well-documented. PITFALLS.md's "Looks Done But Isn't" checklist (26 items) provides sufficient implementation guidance without additional research.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All crate versions verified against official docs (docs.rs, Cargo.toml in repo). reqwest 0.13 pin and rusqlite 0.37 pin confirmed against actual binary. async-openai exclusion confirmed via Cargo.toml inspection. All 5 new v1.2 dependencies at specific verified versions. |
| Features | HIGH | Patterns sourced from Stripe, GitHub, Paddle, Ollama, OWASP. Authentication UX conventions are well-established across the industry. Scope enforcement anti-pattern (IDOR) is well-documented in OWASP. |
| Architecture | HIGH | Based primarily on direct inspection of existing v1.1 source (3,678 lines, 10 files). Axum middleware patterns verified against official axum 0.8 docs. SQLite schema patterns mirror existing codebase conventions exactly. All component boundaries derived from existing code structure. |
| Pitfalls | HIGH | v1.2 auth pitfalls grounded in real CVEs (GHSA-wr9h-g72x-mwhm, 2025), OWASP guidance, and axum project discussions. v1.0/v1.1 pitfalls verified via benchmarks and official SQLite/candle documentation. The 26-item "Looks Done But Isn't" checklist covers the full implementation surface. |

**Overall confidence:** HIGH

### Gaps to Address

- **BLAKE3 vs SHA-256 inconsistency across research files:** STACK.md recommends `blake3` + `constant_time_eq`; ARCHITECTURE.md uses SHA-256 + `subtle` in code examples; FEATURES.md says "BLAKE3 or SHA-256, either is fine." Must pick one before Phase 2 begins. Recommendation: BLAKE3 (already researched at HIGH confidence, faster, pure Rust, simpler API) with `constant_time_eq` for comparison. All code examples in ARCHITECTURE.md should be treated as pseudocode for this specific detail.

- **Key display identifier scheme conflict:** PITFALLS.md (Auth Pitfall 7) warns against displaying key prefixes in `keys list` (reduces brute-force search space). ARCHITECTURE.md stores a `prefix` column (first 8 chars of raw token). These conflict. Correct approach per pitfall research: the display ID should be derived from the hash of the key, not a substring of the raw key. Resolve this in Phase 1 (schema design) before Phase 2 sets the generation pattern.

- **Open-mode + invalid-token behavior (subtle edge case):** PITFALLS.md Auth Pitfall 10 defines the correct behavior: a request with an invalid Bearer token in open mode should return 401, not be passed through. This is not explicit in the FEATURES.md MVP definition. Phase 3 must include an explicit test for `open mode + wrong key = 401` before the middleware is considered done.

## Sources

### Primary (HIGH confidence)
- [rand_core 0.9.0 docs.rs](https://docs.rs/rand_core/0.9.0/rand_core/) — OsRng, os_rng feature, TryRngCore::try_fill_bytes API
- [blake3 1.8.3 docs.rs](https://docs.rs/blake3/latest/blake3/) — hash() API, 32-byte output, version 1.8.3 confirmed
- [constant_time_eq 0.4.2 docs.rs](https://docs.rs/constant_time_eq/latest/constant_time_eq/) — constant_time_eq_32() for 32-byte comparison
- [clap 4.6.0 docs.rs](https://docs.rs/clap/latest/clap/) — Parser + Subcommand derive macros, version 4.6.0 confirmed
- [axum 0.8.4 middleware docs](https://docs.rs/axum/0.8.0/axum/middleware/index.html) — from_fn_with_state pattern, route_layer vs layer
- [axum route_layer vs layer discussion](https://github.com/tokio-rs/axum/discussions/2878) — 401 vs 404 on unmatched routes confirmed
- [CVE GHSA-wr9h-g72x-mwhm — vLLM timing attack on API key comparison](https://github.com/vllm-project/vllm/security/advisories/GHSA-wr9h-g72x-mwhm) — real-world disclosure of == comparison on Bearer tokens, High severity, 2025
- [OWASP Password Storage Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html) — fast hashes appropriate for high-entropy tokens; Argon2/bcrypt only for low-entropy passwords
- Existing v1.1 source code (`src/*.rs`) — direct inspection of AppState, Config, db::open(), error.rs patterns
- Existing Cargo.toml — confirmed reqwest 0.13, rusqlite 0.37, axum 0.8 (definitive source of truth)

### Secondary (MEDIUM confidence)
- [async-openai Cargo.toml on GitHub](https://github.com/64bit/async-openai/blob/main/async-openai/Cargo.toml) — reqwest 0.12 dependency confirmed; justifies exclusion
- [How prefix.dev implemented API keys](https://prefix.dev/blog/how_we_implented_api_keys) — pfx_ prefix pattern, show-once UX; Argon2 recommendation overridden by OWASP reasoning
- [Stripe API Keys Documentation](https://docs.stripe.com/keys) — sk_live_/sk_test_ prefix conventions, restricted keys, show-once pattern
- [Common Risks of Giving Your API Keys to AI Agents — Auth0](https://auth0.com/blog/api-key-security-for-ai-agents/) — scope enforcement failures, overly broad permissions, single-key-for-all-agents anti-pattern
- [Zero Downtime Migration of API Authentication — Zuplo](https://dev.to/zuplo/zero-downtime-migration-of-api-authentication-h9c) — dual-mode auth, migration cliff, backward compatibility
- [PSA: SQLite connection pool write performance — Evan Schwartz](https://emschwartz.me/psa-your-sqlite-connection-pool-might-be-ruining-your-write-performance/) — single-writer performance data (20x difference)
- [Unit 42 / Palo Alto: Indirect Prompt Injection Poisons AI Long-Term Memory](https://unit42.paloaltonetworks.com/indirect-prompt-injection-poisons-ai-longterm-memory/) — persistent memory attack via summarization (v1.1 compaction risk)

### Tertiary (LOW confidence — needs validation during implementation)
- None identified for v1.2 scope. All material claims are HIGH or MEDIUM confidence.

---
*Research completed: 2026-03-20*
*Ready for roadmap: yes*
