# Phase 20: output-polish - Research

**Researched:** 2026-03-21
**Domain:** Rust CLI output formatting — clap global flags, serde_json serialization, stdout/stderr split, exit codes
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**--json flag design**
- D-01: `--json` is a global flag on `Cli` struct (same level as `--db`), not per-subcommand.
- D-02: `--json` applies to data-producing subcommands: `recall`, `recall --id`, `remember`, `search`, `compact`, `keys list`, `keys create`. For `serve` and `keys revoke`, `--json` is accepted but has no effect.
- D-03: JSON output is a single JSON object per invocation printed to stdout, never streaming NDJSON. One `serde_json::to_string_pretty()` call at the end.
- D-04: JSON output replaces ALL stdout — no table headers, no footers, no progress text. Stderr messages remain unchanged regardless of `--json`.
- D-05: When `--json` and an error occurs, errors still go to stderr as plain text and exit 1.

**JSON output shapes per subcommand**
- D-06: `recall` (list mode): `{"memories": [...], "total": N}` — serialize `ListResponse` directly.
- D-07: `recall --id`: serialize the single `Memory` object directly — `{"id": "...", "content": "...", ...}`.
- D-08: `remember`: `{"id": "<full-uuid>"}`.
- D-09: `search`: serialize `SearchResponse` directly — `{"memories": [{"id": "...", "content": "...", "distance": 0.1234, ...}]}`.
- D-10: `compact`: serialize `CompactResponse` directly — includes `run_id`, `clusters_found`, `memories_merged`, `memories_created`, `id_mapping`, `truncated`. Full cluster detail deferred from Phase 19.
- D-11: `keys list`: serialize the `Vec<ApiKey>` directly (ApiKey must derive Serialize — currently missing).
- D-12: `keys create`: `{"token": "<raw>", "id": "<display_id>", "name": "...", "scope": "..."}`.

**Exit code audit**
- D-13: Exit 0 on success — already correct for all subcommands.
- D-14: Exit 1 on all error paths — already consistent.
- D-15: "Not found" cases correctly use exit 1 + stderr already.

**stdout/stderr split audit**
- D-16: After audit — no stdout/stderr split fixes needed. Existing pattern is already correct.
- D-17: Data to stdout, progress/warnings/errors to stderr — all handlers already follow this.

**Implementation approach**
- D-18: Add `#[arg(long, global = true)]` `pub json: bool` field to `Cli` struct.
- D-19: Each handler gains an `if json { ... } else { ... }` branch wrapping the output section.
- D-20: Handler signatures change: pass `json: bool` as an additional parameter. No output trait/strategy pattern.
- D-21: For `recall` list mode, reuse same query, serialize result vec + total as `ListResponse` JSON.

### Claude's Discretion
- JSON pretty-print vs compact (recommend pretty for CLI, but up to implementation)
- Whether `keys revoke` outputs `{"revoked": true}` in JSON mode or stays silent
- ApiKey struct Serialize derivation approach if not already present
- Test strategy for JSON output verification

### Deferred Ideas (OUT OF SCOPE)
- Color-coded output with owo-colors — CLR-01 in future requirements (v1.4+)
- `--format csv/table/json` multi-format — explicitly out of scope per REQUIREMENTS.md
- `--quiet` / `-q` flag for silent operation — not in v1.3 requirements
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| OUT-01 | All subcommands default to human-readable formatted text output | Verified: all existing handlers use `println!` table format. No changes to existing human output paths. |
| OUT-02 | All subcommands support `--json` flag for machine-readable JSON output | Covered by D-01 through D-21. All response types already derive `serde::Serialize`. `serde_json` is an existing dependency. |
| OUT-03 | All subcommands use exit code 0 on success, 1 on error | Verified: all existing handlers call `std::process::exit(1)` on error. `main() -> Result<()>` returns 0 on `Ok`. No changes needed. |
| OUT-04 | All subcommands send data to stdout and errors/warnings to stderr | Verified: audit complete (D-16, D-17). No violations found. Pattern is already correct across all handlers. |
</phase_requirements>

## Summary

Phase 20 is a cross-cutting polish phase adding `--json` output to every data-producing subcommand in `cli.rs` plus wiring the global flag through `main.rs`. The foundation is already excellent: all response types (`Memory`, `ListResponse`, `SearchResponse`, `CompactResponse`, `ClusterMapping`) already derive `serde::Serialize`, and `serde_json` is an existing `[dependencies]` entry. The pattern to follow is identical to how `--db` works as a global flag in clap.

The one missing piece is `ApiKey` in `src/auth.rs`: it currently derives only `Debug` and `Clone` — it needs `serde::Serialize` added before `keys list --json` can work. This is a one-line change.

OUT-03 (exit codes) and OUT-04 (stdout/stderr split) require zero implementation work — the audit confirms both are already correct across all handlers. The entire implementation effort is OUT-02: adding the global `--json` flag and `if json { ... } else { ... }` branches in six handler functions.

**Primary recommendation:** Add `pub json: bool` to `Cli` struct with `#[arg(long, global = true)]`, add `#[derive(serde::Serialize)]` to `ApiKey`, then add JSON output branches to all six `run_*` / `cmd_*` handlers.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde_json | 1 (already in Cargo.toml) | Serialize response types to JSON string | Already a project dependency; used by server.rs API handlers |
| serde | 1 (already in Cargo.toml) | `#[derive(Serialize)]` on ApiKey | Already in project with `features = ["derive"]` |
| clap | 4 (already in Cargo.toml) | `#[arg(long, global = true)]` for `--json` flag | Same mechanism as existing `--db` global flag |

### No New Dependencies
Zero new entries in `Cargo.toml` are required for this phase. This matches the established v1.3 pattern (from STATE.md: "Zero new Cargo.toml dependencies — all v1.3 needs covered by locked stack").

**Installation:** None needed.

## Architecture Patterns

### Recommended Project Structure
No structural changes. All edits are in existing files:
```
src/
├── cli.rs         # Cli struct (add json field), all run_*/cmd_* handlers (add json param + branch)
├── main.rs        # extract json bool alongside db_override, pass to handlers
└── auth.rs        # ApiKey struct (add Serialize derive)
```

### Pattern 1: Global Flag — Same as `--db`

**What:** clap `global = true` on a field in the top-level `Cli` struct makes the flag available in all subcommand positions.

**When to use:** Cross-cutting flags that apply to every subcommand without duplication.

**Example:**
```rust
// src/cli.rs — Cli struct (add alongside existing --db field)
#[derive(Parser)]
#[command(name = "mnemonic", version, about = "Agent memory server")]
pub struct Cli {
    /// Override database path (default: from config)
    #[arg(long, global = true, value_name = "PATH")]
    pub db: Option<String>,

    /// Output as JSON (machine-readable)
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}
```

### Pattern 2: Extract Before Match in main.rs

**What:** Extract both `db_override` and `json` before the `match cli_args.command` to avoid partial-move compiler errors.

**When to use:** Any time a global field needs to be passed to multiple match arms.

**Example:**
```rust
// src/main.rs — extract json alongside db_override
let cli_args = cli::Cli::parse();
let db_override = cli_args.db;
let json = cli_args.json;  // add this line

match cli_args.command {
    Some(cli::Commands::Recall(recall_args)) => {
        let (conn_arc, _config) = cli::init_db(db_override).await?;
        cli::run_recall(recall_args, conn_arc, json).await;  // pass json
        return Ok(());
    }
    // ... same pattern for all arms
}
```

### Pattern 3: Handler JSON Branch

**What:** Each handler receives `json: bool` as a final parameter and wraps only the output section.

**When to use:** This phase — all six data-producing handlers.

**Example (remember):**
```rust
pub async fn run_remember(content: String, args: RememberArgs, service: crate::service::MemoryService, json: bool) {
    // ... existing logic unchanged ...
    match service.create_memory(req).await {
        Ok(memory) => {
            if json {
                let obj = serde_json::json!({"id": memory.id});
                println!("{}", serde_json::to_string_pretty(&obj).unwrap());
            } else {
                println!("{}", memory.id);
                let short_id = &memory.id[..8.min(memory.id.len())];
                eprintln!("Stored memory {}", short_id);
            }
        }
        Err(e) => {
            eprintln!("error: failed to store memory: {}", e);
            std::process::exit(1);
        }
    }
}
```

### Pattern 4: ApiKey Serialize

**What:** `ApiKey` in `auth.rs` currently derives only `Debug, Clone`. Add `serde::Serialize`.

**When to use:** Required for `keys list --json` (D-11).

**Example:**
```rust
// src/auth.rs
#[derive(Debug, Clone, serde::Serialize)]
pub struct ApiKey {
    pub id: String,
    pub name: String,
    pub display_id: String,
    pub agent_id: Option<String>,
    pub created_at: String,
    pub revoked_at: Option<String>,
}
```

Note: `id` field serializes the full UUID (not display_id). This is correct — machine consumers need the full UUID for revoke operations. The existing human table output uses `display_id` (truncated), but JSON consumers get both fields.

### Pattern 5: `serde_json::json!` macro for ad-hoc shapes

**What:** For shapes that don't map to an existing struct (D-08 `remember`, D-12 `keys create`), use `serde_json::json!({...})` macro.

**Example:**
```rust
// remember --json
let output = serde_json::json!({"id": memory.id});
println!("{}", serde_json::to_string_pretty(&output).unwrap());

// keys create --json (includes one-time raw token)
let output = serde_json::json!({
    "token": raw_token,
    "id": api_key.display_id,
    "name": api_key.name,
    "scope": api_key.agent_id,
});
println!("{}", serde_json::to_string_pretty(&output).unwrap());
```

### Pattern 6: Direct struct serialization for existing response types

**What:** `ListResponse`, `SearchResponse`, `CompactResponse`, `Memory` already derive `Serialize`. Call `serde_json::to_string_pretty()` directly.

**Example:**
```rust
// recall --json (list mode) — build ListResponse and serialize
let list_resp = crate::service::ListResponse { memories, total };
println!("{}", serde_json::to_string_pretty(&list_resp).unwrap());

// compact --json
println!("{}", serde_json::to_string_pretty(&resp).unwrap());

// search --json
println!("{}", serde_json::to_string_pretty(&resp).unwrap());
```

### Anti-Patterns to Avoid

- **Separate `--json` per subcommand Args struct:** Creates 6x duplication. Use `global = true` on `Cli`.
- **Output trait / strategy pattern:** Overengineering per D-20. Six `if json { } else { }` branches is the right scope for this phase.
- **Streaming NDJSON:** Per D-03, one JSON object per invocation. Never multiple JSON lines.
- **serde_json errors as panic:** `to_string_pretty()` returns `Result`. For these types (all valid Rust structs with string/number fields), it cannot fail in practice, but prefer `.unwrap()` over `.expect()` to keep noise low.
- **JSON on stderr:** D-05 is explicit: `--json` applies to success-path stdout only. Error paths always emit plain text to stderr.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON serialization | Manual format! string building for JSON | `serde_json::to_string_pretty()` on existing Serialize types | Manual JSON building mishandles escaping, Unicode, nested structures |
| Global CLI flag | Per-subcommand `--json` on every Args struct | `#[arg(long, global = true)]` on `Cli` | clap handles routing to every arm automatically |
| Ad-hoc JSON shape | Custom struct just for one output | `serde_json::json!({...})` macro | Macro handles type coercion and no struct definition needed |

**Key insight:** All serializable response types already exist in the codebase. This phase is purely about wiring the flag through to the output section of each handler — not new data structures.

## Common Pitfalls

### Pitfall 1: Partial Move of `cli_args` Before `json` is Extracted
**What goes wrong:** If `json` is extracted inside the match arm after `cli_args.command` has moved, the compiler rejects it.
**Why it happens:** `cli_args.command` is moved into the match. Fields accessed after a move are rejected.
**How to avoid:** Extract `json = cli_args.json` immediately after extracting `db_override`, before the `match` statement (same pattern as `db_override` extraction already in main.rs lines 21-22).
**Warning signs:** Compiler error "use of partially moved value: `cli_args`".

### Pitfall 2: `keys list --json` With Unserializable ApiKey
**What goes wrong:** `ApiKey` does not currently derive `serde::Serialize` (confirmed by reading auth.rs). Attempting to call `serde_json::to_string_pretty(&keys)` where `keys: Vec<ApiKey>` will fail to compile.
**Why it happens:** The struct was designed for server-internal use only; serialization was not needed until now.
**How to avoid:** Add `serde::Serialize` to the `#[derive(...)]` on `ApiKey` as the first task in the plan.
**Warning signs:** Compiler error "the trait `Serialize` is not implemented for `ApiKey`".

### Pitfall 3: `keys create --json` Exposing Raw Token Path
**What goes wrong:** In human mode, `cmd_create` calls `println!` for raw token and metadata. In JSON mode, all three printlns must be replaced by a single JSON object — otherwise the token appears twice (once as the first line, once in the JSON).
**Why it happens:** `cmd_create` currently prints raw token on line 1, then metadata on lines 2-4.
**How to avoid:** The `if json { }` branch must print the single JSON object and return early (not fall through to the else branches).
**Warning signs:** Raw token appearing in JSON output as a plain line before the JSON object.

### Pitfall 4: Recall List Mode — `cmd_list_memories` vs `run_recall`
**What goes wrong:** `run_recall` calls the private `cmd_list_memories` function for list mode. The `json: bool` parameter needs to reach `cmd_list_memories` — which is a private function with a fixed signature.
**Why it happens:** The recall handler splits into two private inner functions. The json flag needs to be threaded through both.
**How to avoid:** Pass `json: bool` to both `cmd_list_memories` and `cmd_get_memory` from `run_recall`. Both are private functions in the same file — signature change is safe.
**Warning signs:** json flag silently ignored for recall list mode because it was not passed to `cmd_list_memories`.

### Pitfall 5: `compact --json` and the "No clusters found" Early Return
**What goes wrong:** `run_compact` has an early return path: `if resp.clusters_found == 0 { println!("No similar memories found to compact."); return; }`. In JSON mode, this must serialize `CompactResponse` (not print the plain text string), because `clusters_found == 0` is a valid success response with data.
**Why it happens:** The early-return optimization for the "no work done" case was written for human output only.
**How to avoid:** The `if resp.clusters_found == 0` early return must be inside the `else { }` (human) branch. The JSON branch always serializes `resp` regardless of `clusters_found`.
**Warning signs:** `compact --json` with no clusters returns a plain-text message instead of JSON, breaking pipe consumers.

## Code Examples

Verified patterns from codebase inspection:

### Existing `--db` global flag pattern (cli.rs lines 11-18)
```rust
// Source: src/cli.rs lines 11-18 (verified)
#[derive(Parser)]
#[command(name = "mnemonic", version, about = "Agent memory server")]
pub struct Cli {
    #[arg(long, global = true, value_name = "PATH")]
    pub db: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}
```
Replicate with `pub json: bool` — same position, same `global = true`.

### Existing extraction pattern in main.rs (lines 21-22)
```rust
// Source: src/main.rs lines 21-22 (verified)
let cli_args = cli::Cli::parse();
let db_override = cli_args.db;
// Add: let json = cli_args.json;
```

### Serializable response types (service.rs lines 58-86, compaction.rs lines 20-35)
```rust
// Source: src/service.rs lines 58-86 (verified)
#[derive(Debug, Clone, serde::Serialize)]
pub struct Memory { ... }  // already Serialize

#[derive(Debug, serde::Serialize)]
pub struct ListResponse { pub memories: Vec<Memory>, pub total: u64 }  // already Serialize

#[derive(Debug, serde::Serialize)]
pub struct SearchResponse { pub memories: Vec<SearchResultItem> }  // already Serialize

// Source: src/compaction.rs lines 20-35 (verified)
#[derive(Debug, serde::Serialize)]
pub struct CompactResponse { ... }  // already Serialize

#[derive(Debug, serde::Serialize)]
pub struct ClusterMapping { ... }  // already Serialize
```

### Missing Serialize on ApiKey (auth.rs lines 20-28)
```rust
// Source: src/auth.rs lines 20-28 (verified — currently only Debug, Clone)
#[derive(Debug, Clone)]  // <-- needs serde::Serialize added
pub struct ApiKey {
    pub id: String,
    pub name: String,
    pub display_id: String,
    pub agent_id: Option<String>,
    pub created_at: String,
    pub revoked_at: Option<String>,
}
```

### Integration test binary invocation pattern (tests/cli_integration.rs)
```rust
// Source: tests/cli_integration.rs (verified pattern for new --json tests)
let output = Command::new(&bin)
    .args(["--db", db.path_str(), "--json", "recall"])
    .output()
    .expect("failed to run mnemonic binary");

let stdout = String::from_utf8_lossy(&output.stdout);
assert!(output.status.success());

// Parse and assert JSON structure
let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
assert!(parsed["memories"].is_array());
assert!(parsed["total"].is_number());
```

## Validation Architecture

nyquist_validation is enabled in .planning/config.json.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test + integration tests via `std::process::Command` |
| Config file | Cargo.toml (no separate test config) |
| Quick run command | `cargo test --test cli_integration 2>/dev/null` |
| Full suite command | `cargo test 2>/dev/null` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| OUT-01 | Human output still works after --json flag added | regression | `cargo test --test cli_integration 2>/dev/null` | Yes (existing tests) |
| OUT-02 | `--json` flag produces valid JSON on stdout for recall | integration | `cargo test --test cli_integration test_recall_json 2>/dev/null` | No (Wave 0) |
| OUT-02 | `--json` flag produces valid JSON on stdout for remember | integration | `cargo test --test cli_integration test_remember_json 2>/dev/null` | No (Wave 0) |
| OUT-02 | `--json` flag produces valid JSON on stdout for search | integration | `cargo test --test cli_integration test_search_json 2>/dev/null` | No (Wave 0) |
| OUT-02 | `--json` flag produces valid JSON on stdout for compact | integration | `cargo test --test cli_integration test_compact_json 2>/dev/null` | No (Wave 0) |
| OUT-02 | `--json` flag produces valid JSON on stdout for keys list | integration | `cargo test --test cli_integration test_keys_list_json 2>/dev/null` | No (Wave 0) |
| OUT-02 | `--json` flag produces valid JSON on stdout for keys create | integration | `cargo test --test cli_integration test_keys_create_json 2>/dev/null` | No (Wave 0) |
| OUT-03 | All subcommands exit 0 on success, 1 on error | regression | `cargo test --test cli_integration 2>/dev/null` | Yes (existing tests cover this) |
| OUT-04 | Errors go to stderr, data to stdout | regression | `cargo test --test cli_integration 2>/dev/null` | Yes (existing tests cover this) |

### Sampling Rate
- **Per task commit:** `cargo build 2>/dev/null` (compilation check)
- **Per wave merge:** `cargo test --test cli_integration 2>/dev/null`
- **Phase gate:** `cargo test 2>/dev/null` (full suite green before `/gsd:verify-work`)

### Wave 0 Gaps
- [ ] `tests/cli_integration.rs` — new test functions for OUT-02 (6 JSON tests): `test_recall_json`, `test_remember_json`, `test_search_json`, `test_compact_json`, `test_keys_list_json`, `test_keys_create_json`
- [ ] Note: `serde_json` is already in `[dev-dependencies]` is NOT needed — it is already in `[dependencies]`. The binary parses JSON natively. Test parsing uses `serde_json::from_str` which is available from `[dependencies]`.

*Existing test infrastructure: `tests/cli_integration.rs` already has `binary()` helper and `TempDb` pattern. All new JSON tests follow the same pattern — no new framework setup needed.*

## State of the Art

| Old Approach | Current Approach | Notes |
|--------------|------------------|-------|
| Per-subcommand output flag | clap `global = true` on top-level struct | Clap 4 supports this cleanly; same as `--db` already in use |
| Manual JSON string formatting | `serde_json::to_string_pretty()` on Serialize types | All response types already derive Serialize |

**No deprecated features involved.** This is pure greenfield addition of a flag and output branches.

## Open Questions

1. **`keys revoke --json`: silent or `{"revoked": true}`?**
   - What we know: D-02 says `keys revoke` produces "a single-line confirmation" that is "too trivial" for JSON, `--json` accepted but no effect.
   - What's unclear: Should it output `{"revoked": true}` anyway for consistency, or truly stay silent?
   - Recommendation: Output `{"revoked": true}` — it is a trivial addition and makes the behavior consistent. Script consumers invoking `mnemonic keys revoke ... --json` likely expect valid JSON, not silence. This falls under "Claude's Discretion" per CONTEXT.md.

2. **Pretty vs compact JSON?**
   - What we know: D-03 says `serde_json::to_string_pretty()`. Claude's Discretion allows reconsidering.
   - What's unclear: Scripts piping to jq don't need pretty-print; human inspection benefits from it.
   - Recommendation: Use `serde_json::to_string_pretty()` — the output is for CLI usage. A human will sometimes eyeball it without jq. jq ignores whitespace. Zero downside to pretty.

## Sources

### Primary (HIGH confidence)
- `src/cli.rs` (read directly) — all handler signatures, existing `--db` global flag pattern, stdout/stderr split audit
- `src/main.rs` (read directly) — db_override extraction pattern before match, all dispatch arms
- `src/service.rs` (read directly) — Memory, ListResponse, SearchResponse all confirmed derive Serialize
- `src/compaction.rs` (read directly) — CompactResponse, ClusterMapping confirmed derive Serialize
- `src/auth.rs` (read directly) — ApiKey confirmed does NOT derive Serialize (only Debug, Clone)
- `Cargo.toml` (read directly) — serde_json = "1", serde with derive feature confirmed in [dependencies]
- `tests/cli_integration.rs` (grep verified) — binary() helper and TempDb pattern confirmed available
- `.planning/config.json` (read directly) — nyquist_validation = true confirmed

### Secondary (MEDIUM confidence)
- `.planning/phases/20-output-polish/20-CONTEXT.md` — all decisions D-01 through D-21 verified against source code
- `STATE.md` — "Zero new Cargo.toml dependencies" pattern for v1.3 confirmed applicable

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — serde_json and serde confirmed in Cargo.toml; clap global flag confirmed working via existing --db
- Architecture: HIGH — all patterns verified by reading actual source files
- Pitfalls: HIGH — Pitfalls 1, 2, 4, 5 identified from direct source code inspection; Pitfall 3 from logic analysis of cmd_create's multi-println structure

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable dependencies, no fast-moving ecosystem)
