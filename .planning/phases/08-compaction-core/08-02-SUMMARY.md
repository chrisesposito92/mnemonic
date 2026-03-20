---
phase: 08-compaction-core
plan: 02
subsystem: testing
tags: [rust, tokio, rusqlite, sqlite-vec, compaction, integration-tests]

# Dependency graph
requires:
  - phase: 08-01
    provides: CompactionService with compact(), CompactRequest, CompactResponse, ClusterMapping
provides:
  - 6 integration tests covering all 4 DEDUP requirements through the full CompactionService pipeline
  - build_test_compaction() helper for reuse in future compaction tests
  - Regression test suite protecting Phase 9 HTTP integration from compaction regressions
affects:
  - 09-http-integration

# Tech tracking
tech-stack:
  added: []
  patterns:
    - build_test_compaction(with_llm: bool) helper pattern: creates isolated in-memory DB shared by CompactionService + MemoryService
    - Each integration test uses fresh DB (no shared state) — prevents test interference

key-files:
  created: []
  modified:
    - tests/integration.rs

key-decisions:
  - "dry_run returns memories_created=0 (not a preview count) — the code in compaction.rs is authoritative; plan's test assertion of 1 was incorrect, corrected to 0"
  - "Top-level use mnemonic::compaction::{CompactionService, CompactRequest} and use mnemonic::summarization::MockSummarizer added at file scope; CreateMemoryRequest kept as local use in each test function to match existing style"

patterns-established:
  - "build_test_compaction(bool) pattern: reusable helper that creates a fully wired CompactionService + MemoryService over shared in-memory DB — mirrors build_test_state() for API tests"

requirements-completed: [DEDUP-01, DEDUP-02, DEDUP-03, DEDUP-04]

# Metrics
duration: 8min
completed: 2026-03-20
---

# Phase 08 Plan 02: Compaction Integration Tests Summary

**6 integration tests verifying CompactionService end-to-end: atomic write + source deletion, dry_run no-op, agent namespace isolation, max_candidates truncation, and MockSummarizer Tier 2 LLM path**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-20T15:30:00Z
- **Completed:** 2026-03-20T15:38:00Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- All 6 integration tests pass covering DEDUP-01 through DEDUP-04
- Atomic write test proves source memories are deleted and merged memory has correct tag union and earliest created_at
- dry_run test verifies zero data modification while still returning cluster preview
- Agent isolation test proves compacting Agent A leaves Agent B memories untouched
- max_candidates truncation test verifies truncated=true flag in response
- MockSummarizer test proves Tier 2 LLM path produces "MOCK_SUMMARY:" prefixed content
- Full test suite green: 35 lib unit tests + 35 main tests + 29 integration tests (1 ignored)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add compaction integration tests** - `c7d284f` (test)

**Plan metadata:** (docs commit follows)

## Files Created/Modified
- `tests/integration.rs` - Added 301 lines: 2 new top-level imports, build_test_compaction() helper, and 6 integration test functions

## Decisions Made
- `memories_created=0` in dry_run mode: the plan's test scaffold asserted `memories_created=1` but compaction.rs correctly returns 0 in dry_run (no writes performed). The test was adjusted to match the authoritative implementation. [Rule 1 - Bug fix in test assertion]
- `CreateMemoryRequest` kept as function-local `use` statements to match existing test style; `CompactionService`, `CompactRequest`, and `MockSummarizer` added at file scope since they are new to this plan.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed incorrect dry_run memories_created assertion**
- **Found during:** Task 1 (writing test_compact_dry_run)
- **Issue:** Plan's test scaffold asserted `memories_created = 1` in dry_run, but compaction.rs line 339 sets `memories_created = 0` when `dry_run=true` (no actual writes performed). Test would have failed.
- **Fix:** Changed assertion to `assert_eq!(response.memories_created, 0)` with explanatory comment.
- **Files modified:** tests/integration.rs
- **Verification:** cargo test -- test_compact_dry_run passes
- **Committed in:** c7d284f (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 — bug in plan's test assertion, corrected to match authoritative compaction.rs behavior)
**Impact on plan:** Minimal — single assertion value corrected. No scope creep. All 6 tests pass.

## Issues Encountered
None — implementation was straightforward. MockEmbeddingEngine's deterministic hash-based vectors produce identical embeddings for identical content (cosine sim = 1.0), making clustering tests reliable.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Full CompactionService pipeline validated through 6 integration tests covering all 4 DEDUP requirements
- Phase 9 (HTTP integration) can safely add the `/compact` endpoint knowing the service layer is fully tested
- build_test_compaction() helper available for any additional compaction tests in Phase 9

---
*Phase: 08-compaction-core*
*Completed: 2026-03-20*
