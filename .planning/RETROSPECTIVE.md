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

## Cross-Milestone Trends

### Process Evolution

| Milestone | Phases | Plans | Key Change |
|-----------|--------|-------|------------|
| v1.0 | 5 | 11 | Baseline — established GSD workflow with audit-driven gap closure |
| v1.1 | 4 | 6 | Pattern mirroring reduced design overhead; tiered delivery (Tier 1/2) |
| v1.2 | 5 | 8 | Layered auth (schema->service->middleware->HTTP->CLI); per-request mode detection |
| v1.3 | 6 | 11 | Tiered init helpers for CLI UX; zero new dependencies; global --json consistency |

### Cumulative Quality

| Milestone | Tests | Zero Warnings | Nyquist |
|-----------|-------|---------------|---------|
| v1.0 | 30 | Yes | COMPLIANT |
| v1.1 | 68 (35 unit + 33 integration) | Yes | COMPLIANT |
| v1.2 | 194 (57 unit + 53 integration) | Yes | COMPLIANT |
| v1.3 | 239 (63 unit + 55+54 integration) | Yes | COMPLIANT |

### Top Lessons (Verified Across Milestones)

1. Milestone audits before shipping catch integration gaps that per-phase verification misses (v1.0, v1.1, v1.2, v1.3)
2. Mirror existing trait patterns when adding new engines — consistent architecture and near-zero design friction (v1.0 EmbeddingEngine -> v1.1 SummarizationEngine)
3. SUMMARY frontmatter `requirements_completed` field is consistently skipped during execution — recurring debt across all 4 milestones; needs process fix
4. Init tier design is load-bearing for CLI UX — users feel the difference between 50ms and 3s (v1.3)
5. Zero new dependencies for an entire milestone is achievable when the foundation is well-designed (v1.3)
