# Phase 17: remember subcommand - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Add `mnemonic remember` subcommand for storing memories from the terminal. Accepts content as a positional argument or piped stdin. Requires medium init (DB + embedding model) to generate embeddings before storing. Full metadata flags for agent, session, and tags.

</domain>

<decisions>
## Implementation Decisions

### Content input strategy
- **D-01:** Positional argument takes priority: `mnemonic remember "some content"` uses the positional arg directly
- **D-02:** If no positional arg, check stdin with `std::io::IsTerminal` — if stdin is piped (not a terminal), read all of stdin as content
- **D-03:** If neither positional arg nor piped stdin is available, print usage error to stderr and exit 1: `"error: provide content as an argument or pipe via stdin"`
- **D-04:** If both positional arg AND piped stdin exist, positional arg wins — stdin is ignored (standard CLI convention, avoids ambiguity)

### Medium-init helper
- **D-05:** Extract `init_db_and_embedding(db_override)` in `cli.rs` — the medium-init counterpart to the existing `init_db()` fast-init helper
- **D-06:** Returns `(MemoryService, Config)` — constructs the full MemoryService since both `remember` (Phase 17) and `search` (Phase 18) need it
- **D-07:** Calls `validate_config()` — unlike fast-path commands, embedding needs valid provider config
- **D-08:** Uses `spawn_blocking` for `LocalEngine::new()` — matches the server init pattern in main.rs lines 85-87
- **D-09:** Prints model loading progress to stderr: `"Loading embedding model..."` and `"Model loaded ({elapsed}ms)"` — gives the user feedback during the 2-3s wait without polluting stdout

### CLI args structure
- **D-10:** `Remember` variant in `Commands` enum wraps `RememberArgs` struct:
  - `content` — optional positional arg (String)
  - `--agent-id <ID>` — optional, defaults to empty string (matches API behavior)
  - `--session-id <ID>` — optional, defaults to empty string (matches API behavior)
  - `--tags tag1,tag2` — optional comma-separated string
- **D-11:** Tags parsed by splitting on comma, trimming whitespace per tag, filtering empty strings

### Data access pattern
- **D-12:** Reuse `MemoryService::create_memory()` — it already validates, embeds, and inserts atomically with dual-table transaction. No reimplementation needed.
- **D-13:** Construct `CreateMemoryRequest` from CLI args and pass to `create_memory()`

### Output format
- **D-14:** On success, print the full UUID on stdout (line 1, pipeable for scripting: `id=$(mnemonic remember "content")`)
- **D-15:** Print a confirmation summary to stderr: `"Stored memory <8-char-id>"` — human context without polluting stdout (matches `keys create` pattern)

### Early validation
- **D-16:** Validate content is not empty/whitespace BEFORE loading the embedding model — avoids 2-3s model load penalty for trivially invalid input
- **D-17:** Error message: `"error: content must not be empty"` to stderr, exit 1

### Dispatch entry point
- **D-18:** Add `run_remember(args: RememberArgs, service: MemoryService)` function in `cli.rs`, parallel to `run_recall()` and `run_keys()`
- **D-19:** main.rs gets a new match arm: `Some(Commands::Remember(args))` → calls `init_db_and_embedding()` → resolves content (positional vs stdin) → calls `run_remember()`

### Claude's Discretion
- Whether stdin reading happens in `run_remember()` or in the main.rs match arm before calling it
- Exact stderr formatting for model load progress (tracing vs eprintln)
- Whether to use `eprintln!` directly or a small stderr print helper
- Test structure and mocking strategy for embedding in tests

</decisions>

<specifics>
## Specific Ideas

- The init path should mirror the server init in main.rs but without the LLM engine, CompactionService, KeyService, or server bind — just DB + embedding + MemoryService
- `init_db_and_embedding()` will be reused verbatim by Phase 18 (search) — design it for that reuse
- The model loading stderr output should feel like `cargo build` — brief, informative, not noisy
- Stdin pipe support means `cat notes.txt | mnemonic remember --agent-id researcher` works for bulk single-memory import

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` §Remember — REM-01 (positional content), REM-02 (stdin pipe), REM-03 (agent/session flags), REM-04 (tags)
- `.planning/REQUIREMENTS.md` §Output — OUT-03, OUT-04 (exit codes, stderr/stdout split — implement early)

### CLI patterns (must match)
- `src/cli.rs` — `Commands` enum (line 22), `RecallArgs` pattern (line 39), `run_recall()` entry point (line 106), `init_db()` helper (line 90)
- `src/main.rs` — Match dispatch (line 24), Recall arm (line 34), Serve/None arm (line 40)

### Data access (reuse, not reimplement)
- `src/service.rs` — `MemoryService::new()` (line 15), `create_memory()` (line 89), `CreateMemoryRequest` struct (line 24), `Memory` struct (line 58)
- `src/service.rs` — Dual-table insert transaction (lines 115-133) — already handled by create_memory()

### Embedding init (mirror server pattern)
- `src/main.rs` — Embedding engine init (lines 76-107), spawn_blocking pattern (line 85), validate_config (line 59)
- `src/embedding.rs` — `EmbeddingEngine` trait (line 9), `LocalEngine::new()` (line 38), `OpenAiEngine::new()` (line exists)
- `src/config.rs` — `validate_config()` (line 37), `Config` struct (line 9)

### Prior phase decisions
- `.planning/phases/15-serve-subcommand/15-CONTEXT.md` — D-07/D-08 (deferred init helper extraction)
- `.planning/phases/16-recall-subcommand/16-CONTEXT.md` — D-04/D-05 (init_db helper pattern)

### Research
- `.planning/research/STACK.md` §stdin detection — `std::io::IsTerminal` confirmed, no new crate needed
- `.planning/research/SUMMARY.md` — Medium-init tier design, Phase C (remember) rationale

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `cli::init_db()` — fast-path DB init; medium-init helper extends this with embedding
- `cli::Commands` enum — add `Remember(RememberArgs)` variant
- `cli::truncate()` — may be useful for stderr confirmation if content is long
- `service::MemoryService::create_memory()` — full store pipeline, reuse directly
- `service::CreateMemoryRequest` — request struct, construct from CLI args
- `config::validate_config()` — needed for embedding provider validation

### Established Patterns
- Fast-path init: `init_db()` → DB only, no embedding (~50ms)
- Server full init: main.rs lines 44-174 → DB + embedding + LLM + server (~3s)
- Medium-init (new): DB + embedding, no LLM, no server (~2-3s) — between fast and full
- Error handling: `eprintln!("error: ...")` + `std::process::exit(1)` for failures
- Pipeable output: raw value on stdout line 1, human context on stderr (keys create pattern)

### Integration Points
- `main.rs` line 24: `match cli_args.command` — add `Some(Commands::Remember(args))` arm
- `cli.rs` line 22: `Commands` enum — add `Remember(RememberArgs)` variant
- `cli.rs` line 90: `init_db()` — new `init_db_and_embedding()` sibling

</code_context>

<deferred>
## Deferred Ideas

- `--json` flag for machine-readable output — Phase 20 (OUT-02) handles this across all subcommands
- Batch import from stdin (multiple memories per line) — future `mnemonic import` command (IMP-01)
- Content from file path (`mnemonic remember @file.txt`) — too magical, pipe is sufficient
- Progress bar during model load — overkill for a 2-3s operation; simple stderr message is enough

</deferred>

---

*Phase: 17-remember-subcommand*
*Context gathered: 2026-03-21*
