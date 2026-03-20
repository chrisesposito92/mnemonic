# Roadmap: Mnemonic

## Milestones

- **v1.0 MVP** — Phases 1-5 (shipped 2026-03-20)
- **v1.1 Memory Compaction** — Phases 6-9 (in progress)

## Phases

<details>
<summary>v1.0 MVP (Phases 1-5) — SHIPPED 2026-03-20</summary>

- [x] Phase 1: Foundation (3/3 plans) — completed 2026-03-19
- [x] Phase 2: Embedding (2/2 plans) — completed 2026-03-19
- [x] Phase 3: Service and API (3/3 plans) — completed 2026-03-19
- [x] Phase 4: Distribution (2/2 plans) — completed 2026-03-19
- [x] Phase 5: Config & Embedding Provider Cleanup (1/1 plan) — completed 2026-03-20

</details>

### v1.1 Memory Compaction (In Progress)

**Milestone Goal:** Add agent-triggered memory compaction with algorithmic dedup baseline and optional LLM-powered summarization — no background magic, no LLM required for Tier 1.

- [ ] **Phase 6: Foundation** - Config extensions (llm_provider, llm_api_key) and schema additions (source_ids column, compact_runs table)
- [ ] **Phase 7: Summarization Engine** - SummarizationEngine trait, OpenAiSummarizer, prompt injection prevention, LLM fallback behavior
- [ ] **Phase 8: Compaction Core** - CompactionService with greedy pairwise clustering, metadata merge, atomic write, dry_run mode
- [ ] **Phase 9: HTTP Integration** - POST /memories/compact endpoint wired into AppState with full integration tests

## Phase Details

### Phase 6: Foundation
**Goal**: The server starts cleanly on v1.0 databases and is ready to accept new compaction config, with error types and schema in place for all downstream phases
**Depends on**: Phase 5 (v1.0 complete)
**Requirements**: LLM-01
**Success Criteria** (what must be TRUE):
  1. Server starts with existing v1.0 database without errors — schema migration is idempotent (ALTER TABLE ADD COLUMN IF NOT EXISTS)
  2. User can configure llm_provider, llm_api_key, llm_base_url, and llm_model via env vars or TOML without touching any other config
  3. validate_config() rejects invalid LLM config combinations (provider set but api_key missing) at startup, not at request time
  4. The memories table has a source_ids column and a compact_runs table exists, both queryable from a fresh database
**Plans:** 1/2 plans executed
Plans:
- [ ] 06-01-PLAN.md — Config fields + LLM validation + LlmError enum
- [ ] 06-02-PLAN.md — Schema DDL (source_ids column, compact_runs table) + integration test updates

### Phase 7: Summarization Engine
**Goal**: A tested, prompt-injection-resistant SummarizationEngine is available for CompactionService to use — real LLM calls with OpenAiSummarizer, deterministic tests with MockSummarizer
**Depends on**: Phase 6
**Requirements**: LLM-02, LLM-03, LLM-04
**Success Criteria** (what must be TRUE):
  1. OpenAiSummarizer sends a request to the configured LLM and returns a consolidated summary string for a list of memory texts
  2. All memory content in LLM prompts is wrapped in explicit data-framing delimiters — raw content never reaches the prompt template directly
  3. If the LLM call times out or returns an error, the engine returns an Err that CompactionService can catch and fall back from — it does not panic
  4. MockSummarizer returns deterministic output without any network calls, enabling unit tests with zero external dependencies
**Plans**: TBD

### Phase 8: Compaction Core
**Goal**: CompactionService implements the full compaction pipeline — fetch, cluster, synthesize, atomic write — and dry_run mode returns proposed clusters without modifying any data
**Depends on**: Phase 7
**Requirements**: DEDUP-01, DEDUP-02, DEDUP-03, DEDUP-04
**Success Criteria** (what must be TRUE):
  1. Given an agent's memories, CompactionService identifies clusters where cosine similarity exceeds the configured threshold (default 0.85) and groups them for merge
  2. A merged memory inherits the tag union of all source memories, the earliest created_at timestamp, and combined content — verified by assertion
  3. The merge write is atomic: the new memory is inserted and source memories are deleted within a single SQLite transaction; a simulated failure between insert and delete leaves the database consistent (no data lost, no orphans)
  4. When max_candidates is set, the clustering algorithm caps the candidate set and returns without processing beyond the limit — preventing O(n²) on large memory sets
**Plans**: TBD

### Phase 9: HTTP Integration
**Goal**: Agents can call POST /memories/compact and receive compaction results or dry-run previews — multi-agent namespace isolation is verified by integration test
**Depends on**: Phase 8
**Requirements**: API-01, API-02, API-03, API-04
**Success Criteria** (what must be TRUE):
  1. An agent calling POST /memories/compact with agent_id receives a response with clusters_found, memories_merged, and memories_created counts
  2. An agent calling with dry_run: true receives the proposed cluster preview with no changes written to the database — a subsequent GET /memories returns the original count
  3. The compaction response includes an old-to-new ID mapping for every merged cluster so agents can update stale cached memory IDs
  4. Compacting Agent A's memories leaves Agent B's memories completely untouched — verified by an integration test that asserts Agent B's count is unchanged
**Plans**: TBD

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation | v1.0 | 3/3 | Complete | 2026-03-19 |
| 2. Embedding | v1.0 | 2/2 | Complete | 2026-03-19 |
| 3. Service and API | v1.0 | 3/3 | Complete | 2026-03-19 |
| 4. Distribution | v1.0 | 2/2 | Complete | 2026-03-19 |
| 5. Config Cleanup | v1.0 | 1/1 | Complete | 2026-03-20 |
| 6. Foundation | 1/2 | In Progress|  | - |
| 7. Summarization Engine | v1.1 | 0/? | Not started | - |
| 8. Compaction Core | v1.1 | 0/? | Not started | - |
| 9. HTTP Integration | v1.1 | 0/? | Not started | - |
