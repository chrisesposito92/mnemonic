---
phase: 07-summarization-engine
plan: 01
subsystem: llm
tags: [rust, reqwest, async-trait, serde, openai, chat-completions, prompt-injection, xml]

# Dependency graph
requires:
  - phase: 06-schema-migrations
    provides: LlmError enum with ApiCall, Timeout, ParseError variants in error.rs
  - phase: 05-embedding-engine
    provides: EmbeddingEngine trait pattern (async_trait, Send+Sync, Arc<dyn>, reqwest client)
provides:
  - SummarizationEngine trait (object-safe, Send+Sync, async summarize)
  - OpenAiSummarizer with XML-delimited prompt injection prevention and typed error mapping
  - MockSummarizer for deterministic testing without network calls
  - build_data_block() helper producing <memories>/<memory index="N"> structure
  - main.rs LLM engine init block (optional OpenAI or None)
affects: [08-compaction-core, 09-compaction-api]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - SummarizationEngine mirrors EmbeddingEngine pattern exactly (async_trait, Send+Sync, Arc<dyn>)
    - XML delimiter wrapping for prompt injection prevention (system message = instructions only, user message = data block)
    - e.is_timeout() method check for timeout detection (not string matching)
    - Optional engine pattern: Option<Arc<dyn SummarizationEngine>> with None when not configured
    - _llm_engine prefix with underscore for not-yet-consumed optional engines

key-files:
  created:
    - src/summarization.rs
  modified:
    - src/lib.rs
    - src/main.rs

key-decisions:
  - "OpenAI chat completions API via raw reqwest — no async-openai (conflicts with reqwest 0.13)"
  - "XML delimiters (<memory index=\"N\">) wrap all user data; system message contains instructions only — prevents prompt injection"
  - "Timeout detected via e.is_timeout() method, not string matching — compile-time safe"
  - "_llm_engine stored as Option<Arc<dyn SummarizationEngine>> — None when no llm_provider configured"
  - "base_url defaults to https://api.openai.com/v1, model defaults to gpt-4o-mini"

patterns-established:
  - "build_data_block() is a standalone file-private function — testable independently of HTTP calls"
  - "MockSummarizer is pub — available to integration tests in Phase 8/9"

requirements-completed: [LLM-02, LLM-03, LLM-04]

# Metrics
duration: 8min
completed: 2026-03-20
---

# Phase 7 Plan 01: Summarization Engine Summary

**SummarizationEngine trait with OpenAiSummarizer (XML-delimited prompt injection prevention, typed error mapping via reqwest) and MockSummarizer (deterministic), wired into main.rs as optional engine**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-20T14:45:00Z
- **Completed:** 2026-03-20T14:53:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- SummarizationEngine async trait (object-safe, Send+Sync) mirroring EmbeddingEngine pattern exactly
- OpenAiSummarizer posts to `/chat/completions` with XML-delimited data block — system message contains only instructions, never user data
- All LLM errors mapped to typed variants: timeout via `e.is_timeout()` → `LlmError::Timeout`, HTTP errors → `LlmError::ApiCall`, JSON parse failures → `LlmError::ParseError`
- MockSummarizer returns deterministic `MOCK_SUMMARY: text1 | text2` without network calls
- main.rs constructs `OpenAiSummarizer` when `llm_provider == "openai"`, `None` otherwise
- 7 unit tests covering trait safety, Send+Sync bounds, mock output, prompt XML structure, empty input guard

## Task Commits

Each task was committed atomically:

1. **Task 1: SummarizationEngine trait, OpenAiSummarizer, MockSummarizer, unit tests** - `d02289a` (feat)
2. **Task 2: Wire LLM engine initialization in main.rs** - `1e1a9c9` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified
- `src/summarization.rs` (created) — SummarizationEngine trait, OpenAiSummarizer, build_data_block helper, MockSummarizer, 7 unit tests
- `src/lib.rs` (modified) — added `pub mod summarization;`
- `src/main.rs` (modified) — added `mod summarization;` and LLM engine init block after embedding init

## Decisions Made
- Used raw reqwest for OpenAI HTTP calls — async-openai conflicts with reqwest 0.13 (project constraint)
- XML delimiters chosen as prompt injection prevention: `<memories>/<memory index="N">` wraps all user data; the system message is static instructions only
- `e.is_timeout()` method check for timeout detection (compile-time safe, no string matching fragility)
- `_llm_engine` with underscore prefix — not yet consumed by AppState; Phase 8 wires it in
- Default base_url `https://api.openai.com/v1`, default model `gpt-4o-mini` — both match CONTEXT.md

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None — no external service configuration required (LLM is optional; engine is None when `llm_provider` is not set).

## Next Phase Readiness
- SummarizationEngine trait ready for Phase 8 CompactionService to depend on
- MockSummarizer available for Phase 8 unit and integration tests
- `_llm_engine` in main.rs ready to be renamed and added to AppState in Phase 8/9

## Self-Check: PASSED

- `src/summarization.rs`: FOUND
- `src/lib.rs`: FOUND
- `src/main.rs`: FOUND
- commit d02289a: FOUND
- commit 1e1a9c9: FOUND

---
*Phase: 07-summarization-engine*
*Completed: 2026-03-20*
