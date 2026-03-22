# Milestones

## v1.5 gRPC (Shipped: 2026-03-22)

**Phases completed:** 4 phases, 7 plans, 13 tasks

**Key accomplishments:**

- MnemonicService proto contract locked with tonic 0.13 / prost 0.13, feature-gated build pipeline verified — default binary unchanged, feature build generates Rust types in 10s, incremental build clean at 0.15s
- Dual-port REST+gRPC startup via tokio::try_join! with configurable grpc_port, tonic-health for health checks
- Async Tower auth middleware for gRPC — reuses KeyService for open-mode bypass, bearer token validation, and AuthContext injection
- All 4 gRPC handlers (StoreMemory, SearchMemories, ListMemories, DeleteMemory) with scope enforcement, error mapping, and tonic-reflection for grpcurl discoverability
- 14 gRPC integration tests covering happy paths, input validation, per-handler scope enforcement, and health/reflection
- Recall CLI routed through StorageBackend trait — v1.4 tech debt (DEBT-01) resolved

**Delivered:** gRPC interface for high-throughput agent-to-server communication — 4 unary RPCs mirroring REST behavior, async Tower auth layer, tonic-health and tonic-reflection, dual-port serving alongside REST, all behind the `interface-grpc` feature flag.

**Stats:**

- Lines of Rust: 11,940 (total)
- Lines changed: +1,477 / -90
- Files modified: 11
- Feature commits: 14
- Timeline: 2026-03-22 (1 day)
- Tests: 286 passing (91 lib + 54 integration + 14 gRPC integration), 1 ignored
- Requirements: 18/18 satisfied
- Nyquist: COMPLIANT (all 4 phases)
- Audit: TECH_DEBT (8 non-critical items, no blockers)
- Git range: feat(26-01) → feat(29-01)

---

## v1.4 Pluggable Storage Backends (Shipped: 2026-03-22)

**Phases completed:** 5 phases, 9 plans

**Key accomplishments:**

- StorageBackend async trait with 7 methods + SqliteBackend wrapping existing code with zero behavior change — all 247 tests passing
- MemoryService and CompactionService fully decoupled from SQLite via Arc<dyn StorageBackend> with dual-connection design for audit logging
- Config extension with storage_provider field, create_backend() factory, `mnemonic config show` with secret redaction, health endpoint reports backend name
- QdrantBackend (807 lines) behind `backend-qdrant` feature flag — gRPC via qdrant-client, score-to-distance normalization, multi-agent payload filtering
- PostgresBackend (548 lines) behind `backend-postgres` feature flag — pgvector cosine distance, atomic transactional compaction via BEGIN/COMMIT
- All secrets redacted (including postgres_url), dead code resolved, SUMMARY frontmatter backfilled across all phases

**Delivered:** Pluggable storage layer — SQLite remains the zero-config default while Qdrant and Postgres are available as opt-in backends behind feature flags, with a config CLI for backend inspection and full secret redaction.

**Stats:**

- Lines of Rust: 10,763 (total)
- Lines changed: +12,969 / -487
- Files modified: 57
- Commits: 41
- Timeline: 2026-03-21 → 2026-03-22 (2 days)
- Tests: 286 passing (84+84+60+4+54 across crates), 1 ignored
- Requirements: 17/17 satisfied
- Nyquist: COMPLIANT (all 5 phases)
- Audit: TECH_DEBT (3 minor items, no blockers)
- Git range: docs(21) → docs(25-01)

### Known Gaps

- `recall` CLI bypasses StorageBackend — uses raw SQLite regardless of storage_provider (cli.rs:455, v1.3 scope — defer to v1.5)

---

## v1.3 CLI (Shipped: 2026-03-21)

**Phases completed:** 6 phases, 11 plans, 19 tasks

**Key accomplishments:**

- CLI scaffolding with `mnemonic serve` subcommand and backward-compatible bare invocation via Commands enum dispatch
- Fast-path `mnemonic recall` subcommand with DB-only init (~50ms), list/get-by-id/filter modes, and shared init_db() helper
- `mnemonic remember` subcommand with stdin pipe support, medium-init embedding helper, and full agent/session/tag metadata
- `mnemonic search` subcommand with semantic search, distance-ranked tabular output, and early empty-query validation
- `mnemonic compact` subcommand with full CompactionService init, dry-run preview, agent scoping, and threshold control
- Global `--json` flag across all subcommands with consistent exit codes and stdout/stderr separation

**Delivered:** Full CLI toolset — every operation available from the terminal: serve, remember, recall, search, compact, and key management, with machine-readable JSON output and consistent exit codes.

**Stats:**

- Lines of Rust: 22,198 (total)
- Lines changed: +12,891 / -160
- Files modified: 55
- Commits: 70
- Timeline: 2026-03-19 → 2026-03-21 (2 days)
- Tests: 239 passing, zero compiler warnings
- Requirements: 18/18 satisfied
- Nyquist: COMPLIANT (all 6 phases)
- Git range: feat(15-01) → docs(v1.3)

---

## v1.2 Authentication / API Keys (Shipped: 2026-03-21)

**Phases completed:** 5 phases, 8 plans, 10 tasks

**Key accomplishments:**

- Auth schema foundation with api_keys DDL, Unauthorized error variant, and auth module wired into AppState with startup auth-mode log (OPEN/ACTIVE)
- KeyService with BLAKE3 hashing, OsRng token generation, and constant_time_eq_32 validation — create/list/revoke/validate with 11 unit tests
- Axum auth middleware via route_layer with Bearer token enforcement, open-mode bypass (per-request COUNT, no startup flag), and health-check exemption
- Scope enforcement across 5 handlers and REST key management endpoints (POST/GET/DELETE /keys) with 8 integration tests proving AUTH-04 end-to-end
- CLI key management (`mnemonic keys create/list/revoke`) with dual-mode binary — fast DB-only path, no embedding model loading

**Delivered:** Optional API key authentication with agent-scoped namespace isolation, REST key management, and CLI tooling — off by default, auth activates live when keys exist.

**Stats:**

- Lines of Rust: 5,925 (total)
- Lines changed: +10,077 / -112
- Files modified: 53
- Commits: 66
- Timeline: 2026-03-19 → 2026-03-21 (2 days)
- Tests: 194 passing (57 unit + 53 integration), zero compiler warnings
- Requirements: 15/15 satisfied
- Nyquist: COMPLIANT (all 5 phases)
- Git range: feat(10-01) → docs(phase-14)

---

## v1.1 Memory Compaction (Shipped: 2026-03-20)

**Phases completed:** 4 phases, 6 plans, 11 tasks

**Key accomplishments:**

- Config struct extended with 4 LLM Option<String> fields, validate_config() gains independent LLM validation block, and LlmError enum with 3 variants wired into MnemonicError via #[from]
- source_ids column and compact_runs audit table added to SQLite schema with idempotent migration, verified by 3 new integration tests (23 total passing)
- SummarizationEngine trait with OpenAiSummarizer (XML-delimited prompt injection prevention, typed error mapping via reqwest) and MockSummarizer (deterministic), wired into main.rs as optional engine
- Greedy-pairwise vector clustering with atomic SQLite merge transaction, Tier 1/2 content synthesis, dry_run audit mode, and 10 pure-function unit tests — all wired to main.rs via shared db_arc and embedding
- 6 integration tests verifying CompactionService end-to-end: atomic write + source deletion, dry_run no-op, agent namespace isolation, max_candidates truncation, and MockSummarizer Tier 2 LLM path
- POST /memories/compact endpoint exposing CompactionService via axum with input validation and 4 HTTP-layer integration tests covering all API-01 through API-04 requirements

**Delivered:** Agent-triggered memory compaction with algorithmic dedup baseline (Tier 1) and optional LLM-powered summarization (Tier 2) — no background magic, no LLM required.

**Stats:**

- Lines of Rust: 3,678 (total)
- Lines changed: +7,533 / -83
- Commits: 47
- Timeline: 2026-03-20 (1 day)
- Tests: 68 (35 unit + 33 integration), zero compiler warnings
- Requirements: 12/12 satisfied
- Nyquist: COMPLIANT (all 4 phases)
- Git range: feat(06-01) → docs(v1.1)

---

## v1.0 MVP (Shipped: 2026-03-20)

**Phases completed:** 5 phases, 11 plans, 0 tasks

**Delivered:** A single Rust binary that gives any AI agent persistent memory via a simple REST API — zero external dependencies, download and run.

**Key accomplishments:**

1. SQLite+sqlite-vec foundation with WAL mode, async db access via tokio-rusqlite, and layered configuration (env vars + TOML)
2. Local all-MiniLM-L6-v2 embeddings via candle (pure Rust) with optional OpenAI API fallback
3. Full REST API: 5 endpoints (POST/GET/DELETE /memories, GET /memories/search, GET /health) with MemoryService orchestrator
4. Multi-agent namespacing by agent_id with KNN pre-filtering; session-scoped retrieval via session_id
5. Distribution: comprehensive README with quickstart/API reference/examples, MIT license, GitHub Actions cross-platform release workflow
6. Config validation (validate_config()) and dead code cleanup closing all v1.0 audit integration gaps

**Stats:**

- Lines of Rust: 1,932
- Files modified: 69
- Timeline: 2026-03-19 → 2026-03-20 (1 day)
- Tests: 30 passing, zero compiler warnings
- Requirements: 24/24 satisfied
- Nyquist: COMPLIANT (all 5 phases)

---
