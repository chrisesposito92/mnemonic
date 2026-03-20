---
phase: 08-compaction-core
plan: "01"
subsystem: database
tags: [rust, sqlite, tokio-rusqlite, cosine-similarity, clustering, compaction, zerocopy, serde_json, uuid]

requires:
  - phase: 07-summarization-engine
    provides: SummarizationEngine trait and OpenAiSummarizer for Tier 2 content synthesis
  - phase: 06-foundation
    provides: compact_runs table DDL, source_ids column on memories, vec_memories virtual table schema
  - phase: 05-embedding-core
    provides: EmbeddingEngine trait and L2-normalized embedding guarantees (dot product = cosine sim)

provides:
  - CompactionService struct in src/compaction.rs with full compact() pipeline
  - Greedy pairwise clustering with configurable threshold (default 0.85) and max_candidates cap (default 100)
  - Atomic write: INSERT merged memory + vec embedding + DELETE source memories in a single SQLite transaction
  - dry_run mode: full pipeline without writes, compact_runs audit row still created
  - 10 unit tests for all pure helper functions (cosine_similarity, clustering, tier1_concat, union_tags)

affects:
  - 09-compaction-endpoint (wires CompactionService to HTTP endpoint)
  - Any phase that reads compact_runs or merged memories

tech-stack:
  added: []
  patterns:
    - "CompactionService mirrors MemoryService: Arc<Connection> + engine deps, db.call(move |c| ...) for all SQLite access"
    - "Greedy pairwise clustering: compute_pairs (all i<j above threshold) sorted desc, then cluster_candidates first-match assignment"
    - "Atomic merge transaction: INSERT merged + INSERT vec + DELETE source vecs + DELETE source memories in one c.transaction()"
    - "Embedding read-back: unsafe slice reinterpret from Vec<u8> BLOB with // SAFETY comment documenting sqlite-vec invariant"
    - "Tier 1/2 synthesis: SummarizationEngine.summarize() with warn-level fallback to chronological concat on any LlmError"

key-files:
  created:
    - src/compaction.rs
  modified:
    - src/lib.rs
    - src/main.rs
    - src/summarization.rs

key-decisions:
  - "cosine_similarity = dot product — valid because all EmbeddingEngine implementations guarantee L2 norm ≈ 1.0"
  - "compute_pairs filters by threshold before sorting — avoids sorting all O(n²) pairs when most are below threshold"
  - "Atomic write uses single db.call closure containing entire c.transaction() — required by tokio-rusqlite architecture"
  - "dry_run still creates compact_runs row (dry_run=1) per CONTEXT.md audit requirement"
  - "Removed #![allow(dead_code)] from summarization.rs — CompactionService now consumes SummarizationEngine"
  - "MemoryService construction uses embedding_model.clone() instead of move so CompactionService can share it"

patterns-established:
  - "Embedding BLOB read-back: row.get::<_, Vec<u8>>(N)? then unsafe slice::from_raw_parts cast with SAFETY doc comment"
  - "Greedy first-match: cluster_id Vec<Option<usize>> with match on (cluster_id[i], cluster_id[j]) four-arm pattern"
  - "compact_runs lifecycle: INSERT status='running' before pipeline, UPDATE status='completed'|'failed' after"

requirements-completed: [DEDUP-01, DEDUP-02, DEDUP-03, DEDUP-04]

duration: 3min
completed: "2026-03-20"
---

# Phase 08 Plan 01: CompactionService Core Summary

**Greedy-pairwise vector clustering with atomic SQLite merge transaction, Tier 1/2 content synthesis, dry_run audit mode, and 10 pure-function unit tests — all wired to main.rs via shared db_arc and embedding**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-20T15:23:47Z
- **Completed:** 2026-03-20T15:27:00Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Implemented CompactionService with full compact() pipeline: fetch_candidates -> compute_pairs -> cluster_candidates -> synthesize_content -> atomic write
- Greedy pairwise clustering uses dot-product cosine similarity (valid since EmbeddingEngine L2-normalizes all embeddings), with configurable threshold and max_candidates cap
- Atomic write in a single SQLite transaction prevents orphaned rows on failure; dry_run mode skips write but creates compact_runs audit row
- Wired CompactionService in main.rs consuming llm_engine (renamed from _llm_engine), sharing db_arc and embedding with MemoryService
- All 35 lib tests pass; zero build errors

## Task Commits

Each task was committed atomically:

1. **Task 1: Create CompactionService with types, pure helpers, and unit tests** - `7ab5c8b` (feat)
2. **Task 2: Wire CompactionService in lib.rs and main.rs** - `b7f7034` (feat)

## Files Created/Modified

- `src/compaction.rs` - CompactionService struct, CompactRequest/Response/ClusterMapping types, pure helpers (cosine_similarity, compute_pairs, cluster_candidates, tier1_concat, union_tags, earliest_created_at), async fetch_candidates/synthesize_content/compact, 10 unit tests
- `src/lib.rs` - Added pub mod compaction
- `src/main.rs` - Added mod compaction, renamed _llm_engine to llm_engine, constructed CompactionService as _compaction_service
- `src/summarization.rs` - Removed #![allow(dead_code)] suppressor

## Decisions Made

- Kept `_compaction_service` with underscore prefix in main.rs — Phase 9 will add it to AppState and HTTP routing
- Used `embedding_model.clone()` in MemoryService construction so the string can also be passed to CompactionService (both share the same model name)
- Removed LlmError/MnemonicError from compaction.rs imports — error conversion uses `.into()` via existing From impls, no explicit type references needed
- Warnings from dead_code in binary crate are expected — Phase 9 will wire the HTTP endpoint and eliminate them

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None - all patterns from service.rs translated directly. The borrow conflict pitfall (statement drop before transaction) was pre-empted by placing the entire transaction inside the db.call closure without any prior c.prepare() calls at the closure level.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- CompactionService is constructed and fully functional, ready for Phase 9 to add it to AppState and expose the POST /compact HTTP endpoint
- The only remaining blocker from STATE.md (multi-agent isolation integration test) applies to Phase 9 — cross-namespace compaction must be tested before ship

---
*Phase: 08-compaction-core*
*Completed: 2026-03-20*
