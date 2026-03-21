# Phase 14: CLI Key Management - Research

**Researched:** 2026-03-20
**Domain:** Rust CLI (clap derive API), dual-mode binary pattern, terminal output formatting
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**CLI argument parsing**
- D-01: Use `clap` crate with derive macros — standard Rust CLI library
- D-02: `mnemonic` with no args starts the server; `mnemonic keys <subcommand>` branches to CLI path
- D-03: Subcommand is optional (`None` = serve mode); no `mnemonic serve` subcommand

**Binary restructuring (dual-mode)**
- D-04: Parse CLI args at the very top of `main()` before any initialization; CLI path: register sqlite-vec → load config → open DB → construct KeyService → run command → exit
- D-05: CLI path skips embedding model loading, LLM engine init, MemoryService, CompactionService, server bind/listen
- D-06: CLI path reuses existing `config::load_config()` for DB path resolution
- D-07: Add `--db <path>` global flag as an override for the DB path

**`keys create` command**
- D-08: `mnemonic keys create <name> [--agent-id <agent_id>]` — name is required positional, agent_id is optional flag
- D-09: Raw `mnk_...` token goes to stdout; "Save this key" warning goes to stderr
- D-10: Print key metadata (ID, name, scope) alongside raw token

**`keys list` command**
- D-11: `mnemonic keys list` — no arguments
- D-12: Hand-formatted table; columns: ID (8-char display_id), NAME, SCOPE, CREATED, STATUS
- D-13: STATUS column: "active" or "revoked (date)"
- D-14: Empty state: "No API keys found. Create one with: mnemonic keys create <name>"

**`keys revoke` command**
- D-15: `mnemonic keys revoke <id>` — accepts full UUID or 8-char display_id
- D-16: If 8-char input matches multiple keys: error with list of matches; use "Ambiguous" message
- D-17: Success: print "Key <display_id> revoked"
- D-18: Not found: print error and exit code 1

**Output style**
- D-19: Normal output → stdout; warnings/errors → stderr
- D-20: Exit codes: 0 = success, 1 = error
- D-21: No tracing/logging on CLI path — clean output only

**Crate dependencies**
- D-22: Add `clap` with `derive` feature to Cargo.toml

### Claude's Discretion
- Exact table column widths and alignment
- Whether to truncate long key names in the table
- Internal code organization (CLI handler functions in main.rs vs separate module)
- Whether `keys revoke` needs a `--force` flag or confirmation prompt (recommended: no)

### Deferred Ideas (OUT OF SCOPE)
- `--json` output flag for programmatic use
- `mnemonic serve` explicit subcommand
- Interactive confirmation for revoke
- `keys info <id>` command to show single key details
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CLI-01 | `mnemonic keys create` creates an API key and displays the raw key | `KeyService::create()` already exists; clap derive maps `create <name> [--agent-id]`; output formatting documented below |
| CLI-02 | `mnemonic keys list` displays all keys with metadata | `KeyService::list()` already exists; hand-formatted table pattern documented below |
| CLI-03 | `mnemonic keys revoke` invalidates a key by ID or prefix | `KeyService::revoke()` exists; display_id lookup pattern documented; exit code 1 on not-found |
</phase_requirements>

---

## Summary

Phase 14 wraps already-complete `KeyService` methods in a CLI surface using `clap` 4.6.0 with derive macros. The core challenge is the dual-mode binary: `main()` must parse args before any heavy initialization (embedding model, LLM engine, server stack) and take a CLI fast-path that exits in under 1 second.

The pattern is well-established in the Rust ecosystem. `clap` 4.x derive makes the dual-mode shape concise: a top-level `#[derive(Parser)]` struct with `command: Option<Commands>` — `None` falls through to serve mode, `Some(Commands::Keys(...))` takes the CLI path. No async runtime is needed for the CLI path; the only async calls are `db::open()` and `KeyService` methods, which all use `tokio::main` already present on `main()`.

The `revoke` subcommand requires one piece of new logic not already in `KeyService`: looking up a key by `display_id` (8-char prefix) in addition to full UUID. This requires a DB query by `display_id` column, then branching on result count (0 = not found, 1 = revoke it, >1 = ambiguous error). The `api_keys` table has `display_id` indexed via `idx_api_keys_agent_id`; a targeted SELECT on `display_id` is fast.

**Primary recommendation:** Create a `src/cli.rs` module for CLI structs and handler logic. Keep `main.rs` as the dispatch point only. Use `std::process::exit(1)` for error paths to avoid Rust's `Err` propagation printing noise on CLI output.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4.6.0 | CLI arg parsing, subcommands, help generation | De-facto Rust CLI standard; derive feature eliminates boilerplate |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio_rusqlite | 0.7 (already in use) | Async DB access for CLI path | Same pattern as server; CLI path stays async |
| anyhow | 1 (already in use) | Error propagation in CLI handlers | Consistent with rest of codebase |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| clap derive | clap builder API | Builder is more verbose; derive is idiomatic for this shape |
| clap derive | structopt | structopt is deprecated — clap 3+ absorbed it |
| hand-formatted table | prettytable-rs | No new dep needed; table is simple enough for `format!` alignment |

**Installation:**
```bash
# Add to Cargo.toml [dependencies]
clap = { version = "4", features = ["derive"] }
```

**Version verification:** Confirmed via `cargo search clap --limit 1` on 2026-03-20: clap 4.6.0 is current.

---

## Architecture Patterns

### Recommended Project Structure
```
src/
├── main.rs          # Arg parsing + dispatch only (serve vs CLI path)
├── cli.rs           # Clap structs (Cli, Commands, KeysSubcommand) + CLI handler fns
├── auth.rs          # KeyService — unchanged, no new methods needed for create/list
│                    # (revoke by display_id needs a new helper or inline query in cli.rs)
├── config.rs        # Unchanged — load_config() reused on CLI path
├── db.rs            # Unchanged — register_sqlite_vec() + open() reused on CLI path
└── ...              # All other modules unchanged
```

### Pattern 1: Dual-Mode Binary with Optional Subcommand

**What:** Parse args before any I/O. Branch at top of `main()` on `cli.command`.
**When to use:** Any binary that needs instant CLI response time without loading heavy resources.

```rust
// Source: docs.rs/clap/4.6.0/clap/_derive
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "mnemonic", version, about = "Agent memory server")]
struct Cli {
    /// Override database path (default: from config)
    #[arg(long, global = true, value_name = "PATH")]
    db: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage API keys
    Keys(KeysArgs),
}

#[derive(clap::Args)]
struct KeysArgs {
    #[command(subcommand)]
    subcommand: KeysSubcommand,
}

#[derive(Subcommand)]
enum KeysSubcommand {
    /// Create a new API key
    Create {
        /// Name for the key
        name: String,
        /// Scope key to a specific agent_id
        #[arg(long, value_name = "AGENT_ID")]
        agent_id: Option<String>,
    },
    /// List all API keys
    List,
    /// Revoke an API key by ID or 8-char display prefix
    Revoke {
        /// Full UUID or 8-char display_id
        id: String,
    },
}
```

**main.rs dispatch pattern:**
```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if let Some(Commands::Keys(keys_args)) = cli.command {
        // CLI path: register sqlite-vec, load config, open DB only
        db::register_sqlite_vec();
        let mut config = config::load_config().map_err(|e| anyhow::anyhow!(e))?;
        if let Some(db_override) = cli.db {
            config.db_path = db_override;
        }
        let conn = db::open(&config).await.map_err(|e| anyhow::anyhow!(e))?;
        let conn_arc = std::sync::Arc::new(conn);
        let key_service = auth::KeyService::new(conn_arc);
        cli::run_keys(keys_args.subcommand, key_service).await;
        return Ok(());
    }

    // Server path — existing initialization continues here
    db::register_sqlite_vec();
    server::init_tracing();
    // ... rest of existing main() ...
}
```

### Pattern 2: CLI Output Convention (stdout/stderr split)

**What:** Raw token and table rows to stdout; warnings and errors to stderr. Exit with `std::process::exit(1)` on error.
**When to use:** All CLI output in this phase.

```rust
// keys create — stdout gets the piped value, stderr gets the warning
println!("{}", raw_token);                            // stdout: pipeable
eprintln!("Save this key — it will not be shown again"); // stderr: warning
println!("ID:    {}", api_key.display_id);
println!("Name:  {}", api_key.name);
println!("Scope: {}", api_key.agent_id.as_deref().unwrap_or("(unscoped — all agents)"));
```

```rust
// keys revoke — not found exits 1
eprintln!("No key found with ID {}", id);
std::process::exit(1);
```

### Pattern 3: Display_id Lookup for Revoke

**What:** `KeyService::revoke()` takes a full UUID. For display_id (8-char) input, add a helper that queries by `display_id` and returns the matching UUID(s).

**Implementation approach** (new method or inline in cli.rs):
```rust
// In auth.rs or cli.rs — query by display_id
async fn find_by_display_id(conn: &Arc<Connection>, display_id: &str)
    -> Result<Vec<ApiKey>, DbError>
{
    let did = display_id.to_string();
    conn.call(move |c| -> Result<Vec<ApiKey>, rusqlite::Error> {
        let mut stmt = c.prepare(
            "SELECT id, name, display_id, agent_id, created_at, revoked_at
             FROM api_keys WHERE display_id = ?1"
        )?;
        let rows = stmt.query_map(rusqlite::params![did], |row| {
            Ok(ApiKey {
                id: row.get(0)?,
                name: row.get(1)?,
                display_id: row.get(2)?,
                agent_id: row.get(3)?,
                created_at: row.get(4)?,
                revoked_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    })
    .await
    .map_err(DbError::from)
}
```

**Disambiguation logic:**
- Input length == 8 and all hex chars → treat as display_id
- Otherwise → treat as full UUID (pass directly to `KeyService::revoke()`)
- On display_id path: 0 results → not found error + exit 1; 1 result → revoke by `.id`; >1 results → ambiguous error listing matches + exit 1

### Pattern 4: Hand-Formatted Table

**What:** Simple `format!` alignment with hard-coded column widths. No external dep.
**When to use:** `keys list` output.

```rust
// Header + separator
println!("{:<8}  {:<20}  {:<20}  {:<19}  {}",
    "ID", "NAME", "SCOPE", "CREATED", "STATUS");
println!("{}", "-".repeat(80));

for key in &keys {
    let scope = key.agent_id.as_deref().unwrap_or("(all)");
    let status = match &key.revoked_at {
        None => "active".to_string(),
        Some(ts) => format!("revoked ({})", &ts[..10]), // date only
    };
    // Truncate long names to fit table
    let name = if key.name.len() > 20 {
        format!("{}...", &key.name[..17])
    } else {
        key.name.clone()
    };
    let scope_display = if scope.len() > 20 {
        format!("{}...", &scope[..17])
    } else {
        scope.to_string()
    };
    println!("{:<8}  {:<20}  {:<20}  {:<19}  {}",
        key.display_id, name, scope_display, &key.created_at[..19], status);
}
```

### Anti-Patterns to Avoid

- **Calling `server::init_tracing()` on CLI path:** Adds noisy INFO lines to clean CLI output. CLI path must skip tracing init entirely (D-21).
- **Loading the embedding model on CLI path:** Even partial initialization of `LocalEngine` triggers HuggingFace Hub downloads. The entire embedding block must be in the `else` branch.
- **Using `?` operator for CLI error propagation:** Rust prints `Error: ...` automatically when `main()` returns `Err`. For CLI UX, print a clean message to stderr and call `std::process::exit(1)` instead.
- **`validate_config()` on CLI path:** This function errors if `openai_api_key` is missing when `embedding_provider = "openai"` — but the CLI path doesn't use embedding at all. Skip `validate_config()` on the CLI path (or restructure so it only validates what the current mode needs).
- **Using `uuid::Uuid::parse_str()` to detect "is this a UUID?":** May false-positive on 8-char hex strings that happen to parse as short UUIDs. Use explicit length check: `if input.len() == 8 && input.chars().all(|c| c.is_ascii_hexdigit())` to detect display_id.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Argument parsing and --help generation | Custom arg parsing | `clap` 4.6.0 with `derive` | Type-safe, auto-generates `--help`, handles `--version`, manages errors |
| Token formatting for display_id check | Custom UUID parsing | Simple `len() == 8 && all hex` check | UUIDs are 36 chars; 8-char all-hex is unambiguous display_id heuristic |
| Key creation, hashing, DB storage | Anything in auth.rs | `KeyService::create()` | Already implemented and tested in Phase 11 |
| Key listing | Any new query | `KeyService::list()` | Already implemented and tested |
| Key revocation by UUID | Any new query | `KeyService::revoke(id)` | Already implemented and tested; idempotent |

**Key insight:** The only NEW logic in this phase is (1) clap wiring, (2) display_id→UUID lookup for revoke, and (3) formatting. All business logic already exists.

---

## Common Pitfalls

### Pitfall 1: validate_config() Rejects CLI Mode Config

**What goes wrong:** `config::validate_config()` errors when `embedding_provider = "openai"` and no `MNEMONIC_OPENAI_API_KEY` is set — even though the CLI path never touches embeddings. If the CLI path calls `validate_config()`, it fails on servers configured for OpenAI embedding.

**Why it happens:** `validate_config()` was written for server mode where every configured component is used.

**How to avoid:** Do not call `validate_config()` on the CLI path. `load_config()` alone is sufficient — it populates `config.db_path` which is all the CLI needs.

**Warning signs:** `cargo run -- keys list` fails with "MNEMONIC_OPENAI_API_KEY is not set" on OpenAI-configured setups.

---

### Pitfall 2: Clap Parses Before DB Opens — Error Messages Must Be Clap's

**What goes wrong:** If `clap` parsing fails (e.g., missing required `<name>` arg for `create`), the program exits before any DB work. This is correct behavior — clap handles parse errors with its own formatting and exit code 2. Don't try to catch or reformat clap parse errors.

**Why it happens:** `Cli::parse()` calls `std::process::exit()` on error internally.

**How to avoid:** Accept clap's error formatting for parse errors. Only use `eprintln!` + `exit(1)` for runtime errors (DB failures, key not found).

---

### Pitfall 3: register_sqlite_vec() Must Be Called Before db::open()

**What goes wrong:** If `db::open()` is called before `register_sqlite_vec()`, the `vec_memories` virtual table creation fails with a SQLite error about unknown module "vec0".

**Why it happens:** sqlite-vec registers itself via SQLite's auto-extension mechanism. The `Once` guard in `db::rs` prevents double-registration but does not enforce ordering.

**How to avoid:** First line of CLI path must be `db::register_sqlite_vec()`, same as server path. The existing `Once` guard makes it safe to call in both branches.

---

### Pitfall 4: Tokio Runtime on CLI Path

**What goes wrong:** `db::open()` and `KeyService` methods are async. The CLI path is inside `#[tokio::main]`, so the runtime is available. But if someone refactors to a sync `main()`, these calls break.

**Why it happens:** N/A for this phase — `#[tokio::main]` is already present and stays.

**How to avoid:** Keep `#[tokio::main]` on `main()`. The tokio runtime is lightweight to spin up even when not serving HTTP. The embedding model (not the runtime) is the slow part.

---

### Pitfall 5: display_id Ambiguity

**What goes wrong:** Two keys could theoretically share the same `display_id` (first 8 chars of BLAKE3 hash). While astronomically unlikely, the code must handle it to avoid silently revoking the wrong key.

**Why it happens:** `display_id` is not enforced UNIQUE in the schema — it is informational only.

**How to avoid:** Always SELECT by `display_id` and check count. If >1 result: print "Ambiguous — X keys match prefix. Use full UUID:" followed by IDs and names. Exit code 1.

---

### Pitfall 6: std::process::exit() Skips Drop Cleanup

**What goes wrong:** Calling `std::process::exit(1)` skips Rust destructors. For this phase, the only resource held is the DB connection. tokio_rusqlite connections close when dropped — but `exit()` skips Drop.

**Why it happens:** `exit()` is an OS-level call.

**How to avoid:** This is acceptable for CLI tools. The OS cleans up file descriptors. SQLite WAL checkpoint is not needed for read-only operations (list) or simple writes (create/revoke) that are already committed before exit.

---

## Code Examples

Verified patterns from official sources:

### clap 4.x Two-Level Subcommand (keys → create/list/revoke)

```rust
// Source: docs.rs/clap/4.6.0/clap/_derive (verified 2026-03-20)
use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "mnemonic", version, about = "Agent memory server")]
pub struct Cli {
    #[arg(long, global = true, value_name = "PATH", help = "Override database path")]
    pub db: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Manage API keys
    Keys(KeysArgs),
}

#[derive(Args)]
pub struct KeysArgs {
    #[command(subcommand)]
    pub subcommand: KeysSubcommand,
}

#[derive(Subcommand)]
pub enum KeysSubcommand {
    /// Create a new API key (shows raw key once)
    Create {
        name: String,
        #[arg(long, value_name = "AGENT_ID")]
        agent_id: Option<String>,
    },
    /// List all API keys
    List,
    /// Revoke an API key by full UUID or 8-char display prefix
    Revoke { id: String },
}
```

### keys create Output Pattern

```rust
// stdout: the pipeable token
println!("{}", raw_token);
// stdout: metadata (same stream for grouping)
println!("ID:    {}", api_key.display_id);
println!("Name:  {}", api_key.name);
println!("Scope: {}", api_key.agent_id.as_deref().unwrap_or("(unscoped)"));
// stderr: the irreversible warning
eprintln!();
eprintln!("Save this key — it will not be shown again.");
```

### Display_id vs UUID Detection

```rust
fn is_display_id(input: &str) -> bool {
    input.len() == 8 && input.chars().all(|c| c.is_ascii_hexdigit())
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| structopt | clap derive (built-in) | clap 3.0 (2021) | structopt is deprecated; clap derive is the canonical path |
| clap 3.x `#[clap(...)]` | clap 4.x `#[arg(...)]`, `#[command(...)]` | clap 4.0 (2022) | Attribute names changed; use `#[arg]` not `#[clap]` for fields |

**Deprecated/outdated:**
- `structopt`: Merged into clap 3+; do not add as separate dependency
- `#[clap(subcommand)]` attribute: Replaced by `#[command(subcommand)]` in clap 4

---

## Open Questions

1. **Display_id lookup: new KeyService method vs inline in cli.rs?**
   - What we know: `KeyService` owns the `conn: Arc<Connection>`; the CLI module would need access to it
   - What's unclear: Whether to add `find_by_display_id()` to `KeyService` (keeps DB logic in auth.rs) or inline in cli.rs (keeps cli module self-contained)
   - Recommendation: Add `find_by_display_id(display_id: &str) -> Result<Vec<ApiKey>, DbError>` to `KeyService` — consistent with existing KeyService pattern, easy to test

2. **Module placement: src/cli.rs vs inline in main.rs?**
   - What we know: CONTEXT.md lists this as Claude's Discretion
   - Recommendation: Use `src/cli.rs` — keeps `main.rs` as dispatch only, makes the CLI logic separately testable, consistent with the module-per-concern pattern already established (auth.rs, service.rs, etc.)

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in (`cargo test`) |
| Config file | none — standard Cargo test runner |
| Quick run command | `cargo test --lib 2>&1` |
| Full suite command | `cargo test 2>&1` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CLI-01 | `keys create <name>` calls `KeyService::create()` and prints raw token to stdout | unit | `cargo test --lib cli:: -q 2>&1` | Wave 0 |
| CLI-01 | `keys create <name> --agent-id <id>` passes agent_id to create | unit | `cargo test --lib cli:: -q 2>&1` | Wave 0 |
| CLI-01 | Raw token line goes to stdout; warning goes to stderr | unit (capture) | `cargo test --lib cli:: -q 2>&1` | Wave 0 |
| CLI-02 | `keys list` calls `KeyService::list()` and formats table with header | unit | `cargo test --lib cli:: -q 2>&1` | Wave 0 |
| CLI-02 | Empty key set prints empty-state message | unit | `cargo test --lib cli:: -q 2>&1` | Wave 0 |
| CLI-03 | `keys revoke <uuid>` calls `KeyService::revoke()` with UUID | unit | `cargo test --lib cli:: -q 2>&1` | Wave 0 |
| CLI-03 | `keys revoke <display_id>` resolves display_id to UUID and revokes | unit | `cargo test --lib cli:: -q 2>&1` | Wave 0 |
| CLI-03 | Ambiguous display_id (multiple matches) prints error and exits 1 | unit | `cargo test --lib cli:: -q 2>&1` | Wave 0 |
| CLI-03 | Not-found display_id prints error | unit | `cargo test --lib cli:: -q 2>&1` | Wave 0 |
| SC4 | CLI path does not load embedding model (startup < 1s) | manual smoke | `time mnemonic keys list` | manual |

**Note on SC4 (startup time):** The < 1s guarantee is structural — if the embedding block is correctly absent from the CLI path, it holds by construction. No automated timing test is practical in cargo test. Verify manually with `time mnemonic keys list` after build.

### Sampling Rate
- **Per task commit:** `cargo test --lib 2>&1`
- **Per wave merge:** `cargo test 2>&1`
- **Phase gate:** `cargo test 2>&1` green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `src/cli.rs` — module with clap structs and handler logic (does not exist yet)
- [ ] Unit tests for `cli::` module functions (inside `src/cli.rs` #[cfg(test)] block or separate test)
- [ ] `KeyService::find_by_display_id()` method in `src/auth.rs` (does not exist yet)

---

## Sources

### Primary (HIGH confidence)
- `docs.rs/clap/4.6.0/clap/_derive` — derive macro API, Parser/Subcommand/Args traits, attribute reference (fetched 2026-03-20)
- `docs.rs/clap/latest` — current version confirmed 4.6.0, Cargo.toml feature line (fetched 2026-03-20)
- `cargo search clap --limit 1` — confirmed clap 4.6.0 is the current published version (run 2026-03-20)
- `src/auth.rs` — `KeyService` API: `create()`, `list()`, `revoke()` signatures, `ApiKey` struct fields (read directly)
- `src/main.rs` — Current initialization sequence; identifies exactly where CLI branch inserts (read directly)
- `src/config.rs` — `load_config()` and `validate_config()` behavior; pitfall 1 identified from code (read directly)
- `src/db.rs` — `register_sqlite_vec()` + `open()` ordering requirement (read directly)
- `.planning/phases/14-cli-key-management/14-CONTEXT.md` — All locked decisions (read directly)

### Secondary (MEDIUM confidence)
- Rust ecosystem pattern: dual-mode binary (clap optional subcommand) — described in CONTEXT.md D-04, consistent with observed clap docs

### Tertiary (LOW confidence)
- Table formatting column widths: 8/20/20/19 — based on typical display_id (8 chars), reasonable name/scope lengths; Claude's Discretion per CONTEXT.md

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — clap 4.6.0 confirmed via crates.io search and official docs
- Architecture: HIGH — all KeyService methods read directly from source; clap derive patterns verified from official docs
- Pitfalls: HIGH — pitfalls 1-4 derived directly from reading existing source code; pitfall 5 from schema inspection; pitfall 6 is documented Rust behavior
- Validation architecture: HIGH — test framework is cargo test (existing), gaps identified by listing what doesn't exist yet

**Research date:** 2026-03-20
**Valid until:** 2026-06-20 (clap 4.x is stable; no breaking changes expected in 90 days)
