# Phase 19: compact subcommand - Research

**Researched:** 2026-03-21
**Domain:** Rust CLI (clap), CompactionService wiring, full-init helper pattern
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Full-init helper**
- D-01: Create `init_compaction(db_override)` in `cli.rs` — the full-init counterpart to `init_db()` (fast) and `init_db_and_embedding()` (medium)
- D-02: Returns `(CompactionService, Config)` — constructs the full CompactionService with optional LLM engine
- D-03: Init sequence mirrors server init in main.rs lines 86-216: register_sqlite_vec → load_config → apply --db override → validate_config → open DB → init embedding → init optional LLM → construct CompactionService
- D-04: Cannot reuse `init_db_and_embedding()` — that returns `MemoryService`, but compact needs the individual components (`conn_arc`, `embedding`, optional `llm_engine`) to construct CompactionService
- D-05: LLM engine init follows the server pattern (main.rs lines 152-171): if `config.llm_provider` is `Some("openai")`, construct `OpenAiSummarizer`; if `None`, pass `None` to CompactionService (algorithmic merge only)
- D-06: Stderr progress messages: `"Loading embedding model..."` and `"Model loaded (Xms)"` for embedding (matching init_db_and_embedding pattern); `"LLM summarization: enabled (provider)"` or `"LLM summarization: disabled (algorithmic merge only)"` for LLM status

**CLI args structure**
- D-07: `Compact` variant in `Commands` enum wraps `CompactArgs` struct:
  - `--agent-id <ID>` — optional, defaults to empty string `""` (compacts default namespace where memories have no agent_id)
  - `--threshold <F>` — optional (CompactionService applies default 0.85 internally via `unwrap_or(0.85)`)
  - `--max-candidates <N>` — optional (CompactionService applies default 100 internally via `unwrap_or(100)`)
  - `--dry-run` — boolean flag (clap `#[arg(long)]`, defaults to false)
- D-08: No positional arguments — all parameters are flags (compaction is a system operation, not content-oriented like remember/search)
- D-09: agent_id defaults to empty string `""` — matches how memories stored without --agent-id are recorded (agent_id="" in DB). Bare `mnemonic compact` compacts the default namespace.

**Data access pattern**
- D-10: Construct `CompactRequest` from CLI args and pass to `CompactionService::compact()` — the existing method handles the full pipeline: fetch_candidates → compute_pairs → cluster → synthesize → atomic write
- D-11: No new SQL, no new service methods — CompactRequest maps directly: `agent_id` from `--agent-id` (default ""), `threshold`/`max_candidates`/`dry_run` as Option from clap

**Output format**
- D-12: On success with clusters found, print a summary to stdout:
  ```
  Compacted: 3 clusters, 8 memories merged → 3 new memories
  ```
  For dry-run:
  ```
  Dry run: 3 clusters, 8 memories would be merged → 3 new memories
  ```
- D-13: When 0 clusters found: `"No similar memories found to compact."` to stdout — exit 0 (no error, just nothing to do)
- D-14: If `truncated` is true in the response, append to stderr: `"Note: only {max_candidates} most recent memories were evaluated. Increase --max-candidates for broader coverage."`
- D-15: Run ID printed to stderr: `"Run: {run_id_short}"` — useful for audit trail but doesn't pollute stdout
- D-16: No per-cluster detail output in v1.3 — the summary line covers success criteria. Cluster detail (`id_mapping`) is available in `--json` output (Phase 20).

**Dispatch entry point**
- D-17: Add `run_compact(args: CompactArgs, compaction: CompactionService)` function in `cli.rs`, parallel to `run_remember()`, `run_search()`
- D-18: main.rs gets a new match arm: `Some(Commands::Compact(args))` → calls `init_compaction()` → calls `run_compact()`
- D-19: No early validation needed before init — unlike remember/search, compact has no user content to validate. All args are optional with sensible defaults.

### Claude's Discretion
- Whether `init_compaction()` shares any extracted sub-steps with `init_db_and_embedding()` or duplicates the embedding init code
- Exact stderr formatting for LLM status
- Test structure, mocking strategy for CompactionService in integration tests
- Whether to include cluster count per line or keep it to the single summary line

### Deferred Ideas (OUT OF SCOPE)
- `--json` flag for machine-readable output (including full id_mapping) — Phase 20 (OUT-02) handles this across all subcommands
- Per-cluster detail output (source IDs → new ID for each merge) — available via `--json` in Phase 20; human summary is sufficient for v1.3
- Confirmation prompt before non-dry-run compaction ("Are you sure? Y/n") — adds interactive I/O complexity; dry-run serves the safety role
- `--verbose` flag for per-cluster progress during compaction — future enhancement if users request it
- Progress bar for LLM summarization calls — overkill; LLM calls are per-cluster and fast enough
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CMP-01 | `mnemonic compact` triggers memory compaction from CLI | `init_compaction()` constructs CompactionService; `run_compact()` calls `CompactionService::compact()` and prints summary |
| CMP-02 | `mnemonic compact --dry-run` previews compaction without mutating data | `CompactRequest.dry_run = Some(true)` passed through; CompactionService skips atomic write; output prefix changes to "Dry run:" |
| CMP-03 | `mnemonic compact` accepts `--agent-id` and `--threshold` flags | `CompactArgs` struct with `--agent-id`, `--threshold`, `--max-candidates`; passed directly to `CompactRequest` |
</phase_requirements>

## Summary

Phase 19 adds `mnemonic compact` — the most complex CLI subcommand because it requires all three init tiers (DB + embedding + optional LLM). The implementation is straightforward: build a new `init_compaction()` helper in `cli.rs`, add a `CompactArgs` struct and `Compact` variant to the `Commands` enum, wire a new match arm in `main.rs`, and implement `run_compact()` that constructs a `CompactRequest` and calls the existing `CompactionService::compact()`.

The key architectural insight is that `init_compaction()` cannot reuse `init_db_and_embedding()` (which returns `MemoryService`) because `CompactionService::new()` takes the component parts separately: `Arc<Connection>`, `Arc<dyn EmbeddingEngine>`, `Option<Arc<dyn SummarizationEngine>>`, and `embedding_model: String`. The full init sequence mirrors `main.rs` lines 86-216 but drops tracing initialization, KeyService, server bind, and MemoryService.

The test approach follows the established Phase 18 pattern: binary invocation tests using `std::process::Command`, a `TempDb` helper for isolation, and `mnemonic remember` as the seeding mechanism (no direct rusqlite seeding needed since embeddings are required for compaction candidates — the vec_memories table must be populated). Dry-run tests can run faster since no data is mutated.

**Primary recommendation:** Implement as two files changed (`cli.rs`, `main.rs`) with a single new integration test file section in `tests/cli_integration.rs`. No new dependencies needed.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap (already in Cargo.toml) | existing | `CompactArgs` struct, `--dry-run` flag, optional `--agent-id`/`--threshold`/`--max-candidates` | Project standard; all existing subcommands use it |
| tokio (already in Cargo.toml) | existing | async `init_compaction()` and `run_compact()` functions | Project standard async runtime |
| anyhow (already in Cargo.toml) | existing | `anyhow::Result` return type in `init_compaction()` | Matches `init_db()` and `init_db_and_embedding()` signatures |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `crate::summarization::MockSummarizer` | internal | Test-only mock for CompactionService in unit tests | When testing `run_compact()` output formatting in isolation (if added to cli.rs tests) |

**Installation:** Zero new Cargo.toml dependencies — all requirements covered by the existing locked stack (confirmed from v1.3 research in STATE.md).

## Architecture Patterns

### File Layout (changes only)
```
src/
├── cli.rs          # Add: CompactArgs, Commands::Compact variant, init_compaction(), run_compact()
└── main.rs         # Add: Some(Commands::Compact(args)) match arm (after Search arm, before Serve|None)

tests/
└── cli_integration.rs  # Add: Phase 19 compact subcommand section (CMP-01, CMP-02, CMP-03)
```

### Pattern 1: CompactArgs Struct (mirrors SearchArgs)
**What:** All-optional flags struct — no positional arg since compaction is a system operation.
**When to use:** Always for `mnemonic compact`.

```rust
// Source: src/cli.rs line 81 (SearchArgs pattern)
/// Arguments for the `compact` subcommand.
#[derive(Args)]
pub struct CompactArgs {
    /// Scope compaction to a specific agent (default: compacts default namespace)
    #[arg(long, value_name = "ID")]
    pub agent_id: Option<String>,

    /// Similarity threshold for merging (default: 0.85)
    #[arg(long, value_name = "F")]
    pub threshold: Option<f32>,

    /// Max candidate memories to evaluate (default: 100)
    #[arg(long, value_name = "N")]
    pub max_candidates: Option<u32>,

    /// Preview what would be compacted without mutating data
    #[arg(long)]
    pub dry_run: bool,
}
```

### Pattern 2: Commands enum extension (exact insertion point)
**What:** Add `Compact(CompactArgs)` after `Search(SearchArgs)` in the enum.
**When to use:** Required — clap derives `--help` output order from declaration order.

```rust
// Source: src/cli.rs line 22 (Commands enum)
#[derive(Subcommand)]
pub enum Commands {
    Serve,
    Keys(KeysArgs),
    Recall(RecallArgs),
    Remember(RememberArgs),
    Search(SearchArgs),
    Compact(CompactArgs),  // ADD THIS
}
```

### Pattern 3: init_compaction() — full-init helper
**What:** Constructs `CompactionService` with all components. Mirrors `init_db_and_embedding()` but returns `(CompactionService, Config)` instead of `(MemoryService, Config)`.
**When to use:** Called from main.rs Compact match arm.

```rust
// Source: Derived from src/cli.rs lines 153-195 (init_db_and_embedding) and
//         src/main.rs lines 151-216 (LLM engine init + CompactionService construction)
pub async fn init_compaction(
    db_override: Option<String>,
) -> anyhow::Result<(crate::compaction::CompactionService, crate::config::Config)> {
    crate::db::register_sqlite_vec();
    let mut config = crate::config::load_config()
        .map_err(|e| anyhow::anyhow!(e))?;
    if let Some(ref db_path) = db_override {
        config.db_path = db_path.clone();
    }
    crate::config::validate_config(&config)?;

    let conn = crate::db::open(&config).await
        .map_err(|e| anyhow::anyhow!(e))?;
    let conn_arc = std::sync::Arc::new(conn);

    let embedding_model = match config.embedding_provider.as_str() {
        "openai" => "text-embedding-3-small".to_string(),
        _        => "all-MiniLM-L6-v2".to_string(),
    };

    let embedding: std::sync::Arc<dyn crate::embedding::EmbeddingEngine> =
        match config.embedding_provider.as_str() {
            "local" => {
                eprintln!("Loading embedding model...");
                let start = std::time::Instant::now();
                let engine = tokio::task::spawn_blocking(|| {
                    crate::embedding::LocalEngine::new()
                })
                .await?
                .map_err(|e| anyhow::anyhow!(e))?;
                eprintln!("Model loaded ({}ms)", start.elapsed().as_millis());
                std::sync::Arc::new(engine)
            }
            "openai" => {
                let api_key = config.openai_api_key.as_ref().unwrap();
                std::sync::Arc::new(crate::embedding::OpenAiEngine::new(api_key.clone()))
            }
            _ => unreachable!(),
        };

    let llm_engine: Option<std::sync::Arc<dyn crate::summarization::SummarizationEngine>> =
        match config.llm_provider.as_deref() {
            Some("openai") => {
                let api_key = config.llm_api_key.as_ref().unwrap();
                let base_url = config.llm_base_url.clone()
                    .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
                let model = config.llm_model.clone()
                    .unwrap_or_else(|| "gpt-4o-mini".to_string());
                eprintln!("LLM summarization: enabled (openai/{})", model);
                Some(std::sync::Arc::new(
                    crate::summarization::OpenAiSummarizer::new(api_key.clone(), base_url, model)
                ))
            }
            None => {
                eprintln!("LLM summarization: disabled (algorithmic merge only)");
                None
            }
            _ => unreachable!(),
        };

    let compaction = crate::compaction::CompactionService::new(
        conn_arc,
        embedding,
        llm_engine,
        embedding_model,
    );
    Ok((compaction, config))
}
```

### Pattern 4: run_compact() handler
**What:** Constructs `CompactRequest` from args, calls `compact()`, formats output.
**When to use:** Called after `init_compaction()` in the match arm.

```rust
// Source: Derived from CompactRequest struct (src/compaction.rs line 13)
//         and CompactResponse struct (src/compaction.rs line 21)
pub async fn run_compact(args: CompactArgs, compaction: crate::compaction::CompactionService) {
    let agent_id = args.agent_id.unwrap_or_default();  // "" for default namespace

    let req = crate::compaction::CompactRequest {
        agent_id,
        threshold: args.threshold,
        max_candidates: args.max_candidates,
        dry_run: Some(args.dry_run),
    };

    match compaction.compact(req).await {
        Ok(resp) => {
            // Audit trail to stderr
            let run_id_short = &resp.run_id[..8.min(resp.run_id.len())];
            eprintln!("Run: {}", run_id_short);

            // Truncation warning to stderr
            if resp.truncated {
                let max = args.max_candidates.unwrap_or(100);
                eprintln!(
                    "Note: only {} most recent memories were evaluated. \
                     Increase --max-candidates for broader coverage.",
                    max
                );
            }

            if resp.clusters_found == 0 {
                println!("No similar memories found to compact.");
                return;
            }

            if args.dry_run {
                println!(
                    "Dry run: {} clusters, {} memories would be merged → {} new memories",
                    resp.clusters_found, resp.memories_merged, resp.clusters_found
                );
            } else {
                println!(
                    "Compacted: {} clusters, {} memories merged → {} new memories",
                    resp.clusters_found, resp.memories_merged, resp.memories_created
                );
            }
        }
        Err(e) => {
            eprintln!("error: compaction failed: {}", e);
            std::process::exit(1);
        }
    }
}
```

**Note on dry-run new memories count:** In a dry-run, `memories_created` is 0 (no writes). The count of "new memories that would be created" equals `clusters_found` (one merged memory per cluster). Use `resp.clusters_found` for the dry-run output, not `resp.memories_created`.

### Pattern 5: main.rs match arm (insertion point)
**What:** New arm added after the `Search` arm, before the `Serve | None` fallthrough.
**When to use:** Required dispatch.

```rust
// Source: src/main.rs lines 70-83 (Search arm + Serve|None fallthrough)
Some(cli::Commands::Compact(args)) => {
    let (compaction, _config) = cli::init_compaction(db_override).await?;
    cli::run_compact(args, compaction).await;
    return Ok(());
}
```

### Pattern 6: CompactRequest field mapping (verified against actual struct)
**What:** `CompactRequest` requires `agent_id: String` (not Option) — `args.agent_id.unwrap_or_default()` resolves `Option<String>` to `String`.

```rust
// Source: src/compaction.rs lines 13-18
pub struct CompactRequest {
    pub agent_id: String,          // required — default "" for global namespace
    pub threshold: Option<f32>,    // None → CompactionService uses 0.85
    pub max_candidates: Option<u32>, // None → CompactionService uses 100
    pub dry_run: Option<bool>,     // None → CompactionService treats as false
}
```

### Anti-Patterns to Avoid
- **Returning MemoryService from init_compaction:** init_db_and_embedding() builds MemoryService as the output. init_compaction() must NOT call init_db_and_embedding() and then try to decompose — MemoryService fields are private. Build components directly.
- **Calling validate_config() twice:** The server path already calls it after load_config(). In the CLI path, validate_config() is called once in init_compaction(), which validates both embedding AND LLM provider config. Do not add a second call.
- **Forgetting dry_run count difference:** For dry-run output, use `resp.clusters_found` (not `resp.memories_created` which is 0) to show how many new memories would be created.
- **Seeding compaction tests with seed_memory():** The `seed_memory()` helper in cli_integration.rs only creates a row in the `memories` table — it does NOT populate `vec_memories`. CompactionService.fetch_candidates() does a JOIN on `vec_memories`, so seeded memories without embeddings produce zero candidates. Use `mnemonic remember` binary invocations for seeding instead.
- **Using tracing macros in CLI init path:** The CLI path does not init tracing (that only happens in the server path). Use `eprintln!()` for progress messages, not `tracing::info!()`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Compaction pipeline | Custom SQL merge logic | `CompactionService::compact()` | Full pipeline: fetch → cluster → synthesize → atomic write. Already handles dry_run, truncation, audit log. |
| LLM engine construction | Custom HTTP client | `OpenAiSummarizer::new()` | Handles reqwest client config, timeout (30s), XML prompt injection prevention, typed error variants |
| Config validation | Custom env var checks | `validate_config()` | Validates both embedding AND LLM provider fields; handles llm_api_key requirement for "openai" provider |
| Mock for tests | Fake HTTP server | `MockSummarizer` (already exists in summarization.rs) | Deterministic output, no network, already Send+Sync |

**Key insight:** All the hard parts (similarity clustering, atomic write, audit runs table) are already implemented in CompactionService. This phase is pure CLI wiring.

## Common Pitfalls

### Pitfall 1: vec_memories JOIN excludes seed_memory() rows
**What goes wrong:** Tests using `seed_memory()` see 0 clusters even when multiple memories are seeded, because `fetch_candidates()` JOINs `vec_memories ON vec_memories.memory_id = memories.id` — rows without embeddings are excluded.
**Why it happens:** `seed_memory()` pre-dates embedding-required subcommands. It creates `memories` rows but not `vec_memories` rows.
**How to avoid:** Use `mnemonic remember` binary invocation to seed memories for compaction tests. This ensures both `memories` and `vec_memories` rows are created.
**Warning signs:** Test shows `clusters_found: 0` and `memories_merged: 0` despite seeded data.

### Pitfall 2: dry-run memories_created is always 0
**What goes wrong:** Using `resp.memories_created` in dry-run output prints "0 new memories" instead of the expected cluster count.
**Why it happens:** `CompactionService::compact()` sets `memories_created = 0` for dry_run (line 337 of compaction.rs: `memories_created = 0`). The number of would-be new memories equals `clusters_found`.
**How to avoid:** In `run_compact()`, use `resp.clusters_found` as the new memories count for dry-run output.
**Warning signs:** Dry-run output shows `→ 0 new memories` instead of the cluster count.

### Pitfall 3: Partial-move of args before run_compact() call
**What goes wrong:** Compiler error "use of partially moved value: args" if any field of `CompactArgs` is moved before passing `args` to `run_compact()`.
**Why it happens:** Phase 18 encountered this with `args.query.clone()` (documented in STATE.md: "args.query.clone() passed as first param before moving args -- avoids partial-move compiler error"). For compact, `args` contains only optional fields, but the same rule applies.
**How to avoid:** In main.rs, pass `args` directly to `run_compact()` — do not extract fields before passing. The `run_compact()` function receives ownership of `args` and handles all field access internally.
**Warning signs:** Compiler error mentioning partial move or borrow of moved value.

### Pitfall 4: agent_id Option<String> vs String type mismatch
**What goes wrong:** `CompactRequest.agent_id` is `String`, but `CompactArgs.agent_id` is `Option<String>`. Direct assignment fails to compile.
**Why it happens:** `CompactRequest` was designed for the HTTP layer where agent_id is required in the JSON body. The CLI provides it as optional with a default.
**How to avoid:** Use `args.agent_id.unwrap_or_default()` to convert `Option<String>` to `String` (`""` when not provided).
**Warning signs:** Compiler type error: expected `String`, found `Option<String>`.

### Pitfall 5: test suite runtime from embedding model loads
**What goes wrong:** Compact integration tests each load the embedding model (~2-3s) per `mnemonic remember` and per `mnemonic compact` invocation. Multiple seeded memories multiply runtime.
**Why it happens:** Each binary invocation is independent — no warm model state between tests.
**How to avoid:** Keep seeding minimal (2-3 memories per test). Prefer one test that seeds enough memories to verify cluster behavior rather than many tests that each seed independently. The dry-run test can share a DB with the basic test if structured carefully, but TempDb isolation is safer.
**Warning signs:** Test suite takes >60s for compact tests alone.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in + cargo test |
| Config file | `tests/cli_integration.rs` (existing) |
| Quick run command | `cargo test --test cli_integration compact 2>&1` |
| Full suite command | `cargo test 2>&1` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CMP-01 | `mnemonic compact` on 2+ similar memories exits 0 and prints "Compacted:" summary | integration (binary) | `cargo test --test cli_integration test_compact_basic` | No — Wave 0 |
| CMP-01 | `mnemonic compact` on empty DB exits 0 and prints "No similar memories found to compact." | integration (binary) | `cargo test --test cli_integration test_compact_no_results` | No — Wave 0 |
| CMP-01 | `mnemonic compact` appears in `--help` | integration (binary) | `cargo test --test cli_integration test_compact_appears_in_help` | No — Wave 0 |
| CMP-02 | `mnemonic compact --dry-run` exits 0 and prints "Dry run:" summary without mutating | integration (binary) | `cargo test --test cli_integration test_compact_dry_run` | No — Wave 0 |
| CMP-03 | `mnemonic compact --agent-id <id>` scopes compaction to one agent namespace | integration (binary) | `cargo test --test cli_integration test_compact_agent_id_flag` | No — Wave 0 |
| CMP-03 | `mnemonic compact --threshold 0.5` uses custom threshold | integration (binary) | `cargo test --test cli_integration test_compact_threshold_flag` | No — Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test --test cli_integration compact 2>&1`
- **Per wave merge:** `cargo test 2>&1`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `tests/cli_integration.rs` section for Phase 19 compact — covers CMP-01, CMP-02, CMP-03
- Framework and test infrastructure already exist (no new setup needed).

## Code Examples

### CompactionService constructor signature (verified from source)
```rust
// Source: src/compaction.rs lines 66-73
pub fn new(
    db: Arc<Connection>,
    embedding: Arc<dyn EmbeddingEngine>,
    summarization: Option<Arc<dyn SummarizationEngine>>,
    embedding_model: String,
) -> Self
```

### CompactResponse fields (verified from source)
```rust
// Source: src/compaction.rs lines 21-28
pub struct CompactResponse {
    pub run_id: String,           // full UUID v7 string
    pub clusters_found: u32,
    pub memories_merged: u32,     // total source memories consumed across all clusters
    pub memories_created: u32,    // 0 for dry_run=true, == clusters_found for dry_run=false
    pub id_mapping: Vec<ClusterMapping>,
    pub truncated: bool,          // true if max_candidates was hit
}
```

### Existing init_db_and_embedding return signature (for comparison)
```rust
// Source: src/cli.rs lines 153-155
pub async fn init_db_and_embedding(
    db_override: Option<String>,
) -> anyhow::Result<(crate::service::MemoryService, crate::config::Config)>
```

### Compact integration test skeleton (follows Phase 18 pattern)
```rust
// Source: tests/cli_integration.rs lines 991-1026 (test_search_returns_ranked_results pattern)
#[test]
fn test_compact_basic() {
    let db = TempDb::new("compact_basic");
    let bin = binary();

    // Seed 2 similar memories via `mnemonic remember` (embedding required for compaction)
    for content in &["Paris is the capital of France", "France's capital city is Paris"] {
        let seed = Command::new(&bin)
            .args(["--db", db.path_str(), "remember", content])
            .output()
            .expect("failed to run mnemonic binary");
        assert!(seed.status.success(), "remember must succeed");
    }

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "compact"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "compact must exit 0; stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(stdout.contains("Compacted:"), "stdout must contain 'Compacted:'");
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| init_db_and_embedding (Phase 17) returns MemoryService | init_compaction (Phase 19) returns CompactionService | Phase 19 | Establishes three-tier init pattern: fast/medium/full |
| Server-only compaction trigger | CLI compact subcommand | Phase 19 | Users can trigger compaction without running HTTP server |

**Pattern evolution — init tiers:**
- Fast (~50ms): `init_db()` → DB only → used by `keys`, `recall`
- Medium (~2-3s): `init_db_and_embedding()` → DB + embedding → `MemoryService` → used by `remember`, `search`
- Full (~2-3s + LLM setup): `init_compaction()` → DB + embedding + optional LLM → `CompactionService` → used by `compact`

## Open Questions

1. **Embedding code duplication in init_compaction()**
   - What we know: `init_db_and_embedding()` and `init_compaction()` will contain identical embedding init code (~20 lines). D-04 explicitly prohibits reusing `init_db_and_embedding()` because it returns `MemoryService`.
   - What's unclear: Whether to extract a private `init_embedding_engine()` helper to avoid duplication, or simply duplicate the embedding block.
   - Recommendation (Claude's Discretion): Extract a `pub(crate) async fn init_embedding(config: &Config) -> anyhow::Result<(Arc<dyn EmbeddingEngine>, String)>` helper that both `init_db_and_embedding()` and `init_compaction()` call. This avoids ~20 lines of duplication and makes future changes to embedding init (new provider, etc.) a single edit. If the planner judges this adds unnecessary complexity to a simple phase, duplication is also acceptable.

2. **Compact test similarity threshold for test reliability**
   - What we know: The default threshold is 0.85. Two very similar sentences will exceed this. The test uses "Paris is the capital of France" and "France's capital city is Paris" — these should produce similarity > 0.85 with the local model.
   - What's unclear: Whether the default threshold reliably clusters these sentences on all platforms/model versions.
   - Recommendation: In the basic compaction test, pass `--threshold 0.7` explicitly to ensure clustering is reliable regardless of minor embedding model variations. The threshold test (CMP-03) can then verify that a very high threshold (e.g., 0.99) finds 0 clusters.

## Sources

### Primary (HIGH confidence)
- `src/compaction.rs` — CompactionService struct, CompactRequest/CompactResponse types, compact() pipeline, verified directly
- `src/cli.rs` — Commands enum, SearchArgs pattern, init_db_and_embedding() pattern, existing handlers, verified directly
- `src/main.rs` — Match dispatch pattern, LLM engine init (lines 151-171), CompactionService construction (lines 208-216), verified directly
- `src/config.rs` — Config struct fields (llm_provider, llm_api_key, llm_base_url, llm_model), validate_config() behavior, verified directly
- `src/summarization.rs` — OpenAiSummarizer::new() signature, MockSummarizer for tests, verified directly
- `tests/cli_integration.rs` — TempDb pattern, binary() helper, search test structure as Phase 18 reference, verified directly
- `.planning/phases/19-compact-subcommand/19-CONTEXT.md` — All locked decisions D-01 through D-19, verified directly

### Secondary (MEDIUM confidence)
- STATE.md accumulated decisions — Phase 17/18 pitfalls (partial-move pattern, early validation) cross-referenced with source

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — zero new dependencies; all reuse existing locked Cargo.toml crates
- Architecture: HIGH — all patterns verified directly against source files; no guessing
- Pitfalls: HIGH — Pitfalls 1-3 cross-verified against actual source (seed_memory() SQL, compaction.rs line 337, STATE.md Phase 18 partial-move entry)

**Research date:** 2026-03-21
**Valid until:** Until compaction.rs, cli.rs, or main.rs are modified (stable — these are not fast-moving)
