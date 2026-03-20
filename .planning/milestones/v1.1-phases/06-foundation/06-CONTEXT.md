# Phase 6: Foundation - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Extend the Mnemonic config system with LLM provider fields and add schema foundations (source_ids column, compact_runs table) so downstream phases can build summarization and compaction on top. The server must start cleanly on v1.0 databases with no manual migration steps.

</domain>

<decisions>
## Implementation Decisions

### Config field naming and structure
- Flat fields on Config struct: `llm_provider`, `llm_api_key`, `llm_base_url`, `llm_model` — mirrors existing `embedding_provider` / `openai_api_key` pattern
- All fields are `Option<String>` — LLM config is entirely opt-in
- Default `llm_provider` is `None` (no LLM) — Tier 1 algorithmic compaction works without any LLM config
- `llm_base_url` defaults to `https://api.openai.com/v1` when provider is `openai` — allows override for Azure/local endpoints
- `llm_model` defaults per-provider (e.g. `gpt-4o-mini` for `openai`) — overridable via `MNEMONIC_LLM_MODEL`
- Env var mapping: `MNEMONIC_LLM_PROVIDER`, `MNEMONIC_LLM_API_KEY`, `MNEMONIC_LLM_BASE_URL`, `MNEMONIC_LLM_MODEL`

### Config validation rules
- `validate_config()` extended: if `llm_provider` is set to `"openai"`, require `llm_api_key` — same pattern as embedding_provider validation
- If `llm_provider` is `None`, all other llm_* fields are ignored (no error for orphaned keys)
- Unknown `llm_provider` values rejected at startup with clear error message

### Schema migration strategy
- `source_ids` column added to memories table via `ALTER TABLE ADD COLUMN IF NOT EXISTS` in `db::open()` — inline with existing DDL
- `source_ids` is `TEXT NOT NULL DEFAULT '[]'` — JSON array of memory IDs, mirrors tags column pattern
- `compact_runs` table created with `CREATE TABLE IF NOT EXISTS` — idempotent
- No migration framework — inline DDL in `db::open()` is sufficient for this project's scale

### compact_runs table schema
- Columns: `id TEXT PRIMARY KEY`, `agent_id TEXT NOT NULL`, `started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP`, `completed_at DATETIME`, `clusters_found INTEGER NOT NULL DEFAULT 0`, `memories_merged INTEGER NOT NULL DEFAULT 0`, `memories_created INTEGER NOT NULL DEFAULT 0`, `dry_run BOOLEAN NOT NULL DEFAULT 0`, `threshold REAL NOT NULL`, `status TEXT NOT NULL DEFAULT 'running'`
- `threshold` column captures the similarity threshold used for the run — enables auditing
- `status` tracks run state: 'running', 'completed', 'failed'
- Index on `agent_id` for efficient per-agent run history queries

### Error type additions
- New `LlmError` enum in `error.rs` following existing pattern (ModelLoad, ApiCall, Timeout variants)
- Wire into `MnemonicError` as `#[error("llm error: {0}")] Llm(#[from] LlmError)`
- `CompactionError` deferred to Phase 8 — not needed for foundation

### Claude's Discretion
- Exact error message wording for LLM config validation failures
- Whether to add tracing::info for LLM config at startup (likely yes, mirrors embedding provider log)
- Internal ordering of ALTER TABLE statements in db::open()

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — LLM-01 requirement defines LLM config fields needed

### Architecture patterns
- `src/config.rs` — Existing Config struct, validate_config(), load_config() with figment
- `src/error.rs` — Error hierarchy pattern: DbError, ConfigError, EmbeddingError, ApiError
- `src/db.rs` — Schema DDL in db::open() execute_batch — add new DDL here
- `src/embedding.rs` — EmbeddingEngine trait pattern — SummarizationEngine (Phase 7) will mirror this

### Project decisions
- `.planning/PROJECT.md` §Key Decisions — rusqlite 0.37 pin, candle over ort, tokio-rusqlite pattern
- `.planning/STATE.md` §Accumulated Context — reqwest 0.13 for LLM HTTP calls, no async-openai

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `Config` struct with figment (Serialized defaults → Toml → Env) — add new fields directly
- `validate_config()` match-based validation — extend with llm_provider arm
- `db::open()` execute_batch — append ALTER TABLE and CREATE TABLE statements
- `error.rs` thiserror enum pattern — copy for LlmError

### Established Patterns
- Config fields are flat (not nested) — `embedding_provider`, `openai_api_key`
- JSON-in-TEXT columns for arrays — `tags TEXT NOT NULL DEFAULT '[]'`
- Env vars prefixed with `MNEMONIC_` — figment handles mapping automatically
- All Option<String> for optional config — figment deserializes from env
- `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS` for idempotent schema

### Integration Points
- `main.rs` line 21: `validate_config(&config)?` — LLM validation runs here
- `main.rs` lines 38-69: embedding engine init — LLM engine init will follow similar pattern (Phase 7)
- `server.rs` AppState — will eventually hold CompactionService (Phase 8/9)
- `db::open()` execute_batch — new DDL appended after existing schema

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. Follow existing codebase patterns exactly.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 06-foundation*
*Context gathered: 2026-03-20*
