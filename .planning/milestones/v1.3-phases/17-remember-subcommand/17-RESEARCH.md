# Phase 17: remember subcommand - Research

**Researched:** 2026-03-21
**Domain:** Rust CLI (clap), stdin detection, embedding init, MemoryService integration
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Content input strategy**
- D-01: Positional argument takes priority: `mnemonic remember "some content"` uses the positional arg directly
- D-02: If no positional arg, check stdin with `std::io::IsTerminal` — if stdin is piped (not a terminal), read all of stdin as content
- D-03: If neither positional arg nor piped stdin is available, print usage error to stderr and exit 1: `"error: provide content as an argument or pipe via stdin"`
- D-04: If both positional arg AND piped stdin exist, positional arg wins — stdin is ignored (standard CLI convention, avoids ambiguity)

**Medium-init helper**
- D-05: Extract `init_db_and_embedding(db_override)` in `cli.rs` — the medium-init counterpart to the existing `init_db()` fast-init helper
- D-06: Returns `(MemoryService, Config)` — constructs the full MemoryService since both `remember` (Phase 17) and `search` (Phase 18) need it
- D-07: Calls `validate_config()` — unlike fast-path commands, embedding needs valid provider config
- D-08: Uses `spawn_blocking` for `LocalEngine::new()` — matches the server init pattern in main.rs lines 85-87
- D-09: Prints model loading progress to stderr: `"Loading embedding model..."` and `"Model loaded ({elapsed}ms)"` — gives the user feedback during the 2-3s wait without polluting stdout

**CLI args structure**
- D-10: `Remember` variant in `Commands` enum wraps `RememberArgs` struct:
  - `content` — optional positional arg (String)
  - `--agent-id <ID>` — optional, defaults to empty string (matches API behavior)
  - `--session-id <ID>` — optional, defaults to empty string (matches API behavior)
  - `--tags tag1,tag2` — optional comma-separated string
- D-11: Tags parsed by splitting on comma, trimming whitespace per tag, filtering empty strings

**Data access pattern**
- D-12: Reuse `MemoryService::create_memory()` — it already validates, embeds, and inserts atomically with dual-table transaction. No reimplementation needed.
- D-13: Construct `CreateMemoryRequest` from CLI args and pass to `create_memory()`

**Output format**
- D-14: On success, print the full UUID on stdout (line 1, pipeable for scripting: `id=$(mnemonic remember "content")`)
- D-15: Print a confirmation summary to stderr: `"Stored memory <8-char-id>"` — human context without polluting stdout (matches `keys create` pattern)

**Early validation**
- D-16: Validate content is not empty/whitespace BEFORE loading the embedding model — avoids 2-3s model load penalty for trivially invalid input
- D-17: Error message: `"error: content must not be empty"` to stderr, exit 1

**Dispatch entry point**
- D-18: Add `run_remember(args: RememberArgs, service: MemoryService)` function in `cli.rs`, parallel to `run_recall()` and `run_keys()`
- D-19: main.rs gets a new match arm: `Some(Commands::Remember(args))` → calls `init_db_and_embedding()` → resolves content (positional vs stdin) → calls `run_remember()`

### Claude's Discretion
- Whether stdin reading happens in `run_remember()` or in the main.rs match arm before calling it
- Exact stderr formatting for model load progress (tracing vs eprintln)
- Whether to use `eprintln!` directly or a small stderr print helper
- Test structure and mocking strategy for embedding in tests

### Deferred Ideas (OUT OF SCOPE)
- `--json` flag for machine-readable output — Phase 20 (OUT-02) handles this across all subcommands
- Batch import from stdin (multiple memories per line) — future `mnemonic import` command (IMP-01)
- Content from file path (`mnemonic remember @file.txt`) — too magical, pipe is sufficient
- Progress bar during model load — overkill for a 2-3s operation; simple stderr message is enough
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| REM-01 | `mnemonic remember <content>` stores a memory with embedded content | D-01 + D-08 + D-12: positional arg → early validation → init_db_and_embedding → create_memory |
| REM-02 | `mnemonic remember` reads content from stdin when piped (no positional arg) | D-02: `std::io::IsTerminal` on stdin; confirmed available in Rust 1.70+ (project uses 1.94) |
| REM-03 | `mnemonic remember` accepts `--agent-id` and `--session-id` flags | D-10 + D-13: RememberArgs fields → CreateMemoryRequest; mirrors RecallArgs pattern |
| REM-04 | `mnemonic remember` accepts `--tags` flag for tagging memories | D-10 + D-11: comma-split, trim, filter-empty; CreateMemoryRequest.tags = Some(vec) |
</phase_requirements>

## Summary

Phase 17 adds the `mnemonic remember` subcommand — a medium-init CLI command that accepts content as a positional argument or piped stdin, embeds it via the existing embedding infrastructure, and stores it using `MemoryService::create_memory()`. No new dependencies are required. The implementation is a direct extension of patterns already established in Phase 16 (recall) with one new element: the `init_db_and_embedding()` helper that sits between the fast-path `init_db()` and the full server init.

The key technical challenge is correct embedding init. `LocalEngine::new()` is a blocking operation (downloads/loads model weights from HuggingFace cache) and must be called inside `tokio::task::spawn_blocking`. This pattern is already live in `main.rs` lines 85-87 — the medium-init helper is a clean extraction of that logic plus the DB init from `init_db()`. The OpenAI embedding path requires no `spawn_blocking` since it's an async HTTP call.

The planner should structure this as two plans: Plan 01 adds the CLI wiring and `init_db_and_embedding` helper (the non-embedding-dependent logic that compiles cleanly), and Plan 02 adds integration tests. Since integration tests require the compiled binary and the embedding model (2-3s load), tests must use the binary invocation pattern established in `cli_integration.rs`, not unit tests.

**Primary recommendation:** Follow D-05 through D-19 verbatim. Every decision is backed by direct inspection of existing code. No design choices are open except the three Claude's Discretion items.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4 (derive) | CLI arg parsing, `RememberArgs` struct | Already in use; derive macros for zero boilerplate |
| tokio | 1 (full) | Async runtime, `spawn_blocking` for model load | Already in use; `spawn_blocking` is the correct primitive |
| std::io::IsTerminal | stdlib (Rust 1.70+) | Detect piped stdin without external crate | Confirmed available; rustc 1.94 in use |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| anyhow | 1 | Error propagation in `init_db_and_embedding` | Same as `init_db()` — `?` operator on mixed error types |
| rusqlite (dev) | 0.37 | Test seeding for integration tests | Same pattern as Phase 16 `seed_memory()` helper |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `std::io::IsTerminal` | `atty` crate | `atty` is deprecated; stdlib is correct choice |
| `eprintln!` for progress | `tracing::info!` | tracing requires init; CLI commands skip tracing init — use `eprintln!` |
| Positional optional arg | Subcommand with required arg | Clap optional positional enables stdin fallback cleanly |

**Installation:** No new dependencies needed. All crates already in `Cargo.toml`.

**Version verification:** All crates confirmed present in Cargo.toml at their stated versions via direct file inspection (HIGH confidence, no npm view needed for Rust crates).

## Architecture Patterns

### Recommended Project Structure
No new files needed. All changes go to existing files:
```
src/
├── cli.rs        # Add: RememberArgs, Commands::Remember, init_db_and_embedding, run_remember
└── main.rs       # Add: Some(Commands::Remember(args)) match arm
tests/
└── cli_integration.rs  # Add: Phase 17 remember integration tests
```

### Pattern 1: Medium-Init Helper (init_db_and_embedding)

**What:** Shared init function that does DB + embedding initialization without LLM, CompactionService, KeyService, or server bind. Returns `(MemoryService, Config)`.

**When to use:** `remember` (Phase 17) and `search` (Phase 18) — any CLI command requiring embeddings.

**Example:**
```rust
// Mirrors: src/main.rs lines 44-107, but extracts only DB + embedding + MemoryService
pub async fn init_db_and_embedding(
    db_override: Option<String>,
) -> anyhow::Result<(crate::service::MemoryService, crate::config::Config)> {
    crate::db::register_sqlite_vec();
    let mut config = crate::config::load_config()
        .map_err(|e| anyhow::anyhow!(e))?;
    if let Some(ref db_path) = db_override {
        config.db_path = db_path.clone();
    }
    crate::config::validate_config(&config)?;  // D-07: required for embedding

    let conn = crate::db::open(&config).await
        .map_err(|e| anyhow::anyhow!(e))?;
    let conn_arc = std::sync::Arc::new(conn);

    // Mirror main.rs lines 76-107 (local vs openai embedding branches)
    let embedding_model = match config.embedding_provider.as_str() {
        "openai" => "text-embedding-3-small".to_string(),
        _        => "all-MiniLM-L6-v2".to_string(),
    };
    let embedding: std::sync::Arc<dyn crate::embedding::EmbeddingEngine> =
        match config.embedding_provider.as_str() {
            "local" => {
                eprintln!("Loading embedding model...");  // D-09
                let start = std::time::Instant::now();
                let engine = tokio::task::spawn_blocking(|| {
                    crate::embedding::LocalEngine::new()
                })
                .await?
                .map_err(|e| anyhow::anyhow!(e))?;
                eprintln!("Model loaded ({}ms)", start.elapsed().as_millis());  // D-09
                std::sync::Arc::new(engine)
            }
            "openai" => {
                let api_key = config.openai_api_key.as_ref().unwrap();
                std::sync::Arc::new(crate::embedding::OpenAiEngine::new(api_key.clone()))
            }
            _ => unreachable!(),
        };

    let service = crate::service::MemoryService::new(conn_arc, embedding, embedding_model);
    Ok((service, config))
}
```

### Pattern 2: RememberArgs Struct (mirrors RecallArgs)

**What:** Clap `Args` struct with optional positional + optional named flags.

**Example:**
```rust
// Source: mirrors src/cli.rs lines 39-56 (RecallArgs pattern)
#[derive(Args)]
pub struct RememberArgs {
    /// Memory content (or pipe via stdin)
    pub content: Option<String>,

    /// Associate memory with an agent
    #[arg(long, value_name = "ID")]
    pub agent_id: Option<String>,

    /// Associate memory with a session
    #[arg(long, value_name = "ID")]
    pub session_id: Option<String>,

    /// Comma-separated tags (e.g. "work,important")
    #[arg(long, value_name = "TAGS")]
    pub tags: Option<String>,
}
```

### Pattern 3: Content Resolution (stdin detection)

**What:** Resolve content from positional arg or piped stdin, with early validation.

**Example:**
```rust
// Source: std::io::IsTerminal (stable since Rust 1.70)
use std::io::IsTerminal;

// In main.rs match arm or run_remember:
let content = if let Some(c) = args.content {
    c  // D-01: positional arg takes priority
} else if !std::io::stdin().is_terminal() {
    // D-02: stdin is piped — read all of it
    use std::io::Read;
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf)
        .map_err(|e| { eprintln!("error: failed to read stdin: {}", e); std::process::exit(1); });
    buf
} else {
    // D-03: neither provided
    eprintln!("error: provide content as an argument or pipe via stdin");
    std::process::exit(1);
};

// D-16: Early validation BEFORE model load
if content.trim().is_empty() {
    eprintln!("error: content must not be empty");
    std::process::exit(1);
}
```

### Pattern 4: run_remember Entry Point

**What:** Handler function called after init and content resolution.

**Example:**
```rust
// Source: mirrors cli.rs run_recall() pattern (line 106)
pub async fn run_remember(content: String, args: RememberArgs, service: crate::service::MemoryService) {
    // D-11: Parse tags from comma-separated string
    let tags: Vec<String> = args.tags
        .unwrap_or_default()
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    let tags_opt = if tags.is_empty() { None } else { Some(tags) };

    let req = crate::service::CreateMemoryRequest {
        content,
        agent_id: args.agent_id,
        session_id: args.session_id,
        tags: tags_opt,
    };

    match service.create_memory(req).await {
        Ok(memory) => {
            println!("{}", memory.id);  // D-14: full UUID on stdout line 1
            let short_id = &memory.id[..8.min(memory.id.len())];
            eprintln!("Stored memory {}", short_id);  // D-15: human context to stderr
        }
        Err(e) => {
            eprintln!("error: failed to store memory: {}", e);
            std::process::exit(1);
        }
    }
}
```

### Pattern 5: main.rs Match Arm

**What:** New dispatch arm added after the Recall arm.

**Example:**
```rust
// Source: mirrors main.rs lines 34-37 (Recall arm pattern)
Some(cli::Commands::Remember(mut args)) => {
    // Content resolution happens here (before init_db_and_embedding)
    let content = /* stdin/positional resolution — see Pattern 3 */;
    // D-16: early validation before model load
    if content.trim().is_empty() {
        eprintln!("error: content must not be empty");
        std::process::exit(1);
    }
    let (service, _config) = cli::init_db_and_embedding(db_override).await?;
    cli::run_remember(content, args, service).await;
    return Ok(());
}
```

### Anti-Patterns to Avoid

- **Calling `LocalEngine::new()` directly in async context:** It blocks the tokio runtime. Always wrap in `spawn_blocking`. See main.rs line 85.
- **Validating content AFTER loading the embedding model:** The model takes 2-3 seconds to load. Validate first (D-16).
- **Reading stdin in `run_remember()` after content is needed in the match arm:** Content must be known before `run_remember()` is called, because the function signature takes a resolved `content: String`. Resolve in the match arm (or a helper called from the match arm).
- **Using `tracing::info!` for model load progress in CLI commands:** CLI commands skip `server::init_tracing()`. Use `eprintln!` directly (D-09).
- **Putting `spawn_blocking` inside `run_remember()`:** The blocking init belongs in `init_db_and_embedding()`, not in the remember handler. This keeps the handler free of init logic.
- **Forgetting `db_override` is consumed:** `db_override` is extracted once before the match (as in main.rs line 21). The pattern passes it by value to `init_db_and_embedding()`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Embedding + vector insert | Custom SQL + vector encode | `MemoryService::create_memory()` | Already handles dual-table atomic transaction, UUID generation, embedding, tags JSON encode |
| IsTerminal check | Signal/ioctl hacks | `std::io::IsTerminal` (stdlib) | Correct, cross-platform, zero-dep since Rust 1.70 |
| Tag parsing | Hand-split and validate | `.split(',').map(trim).filter(non-empty)` (one-liner) | No edge cases beyond this |
| Config validation | Manual provider checks | `config::validate_config()` | Already validates local vs openai vs unknown providers |
| Embedding model loading | Manual candle init | `LocalEngine::new()` inside `spawn_blocking` | Already handles HuggingFace Hub download, cache, tokenizer, model load |

**Key insight:** Every piece of the remember pipeline already exists. This phase is purely wiring — struct definitions, helper extraction, and match arms.

## Common Pitfalls

### Pitfall 1: stdin Consumed at Wrong Time

**What goes wrong:** `read_to_string` on stdin blocks until EOF. If called inside `run_remember()` after the match arm has already tried to parse stdin, or if called twice, you get empty content or a hang.

**Why it happens:** stdin is a one-shot stream — once read, it's consumed. If clap also tries to read stdin (it doesn't by default, but worth knowing), there's a conflict.

**How to avoid:** Resolve content (positional vs stdin) in the main.rs match arm, before calling any other function. Pass the resolved `String` to `run_remember()`.

**Warning signs:** Tests that pipe stdin hang, or content arrives empty despite visible input.

### Pitfall 2: spawn_blocking JoinError Propagation

**What goes wrong:** `spawn_blocking(|| LocalEngine::new()).await?` — the `?` applies to the `JoinError` from tokio, not the `EmbeddingError` from `LocalEngine::new()`. Two layers of error.

**Why it happens:** `spawn_blocking` returns `Result<Result<LocalEngine, EmbeddingError>, JoinError>`. Double-`?` or explicit mapping is needed.

**How to avoid:** Match the pattern from main.rs exactly:
```rust
let engine = tokio::task::spawn_blocking(|| {
    crate::embedding::LocalEngine::new()
})
.await?                           // unwrap JoinError (propagates via anyhow)
.map_err(|e| anyhow::anyhow!(e))?;  // convert EmbeddingError
```

**Warning signs:** Compiler error about mismatched error types at the `?` operator.

### Pitfall 3: CreateMemoryRequest Fields — Option<String> vs String

**What goes wrong:** `CreateMemoryRequest.agent_id` is `Option<String>` (with `#[serde(default)]`). If you pass `Some("")` vs `None` vs `""` incorrectly, storage behavior differs.

**Why it happens:** The `service.create_memory()` path unwraps `agent_id` with `unwrap_or_default()` (line 101 in service.rs) — so `None` and `Some("")` both produce `""`. But the CLI decision (D-10) is to default to empty string, matching API behavior.

**How to avoid:** Pass `args.agent_id` directly (it's already `Option<String>` from clap). If the user didn't supply `--agent-id`, it stays `None` → `unwrap_or_default()` → `""`. This is correct.

**Warning signs:** `agent_id` stored as `"None"` (string) instead of `""` — indicates accidental `.to_string()` on `Option`.

### Pitfall 4: Tags Field Empty vs None

**What goes wrong:** If `--tags` not provided, `args.tags` is `None`. The one-liner `args.tags.unwrap_or_default().split(','). ...collect()` produces an empty `Vec`. Passing `Some(vec![])` to `CreateMemoryRequest` instead of `None` results in `"[]"` stored in DB — which is correct behavior, but confirm it's intentional.

**Why it happens:** `CreateMemoryRequest.tags` is `Option<Vec<String>>`, and `service.create_memory()` does `req.tags.unwrap_or_default()` (line 103). So `None` and `Some(vec![])` both produce `[]` in the DB.

**How to avoid:** Either always pass `None` when tags list is empty, or always pass `Some(vec![])`. The service handles both identically. Per D-11 + D-10, passing `None` when tags is empty is cleanest.

### Pitfall 5: Content Resolution Order in main.rs — stdin blocking before match

**What goes wrong:** If stdin is read before the match dispatch (e.g., speculatively), it blocks for input even when the user ran `mnemonic recall` or `mnemonic keys list`.

**Why it happens:** `stdin().read_to_string()` blocks until EOF on a TTY unless `is_terminal()` is checked first.

**How to avoid:** The `IsTerminal` check and `read_to_string` must live inside the `Some(Commands::Remember(args))` arm only — never at the top of `main()`.

## Code Examples

Verified patterns from direct source inspection:

### Existing init_db Pattern (to extend)
```rust
// Source: src/cli.rs lines 87-103
pub async fn init_db(db_override: Option<String>)
    -> anyhow::Result<(std::sync::Arc<tokio_rusqlite::Connection>, crate::config::Config)>
{
    crate::db::register_sqlite_vec();
    let mut config = crate::config::load_config()
        .map_err(|e| anyhow::anyhow!(e))?;
    if let Some(ref db_path) = db_override {
        config.db_path = db_path.clone();
    }
    let conn = crate::db::open(&config).await
        .map_err(|e| anyhow::anyhow!(e))?;
    let conn_arc = std::sync::Arc::new(conn);
    Ok((conn_arc, config))
}
// NOTE: init_db_and_embedding extends this by adding validate_config() + embedding init
```

### spawn_blocking Pattern (from main.rs lines 85-89)
```rust
// Source: src/main.rs lines 85-89
let engine = tokio::task::spawn_blocking(|| {
    embedding::LocalEngine::new()
})
.await?
.map_err(|e| anyhow::anyhow!(e))?;
```

### CreateMemoryRequest Construction
```rust
// Source: src/service.rs lines 24-33
// CreateMemoryRequest is:
pub struct CreateMemoryRequest {
    pub content: String,
    pub agent_id: Option<String>,     // None → stored as ""
    pub session_id: Option<String>,   // None → stored as ""
    pub tags: Option<Vec<String>>,    // None → stored as "[]"
}
// Construction from CLI args:
let req = crate::service::CreateMemoryRequest {
    content,                    // resolved String
    agent_id: args.agent_id,   // already Option<String> from clap
    session_id: args.session_id,
    tags: if tags.is_empty() { None } else { Some(tags) },
};
```

### Integration Test Pattern (from tests/cli_integration.rs)
```rust
// Source: tests/cli_integration.rs — binary() + TempDb pattern
// For remember tests, no pre-seeding needed — binary creates the memory
// Verify: stdout line 1 is a UUID, stderr contains "Stored memory <8-char-prefix>"
#[test]
fn test_remember_stores_memory_and_prints_id() {
    let db = TempDb::new("remember_basic");
    let bin = binary();
    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "remember", "Hello world"])
        .output()
        .expect("failed to run mnemonic binary");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "remember must exit 0; stderr: {}", stderr);
    // UUID line: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
    let uuid_line = stdout.lines().next().unwrap_or("");
    assert_eq!(uuid_line.len(), 36, "stdout line 1 must be a 36-char UUID; got: {:?}", uuid_line);
    assert!(stderr.contains("Stored memory"), "stderr must say 'Stored memory'; got: {:?}", stderr);
}
```

**Note on integration test scope:** Integration tests for `remember` require the embedding model to load (2-3s per test). Keep test count minimal — cover the 4 requirements with targeted tests rather than exhaustive variations.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `atty` crate for TTY detection | `std::io::IsTerminal` (stdlib) | Rust 1.70 (2023) | Zero dependency, correct cross-platform behavior |
| Full server init for all commands | Tiered init (fast/medium/full) | Phase 15/16 decision | fast=DB-only (~50ms), medium=DB+embedding (~2-3s), full=server |
| Server inline init | Shared helper `init_db()` | Phase 16 | Pattern is now `init_db()` in cli.rs; Phase 17 adds `init_db_and_embedding()` sibling |

## Open Questions

1. **Where to resolve stdin: match arm vs run_remember parameter**
   - What we know: Content must be a resolved `String` before calling `run_remember()` (to keep the function signature clean and testable)
   - What's unclear: Whether the match arm gets messy with the `read_to_string` call inline, or if a small `resolve_content(args: &RememberArgs) -> String` helper is cleaner
   - Recommendation: Inline in the match arm is fine given it's ~8 lines. A helper is also acceptable. Planner decides based on code style.

2. **eprintln! vs tracing for progress output**
   - What we know: CLI commands do not call `server::init_tracing()`, so `tracing::info!` output goes nowhere without a subscriber
   - What's unclear: Whether a future phase might add tracing to CLI commands
   - Recommendation: Use `eprintln!` directly (D-09). It's correct now and easy to swap later.

3. **run_remember signature: pass content separately or leave in args**
   - What we know: D-18 says `run_remember(args: RememberArgs, service: MemoryService)` but content may be resolved from stdin before args is available
   - What's unclear: If `args.content` is consumed during resolution, args needs to be mutated or the content passed separately
   - Recommendation: Pass `content: String` as a separate first argument to `run_remember()`, keeping `args` for the metadata flags only. This is the cleanest separation.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in (`cargo test`) |
| Config file | `Cargo.toml` `[dev-dependencies]` |
| Quick run command | `cargo test --test cli_integration 2>&1 \| grep -E "(test .* ok|FAILED|error)"` |
| Full suite command | `cargo test 2>&1` |

**Important:** Remember integration tests trigger the embedding model load (2-3s each). They must run against the compiled binary. There is no way to mock the embedding model in binary-invocation tests — this is by design (tests verify real end-to-end behavior).

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| REM-01 | `mnemonic remember "content"` stores memory, exits 0, prints UUID to stdout | integration | `cargo test --test cli_integration test_remember` | ❌ Wave 0 |
| REM-02 | `echo "content" \| mnemonic remember` stores memory identically | integration | `cargo test --test cli_integration test_remember_stdin` | ❌ Wave 0 |
| REM-03 | `--agent-id` and `--session-id` flags stored correctly (verified via `recall --id`) | integration | `cargo test --test cli_integration test_remember_metadata` | ❌ Wave 0 |
| REM-04 | `--tags tag1,tag2` stored correctly (verified via `recall --id`) | integration | `cargo test --test cli_integration test_remember_tags` | ❌ Wave 0 |

**Additional coverage needed:**
- Empty content error path (`remember ""` exits 1 with correct stderr message)
- No-args + no-stdin error path (exits 1 with usage error — harder to test from binary since tty detection differs in test harness)
- Help text shows `remember` subcommand

### Sampling Rate
- **Per task commit:** `cargo test --test cli_integration 2>&1 | tail -5`
- **Per wave merge:** `cargo test 2>&1`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] Integration test functions for REM-01 through REM-04 in `tests/cli_integration.rs` — all 4 requirements need new test functions added to the existing file
- [ ] No new test files needed — extend `cli_integration.rs` following established pattern

*(Existing test infrastructure covers the framework; only new test functions are needed for Phase 17 requirements.)*

**Note on stdin test feasibility:** `REM-02` (stdin pipe) can be tested in binary invocation tests by using `Command::stdin(Stdio::piped())` and writing to the child process's stdin. This is standard `std::process::Command` usage and works correctly in `cargo test`.

## Sources

### Primary (HIGH confidence)
- Direct source inspection: `src/cli.rs` — Commands enum, RecallArgs, run_recall, init_db helper (lines 1-533)
- Direct source inspection: `src/main.rs` — match dispatch, embedding init (lines 1-185)
- Direct source inspection: `src/service.rs` — MemoryService, CreateMemoryRequest, create_memory (lines 1-349)
- Direct source inspection: `src/embedding.rs` — LocalEngine::new, spawn_blocking pattern (lines 1-317)
- Direct source inspection: `src/config.rs` — validate_config, Config struct (lines 1-233)
- Direct source inspection: `Cargo.toml` — confirmed all required crates present, no new dependencies needed
- Direct source inspection: `tests/cli_integration.rs` — TempDb, binary(), seed_memory patterns (lines 1-718)
- Rust toolchain verification: `rustc 1.94.0` — confirms `std::io::IsTerminal` (stable Rust 1.70+) available

### Secondary (MEDIUM confidence)
- `std::io::IsTerminal` trait stable since Rust 1.70.0 — verified against Rust 1.70 release notes knowledge; rustc 1.94 in use confirms availability

### Tertiary (LOW confidence)
- None — all claims verified from source code directly

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all crates confirmed present in Cargo.toml, no new dependencies
- Architecture: HIGH — all patterns verified by reading actual source files
- Pitfalls: HIGH — derived from reading implementation in service.rs, main.rs, and embedding.rs
- Test patterns: HIGH — derived from reading existing cli_integration.rs

**Research date:** 2026-03-21
**Valid until:** 2026-06-21 (stable — Rust edition 2021 project, no fast-moving dependencies)
