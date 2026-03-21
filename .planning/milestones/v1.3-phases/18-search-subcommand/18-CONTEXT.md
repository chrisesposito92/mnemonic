# Phase 18: search subcommand - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Add `mnemonic search <query>` subcommand for semantic search from the terminal. Reuses the medium-init path (DB + embedding) established in Phase 17. Calls `MemoryService::search_memories()` directly — no reimplementation of search logic. Displays ranked results with distance scores and supports filtering flags.

</domain>

<decisions>
## Implementation Decisions

### Query input
- **D-01:** Query is a required positional argument: `mnemonic search "what was that about Paris"`
- **D-02:** No stdin pipe support — search queries are short interactive strings, not piped content. This differs from `remember` which supports stdin for bulk content.
- **D-03:** If no positional argument is provided, clap's required-arg validation handles it automatically (no manual check needed)

### Early validation
- **D-04:** Validate query is not empty/whitespace BEFORE calling `init_db_and_embedding()` — avoids 2-3s model load for trivially invalid input (same pattern as remember's D-16)
- **D-05:** Error message: `"error: query must not be empty"` to stderr, exit 1

### CLI args structure
- **D-06:** `Search` variant in `Commands` enum wraps `SearchArgs` struct:
  - `query` — required positional arg (String)
  - `--limit <N>` — max results (default: 10, matches API default)
  - `--threshold <F>` — max distance filter (f32, optional — matches API's threshold param)
  - `--agent-id <ID>` — filter by agent (optional)
  - `--session-id <ID>` — filter by session (optional)
- **D-07:** No `--tag`, `--after`, `--before` flags — SRC-02 only requires limit/threshold/agent-id/session-id. The service supports those extra params but CLI keeps it focused.

### Data access pattern
- **D-08:** Construct `SearchParams` from CLI args and pass to `MemoryService::search_memories()` — the `q` field is `Some(query)`, all optional filter fields map directly from SearchArgs
- **D-09:** No new SQL, no new service methods — the existing `search_memories()` handles embedding, KNN, filtering, and threshold in one call

### Search results table format
- **D-10:** Table columns: `DIST` (distance, 6 chars formatted to 4 decimal places), `ID` (8-char UUID prefix), `CONTENT` (50 chars truncated), `AGENT` (15 chars truncated)
- **D-11:** Distance shown as raw float from sqlite-vec (lower = more similar, 0.0 = exact match) — consistent with the API's `distance` field. Column header `DIST` is compact and clear.
- **D-12:** Results are already ordered by distance ascending from `search_memories()` — no re-sorting needed

### Empty results handling
- **D-13:** When search returns 0 results: `"No matching memories found."` to stdout — matches recall's `"No memories found."` pattern
- **D-14:** No distinction between "no results at all" vs "no results above threshold" — the message is the same

### Footer
- **D-15:** Footer line: `"Found {n} results"` (or `"Found 1 result"` for singular) — no "of total" since search is ranked, not paginated

### Dispatch entry point
- **D-16:** Add `run_search(query: String, args: SearchArgs, service: MemoryService)` function in `cli.rs`, parallel to `run_remember()`
- **D-17:** main.rs gets a new match arm: `Some(Commands::Search(args))` → validate query not empty → `init_db_and_embedding()` → `run_search()`
- **D-18:** The match arm is simpler than Remember's — no stdin detection, no `IsTerminal` check, just positional arg validation + init + run

### Output format
- **D-19:** On success, table with results goes to stdout; `"Found N results"` footer goes to stdout
- **D-20:** Model loading progress (`"Loading embedding model..."`, `"Model loaded (Xms)"`) goes to stderr (handled by `init_db_and_embedding()`)

### Claude's Discretion
- Exact column spacing and alignment
- Whether to add a `--tag` filter beyond the required SRC-02 flags
- Test structure and naming

</decisions>

<specifics>
## Specific Ideas

- This is the simplest medium-init subcommand — positional arg in, table out, no stdin complexity
- The distance value from sqlite-vec for L2-normalized embeddings is cosine distance (0 = identical). Showing raw distance is honest and consistent with the API — no confusing conversion.
- `init_db_and_embedding()` is reused verbatim from Phase 17 — zero changes to the helper
- The match arm in main.rs should look almost identical to Remember but without the stdin/IsTerminal logic — just: extract query, trim-check, init, run

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` §Search — SRC-01 (semantic search with results), SRC-02 (limit/threshold/agent-id/session-id flags)
- `.planning/REQUIREMENTS.md` §Output — OUT-03, OUT-04 (exit codes, stderr/stdout split — implement early)

### CLI patterns (must match)
- `src/cli.rs` — `Commands` enum (line 22), `RememberArgs` struct pattern (line 62), `run_remember()` entry point (line 188), `init_db_and_embedding()` helper (line 128)
- `src/main.rs` — Match dispatch (line 24), Remember arm (lines 39-69) — search arm mirrors this but simpler

### Data access (reuse, not reimplement)
- `src/service.rs` — `SearchParams` struct (line 36), `SearchResponse` struct (line 77), `SearchResultItem` struct (line 82), `search_memories()` method (line 147)
- `src/service.rs` — `SearchParams.q` is `Option<String>`, `threshold` is `Option<f32>`, `limit` is `Option<u32>` — map directly from CLI args

### Prior phase decisions
- `.planning/phases/17-remember-subcommand/17-CONTEXT.md` — D-05/D-06 (init_db_and_embedding helper), D-04/D-16 (early validation before model load)
- `.planning/phases/16-recall-subcommand/16-CONTEXT.md` — D-09/D-12 (table format pattern, truncate reuse)

### Research
- `.planning/research/SUMMARY.md` — Phase D (search) rationale, medium-init reuse from Phase C
- `.planning/research/ARCHITECTURE.md` — Pattern 5: search uses `search_memories()` not raw SQL

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `cli::init_db_and_embedding()` — medium-init helper, reused verbatim (no changes)
- `cli::Commands` enum — add `Search(SearchArgs)` variant
- `cli::truncate()` — reuse for content/agent column truncation in table
- `service::MemoryService::search_memories()` — full search pipeline, reuse directly
- `service::SearchParams` — request struct, construct from CLI args
- `service::SearchResultItem` — contains `memory: Memory` + `distance: f64`

### Established Patterns
- Medium-init path: `init_db_and_embedding()` → `(MemoryService, Config)` (~2-3s with local model)
- Early validation before init: check empty input → error + exit 1 → skip model load
- Table output: padded columns with `format!("{:<width}")`, header + dashes separator, footer line
- Pipeable output: raw data on stdout, human context on stderr
- Handler naming: `run_keys()`, `run_recall()`, `run_remember()` → add `run_search()`

### Integration Points
- `main.rs` line 24: `match cli_args.command` — add `Some(Commands::Search(args))` arm after Remember
- `cli.rs` line 22: `Commands` enum — add `Search(SearchArgs)` variant
- `cli.rs` line 128: `init_db_and_embedding()` — call as-is, no modifications needed

</code_context>

<deferred>
## Deferred Ideas

- `--json` flag for machine-readable output — Phase 20 (OUT-02) handles this across all subcommands
- `--tag` filter flag — not in SRC-02 requirements, but SearchParams supports it; could be added easily if needed later
- `--after` / `--before` date range filters — SearchParams supports these but they add complexity without clear CLI use case
- Color-coded similarity scores (green = high, red = low) — v1.4 (CLR-01 in future requirements)
- Converting distance to similarity percentage — adds confusion about the metric; raw distance is honest

</deferred>

---

*Phase: 18-search-subcommand*
*Context gathered: 2026-03-21*
