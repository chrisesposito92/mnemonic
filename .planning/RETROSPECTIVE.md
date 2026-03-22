# Project Retrospective

*A living document updated after each milestone. Lessons feed forward into future planning.*

## Milestone: v1.0 — MVP

**Shipped:** 2026-03-20
**Phases:** 5 | **Plans:** 11

### What Was Built
- Single Rust binary with embedded SQLite+sqlite-vec for vector search
- Local all-MiniLM-L6-v2 embeddings via candle with optional OpenAI fallback
- REST API (5 endpoints) with multi-agent namespacing and session-scoped retrieval
- GitHub Actions cross-platform release workflow
- Comprehensive README with quickstart, API reference, and examples

### What Worked
- Phase dependency ordering (foundation -> embedding -> API -> distribution) meant each phase had stable inputs from the prior one
- MockEmbeddingEngine with deterministic hash-based vectors enabled fast API integration tests without 90MB model download
- Milestone audit after Phase 4 caught two integration gaps (dead config knob, missing example field) before shipping — Phase 5 closed them cleanly
- Coarse-grained parallelization mode kept execution focused without over-splitting plans

### What Was Inefficient
- SUMMARY.md frontmatter `requirements_completed` fields left empty in Phases 2 and 3 — bookkeeping gap caught only during audit
- Research phase concerns (batch embedding shapes, KNN pre-filter syntax, OpenAI truncation) were resolved during implementation but never formally closed in STATE.md

### Patterns Established
- `validate_config()` at startup — fail-fast for invalid config combinations
- CTE over-fetch pattern for filtered KNN search (10x multiplier, capped at 1000)
- Arc<Mutex<Inner>> pattern for non-Send model types shared across async handlers
- OnceLock shared test fixtures to prevent resource contention in parallel test runs

### Key Lessons
1. Run milestone audit before declaring done — it catches integration-level gaps that phase-level verification misses
2. SUMMARY.md frontmatter is cheap to fill during execution; filling it retroactively during audit is more expensive
3. sqlite-vec CTE over-fetch is a pragmatic KNN pre-filter workaround — document the multiplier rationale

### Cost Observations
- Model mix: balanced profile throughout
- Notable: entire v1.0 delivered in a single day (2026-03-19 → 2026-03-20)

---

## Milestone: v1.1 — Memory Compaction

**Shipped:** 2026-03-20
**Phases:** 4 | **Plans:** 6

### What Was Built
- LLM config + validation foundation with LlmError types (mirrors embedding_provider pattern)
- SummarizationEngine trait with prompt-injection-resistant OpenAiSummarizer (XML delimiters)
- CompactionService with greedy pairwise vector clustering and atomic SQLite merge transactions
- Tier 1 (algorithmic) / Tier 2 (LLM) content synthesis with automatic fallback
- POST /memories/compact endpoint with dry_run, agent isolation, and old-to-new ID mapping
- compact_runs audit table for compaction history tracking

### What Worked
- Mirroring existing patterns (SummarizationEngine = EmbeddingEngine, CompactionService = peer of MemoryService) kept architecture consistent and review fast
- Phase sequencing (config -> engine -> service -> HTTP) meant each phase had stable inputs; no cross-phase rework
- MockSummarizer enabled deterministic compaction integration tests without LLM dependency
- Milestone audit (12/12 requirements, 12/12 integration checks, 6/6 flows) confirmed complete coverage before shipping
- SQLite error-swallowing pattern for idempotent migrations solved ALTER TABLE limitation cleanly

### What Was Inefficient
- None significant — v1.1 was a clean 4-phase execution in a single day with zero rework

### Patterns Established
- XML delimiters for prompt injection prevention in all LLM-facing prompts
- Error-swallowing pattern for SQLite idempotent schema migration (extended_code==1)
- Greedy first-match clustering via 4-arm match on cluster_id pairs
- Separate build_test_compact_state() helper to isolate compaction test setup

### Key Lessons
1. Trait mirroring (new engine trait = existing engine pattern) reduces design decisions and review friction to near-zero
2. Tiered feature delivery (Tier 1 works for everyone, Tier 2 opt-in) prevents LLM dependency from blocking core functionality
3. Keeping CompactionService as a peer (not child) of MemoryService avoids coupling and simplifies AppState wiring

### Cost Observations
- Model mix: balanced profile throughout
- Notable: entire v1.1 delivered same day as v1.0 completion — 4 phases in ~4 hours

---

## Milestone: v1.2 — Authentication / API Keys

**Shipped:** 2026-03-21
**Phases:** 5 | **Plans:** 8

### What Was Built
- Auth schema foundation (api_keys DDL, Unauthorized error variant, auth module skeleton)
- KeyService with BLAKE3 hashing, constant-time comparison, and create/list/revoke/validate methods
- Axum auth middleware via route_layer with Bearer token enforcement and open-mode bypass
- REST key management endpoints (POST/GET/DELETE /keys) with scope enforcement across all handlers
- CLI key management (`mnemonic keys create/list/revoke`) with dual-mode binary (DB-only fast path)

### What Worked
- Layered phase approach (schema -> service -> middleware -> HTTP -> CLI) prevented cross-phase rework
- Per-request COUNT for open mode detection (instead of startup flag) enabled live auth mode switching without restart
- route_layer() scoping prevented 401 on unmatched routes — an early architectural decision that saved debugging time
- Scope enforcement as a free function (not method) kept handlers uniform and testable in isolation
- Dual-mode binary architecture parsed CLI args before any initialization, guaranteeing fast CLI path

### What Was Inefficient
- SUMMARY.md `requirements_completed` frontmatter missing in 9 of 8 plan summaries (same bookkeeping gap as v1.0/v1.1) — tech debt flagged in audit
- Phase 10 required per-item #[allow(dead_code)] annotations on stub types; cleaned up by Phase 12 as stubs got consumed

### Patterns Established
- `route_layer()` not `layer()` for middleware that should only apply to matched routes
- Option<Extension<AuthContext>> for optional auth context (preserves open-mode behavior without middleware changes)
- CLI module as self-contained unit, wired into main.rs dual-dispatch
- `find_by_display_id` pattern — hash-derived 8-char prefix for human-friendly key identification

### Key Lessons
1. Dead code annotations on stubs are acceptable when phases build incrementally — just track cleanup in later phases
2. SUMMARY frontmatter requirements_completed gap persists across all 3 milestones; consider making it part of execution, not retroactive audit
3. Per-request mode detection > startup flags when the mode can change at runtime (key creation/revocation)
4. Auth middleware scoping choice (route_layer vs layer) has outsized impact — decide it early and document why

### Cost Observations
- Model mix: balanced profile throughout
- Notable: 5 phases in 2 days with 66 commits; deepest phase (13) required splitting into 2 plans for scope enforcement + REST endpoints

---

## Milestone: v1.3 — CLI

**Shipped:** 2026-03-21
**Phases:** 6 | **Plans:** 11

### What Was Built
- CLI scaffolding with `mnemonic serve` subcommand and backward-compatible bare invocation
- `mnemonic recall` with DB-only fast path (~50ms), list/get-by-id/filter modes
- `mnemonic remember` with stdin pipe support and full agent/session/tag metadata
- `mnemonic search` with semantic search, distance-ranked tabular output, and filter flags
- `mnemonic compact` with dry-run preview, agent scoping, and threshold control
- Global `--json` flag across all subcommands with consistent exit codes and stdout/stderr separation

### What Worked
- Tiered init helpers (init_db / init_db_and_embedding / init_compaction) kept resource loading minimal per subcommand — recall stays fast at ~50ms
- Early validation before model load (empty content/query rejected before 2-3s embedding load) — pattern established in Phase 17, reused in 18-20
- Zero new Cargo.toml dependencies for entire milestone — all needs covered by existing locked stack
- Sequential phase dependency chain (15→16→17→18→19→20) meant each phase built cleanly on the prior one's init helpers
- Direct rusqlite seeding for fast-path integration tests avoided embedding model overhead in recall/keys tests
- Global --json on Cli struct (single extraction point in main.rs) made output consistency trivial across all handlers

### What Was Inefficient
- Phase 16 recall one-liner SUMMARY was empty (tool extraction returned "One-liner:" with no content) — minor bookkeeping gap
- init_compaction() could not reuse init_db_and_embedding() due to different return types (MemoryService vs individual components for CompactionService) — acceptable but a small violation of DRY

### Patterns Established
- Tiered init helpers: init_db (DB-only, ~50ms), init_db_and_embedding (DB+model, ~2-3s), init_compaction (DB+model+LLM)
- Early input validation before expensive initialization (empty string check before model load)
- Global flag extraction before match dispatch in main.rs (avoids Rust partial-move errors)
- seed_memory() for fast-path tests vs `mnemonic remember` for embedding-dependent tests
- stderr audit trail regardless of --json mode (operators always see progress, scripts parse stdout)

### Key Lessons
1. Init tier design is critical for CLI UX — recall at ~50ms feels instant while remember at ~2-3s is acceptable with user feedback
2. Rust partial-move compile errors are the #1 recurring pattern in CLI dispatch — extract values before match arms
3. Direct rusqlite seeding vs mnemonic remember for test setup is a deliberate choice: use seed_memory() when embeddings don't matter, use the binary when they do (compact needs real embeddings for clustering)
4. Global flags on the Cli struct (not per-subcommand) ensure consistency without per-handler plumbing

### Cost Observations
- Model mix: balanced profile throughout
- Notable: 6 phases in 2 days with 70 commits; most phases completed in single sessions
- Test suite grew from 194 → 239 (45 new tests, all CLI integration)

---

## Milestone: v1.4 — Pluggable Storage Backends

**Shipped:** 2026-03-22
**Phases:** 5 | **Plans:** 9

### What Was Built
- StorageBackend async trait with 7 methods + SqliteBackend wrapping existing code (zero behavior change)
- MemoryService and CompactionService decoupled from SQLite via Arc<dyn StorageBackend>
- Config extension with storage_provider field, create_backend() factory, and `mnemonic config show` CLI
- QdrantBackend (807 lines) behind backend-qdrant feature flag with gRPC, score normalization, multi-agent filtering
- PostgresBackend (548 lines) behind backend-postgres feature flag with pgvector, atomic transactional compaction
- Gap closure phase (25) for secret redaction, dead code, and frontmatter backfill

### What Worked
- Trait-first design (Phase 21) meant QdrantBackend (Phase 23) and PostgresBackend (Phase 24) could follow the SqliteBackend template exactly — no discovery work during implementation
- Feature-gated backends (backend-qdrant, backend-postgres) kept the default binary zero-dependency; each backend is a clean opt-in
- create_backend() factory centralized all backend construction — adding a new backend is one match arm
- Dual-connection CompactionService design (backend + audit_db) preserved audit logging across all backends without leaking SQLite concerns into the trait
- Milestone audit caught postgres_url redaction gap and dead_code annotation before shipping — Phase 25 closed them cleanly
- Research phases (21, 23, 24) upfront resolved API surface questions (qdrant-client scroll vs query, sqlx bind patterns) before hitting code

### What Was Inefficient
- SUMMARY.md one-liner field extraction unreliable — 4 of 9 summaries returned empty "One-liner:" (recurring bookkeeping issue)
- 21-02-SUMMARY.md `requirements-completed` frontmatter left empty — same gap as prior milestones, only partially fixed in Phase 25
- recall CLI bypass of StorageBackend not caught until milestone audit — a v1.3 scope issue surfacing as v1.4 tech debt

### Patterns Established
- #[async_trait] for dyn-compatible async trait objects (native async fn in traits not dyn-compatible in 2026 Rust)
- Feature gate errors at create_backend() not validate_config() — keeps config portable across builds
- Per-cluster write_compaction_result() atomicity — backend-neutral pattern replacing SQLite-specific all-clusters transaction
- sqlx default-features=false to avoid libsqlite3-sys version conflict with rusqlite
- Julian Day Number algorithm for ISO 8601 conversion without chrono dependency (Qdrant backend)

### Key Lessons
1. Trait-first abstraction pays off: defining the contract before any implementation made all 3 backends consistent with no cross-phase rework
2. Feature-gated optional dependencies are the right granularity — users who don't need Qdrant/Postgres never download those crates
3. Milestone audit remains essential — it caught a real security gap (postgres_url not redacted) that phase-level verification missed
4. recall CLI bypassing the storage abstraction shows that older code doesn't automatically benefit from new abstractions — explicit migration is needed
5. Research phases upfront for unfamiliar APIs (qdrant-client, sqlx) prevented implementation-time surprises and kept plans accurate

### Cost Observations
- Model mix: balanced profile throughout
- Notable: 5 phases in 2 days with 41 commits; QdrantBackend and PostgresBackend followed template pattern from SqliteBackend
- Test suite grew from 239 → 286 (47 new tests across default, qdrant, and postgres feature sets)

---

## Milestone: v1.5 — gRPC

**Shipped:** 2026-03-22
**Phases:** 4 | **Plans:** 7

### What Was Built
- MnemonicService proto contract (4 RPCs) with feature-gated tonic 0.13 / prost 0.13 build pipeline
- Dual-port REST+gRPC startup via tokio::try_join! with configurable grpc_port
- Async Tower auth layer (GrpcAuthLayer/GrpcAuthService) with open-mode bypass and health/reflection bypass
- All 4 gRPC handlers (StoreMemory, SearchMemories, ListMemories, DeleteMemory) with scope enforcement and status code mapping
- tonic-reflection for grpcurl discoverability and tonic-health for health checks
- 14 gRPC integration tests covering happy paths, input validation, per-handler scope enforcement
- Recall CLI routed through StorageBackend trait — v1.4 tech debt (DEBT-01) resolved

### What Worked
- Research phases identified critical constraints upfront: tonic 0.13 needed for prost compatibility with qdrant-client, async Tower Layer required (sync interceptor panics in tokio), dual-port mandatory (same-port multiplexing has known bugs)
- Feature-gating pattern from v1.4 (backend-qdrant, backend-postgres) applied cleanly to interface-grpc — default binary unchanged
- Phase 29 (tech debt fix) ran in parallel with Phases 27-28 thanks to independent dependency graph — efficient use of worktrees
- Milestone audit (18/18 requirements, 17/18 integration links, 5/6 E2E flows) confirmed coverage; the one broken flow (CI release binary REST-only) is by-design
- MockEmbeddingEngine pattern from v1.0 reused in gRPC integration tests — no model download, fast test suite

### What Was Inefficient
- SUMMARY.md one-liner extraction still unreliable — 3 of 7 summaries returned malformed one-liners during milestone complete (fixed manually in MILESTONES.md)
- tonic-build must be non-optional in [build-dependencies] (build scripts always compile) — plan assumed optional=true would work; caught and fixed during Phase 26 execution
- 4 auto-fix deviations in Phase 27 Plan 02 (http crate as direct dep, tower as optional dep, lib.rs mod for test discovery, BoxCloneService in tests) — plan didn't account for Rust's transitive crate visibility rules

### Patterns Established
- CARGO_FEATURE_INTERFACE_GRPC env var check in build.rs for conditional build-time codegen
- Tower Layer+Service pattern for async gRPC auth (clone+swap in Service::call)
- Duplicate enforce_scope between REST (server.rs) and gRPC (grpc/mod.rs) to avoid axum import coupling
- api_error_to_status() mapping all ApiError variants to tonic::Status codes
- cfg-gated dual-port startup in main.rs — falls through to REST-only without feature

### Key Lessons
1. Build-dependency optionality doesn't work as expected in Rust — build scripts always compile regardless of features; use env var gates instead
2. Transitive crate dependencies (http via axum, tower via dev-deps) are not usable in library code — always declare direct dependencies for crates you import
3. Proto-first design with explicit rerun-if-changed prevents always-dirty incremental build bug (tonic-build #2239)
4. Tower auth layer is strictly required when auth logic is async — sync interceptors panic inside tokio runtime
5. Two tonic versions in the tree (0.12 from qdrant-client, 0.13 from our server) work fine when they share prost — version duplication is acceptable when types don't cross boundaries

### Cost Observations
- Model mix: balanced profile throughout
- Notable: entire v1.5 delivered in 1 day (2026-03-22) — 4 phases, 7 plans, 14 feature commits
- Test suite: 286 passing (91 lib + 54 integration + 14 gRPC integration), 1 ignored

---

## Cross-Milestone Trends

### Process Evolution

| Milestone | Phases | Plans | Key Change |
|-----------|--------|-------|------------|
| v1.0 | 5 | 11 | Baseline — established GSD workflow with audit-driven gap closure |
| v1.1 | 4 | 6 | Pattern mirroring reduced design overhead; tiered delivery (Tier 1/2) |
| v1.2 | 5 | 8 | Layered auth (schema->service->middleware->HTTP->CLI); per-request mode detection |
| v1.3 | 6 | 11 | Tiered init helpers for CLI UX; zero new dependencies; global --json consistency |
| v1.4 | 5 | 9 | Trait-first abstraction; feature-gated optional backends; research phases upfront |
| v1.5 | 4 | 7 | Dual-protocol serving; Tower auth layer; feature-gated interface; parallel phase execution |

### Cumulative Quality

| Milestone | Tests | Zero Warnings | Nyquist |
|-----------|-------|---------------|---------|
| v1.0 | 30 | Yes | COMPLIANT |
| v1.1 | 68 (35 unit + 33 integration) | Yes | COMPLIANT |
| v1.2 | 194 (57 unit + 53 integration) | Yes | COMPLIANT |
| v1.3 | 239 (63 unit + 55+54 integration) | Yes | COMPLIANT |
| v1.4 | 286 (across default + feature sets) | Yes | COMPLIANT |
| v1.5 | 286 (91 lib + 54 int + 14 gRPC int) | Yes | COMPLIANT |

### Top Lessons (Verified Across Milestones)

1. Milestone audits before shipping catch integration gaps that per-phase verification misses (v1.0, v1.1, v1.2, v1.3, v1.4 — caught postgres_url redaction gap)
2. Mirror existing trait patterns when adding new engines — consistent architecture and near-zero design friction (v1.0 EmbeddingEngine -> v1.1 SummarizationEngine -> v1.4 StorageBackend)
3. SUMMARY frontmatter `requirements_completed` field is consistently skipped during execution — recurring debt across all 5 milestones; needs process fix
4. Init tier design is load-bearing for CLI UX — users feel the difference between 50ms and 3s (v1.3)
5. Zero new dependencies for an entire milestone is achievable when the foundation is well-designed (v1.3)
6. Trait-first abstraction design (define contract before implementation) makes subsequent implementations template-driven with near-zero rework (v1.4)
7. Build-dependency optionality in Rust doesn't work as expected — use env var gates for conditional build-time codegen (v1.5)
8. Feature-gating pattern (v1.4 backends) transfers cleanly to new domains (v1.5 interface-grpc) — consistent opt-in architecture
