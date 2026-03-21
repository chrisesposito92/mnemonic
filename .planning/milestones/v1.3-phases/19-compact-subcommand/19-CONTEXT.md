# Phase 19: compact subcommand - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Add `mnemonic compact` subcommand for triggering memory compaction from the terminal. Most complex init path — requires DB + embedding + optional LLM summarization engine to construct CompactionService. Supports dry-run preview, agent scoping, and threshold control. Reuses `CompactionService::compact()` directly — no reimplementation of compaction logic.

</domain>

<decisions>
## Implementation Decisions

### Full-init helper
- **D-01:** Create `init_compaction(db_override)` in `cli.rs` — the full-init counterpart to `init_db()` (fast) and `init_db_and_embedding()` (medium)
- **D-02:** Returns `(CompactionService, Config)` — constructs the full CompactionService with optional LLM engine
- **D-03:** Init sequence mirrors server init in main.rs lines 86-216: register_sqlite_vec → load_config → apply --db override → validate_config → open DB → init embedding → init optional LLM → construct CompactionService
- **D-04:** Cannot reuse `init_db_and_embedding()` — that returns `MemoryService`, but compact needs the individual components (`conn_arc`, `embedding`, optional `llm_engine`) to construct CompactionService
- **D-05:** LLM engine init follows the server pattern (main.rs lines 152-171): if `config.llm_provider` is `Some("openai")`, construct `OpenAiSummarizer`; if `None`, pass `None` to CompactionService (algorithmic merge only)
- **D-06:** Stderr progress messages: `"Loading embedding model..."` and `"Model loaded (Xms)"` for embedding (matching init_db_and_embedding pattern); `"LLM summarization: enabled (provider)"` or `"LLM summarization: disabled (algorithmic merge only)"` for LLM status

### CLI args structure
- **D-07:** `Compact` variant in `Commands` enum wraps `CompactArgs` struct:
  - `--agent-id <ID>` — optional, defaults to empty string `""` (compacts default namespace where memories have no agent_id)
  - `--threshold <F>` — optional (CompactionService applies default 0.85 internally via `unwrap_or(0.85)`)
  - `--max-candidates <N>` — optional (CompactionService applies default 100 internally via `unwrap_or(100)`)
  - `--dry-run` — boolean flag (clap `#[arg(long)]`, defaults to false)
- **D-08:** No positional arguments — all parameters are flags (compaction is a system operation, not content-oriented like remember/search)
- **D-09:** agent_id defaults to empty string "" — matches how memories stored without --agent-id are recorded (agent_id="" in DB). Bare `mnemonic compact` compacts the default namespace.

### Data access pattern
- **D-10:** Construct `CompactRequest` from CLI args and pass to `CompactionService::compact()` — the existing method handles the full pipeline: fetch_candidates → compute_pairs → cluster → synthesize → atomic write
- **D-11:** No new SQL, no new service methods — CompactRequest maps directly: `agent_id` from `--agent-id` (default ""), `threshold`/`max_candidates`/`dry_run` as Option from clap

### Output format
- **D-12:** On success with clusters found, print a summary to stdout:
  ```
  Compacted: 3 clusters, 8 memories merged → 3 new memories
  ```
  For dry-run:
  ```
  Dry run: 3 clusters, 8 memories would be merged → 3 new memories
  ```
- **D-13:** When 0 clusters found: `"No similar memories found to compact."` to stdout — exit 0 (no error, just nothing to do)
- **D-14:** If `truncated` is true in the response, append to stderr: `"Note: only {max_candidates} most recent memories were evaluated. Increase --max-candidates for broader coverage."`
- **D-15:** Run ID printed to stderr: `"Run: {run_id_short}"` — useful for audit trail but doesn't pollute stdout
- **D-16:** No per-cluster detail output in v1.3 — the summary line covers success criteria. Cluster detail (`id_mapping`) is available in `--json` output (Phase 20).

### Dispatch entry point
- **D-17:** Add `run_compact(args: CompactArgs, compaction: CompactionService)` function in `cli.rs`, parallel to `run_remember()`, `run_search()`
- **D-18:** main.rs gets a new match arm: `Some(Commands::Compact(args))` → calls `init_compaction()` → calls `run_compact()`
- **D-19:** No early validation needed before init — unlike remember/search, compact has no user content to validate. All args are optional with sensible defaults.

### Claude's Discretion
- Whether `init_compaction()` shares any extracted sub-steps with `init_db_and_embedding()` or duplicates the embedding init code
- Exact stderr formatting for LLM status
- Test structure, mocking strategy for CompactionService in integration tests
- Whether to include cluster count per line or keep it to the single summary line

</decisions>

<specifics>
## Specific Ideas

- This is the only CLI subcommand that needs the LLM engine — it's the "full init" tier. All other subcommands need at most DB + embedding.
- The init path should feel like the server init in main.rs but without KeyService or server bind — just the CompactionService construction
- Compaction is the only destructive CLI operation (deletes and merges memories) — the `--dry-run` flag is important for safety. The output should clearly distinguish dry-run from actual compaction.
- The CompactResponse includes `id_mapping` which maps source_ids → new_id for each cluster. In v1.3, this is only useful for `--json` output (Phase 20). The human-readable summary just shows counts.

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/ROADMAP.md` §Phase 19 — CMP-01 (trigger compaction), CMP-02 (dry-run preview), CMP-03 (agent scoping + threshold)
- `.planning/ROADMAP.md` §Phase 19 Success Criteria — 4 criteria defining what must be TRUE

### CLI patterns (must match)
- `src/cli.rs` — `Commands` enum (line 22), `SearchArgs` struct pattern (line 82), `run_search()` entry point (line 245), `init_db_and_embedding()` helper (line 153)
- `src/main.rs` — Match dispatch (line 24), Search arm (lines 70-79) — compact arm follows same pattern but calls `init_compaction()` instead

### Data access (reuse, not reimplement)
- `src/compaction.rs` — `CompactionService::new()` (line 66), `CompactionService::compact()` (line 247), `CompactRequest` struct (line 13), `CompactResponse` struct (line 21), `ClusterMapping` struct (line 31)
- `src/compaction.rs` — Constructor args: `db: Arc<Connection>`, `embedding: Arc<dyn EmbeddingEngine>`, `summarization: Option<Arc<dyn SummarizationEngine>>`, `embedding_model: String`

### LLM engine init (mirror server pattern)
- `src/main.rs` lines 152-171 — LLM summarization engine init: matches `config.llm_provider`, constructs `OpenAiSummarizer` if configured, `None` otherwise
- `src/summarization.rs` — `SummarizationEngine` trait (line 12), `OpenAiSummarizer::new()` constructor
- `src/config.rs` — `Config.llm_provider`, `Config.llm_api_key`, `Config.llm_base_url`, `Config.llm_model` fields (lines 14-18), `validate_config()` handles LLM validation (lines 57-73)

### Prior phase decisions
- `.planning/phases/17-remember-subcommand/17-CONTEXT.md` — D-05/D-06 (init_db_and_embedding helper pattern), D-09 (stderr progress for model loading)
- `.planning/phases/18-search-subcommand/18-CONTEXT.md` — D-17 (main.rs match arm pattern)
- `.planning/phases/15-serve-subcommand/15-CONTEXT.md` — D-08 (phase-specific init helper extraction)

### Research
- `.planning/research/SUMMARY.md` — Full-init tier design for compact subcommand
- `.planning/STATE.md` §Blockers — "Phase 19: Inspect CompactionService constructor sequence before planning — optional LLM engine init has more moving parts"

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `cli::Commands` enum — add `Compact(CompactArgs)` variant
- `cli::truncate()` — may be useful if outputting agent_id in summary
- `compaction::CompactionService::compact()` — full compaction pipeline, reuse directly
- `compaction::CompactRequest` — request struct, construct from CLI args
- `compaction::CompactResponse` — response with stats and id_mapping
- `config::validate_config()` — validates both embedding AND LLM provider config

### Established Patterns
- Fast-path init: `init_db()` → DB only (~50ms) for keys/recall
- Medium-init: `init_db_and_embedding()` → DB + embedding (~2-3s) for remember/search
- Full-init (new): DB + embedding + optional LLM (~2-3s + LLM setup) for compact
- Error handling: `eprintln!("error: ...")` + `std::process::exit(1)` for failures
- Handler naming: `run_keys()`, `run_recall()`, `run_remember()`, `run_search()` → add `run_compact()`
- LLM engine construction: main.rs lines 152-171 — pattern to mirror

### Integration Points
- `main.rs` line 24: `match cli_args.command` — add `Some(Commands::Compact(args))` arm after Search
- `cli.rs` line 22: `Commands` enum — add `Compact(CompactArgs)` variant
- `cli.rs` — new `init_compaction()` function alongside existing `init_db()` and `init_db_and_embedding()`

</code_context>

<deferred>
## Deferred Ideas

- `--json` flag for machine-readable output (including full id_mapping) — Phase 20 (OUT-02) handles this across all subcommands
- Per-cluster detail output (source IDs → new ID for each merge) — available via `--json` in Phase 20; human summary is sufficient for v1.3
- Confirmation prompt before non-dry-run compaction ("Are you sure? Y/n") — adds interactive I/O complexity; dry-run serves the safety role
- `--verbose` flag for per-cluster progress during compaction — future enhancement if users request it
- Progress bar for LLM summarization calls — overkill; LLM calls are per-cluster and fast enough

</deferred>

---

*Phase: 19-compact-subcommand*
*Context gathered: 2026-03-21*
