# Milestones

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
