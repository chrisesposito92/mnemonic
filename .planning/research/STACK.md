# Stack Research

**Domain:** Rust agent memory server (embedded vector search, local ML inference, REST API)
**Researched:** 2026-03-20
**Confidence:** HIGH (all new-addition versions verified against official sources)

---

## Existing Stack (LOCKED — do not re-research)

The following are validated from v1.0 and must not change:

| Technology | Locked Version | Role |
|------------|---------------|------|
| tokio | 1 | Async runtime |
| axum | 0.8 | HTTP server |
| rusqlite | 0.37 (bundled) | SQLite access |
| sqlite-vec | 0.1.7 | Vector KNN extension |
| tokio-rusqlite | 0.7 | Async SQLite wrapper |
| candle-core/nn/transformers | 0.9 | Local ML inference |
| tokenizers | 0.22 | HuggingFace tokenization |
| hf-hub | 0.5 | Model weight download/cache |
| serde + serde_json | 1 | Serialization |
| reqwest | 0.13 | HTTP client (used for OpenAI embedding fallback) |
| zerocopy | 0.8 | Vec<f32>-to-bytes for sqlite-vec |
| tracing + tracing-subscriber | 0.1 / 0.3 | Structured logging |
| thiserror + anyhow | 2 / 1 | Error handling |
| uuid | 1 (v7) | Memory ID generation |
| figment | 0.10 | Config (TOML + env) |
| async-trait | 0.1 | EmbeddingEngine trait |

**Note on reqwest version:** Cargo.toml pins `reqwest = "0.13"`. This is intentional. The existing STACK.md (researched 2026-03-19) incorrectly lists `reqwest 0.12` — the actual binary uses 0.13. This matters for the LLM integration decision (see below).

---

## New Additions for v1.1

The following three capability areas require new stack decisions.

### 1. Vector Similarity Clustering / Dedup

**Recommendation: No new crate. Implement cosine similarity inline.**

**Rationale:**

The all-MiniLM-L6-v2 embeddings stored in `vec_memories` are **not pre-normalized** (confirmed by inspecting the existing inference path in `embedding.rs`). For deduplication at compact time, cosine similarity between embedding pairs is sufficient — no full clustering algorithm is needed for the Tier 1 (algorithmic dedup) use case.

The compaction workflow is:
1. Fetch all embeddings for the scoped agent_id
2. Compute pairwise cosine similarity in-memory
3. Apply greedy threshold clustering (mark pairs above threshold as duplicates)
4. Delete duplicates, insert merged/summarized replacement

This is O(n²) over n memories per agent — acceptable because compaction runs on demand and typical agent scopes are 50–5000 memories, not millions.

**Cosine similarity is four lines of arithmetic.** Adding a crate for this is over-engineering.

```rust
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 { 0.0 } else { dot / (norm_a * norm_b) }
}
```

**Why not hdbscan (0.12.0)?**

Investigated. The `hdbscan` crate's `DistanceMetric` enum does not include cosine similarity — it supports Chebyshev, Cylindrical, Euclidean, Haversine, Manhattan, and Precalculated. The `Precalculated` variant is a workaround (pass in a precomputed distance matrix), but this forces building the full n×n matrix before clustering, then running the algorithm — more complexity and memory than the greedy approach for this use case. HDBSCAN is designed for exploratory clustering of ambiguous data; mnemonic's compaction is threshold-based deduplication with a user-provided similarity cutoff. The simpler tool is correct here.

**Why not linfa-clustering?**

linfa-clustering uses ndarray `Array2<f32>` as its data format, which means converting our `Vec<Vec<f32>>` embeddings into a dense ndarray matrix. linfa's KMeans doesn't support cosine distance (Euclidean only). k-means also requires specifying k upfront, which is inappropriate when the number of duplicate clusters is unknown. Adding ndarray as a dependency for a use case that doesn't need it violates the project's single-binary minimalism.

**Verdict:** Zero new crates for clustering/dedup.

---

### 2. LLM API Integration (Tier 2 Summarization)

**Recommendation: Use reqwest directly. Do not add async-openai.**

**Rationale:**

The project already has `reqwest = "0.13"` in Cargo.toml. `async-openai` 0.33.x depends on `reqwest = "0.12"`. Adding async-openai would pull in **two incompatible versions of reqwest** simultaneously — Cargo resolves this by compiling both, bloating the binary and compile times. This is directly contrary to the single-binary simplicity constraint.

The LLM integration for summarization is a single API call:

```
POST {llm_base_url}/v1/chat/completions
Content-Type: application/json
Authorization: Bearer {api_key}

{
  "model": "{model}",
  "messages": [{"role": "user", "content": "...summarize these memories..."}]
}
```

The response parsing needs one `serde_json` struct (already a dependency). The existing `reqwest` client plus `serde_json` handles this in ~40 lines of Rust. No new crate is justified.

**Following the existing embedding_provider pattern:** The project already implements `EmbeddingEngine` as a trait with local (candle) and remote (OpenAI API via reqwest) backends. The LLM provider should follow the same pattern: a `LlmProvider` trait with a `summarize(memories: &[Memory]) -> Result<String>` method, backed by an HTTP client using the existing reqwest instance.

**OpenAI-compatible endpoint support:** The project's config pattern (following `embedding_provider`) should support:
- `llm_provider = "openai"` (or `"ollama"`, `"anthropic"`, etc.)
- `llm_api_base` — URL override (defaults to `https://api.openai.com`)
- `llm_api_key` — env var or config value
- `llm_model` — model name string

This mirrors how `OPENAI_API_KEY` and `OPENAI_API_BASE` work in async-openai, without the dependency.

**Configuration additions (figment):** No new config crate needed. The existing `figment` setup handles additional keys transparently.

**Verdict:** Zero new crates for LLM integration. Use existing reqwest 0.13 + serde_json.

---

### 3. SQLite Schema Additions for Compaction State

**Recommendation: Two schema additions, applied via `execute_batch` in db.rs.**

No new crates are needed. The existing `rusqlite` + `tokio-rusqlite` handles DDL changes the same way the current schema is managed.

#### Addition 1: `compact_runs` table

Tracks each compaction invocation for auditability and idempotency.

```sql
CREATE TABLE IF NOT EXISTS compact_runs (
    id TEXT PRIMARY KEY,                    -- uuid v7
    agent_id TEXT NOT NULL DEFAULT '',
    started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at DATETIME,
    memories_before INTEGER NOT NULL DEFAULT 0,
    memories_after INTEGER NOT NULL DEFAULT 0,
    clusters_found INTEGER NOT NULL DEFAULT 0,
    llm_used INTEGER NOT NULL DEFAULT 0,    -- boolean: 0/1
    similarity_threshold REAL NOT NULL,
    status TEXT NOT NULL DEFAULT 'running'  -- 'running' | 'complete' | 'failed'
);

CREATE INDEX IF NOT EXISTS idx_compact_runs_agent_id ON compact_runs(agent_id);
```

**Why:** Agents need to know when compaction last ran, how many memories were reduced, and whether LLM summarization was applied. This also enables `GET /memories/compact/status` as a future endpoint without schema changes.

#### Addition 2: `source_ids` column on `memories`

Tracks provenance of merged/summarized memories back to their source memory IDs.

```sql
ALTER TABLE memories ADD COLUMN source_ids TEXT NOT NULL DEFAULT '[]';
```

**Why:** After compaction, the merged/summary memory replaces N originals. `source_ids` is a JSON array of the deleted memory IDs (same format as `tags`). This lets agents understand that a compact memory represents a consolidation, supports future "expand" operations, and provides audit trail. The format follows the existing `tags` column pattern (JSON array as TEXT) — no schema complexity added.

#### No changes needed to `vec_memories`

The `vec_memories` virtual table stores only `(memory_id, embedding float[384])`. The embedding for a merged memory is either:
- The centroid of the cluster embeddings (algorithmic Tier 1: average the vectors), or
- The embedding of the LLM-generated summary (Tier 2)

Either way, it's just a new `INSERT` into `vec_memories` with the merged memory's ID. No structural change required.

#### Migration strategy

The schema uses `CREATE TABLE IF NOT EXISTS` and `ALTER TABLE ... ADD COLUMN IF NOT EXISTS` (SQLite 3.37+, available since 2021 — safe assumption for the bundled SQLite in rusqlite 0.37's `bundled` feature). Apply additions in `db::open()` after the existing `execute_batch`.

---

## Recommended Cargo.toml Changes (v1.1)

```toml
# No new dependencies required for v1.1.
# All three capability areas (clustering, LLM API, schema) are served by
# the existing dependency set.

# Verify existing versions are consistent with these notes:
rusqlite = { version = "0.37", features = ["bundled"] }  # DO NOT upgrade to 0.38/0.39 — sqlite-vec 0.1.7 has known conflict with 0.39's libsqlite3-sys
reqwest = { version = "0.13", features = ["json"] }      # Required for LLM summarization HTTP calls — already present
```

---

## New Additions for v1.2 — API Key Authentication

The following four capability areas require new stack decisions for the v1.2 authentication milestone.

### 1. Cryptographic Key Generation

**Recommendation: `rand_core 0.9` with `os_rng` feature.**

**Rationale:**

API keys require 32 bytes of OS-provided cryptographic entropy formatted as a hex string, yielding a 64-character token prefixed with `mnk_`. The project needs exactly one thing: `OsRng.try_fill_bytes(&mut buf)`.

`rand_core` is the minimal crate for this — it contains only the core RNG traits plus `OsRng`. The full `rand` crate adds PRNGs, sampling distributions, and thread-local state that this use case does not need, adding ~10 transitive compilation units for no benefit.

`rand_core` version 0.9.0 ships `OsRng` behind the `os_rng` feature flag. The `try_fill_bytes` method returns `Result<(), OsError>`, which should be unwrapped at key-generation time (OS entropy failure is non-recoverable).

```toml
rand_core = { version = "0.9", features = ["os_rng"] }
```

Usage:
```rust
use rand_core::{OsRng, TryRngCore};
let mut bytes = [0u8; 32];
OsRng.try_fill_bytes(&mut bytes).expect("OS entropy unavailable");
```

**Why not getrandom directly?** `rand_core` 0.9 already depends on `getrandom` as its implementation. Using `rand_core` is the idiomatic Rust layer; direct `getrandom` usage is lower-level with a less stable API surface.

**Why not `uuid` (already present)?** uuid v7 IDs are already used for memory IDs. They are time-ordered identifiers, not opaque secrets — the first 48 bits encode a timestamp making them partially predictable. API keys must be unpredictable across their full bit length.

**Verdict:** Add `rand_core = { version = "0.9", features = ["os_rng"] }`.

---

### 2. Key Hashing and Storage

**Recommendation: `blake3 1.8` for hashing, `hex 0.4` for encoding, `constant_time_eq 0.4` for comparison.**

**Rationale:**

API keys are stored **only as their hash** in SQLite — the plaintext is shown once at creation and never again. On each request, the presented Bearer token is hashed and compared against stored hashes.

**Why BLAKE3 instead of SHA-256 or Argon2?**

API keys are long (32 random bytes = 256 bits of entropy) — they are not passwords guessable by brute force. Password-hashing algorithms like Argon2 add intentional slowness to resist dictionary attacks. That slowness is inappropriate here: every authenticated HTTP request hashes the presented key, and introducing 100ms+ of deliberate delay for per-request auth is a non-starter. BLAKE3 is the correct choice:
- Cryptographically secure (collision/preimage resistant)
- Extremely fast (no intentional slowness needed — entropy is not brute-forceable)
- Zero external C dependencies (pure Rust, no libsodium)
- Returns a fixed 32-byte output, encodable as a 64-char hex string for SQLite TEXT storage

BLAKE3 1.8.3 is the current stable version (verified via docs.rs).

**Constant-time comparison:** Comparing the presented hash against the stored hash must use constant-time comparison to prevent timing attacks. `constant_time_eq 0.4.2` provides `constant_time_eq_32()` which takes two `&[u8; 32]` slices and compares them in constant time with zero dependencies. This is simpler than the `subtle` crate (which introduces `Choice` wrappers and a more complex API) for the specific use case of comparing two 32-byte hash outputs.

**Hex encoding:** The 32-byte BLAKE3 output is stored in SQLite as a 64-character hex string. The `hex` crate 0.4.3 provides `hex::encode(&[u8])` and `hex::decode(&str)` for this round-trip. Alternatively, `blake3::Hash` has a built-in `to_hex()` method — but using `hex` crate is more explicit and consistent with decoding stored hashes for comparison.

```toml
blake3 = "1.8"
hex = "0.4"
constant_time_eq = "0.4"
```

Usage:
```rust
// Hashing a newly-generated key:
let hash: [u8; 32] = *blake3::hash(key_bytes).as_bytes();
let stored_hash = hex::encode(hash);

// Verifying a presented Bearer token:
let presented_hash: [u8; 32] = *blake3::hash(presented_bytes).as_bytes();
let stored_bytes: [u8; 32] = hex::decode(&stored_hex)?.try_into().unwrap();
if !constant_time_eq::constant_time_eq_32(&presented_hash, &stored_bytes) {
    return Err(ApiError::Unauthorized);
}
```

**SQLite schema addition (no new crate):**

```sql
CREATE TABLE IF NOT EXISTS api_keys (
    id TEXT PRIMARY KEY,            -- uuid v7
    key_hash TEXT NOT NULL UNIQUE,  -- hex(blake3(key_bytes)), 64 chars
    agent_id TEXT NOT NULL,         -- scope: key only grants access to this agent_id
    label TEXT NOT NULL DEFAULT '', -- human-readable label (e.g. "prod-agent-1")
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_used_at DATETIME           -- updated on each successful auth
);

CREATE INDEX IF NOT EXISTS idx_api_keys_key_hash ON api_keys(key_hash);
CREATE INDEX IF NOT EXISTS idx_api_keys_agent_id ON api_keys(agent_id);
```

**Verdict:** Add `blake3 = "1.8"`, `hex = "0.4"`, `constant_time_eq = "0.4"`.

---

### 3. Axum Middleware for Auth Enforcement

**Recommendation: Use `axum::middleware::from_fn` — no new crate needed.**

**Rationale:**

Axum 0.8.4 (already in Cargo.toml) includes `axum::middleware::from_fn` as a first-class primitive. It creates a Tower middleware layer from an `async fn(Request, Next) -> Response` function. This is the documented, idiomatic approach for custom auth middleware in axum 0.8.

The middleware pattern:
1. Extract `Authorization: Bearer mnk_<hex>` header
2. Hash the presented token with BLAKE3
3. Look up the hash in the `api_keys` table
4. If found: insert `AuthContext { agent_id, key_id }` into request extensions, call `next.run(req).await`
5. If not found or header missing: return `401 Unauthorized` with `{"error": "unauthorized"}`
6. Handlers extract `Extension<AuthContext>` to get the authenticated agent_id

**Auth is optional — open mode by default:** The middleware checks whether any keys exist in the database at all. If the `api_keys` table is empty, all requests pass through without auth. This preserves the zero-config local development experience. Once any key is created, auth is enforced globally.

**Approach — apply layer to entire router:**

```rust
Router::new()
    .route("/health", get(health_handler))  // health is exempt from auth
    .route("/memories", post(create_memory).get(list_memories))
    // ...other routes...
    .layer(axum::middleware::from_fn_with_state(
        state.clone(),
        auth_middleware,
    ))
    .with_state(state)
```

The health endpoint needs special treatment: it should remain unauthenticated so monitoring systems can probe it. This can be accomplished by returning early in the middleware when the path is `/health`.

**Why not `tower-http`'s `ValidateRequestHeaderLayer`?** That layer supports only static Bearer tokens set at compile time — not dynamic database-backed key lookups. It is insufficient for this use case.

**Why not `axum-login`?** That crate is designed for session-based user authentication with login/logout flows. It adds significant complexity (session storage, cookie handling, user management) that is completely inappropriate for a machine-to-machine API key system.

**Verdict:** Zero new crates for middleware. Use `axum::middleware::from_fn_with_state` (already in axum 0.8).

---

### 4. CLI Argument Parsing for Key Management Subcommands

**Recommendation: `clap 4.6` with `derive` feature.**

**Rationale:**

The v1.2 milestone requires `mnemonic keys create/list/revoke` CLI subcommands. The binary currently has no CLI argument parsing — `main()` immediately starts the server. The new structure needs:

```
mnemonic                         # start server (default behavior, unchanged)
mnemonic keys create <agent_id> [--label <label>]
mnemonic keys list [<agent_id>]
mnemonic keys revoke <key_id>
```

`clap 4.6.0` with the derive API is the standard choice for Rust CLI parsing:
- Derive macros eliminate boilerplate: `#[derive(Parser)]` on a struct, `#[derive(Subcommand)]` on an enum
- Automatic help generation (`--help` / `-h`) without any manual work
- Automatic version output from Cargo.toml metadata
- Compile-time validated argument shapes
- The most downloaded CLI parsing crate in the Rust ecosystem

The `derive` feature is required — it enables `#[derive(Parser, Subcommand)]`. Without it, clap requires builder-pattern code that is significantly more verbose.

```toml
clap = { version = "4.6", features = ["derive"] }
```

Usage pattern:
```rust
#[derive(Parser)]
#[command(name = "mnemonic", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Keys {
        #[command(subcommand)]
        action: KeysAction,
    },
}

#[derive(Subcommand)]
enum KeysAction {
    Create { agent_id: String, #[arg(long)] label: Option<String> },
    List   { agent_id: Option<String> },
    Revoke { key_id: String },
}
```

When `cli.command` is `None`, `main()` proceeds with server startup (backwards-compatible behavior). When a `Keys` subcommand is matched, the binary performs the key operation, prints the result, and exits.

**Why not `argh` or `structopt`?** `argh` (Google's CLI parser) is minimal but lacks subcommand output formatting and is less ergonomic for nested subcommands. `structopt` is the deprecated predecessor to clap 3's derive API — clap 4 is its direct successor.

**Verdict:** Add `clap = { version = "4.6", features = ["derive"] }`.

---

## Recommended Cargo.toml Changes (v1.2)

```toml
# New dependencies for v1.2 authentication milestone:
rand_core = { version = "0.9", features = ["os_rng"] }  # OsRng for key generation
blake3 = "1.8"                                           # Key hashing at rest
hex = "0.4"                                              # Hex-encode blake3 hashes for SQLite storage
constant_time_eq = "0.4"                                 # Constant-time comparison (timing attack prevention)
clap = { version = "4.6", features = ["derive"] }        # CLI subcommands: keys create/list/revoke

# No changes to existing dependencies.
```

No existing dependencies need version changes. The auth middleware is implemented using axum's existing `middleware::from_fn_with_state` (already in axum 0.8). The `api_keys` schema table is created using the existing `rusqlite` + `tokio-rusqlite` setup.

---

## Alternatives Considered (v1.2)

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| `rand_core` (os_rng feature) | `rand` full crate | `rand` adds PRNGs/distributions not needed; `rand_core` is the minimal subset |
| `rand_core` (os_rng feature) | `getrandom` directly | `rand_core` is the idiomatic layer over `getrandom`; same transitive dep |
| BLAKE3 for hashing | Argon2 / bcrypt | Memory-hard hash designed for passwords, not random-entropy keys; 100ms+ per request unacceptable |
| BLAKE3 for hashing | SHA-256 (sha2 crate) | SHA-256 is fine technically, but BLAKE3 is faster with equivalent security and has a simpler API |
| `constant_time_eq` | `subtle` crate | `subtle`'s `Choice` API is more complex; `constant_time_eq_32` is a direct function call for 32-byte comparison |
| `axum::middleware::from_fn_with_state` | `tower-http` ValidateRequestHeaderLayer | Tower-http's auth layer supports only static compile-time tokens, not database-backed dynamic keys |
| `axum::middleware::from_fn_with_state` | `axum-login` | Session/cookie-based user auth library; massively over-engineered for machine-to-machine API keys |
| `clap` derive API | `argh` | argh is minimal but less ergonomic for nested subcommands; smaller ecosystem |
| `clap` derive API | `structopt` | Deprecated — clap 4 is its direct successor |

---

## What NOT to Add (v1.2)

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `argon2` / `bcrypt` | Memory-hard algorithms designed for password guessing resistance; 100ms+ per hash unacceptable for per-request auth | BLAKE3 — cryptographically secure and fast |
| `jsonwebtoken` / JWT libraries | Stateless JWT requires a signing secret and token expiry management; overkill for simple API keys with explicit revocation | Static bearer tokens backed by SQLite |
| `axum-login` | Full user-session library (login flows, cookies, sessions); adds significant complexity for M2M auth | `axum::middleware::from_fn_with_state` |
| `tower-http` ValidateRequestHeaderLayer | Only supports static compile-time tokens; cannot do dynamic DB lookups | `axum::middleware::from_fn_with_state` |
| `rand` full crate | Includes PRNGs, distributions, thread-local state — unused in this context | `rand_core` with `os_rng` feature only |
| `api-keys-simplified` crate | Uses Argon2id (correct for passwords, wrong for high-entropy keys) + adds niche transitive deps | `blake3` + `constant_time_eq` (fewer, simpler deps) |

---

## Version Compatibility Notes (v1.2)

| Package | Note |
|---------|------|
| rusqlite 0.37 | Must stay at 0.37. sqlite-vec 0.1.7 has a documented conflict with rusqlite 0.39's libsqlite3-sys version. None of the v1.2 additions affect this. |
| reqwest 0.13 | Unchanged. v1.2 adds no HTTP client dependency. |
| rand_core 0.9 | Compatible with Rust edition 2021 (MSRV is 1.65). No conflicts with existing deps. |
| blake3 1.8 | Pure Rust. No C dependencies. Compatible with all existing crates. |
| clap 4.6 | Requires Rust 1.74+. The project uses `edition = "2021"` with no MSRV constraint — modern toolchains satisfy this. |

---

## Sources

**v1.1 sources:**
- [async-openai docs.rs 0.33.1](https://docs.rs/async-openai/latest/async_openai/) — confirmed reqwest 0.12 dependency, confirmed OpenAIConfig.with_api_base() builder method (HIGH confidence)
- [async-openai Cargo.toml on GitHub](https://github.com/64bit/async-openai/blob/main/async-openai/Cargo.toml) — confirmed `reqwest = "0.12"` dependency (HIGH confidence)
- [hdbscan 0.12.0 docs.rs DistanceMetric enum](https://docs.rs/hdbscan/0.12.0/hdbscan/enum.DistanceMetric.html) — confirmed variants: Chebyshev, Cylindrical, Euclidean, Haversine, Manhattan, Precalculated — no cosine (HIGH confidence)
- [hdbscan 0.12.0 docs.rs Hdbscan struct](https://docs.rs/hdbscan/0.12.0/hdbscan/struct.Hdbscan.html) — confirmed Vec<Vec<f32>> input format (HIGH confidence)
- [reqwest 0.13 breaking changes](https://github.com/openapitools/openapi-generator/issues/22621) — confirmed 0.12→0.13 is a breaking change (query/form now feature-gated; rustls default changed) (MEDIUM confidence)
- [Existing Cargo.toml](../../../Cargo.toml) — confirmed reqwest 0.13 in use, confirmed rusqlite 0.37 pinned (HIGH confidence — source of truth)
- [Existing db.rs](../../../src/db.rs) — confirmed schema structure: memories table, vec_memories virtual table (HIGH confidence — source of truth)
- [arewelearningyet.com clustering](https://www.arewelearningyet.com/clustering/) — surveyed full Rust ML ecosystem for clustering options (MEDIUM confidence)

**v1.2 sources:**
- [rand_core 0.9.0 docs.rs](https://docs.rs/rand_core/0.9.0/rand_core/) — confirmed version 0.9.0, `os_rng` feature flag, `OsRng` struct, `TryRngCore::try_fill_bytes` API (HIGH confidence)
- [rand 0.9.1 docs.rs rngs module](https://docs.rs/rand/0.9.1/rand/) — confirmed rand_core 0.9 is the upstream crate; rand re-exports rand_core (HIGH confidence)
- [blake3 1.8.3 docs.rs](https://docs.rs/blake3/latest/blake3/) — confirmed version 1.8.3, `hash()` one-shot API, 32-byte output, constant-time `PartialEq` on `Hash` type (HIGH confidence)
- [constant_time_eq 0.4.2 docs.rs](https://docs.rs/constant_time_eq/latest/constant_time_eq/) — confirmed version 0.4.2, `constant_time_eq_32()` for 32-byte comparison (HIGH confidence)
- [hex 0.4.3 docs.rs](https://docs.rs/hex/latest/hex/) — confirmed version 0.4.3, `encode()`/`decode()` API (HIGH confidence)
- [axum 0.8.4 middleware docs](https://docs.rs/axum/0.8.0/axum/middleware/index.html) — confirmed `from_fn` pattern, auth middleware example extracting Authorization header and inserting extension (HIGH confidence)
- [axum 0.8.4 dependency tree](https://docs.rs/axum/0.8.4/axum/) — confirmed version 0.8.4, tower-http is optional dep (HIGH confidence)
- [clap 4.6.0 docs.rs](https://docs.rs/clap/latest/clap/) — confirmed version 4.6.0, `derive` feature, `Parser` + `Subcommand` derive macros (HIGH confidence)

---
*Stack research for: Mnemonic v1.1 — memory summarization/compaction additions; v1.2 — API key authentication*
*Researched: 2026-03-20*
