# Requirements: Mnemonic

**Defined:** 2026-03-20
**Core Value:** Any AI agent can store and semantically search memories out of the box with zero configuration — just download and run

## v1.1 Requirements

Requirements for memory summarization / compaction. Each maps to roadmap phases.

### Compaction API

- [ ] **API-01**: Agent can trigger memory compaction via POST /memories/compact with required agent_id
- [ ] **API-02**: Agent can preview compaction results without committing via dry_run parameter
- [ ] **API-03**: Compaction response includes stats (clusters found, memories merged, memories created)
- [ ] **API-04**: Compaction response includes old-to-new ID mapping for each merged cluster

### Algorithmic Dedup (Tier 1)

- [ ] **DEDUP-01**: System clusters memories by vector cosine similarity using configurable threshold (default 0.85)
- [ ] **DEDUP-02**: System merges metadata for deduplicated clusters (tags union, earliest timestamp, combined content)
- [ ] **DEDUP-03**: Merge operation is atomic — new memory inserted before source memories deleted, within single transaction
- [ ] **DEDUP-04**: System enforces max candidates limit to prevent O(n²) on large memory sets

### LLM Summarization (Tier 2)

- [x] **LLM-01**: User can configure LLM provider via llm_provider and llm_api_key (mirrors embedding_provider pattern)
- [ ] **LLM-02**: When LLM is configured, compaction consolidates memory clusters into rich summaries via LLM
- [ ] **LLM-03**: LLM prompts use structured delimiters to prevent prompt injection from memory content
- [ ] **LLM-04**: If LLM call fails, system falls back to Tier 1 algorithmic merge instead of erroring

## Future Requirements

Deferred to future release. Tracked but not in current roadmap.

### Time-Based Weighting

- **TIME-01**: recency_bias parameter weights age vs similarity when forming clusters
- **TIME-02**: Configurable temporal decay constant for age-aware compaction aggressiveness

### Hierarchical Summaries

- **HIER-01**: Create higher-level summary memories that link back to detail memories
- **HIER-02**: Query logic to traverse parent-child memory hierarchy

### Session Scoping

- **SESS-01**: Compaction scoped by session_id in addition to agent_id

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Automatic background compaction | Agent stays in control — no silent data mutation |
| Hierarchical summaries | Too complex for v1.1; cluster-and-replace covers 90% of use cases |
| Time-based weighting | Deferred — needs empirical tuning with real user data first |
| Session-scoped compaction | agent_id scoping sufficient for v1.1; session_id adds complexity |
| DBSCAN/HDBSCAN clustering | Overkill for N<500; greedy pairwise with single threshold is simpler and sufficient |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| API-01 | Phase 9 | Pending |
| API-02 | Phase 9 | Pending |
| API-03 | Phase 9 | Pending |
| API-04 | Phase 9 | Pending |
| DEDUP-01 | Phase 8 | Pending |
| DEDUP-02 | Phase 8 | Pending |
| DEDUP-03 | Phase 8 | Pending |
| DEDUP-04 | Phase 8 | Pending |
| LLM-01 | Phase 6 | Complete |
| LLM-02 | Phase 7 | Pending |
| LLM-03 | Phase 7 | Pending |
| LLM-04 | Phase 7 | Pending |

**Coverage:**
- v1.1 requirements: 12 total
- Mapped to phases: 12
- Unmapped: 0

---
*Requirements defined: 2026-03-20*
*Last updated: 2026-03-20 after roadmap creation — all 12 requirements mapped to phases 6-9*
