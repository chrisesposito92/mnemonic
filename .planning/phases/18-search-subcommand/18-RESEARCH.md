# Phase 18: search subcommand - Research

**Researched:** 2026-03-21
**Domain:** Rust CLI extension — clap 4 subcommand dispatch, MemoryService::search_memories(), tabular terminal output
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Query is a required positional argument: `mnemonic search "what was that about Paris"`
- **D-02:** No stdin pipe support — search queries are short interactive strings, not piped content.
- **D-03:** Clap's required-arg validation handles missing positional arg automatically (no manual check needed).
- **D-04:** Validate query is not empty/whitespace BEFORE calling `init_db_and_embedding()` — avoids 2-3s model load for trivially invalid input.
- **D-05:** Error message: `"error: query must not be empty"` to stderr, exit 1.
- **D-06:** `Search` variant in `Commands` enum wraps `SearchArgs` struct with: `query` (required positional String), `--limit <N>` (default 10), `--threshold <F>` (f32, optional), `--agent-id <ID>` (optional), `--session-id <ID>` (optional).
- **D-07:** No `--tag`, `--after`, `--before` flags — SRC-02 only requires limit/threshold/agent-id/session-id.
- **D-08:** Construct `SearchParams` from CLI args; `q` field is `Some(query)`, all optional filter fields map directly.
- **D-09:** No new SQL, no new service methods — existing `search_memories()` handles embedding, KNN, filtering, and threshold.
- **D-10:** Table columns: `DIST` (6 chars, 4 decimal places), `ID` (8-char UUID prefix), `CONTENT` (50 chars truncated), `AGENT` (15 chars truncated).
- **D-11:** Distance shown as raw float from sqlite-vec (lower = more similar). Column header `DIST`.
- **D-12:** Results already ordered by distance ascending from `search_memories()` — no re-sorting needed.
- **D-13:** Empty results: `"No matching memories found."` to stdout.
- **D-14:** No distinction between "no results at all" vs "no results above threshold" — same message.
- **D-15:** Footer: `"Found {n} results"` (or `"Found 1 result"` for singular).
- **D-16:** `run_search(query: String, args: SearchArgs, service: MemoryService)` function in `cli.rs`, parallel to `run_remember()`.
- **D-17:** main.rs match arm: `Some(Commands::Search(args))` → validate query not empty → `init_db_and_embedding()` → `run_search()`.
- **D-18:** Match arm simpler than Remember's — no stdin detection, no `IsTerminal` check.
- **D-19:** Table and footer go to stdout; model loading messages go to stderr.
- **D-20:** `"Loading embedding model..."` / `"Model loaded (Xms)"` go to stderr (handled by `init_db_and_embedding()`).

### Claude's Discretion

- Exact column spacing and alignment
- Whether to add a `--tag` filter beyond the required SRC-02 flags
- Test structure and naming

### Deferred Ideas (OUT OF SCOPE)

- `--json` flag — Phase 20 (OUT-02) handles this across all subcommands
- `--tag` filter flag — not in SRC-02; SearchParams supports it but CLI keeps it focused
- `--after` / `--before` date range filters — SearchParams supports these but adds complexity
- Color-coded similarity scores — v1.4 (CLR-01)
- Converting distance to similarity percentage — raw distance is honest
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SRC-01 | `mnemonic search <query>` performs semantic search and displays results | `search_memories()` in `service.rs` handles full pipeline; table output pattern established by recall (Phase 16) |
| SRC-02 | `mnemonic search` accepts `--limit`, `--threshold`, `--agent-id`, `--session-id` flags | `SearchParams` struct in `service.rs` has all four fields; clap derive flags map directly |
</phase_requirements>

---

## Summary

Phase 18 is the simplest medium-init subcommand in the v1.3 milestone. Every architectural decision is locked and all infrastructure is already in place from Phases 16 and 17. The implementation is purely additive: new `SearchArgs` struct, new `Search` variant in `Commands`, new `run_search()` handler, and one new match arm in `main.rs`.

The key insight is that this phase reuses three already-shipped components without modification: `init_db_and_embedding()` (Phase 17), `MemoryService::search_memories()` (v1.2 server), and `truncate()` (Phase 14). The entire implementation is approximately 60-80 lines of new code spread across two files (`cli.rs` and `main.rs`). No new Cargo.toml dependencies, no new modules.

The only design care required is the table output: column widths are specified (DIST=6, ID=8, CONTENT=50, AGENT=15), distance formatting is 4 decimal places, and the singular/plural footer rule must be implemented correctly. The `f64` distance from `SearchResultItem` must be formatted with `format!("{:.4}", distance)` to match D-10.

**Primary recommendation:** Two-plan structure matches Phase 17's pattern — Plan 01 implements the subcommand in `cli.rs` and `main.rs`; Plan 02 adds integration tests in `tests/cli_integration.rs`.

## Standard Stack

### Core (already in Cargo.toml — no new dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4.x (derive) | `SearchArgs` struct + `Search` variant in Commands | Already used for all CLI subcommands |
| tokio-rusqlite | 0.7 | Async DB access via `conn.call()` | Already used by all CLI handlers |
| sqlite-vec | 0.1.7 | KNN vector search | Already used by `search_memories()` |
| candle-core / hf-hub | 0.9 / 0.5 | Local embedding model | Already loaded by `init_db_and_embedding()` |
| anyhow | 1 | Error propagation | Already used throughout |

**No new Cargo.toml entries required.**

**Installation:** None — all dependencies already present.

## Architecture Patterns

### Recommended Project Structure

No new files or directories. All changes are additive to existing files:

```
src/
├── cli.rs        # Add: SearchArgs struct, Search variant in Commands, run_search()
├── main.rs       # Add: Some(Commands::Search(args)) match arm
tests/
└── cli_integration.rs  # Add: Phase 18 test section
```

### Pattern 1: SearchArgs Struct (clap derive)

**What:** Required positional arg + optional flags, mirrors `RememberArgs` pattern.

**When to use:** Every CLI subcommand with a primary positional argument.

```rust
// Source: src/cli.rs (established pattern, mirrored from RememberArgs at line 62)
/// Arguments for the `search` subcommand.
#[derive(Args)]
pub struct SearchArgs {
    /// Search query
    pub query: String,                        // required positional — clap enforces presence

    /// Filter by agent_id
    #[arg(long, value_name = "ID")]
    pub agent_id: Option<String>,

    /// Filter by session_id
    #[arg(long, value_name = "ID")]
    pub session_id: Option<String>,

    /// Maximum number of results (default: 10)
    #[arg(long, value_name = "N", default_value_t = 10)]
    pub limit: u32,

    /// Maximum distance threshold (0.0 = exact match, higher = less similar)
    #[arg(long, value_name = "F")]
    pub threshold: Option<f32>,
}
```

Note: `query` is `String` (not `Option<String>`) — clap enforces it is present. D-03 confirms no manual presence check needed.

### Pattern 2: Commands Enum Extension

**What:** Add `Search(SearchArgs)` variant to the existing `Commands` enum.

```rust
// Source: src/cli.rs (Commands enum at line 22)
#[derive(Subcommand)]
pub enum Commands {
    Serve,
    Keys(KeysArgs),
    Recall(RecallArgs),
    Remember(RememberArgs),
    Search(SearchArgs),   // ADD THIS
}
```

### Pattern 3: main.rs Match Arm (simpler than Remember)

**What:** Extract query, validate non-empty, call `init_db_and_embedding()`, call `run_search()`.

**Key difference from Remember:** No stdin detection, no `IsTerminal` check — just positional arg validation.

```rust
// Source: src/main.rs (mirrors Remember arm at lines 39-69, but simpler)
Some(cli::Commands::Search(args)) => {
    // D-04: validate BEFORE model load (early exit, no 2-3s wait)
    if args.query.trim().is_empty() {
        eprintln!("error: query must not be empty");
        std::process::exit(1);
    }

    let (service, _config) = cli::init_db_and_embedding(db_override).await?;
    cli::run_search(args.query.clone(), args, service).await;
    return Ok(());
}
```

### Pattern 4: run_search() Handler

**What:** Construct `SearchParams`, call `search_memories()`, print table.

```rust
// Source: src/cli.rs (parallel to run_remember() at line 188)
pub async fn run_search(query: String, args: SearchArgs, service: crate::service::MemoryService) {
    let params = crate::service::SearchParams {
        q: Some(query),
        agent_id: args.agent_id,
        session_id: args.session_id,
        tag: None,          // D-07: no --tag flag in this phase
        limit: Some(args.limit),
        threshold: args.threshold,
        after: None,
        before: None,
    };

    match service.search_memories(params).await {
        Ok(resp) => {
            if resp.memories.is_empty() {
                println!("No matching memories found.");  // D-13
                return;
            }

            // D-10: table header
            let header = format!("{:<6}  {:<8}  {:<50}  {}", "DIST", "ID", "CONTENT", "AGENT");
            println!("{}", header);
            println!("{}", "-".repeat(header.len()));

            for item in &resp.memories {
                let dist = format!("{:.4}", item.distance);   // D-11: 4 decimal places
                let id_short = if item.memory.id.len() >= 8 { &item.memory.id[..8] } else { &item.memory.id };
                let content = truncate(&item.memory.content, 50);  // D-10: 50 chars
                let agent = if item.memory.agent_id.is_empty() {
                    "(none)".to_string()
                } else {
                    truncate(&item.memory.agent_id, 15)  // D-10: 15 chars
                };
                println!("{:<6}  {:<8}  {:<50}  {}", dist, id_short, content, agent);
            }

            // D-15: singular/plural footer
            let n = resp.memories.len();
            if n == 1 {
                println!("Found 1 result");
            } else {
                println!("Found {} results", n);
            }
        }
        Err(e) => {
            eprintln!("error: search failed: {}", e);
            std::process::exit(1);
        }
    }
}
```

### Pattern 5: SearchParams Field Mapping

`service::SearchParams` (line 36 of service.rs) has these fields that map directly from `SearchArgs`:

| SearchParams field | SearchArgs field | Notes |
|---|---|------|
| `q` | `args.query` | Wrap in `Some()` |
| `limit` | `args.limit` | Wrap in `Some()` |
| `threshold` | `args.threshold` | Already `Option<f32>` |
| `agent_id` | `args.agent_id` | Already `Option<String>` |
| `session_id` | `args.session_id` | Already `Option<String>` |
| `tag` | (none) | Set to `None` — D-07 |
| `after` | (none) | Set to `None` — deferred |
| `before` | (none) | Set to `None` — deferred |

### Pattern 6: Integration Test Structure (established in Phase 17)

Tests invoke the compiled binary via `std::process::Command` with `--db` pointing at a `TempDb`. Each search test that actually runs semantic search will trigger the embedding model load (~2-3s). Error path tests (empty query) must NOT load the model and are fast.

```rust
// Source: tests/cli_integration.rs (established TempDb + binary() pattern)

// ---- Phase 18: search subcommand ------------------------------------------------

#[test]
fn test_search_returns_results() {
    // 1. remember a memory (triggers model load)
    // 2. search for it (triggers model load again — separate process)
    // 3. assert stdout contains table header and DIST column
    // 4. assert exit code 0
}

#[test]
fn test_search_empty_query_exits_one() {
    // args: ["--db", db.path_str(), "search", ""]
    // assert !output.status.success()
    // assert stderr.contains("query must not be empty")
    // NOTE: does NOT trigger model load (early validation, D-04)
}
```

### Anti-Patterns to Avoid

- **Calling `MemoryService::search_memories()` with `q: None`:** The service immediately returns `Err(ApiError::BadRequest(...))` — always pass `q: Some(query)`.
- **Re-sorting results in the handler:** Results arrive ordered by distance ascending from the SQL query — no re-sort needed (D-12).
- **Adding `--tag` flag:** Not in SRC-02 scope; SearchParams supports it but the CLI keeps it focused (D-07). Deferred.
- **Blocking tokio during model load:** `init_db_and_embedding()` already uses `spawn_blocking` — call it as-is, do not inline the model load.
- **Copying model init code:** Do not duplicate the `init_db_and_embedding()` block — call the already-extracted helper from `cli.rs` (line 128).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Semantic search | Custom KNN SQL or embedding distance logic | `MemoryService::search_memories()` | Already implements embedding, KNN via sqlite-vec, filtering, threshold — 80 lines of tested code |
| String truncation | Custom `if s.len() > n` logic | `truncate()` in `cli.rs` | Already handles the `...` suffix correctly; reuse it |
| DB + model initialization | Inline `LocalEngine::new()` + `db::open()` | `init_db_and_embedding()` in `cli.rs` | Phase 17 extracted this helper specifically for reuse by Phase 18 |
| Distance formatting | Manual float to string | `format!("{:.4}", item.distance)` | Standard Rust format specifier; 4 decimal places per D-10 |

**Key insight:** This phase is intentionally thin. The service layer does all the work; `run_search()` is a pure I/O adapter — args in, table out.

## Common Pitfalls

### Pitfall 1: query field ownership in match arm
**What goes wrong:** `args.query` is a `String` owned by `SearchArgs`. After passing `args` to `run_search()`, the value is moved. The match arm must clone or use the value before passing `args`.
**Why it happens:** The Remember arm used `args.content.take()` (Option). Search has a non-optional `String` field.
**How to avoid:** Clone the query before passing: `cli::run_search(args.query.clone(), args, service).await`. Inside `run_search`, the signature takes `query: String` separately from `args: SearchArgs`.
**Warning signs:** Compiler error: "use of partially moved value: `args`".

### Pitfall 2: Forgetting to add Search arm to Commands exhaustive match
**What goes wrong:** The server path at the bottom of main.rs also has a `match` — adding `Search` to the `Commands` enum will produce a compile error if the new variant is not handled.
**Why it happens:** The `Some(cli::Commands::Serve) | None => {}` arm does not cover `Search`.
**How to avoid:** The match in main.rs covers every `Commands` variant explicitly. Adding `Some(Commands::Search(args)) => { ... return Ok(()); }` before the `Serve | None` arm handles it.
**Warning signs:** `non-exhaustive patterns: Some(Search(_)) not covered` compile error.

### Pitfall 3: distance type mismatch (f32 vs f64)
**What goes wrong:** `SearchParams.threshold` is `Option<f32>` but `SearchResultItem.distance` is `f64`. Passing the wrong type to format or comparison causes a type error.
**Why it happens:** The threshold input from CLI is f32 (sufficient precision for user input) but the distance stored in the result is f64 (SQLite returns f64).
**How to avoid:** Format `item.distance` (f64) with `format!("{:.4}", item.distance)`. The threshold comparison happens inside `search_memories()` — no CLI code handles the comparison.
**Warning signs:** Type mismatch error when trying to compare `item.distance` (f64) to `args.threshold` (Option<f32>) in the handler.

### Pitfall 4: Test runtime expectations
**What goes wrong:** Each integration test that calls `mnemonic search "query"` takes ~2-3s for model load. A test that calls both `remember` and `search` will take ~4-6s.
**Why it happens:** Each `std::process::Command` invocation is a fresh process — no warm model.
**How to avoid:** Minimize the number of tests that trigger full store+search. One end-to-end test is sufficient; error path tests should be fast (no model load).
**Warning signs:** Integration test suite taking >60s total (indicates too many model-load tests).

### Pitfall 5: Column width overflow for DIST column
**What goes wrong:** `format!("{:.4}", distance)` produces a 6-character string (e.g., `0.1234`). Using `{:<6}` for left-align is correct. But very small distances like `0.0000` are fine; the issue is if distance > 9 (e.g., `10.1234` = 7 chars) which would overflow.
**Why it happens:** sqlite-vec L2-normalized embeddings produce cosine distance in [0, 2]. A 6-char column can hold up to `9.9999` safely; values >= 10.0 would overflow.
**How to avoid:** In practice, distances from well-formed embeddings stay < 2.0 for cosine distance. The 6-char column is sufficient. No special handling needed.
**Warning signs:** Misaligned table when distance values are unusually large (indicates data integrity issue, not a code bug).

## Code Examples

### Complete run_search() signature and wiring

```rust
// Source: src/cli.rs (new function, parallel to run_remember at line 188)
/// Entry point for `mnemonic search` -- performs semantic search via MemoryService.
/// query must already be validated (not empty) before calling.
pub async fn run_search(query: String, args: SearchArgs, service: crate::service::MemoryService) {
    // ... (see Pattern 4 above for full implementation)
}
```

### main.rs match arm placement

```rust
// Source: src/main.rs (add BEFORE the Serve | None arm)
Some(cli::Commands::Search(args)) => {
    if args.query.trim().is_empty() {
        eprintln!("error: query must not be empty");
        std::process::exit(1);
    }
    let (service, _config) = cli::init_db_and_embedding(db_override).await?;
    cli::run_search(args.query.clone(), args, service).await;
    return Ok(());
}
```

### Integration test: end-to-end search with remember seed

```rust
// Source: tests/cli_integration.rs (established pattern from Phase 17)
#[test]
fn test_search_returns_ranked_results() {
    let db = TempDb::new("search_basic");
    let bin = binary();

    // Seed a memory via remember (triggers model load ~2-3s)
    let remember_out = Command::new(&bin)
        .args(["--db", db.path_str(), "remember", "Paris is the capital of France"])
        .output()
        .expect("failed to run remember");
    assert!(remember_out.status.success());

    // Search for it (triggers model load ~2-3s)
    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "search", "French capital city"])
        .output()
        .expect("failed to run search");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "search must exit 0; stderr: {}", stderr);
    assert!(stdout.contains("DIST"), "stdout must contain DIST column header");
    assert!(stdout.contains("ID"), "stdout must contain ID column header");
    assert!(stdout.contains("Found"), "stdout must contain footer line");
}
```

### Integration test: empty query error path (fast — no model load)

```rust
#[test]
fn test_search_empty_query_exits_one() {
    let db = TempDb::new("search_empty_query");
    let bin = binary();

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "search", ""])
        .output()
        .expect("failed to run mnemonic binary");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "empty query must exit non-zero");
    assert!(
        stderr.contains("query must not be empty"),
        "stderr must contain error message; got: {:?}", stderr
    );
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| CLI calls HTTP server | CLI calls MemoryService directly | Phase 17 | No server dependency; works standalone |
| Model load inline | `init_db_and_embedding()` helper | Phase 17 | Shared across search, compact; no duplication |
| Full init for all commands | Tiered init (minimal/medium/full) | Phase 16/17 | recall stays fast (~50ms); search/remember accept ~2-3s |

**Current and correct:**
- `search_memories()` uses sqlite-vec KNN + post-filter + threshold in one SQL call (not two queries)
- `distance` is raw cosine distance (0 = identical, higher = less similar) — honest representation
- `truncate()` helper already handles the `...` suffix at exactly the right boundary

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `cargo test` |
| Config file | none — cargo test discovers tests automatically |
| Quick run command | `cargo test --test cli_integration test_search` |
| Full suite command | `cargo test` |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SRC-01 | `mnemonic search <query>` performs semantic search and displays results | integration | `cargo test --test cli_integration test_search_returns` | No — Wave 0 |
| SRC-01 | Table output includes DIST, ID, CONTENT, AGENT columns | integration | `cargo test --test cli_integration test_search_table_format` | No — Wave 0 |
| SRC-01 | Empty result set prints "No matching memories found." | integration | `cargo test --test cli_integration test_search_no_results` | No — Wave 0 |
| SRC-01 | Footer shows "Found N result(s)" | integration | `cargo test --test cli_integration test_search_footer` | No — Wave 0 |
| SRC-02 | `--limit` flag caps number of results | integration | `cargo test --test cli_integration test_search_limit_flag` | No — Wave 0 |
| SRC-02 | `--threshold` flag filters by distance | integration | `cargo test --test cli_integration test_search_threshold_flag` | No — Wave 0 |
| SRC-02 | `--agent-id` flag filters by agent | integration | `cargo test --test cli_integration test_search_agent_id_flag` | No — Wave 0 |
| SRC-02 | `--session-id` flag filters by session | integration | `cargo test --test cli_integration test_search_session_id_flag` | No — Wave 0 |
| SRC-01 | Empty query exits 1 with error to stderr | integration | `cargo test --test cli_integration test_search_empty_query_exits_one` | No — Wave 0 |
| SRC-01 | `search` appears in `mnemonic --help` | integration | `cargo test --test cli_integration test_search_appears_in_help` | No — Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test --test cli_integration test_search`
- **Per wave merge:** `cargo test`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `tests/cli_integration.rs` — Phase 18 test section (all test functions listed above), appended to existing file

*(Existing test infrastructure — `binary()`, `TempDb`, file structure — is fully in place from Phases 14-17. No new framework setup needed. Only new test functions are required.)*

## Open Questions

1. **`--limit` integration test precision**
   - What we know: `search_memories()` uses `limit * 10` for the KNN candidate set when filters are active (oversampling). The `--limit` flag sets the final result count.
   - What's unclear: Testing "exactly N results returned" requires seeding enough memories that oversampling doesn't accidentally return fewer. A test seeding 3 memories with `--limit 2` should work reliably.
   - Recommendation: Seed 3 memories, search with `--limit 2`, assert exactly 2 results in the footer line.

2. **`--threshold` integration test precision**
   - What we know: Threshold is applied as post-filter inside `search_memories()` on the distance field. Threshold 0.0 means only exact matches pass. Threshold 2.0 passes everything for L2-normalized vectors.
   - What's unclear: Choosing a threshold value that reliably includes/excludes a seeded memory in a test environment.
   - Recommendation: Seed one very relevant memory, search with `--threshold 2.0` (should include it) and `--threshold 0.0001` (should exclude it unless content is identical). This gives a clean pass/fail boundary without needing calibrated embeddings.

## Sources

### Primary (HIGH confidence)
- `src/cli.rs` (direct inspection) — `Commands` enum, `RememberArgs` struct, `run_remember()`, `init_db_and_embedding()`, `truncate()`, table format patterns
- `src/main.rs` (direct inspection) — dispatch match, Remember arm pattern, db_override extraction
- `src/service.rs` (direct inspection) — `SearchParams` struct (line 36), `SearchResponse` (line 77), `SearchResultItem` (line 82), `search_memories()` (line 147), `distance: f64` field type confirmed
- `tests/cli_integration.rs` (direct inspection) — `binary()`, `TempDb`, test structure, binary invocation patterns
- `.planning/phases/18-search-subcommand/18-CONTEXT.md` — all locked decisions
- `.planning/research/SUMMARY.md` — Phase D (search) rationale and medium-init reuse

### Secondary (MEDIUM confidence)
- `.planning/phases/17-remember-subcommand/17-02-PLAN.md` — test task structure that this phase mirrors
- `.planning/phases/16-recall-subcommand/16-CONTEXT.md` — table format conventions (truncate width patterns)

### Tertiary (LOW confidence)
- None — all critical claims sourced from direct codebase inspection.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — verified via Cargo.toml direct inspection; no new dependencies
- Architecture: HIGH — verified via direct inspection of `src/cli.rs`, `src/main.rs`, `src/service.rs`; all integration points confirmed at exact line numbers
- Pitfalls: HIGH — derived from direct code analysis of ownership patterns, type definitions, and existing test infrastructure
- Test mapping: HIGH — requirements text from REQUIREMENTS.md; test patterns from existing Phase 17 integration tests

**Research date:** 2026-03-21
**Valid until:** 2026-06-21 (stable Rust + clap 4; no fast-moving dependencies in scope)
