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

## Cross-Milestone Trends

### Process Evolution

| Milestone | Phases | Plans | Key Change |
|-----------|--------|-------|------------|
| v1.0 | 5 | 11 | Baseline — established GSD workflow with audit-driven gap closure |

### Cumulative Quality

| Milestone | Tests | Zero Warnings | Nyquist |
|-----------|-------|---------------|---------|
| v1.0 | 30 | Yes | COMPLIANT |

### Top Lessons (Verified Across Milestones)

1. Milestone audits before shipping catch integration gaps that per-phase verification misses
