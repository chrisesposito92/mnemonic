# Architecture Research

**Domain:** Rust single-binary agent memory server — v1.2 API key authentication integration
**Researched:** 2026-03-20
**Confidence:** HIGH (existing system direct inspection) / HIGH (axum middleware patterns) / MEDIUM (clap CLI patterns for dual-mode binary)

---

## Context: What Already Exists (v1.1)

The v1.1 binary is 3,678 lines of Rust across 10 source files with a strict 4-layer architecture:

```
axum HTTP handlers (server.rs)
        |
        v
MemoryService + CompactionService (service.rs, compaction.rs)
   |                     |
   v                     v
EmbeddingEngine      SummarizationEngine (optional)
(embedding.rs)       (summarization.rs)
        |
        v
   Arc<tokio_rusqlite::Connection>  (db.rs)
```

**Key facts that constrain v1.2 design:**

- `AppState` in `server.rs` holds `Arc<MemoryService>` and `Arc<CompactionService>` — both are already `Arc`-wrapped. A third `Arc<KeyService>` follows the same pattern exactly.
- `Config` uses `figment` with TOML + env-var override. The `auth_enabled` pattern (optional bool) follows the existing `llm_provider: Option<String>` pattern.
- All errors flow through `ApiError → MnemonicError` in `error.rs`. A new `Unauthorized` variant on `ApiError` requires no structural change.
- `db.rs::open()` uses idempotent `CREATE TABLE IF NOT EXISTS` blocks and additive `ALTER TABLE ADD COLUMN` migrations. The `api_keys` table follows the same pattern.
- `main.rs` constructs all services before calling `server::serve()`. A `KeyService` is constructed and passed to `AppState` at the same point.
- The binary currently has no CLI subcommand infrastructure — `main()` runs the server unconditionally. The v1.2 `keys create/list/revoke` commands require adding `clap` and a dual-mode main.

---

## v1.2 System Overview

```
┌──────────────────────────────────────────────────────────────────────────┐
│                              Entry Point (main.rs)                        │
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │  clap CLI parse (NEW)                                               │  │
│  │                                                                     │  │
│  │   mnemonic serve           → server mode (existing behavior)        │  │
│  │   mnemonic keys create     → print new key, exit                    │  │
│  │   mnemonic keys list       → print key table, exit                  │  │
│  │   mnemonic keys revoke     → mark key revoked, exit                 │  │
│  │   (no subcommand)          → server mode (default, backward compat) │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────────┘
        |                            |
        v                            v
 [CLI path]                   [Server path]
 KeyService::create/list/     ┌────────────────────────────────────────────┐
 revoke (direct DB call,      │  HTTP Layer (axum)                         │
 print, exit)                 │                                            │
                              │  ┌─────────────────────────────────────┐   │
                              │  │  Auth Middleware  (NEW)              │   │
                              │  │  from_fn_with_state(auth_middleware) │   │
                              │  │                                     │   │
                              │  │  1. If no keys in DB → pass through │   │
                              │  │  2. Extract Authorization: Bearer    │   │
                              │  │  3. Hash token → DB lookup          │   │
                              │  │  4. Insert AuthContext into request  │   │
                              │  │     extensions                      │   │
                              │  └─────────────┬───────────────────────┘   │
                              │                │                           │
                              │  ┌─────────────▼───────────────────────┐   │
                              │  │  Route handlers (existing)          │   │
                              │  │  POST /memories                     │   │
                              │  │  GET  /memories                     │   │
                              │  │  GET  /memories/search              │   │
                              │  │  DELETE /memories/{id}              │   │
                              │  │  POST /memories/compact             │   │
                              │  │  GET  /health  (auth bypassed)      │   │
                              │  └─────────────┬───────────────────────┘   │
                              └───────────────────────────────────────────┘
                                               |
                              ┌────────────────▼────────────────────────────┐
                              │  Service Layer                               │
                              │                                              │
                              │  MemoryService    CompactionService          │
                              │  (scope filter    (scope filter via          │
                              │   via AuthContext  AuthContext if scoped)    │
                              │   if scoped key)                             │
                              │                   KeyService  (NEW)         │
                              │                   create / list / revoke    │
                              └────────────────────────────────────────────┘
                                               |
                              ┌────────────────▼────────────────────────────┐
                              │  Storage Layer (SQLite)                      │
                              │                                              │
                              │  memories + vec_memories (existing)          │
                              │  compact_runs (existing)                     │
                              │  api_keys  (NEW)                             │
                              └────────────────────────────────────────────┘
```

---

## Component Responsibilities

### Existing Components (v1.1 — minimal or no change)

| Component | v1.2 Change | Notes |
|-----------|-------------|-------|
| `MemoryService` (service.rs) | None | Auth scope enforcement is handled at the handler layer via `AuthContext` |
| `CompactionService` (compaction.rs) | None | Same — `AuthContext` enforcement at handler layer |
| `EmbeddingEngine` (embedding.rs) | None | Unaffected |
| `SummarizationEngine` (summarization.rs) | None | Unaffected |
| `db.rs` — schema init | Add `api_keys` table creation | One new `CREATE TABLE IF NOT EXISTS` block in `execute_batch` |
| `config.rs` — Config struct | No change required | Auth is auto-enabled when keys exist; no config flag needed |
| `error.rs` | Add `Unauthorized` variant to `ApiError` | Additive |
| `server.rs` — `AppState` | Add `key_service: Arc<KeyService>` | One new field; existing handlers unmodified |
| `server.rs` — `build_router()` | Add `route_layer` for auth middleware | One `.route_layer(...)` call; `/health` excluded |
| `lib.rs` | Add `pub mod auth;` | Trivial |
| `main.rs` | Add clap parse + dispatch, construct KeyService | Larger change; see dual-mode pattern below |

### New Components (v1.2)

| Component | Responsibility | Location |
|-----------|----------------|----------|
| `KeyService` | Business logic for key CRUD: generate, hash, store, lookup, revoke | `src/auth.rs` |
| `ApiKey` struct | Row type: id, name, prefix, hashed_key, agent_id scope, created_at, revoked_at | `src/auth.rs` |
| `AuthContext` struct | Per-request auth result passed via request extensions: key_id, allowed_agent_id | `src/auth.rs` |
| `auth_middleware` fn | axum `from_fn_with_state` middleware: extract bearer token, validate against DB, insert `AuthContext` | `src/auth.rs` or `src/server.rs` |
| `Cli` struct | Top-level clap `Parser` struct with `Option<Commands>` subcommand | `src/main.rs` |
| `Commands` enum | `Serve` (default) and `Keys(KeysCommand)` variants | `src/main.rs` |
| `KeysCommand` enum | `Create { name, agent_id }`, `List`, `Revoke { id }` variants | `src/main.rs` |

---

## SQLite Schema: `api_keys` Table

Place alongside `memories` and `compact_runs` in `db.rs::open()` inside the same `execute_batch` call:

```sql
CREATE TABLE IF NOT EXISTS api_keys (
    id          TEXT PRIMARY KEY,          -- UUID v7
    name        TEXT NOT NULL,             -- human label, e.g. "agent-prod"
    prefix      TEXT NOT NULL,             -- first 8 chars of raw token for display
    hashed_key  TEXT NOT NULL,             -- SHA-256(raw_token), hex-encoded
    agent_id    TEXT,                      -- NULL = wildcard (any agent); non-NULL = scoped
    created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    revoked_at  DATETIME                   -- NULL = active; non-NULL = revoked
);

CREATE INDEX IF NOT EXISTS idx_api_keys_hashed_key ON api_keys(hashed_key);
CREATE INDEX IF NOT EXISTS idx_api_keys_agent_id   ON api_keys(agent_id);
```

**Design decisions:**

- `hashed_key` is the lookup column. Index on it — the auth middleware executes this query on every authenticated request.
- `agent_id IS NULL` means the key is a wildcard key (permitted to access any agent's memories). `agent_id = 'some-agent'` means the key can only access memories where `agent_id = 'some-agent'`.
- `revoked_at IS NULL` = active key. Soft delete via timestamp — preserves audit history and is idempotent if revoked twice.
- `prefix` stores the first 8 chars of the raw token (e.g., `mnk_abc1`) for `keys list` output so the human can identify which key is which without storing the full token.
- No `bcrypt` — SHA-256 is correct for high-entropy API keys (not passwords). SHA-256 is fast, which matters when validating on every HTTP request. Argon2/bcrypt are designed for low-entropy secrets (passwords) and are intentionally slow — wrong fit here.

**Raw token format:** `mnk_` prefix + 32 random bytes base64url-encoded = `mnk_` + 43 chars. Total ~47 chars. The prefix makes mnemonic keys visually identifiable in logs and code (same pattern as OpenAI `sk-`, Stripe `sk_live_`, etc.).

---

## Architectural Patterns

### Pattern 1: Auth Middleware with `from_fn_with_state`

**What:** A single `from_fn_with_state` middleware wraps all protected routes. It accesses `AppState` (to reach `KeyService`) and inserts an `AuthContext` into request extensions for handlers to extract.

**When to use:** When authentication validation needs application state (the DB connection) and must forward a validated identity object downstream to handlers.

**Trade-offs:** All protected routes pay one DB query per request. Acceptable because: (a) the `api_keys` lookup is a SHA-256 hash equality on an indexed column — sub-millisecond; (b) WAL mode allows this read to proceed concurrently with writes.

```rust
// src/auth.rs

#[derive(Clone, Debug)]
pub struct AuthContext {
    pub key_id: String,
    pub allowed_agent_id: Option<String>,  // None = wildcard
}

pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    // 1. Check if auth is enforced (any active keys exist)
    let auth_required = state.key_service.has_active_keys().await
        .unwrap_or(false);

    if !auth_required {
        // Open mode: no keys configured, pass through
        return next.run(req).await;
    }

    // 2. Extract bearer token from Authorization header
    let token = match extract_bearer_token(req.headers()) {
        Some(t) => t,
        None => return unauthorized_response("missing Authorization header"),
    };

    // 3. Validate: hash token, lookup in DB
    match state.key_service.validate(&token).await {
        Ok(ctx) => {
            req.extensions_mut().insert(ctx);
            next.run(req).await
        }
        Err(_) => unauthorized_response("invalid or revoked API key"),
    }
}
```

**Router wiring** — use `route_layer` (not `layer`) so that non-matching routes return 404 instead of 401:

```rust
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))       // not auth-protected
        .route("/memories", post(create_memory_handler).get(list_memories_handler))
        .route("/memories/search", get(search_memories_handler))
        .route("/memories/{id}", delete(delete_memory_handler))
        .route("/memories/compact", post(compact_memories_handler))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state)
}
```

Note: routes defined **before** `.route_layer()` have the layer applied. `/health` is defined outside the protected group by being added after `.with_state()` or by using a nested router. The simplest approach: add `/health` to a separate `Router` merged after the protected group.

### Pattern 2: `AuthContext` via Request Extensions (not Handler Parameters)

**What:** The middleware inserts `AuthContext` into `req.extensions_mut()`. Handlers that need scope enforcement call `Extension::<AuthContext>` in their parameter list. Handlers that do not need scope enforcement (e.g., `/health`) simply do not extract it.

**When to use:** When not all handlers need the auth context, and the middleware should not impose a parameter on every handler.

**Trade-offs:** Slightly less type-safe than injecting via `State` because the extension is dynamic. Mitigated by the fact that the middleware always inserts `AuthContext` (or returns 401 early), so any handler on a protected route can rely on it being present.

```rust
// Handler that enforces agent_id scope from auth context
async fn create_memory_handler(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(mut body): Json<CreateMemoryRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    // If key is scoped, override agent_id from key scope
    if let Some(allowed_id) = &auth.allowed_agent_id {
        body.agent_id = Some(allowed_id.clone());
    }
    let memory = state.service.create_memory(body).await?;
    Ok((StatusCode::CREATED, Json(serde_json::to_value(memory).unwrap())))
}
```

**Scoped key enforcement:** If `AuthContext.allowed_agent_id` is `Some(id)`, the handler forcibly sets `agent_id` to that value, ignoring the client-supplied `agent_id`. This enforces namespace isolation at the handler boundary — the client cannot cross into another agent's namespace regardless of what they supply in the request body or query string.

### Pattern 3: Optional Auth ("Open Mode")

**What:** Auth is off by default and activates automatically when at least one active API key exists in the DB. The middleware checks this at every request via `key_service.has_active_keys()`.

**When to use:** For local development (no keys = no friction) and production deployment (create first key = auth enabled). No config flag to forget to set.

**Trade-offs:** The `has_active_keys()` call is an additional DB read per request even in open mode. Mitigate by caching the result in `AppState` using a `tokio::sync::RwLock<bool>` that is invalidated whenever a key is created or revoked via the CLI. In practice, for the typical deployment (local dev = no keys, production = keys), this is a one-time check result that rarely changes.

**Simpler alternative (no caching):** Accept the extra `SELECT COUNT(*) FROM api_keys WHERE revoked_at IS NULL` on every request. This is a non-blocking indexed read — sub-100µs for a table that will have < 100 rows. Ship without the cache first; add caching only if profiling shows it matters.

**Security boundary:** The "no keys = open" mode is safe because the condition is checked server-side on every request, not a config flag that an attacker could omit. An attacker cannot bypass auth by omitting the Authorization header once any active key exists.

### Pattern 4: Dual-Mode Binary with clap

**What:** `main.rs` parses CLI arguments with clap before any service startup. If a `keys` subcommand is given, the binary opens only the DB (skipping embedding model load) and runs the key management operation, then exits. If no subcommand (or `serve` subcommand), the binary behaves exactly as v1.1.

**When to use:** When the same binary must support both a long-running server mode and interactive CLI commands. Avoids shipping a separate `mnemonic-cli` binary.

**Trade-offs:** main.rs becomes more complex. The embedding model load (which takes ~2 seconds) must be guarded behind the server path — CLI commands must not wait for model load. Key insight: CLI commands only need the DB open, not the full AppState.

```rust
// src/main.rs — new structure

#[derive(clap::Parser)]
#[command(name = "mnemonic", about = "Agent memory server")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Start the HTTP server (default when no subcommand given)
    Serve,
    /// Manage API keys
    Keys {
        #[command(subcommand)]
        action: KeysAction,
    },
}

#[derive(clap::Subcommand)]
enum KeysAction {
    /// Create a new API key
    Create {
        /// Human-readable name for this key
        #[arg(long)]
        name: String,
        /// Scope key to a specific agent_id (optional; omit for wildcard)
        #[arg(long)]
        agent_id: Option<String>,
    },
    /// List all API keys
    List,
    /// Revoke an API key by ID
    Revoke {
        /// Key ID to revoke
        id: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Serve) {
        Commands::Keys { action } => {
            // Minimal startup: config + DB only, no embedding model
            db::register_sqlite_vec();
            let config = config::load_config()?;
            let conn = Arc::new(db::open(&config).await?);
            let key_service = KeyService::new(conn);
            run_key_command(key_service, action).await?;
        }
        Commands::Serve => {
            // Full startup: existing v1.1 behavior
            run_server().await?;
        }
    }
    Ok(())
}
```

**Cargo.toml addition:**

```toml
clap = { version = "4", features = ["derive"] }
```

No other new dependencies. Key generation uses `rand` (which is already a transitive dependency via uuid) or the `uuid::Uuid::now_v7()` bytes as an entropy source. Use `rand::thread_rng()` directly for the 32-byte token body.

**Raw token generation:**

```rust
use rand::Rng;

pub fn generate_raw_token() -> String {
    let bytes: [u8; 32] = rand::thread_rng().gen();
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    format!("mnk_{}", encoded)
}
```

`rand` is available as a transitive dependency of `uuid`. Add it explicitly to Cargo.toml to pin the version.

---

## Data Flow: Request Authentication

```
Incoming HTTP Request
  Authorization: Bearer mnk_abc1...xyz
        |
        v
auth_middleware (from_fn_with_state)
  1. has_active_keys()
     └── SELECT COUNT(*) FROM api_keys WHERE revoked_at IS NULL
         → 0 → pass through (open mode)
         → > 0 → continue validation

  2. Extract "mnk_abc1...xyz" from Authorization header
     └── parse "Bearer <token>", strip prefix

  3. sha256(token) → hex digest
     └── pure Rust, no DB, sub-microsecond

  4. DB lookup
     └── SELECT id, agent_id
         FROM api_keys
         WHERE hashed_key = ? AND revoked_at IS NULL
         → not found → return 401 {"error": "invalid or revoked API key"}
         → found → AuthContext { key_id, allowed_agent_id }

  5. req.extensions_mut().insert(AuthContext { ... })
  6. next.run(req).await → handler
        |
        v
Handler (e.g., create_memory_handler)
  Extension(auth): Extension<AuthContext>
  └── auth.allowed_agent_id = Some("agent-x")
      → body.agent_id forced to "agent-x"
  └── auth.allowed_agent_id = None
      → body.agent_id from request as-is (wildcard key)
        |
        v
MemoryService::create_memory(body) — unchanged
```

---

## Data Flow: CLI Key Management

```
mnemonic keys create --name "prod-agent" --agent-id "agent-x"
        |
        v
main() → clap parse → Commands::Keys { action: Create { name, agent_id } }
        |
        v
Minimal startup (DB open only, no embedding load)
        |
        v
KeyService::create(name, agent_id):
  1. generate_raw_token() → "mnk_abc1def2..."
  2. prefix = &raw_token[..12]     -- "mnk_abc1def2"
  3. hashed_key = sha256(raw_token) → hex string
  4. id = Uuid::now_v7()
  5. INSERT INTO api_keys (id, name, prefix, hashed_key, agent_id, created_at)
  6. Return (id, raw_token) -- raw_token shown ONCE then discarded
        |
        v
Print to stdout:
  Key created:
    ID:     <uuid>
    Name:   prod-agent
    Token:  mnk_abc1def2...  (shown once — store this securely)
    Scope:  agent-x
        |
        v
exit(0)
```

**Token shown only once:** The raw token is printed at creation and then is gone — only the SHA-256 hash is stored. This matches the GitHub/Stripe/Anthropic pattern for API key management. Document this prominently in the CLI output.

---

## Integration Points: New vs. Modified

### New Components

| Component | Type | Integration |
|-----------|------|-------------|
| `KeyService` | New struct in `src/auth.rs` | Holds `Arc<Connection>`; used by `AppState` (server) and CLI dispatch (main.rs) |
| `ApiKey` | New struct | Row type for `api_keys` table; returned by `key_service.list()` |
| `AuthContext` | New struct | Inserted into request extensions by middleware; extracted by handlers |
| `auth_middleware` fn | New axum middleware | Added to router via `route_layer`; accesses `AppState` via `from_fn_with_state` |
| `api_keys` table | New DB table | Created in `db.rs::open()` alongside existing tables |
| `Cli` / `Commands` / `KeysAction` | New clap structs | Parsed in `main.rs` before any service startup |

### Modified Components

| Component | Modification | Impact Risk |
|-----------|-------------|-------------|
| `main.rs` | Add clap parse; split startup path (server vs. CLI); extract `run_server()` fn | MEDIUM — largest change; no existing logic deleted |
| `server.rs` — `AppState` | Add `key_service: Arc<KeyService>` | Low — additive field; existing handlers compile unchanged |
| `server.rs` — `build_router()` | Add `.route_layer(...)` for auth middleware; restructure `/health` exclusion | Low — one new call; existing routes unchanged |
| `db.rs` — `open()` | Add `api_keys` DDL block | Low — additive; existing tables unaffected |
| `error.rs` | Add `Unauthorized` variant to `ApiError` with 401 response | Low — additive |
| `lib.rs` | Add `pub mod auth;` | Trivial |
| Cargo.toml | Add `clap = { version = "4", features = ["derive"] }`, `rand`, `sha2`, `base64` | Low — new deps, no conflicts expected |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| `auth_middleware` ↔ `KeyService` | Direct async call via `Arc<KeyService>` (from `AppState`) | Middleware holds no state; reads from DB on every request |
| `auth_middleware` → handlers | `req.extensions_mut().insert(AuthContext)` | Extension pattern; handlers opt-in to extraction |
| `main.rs` ↔ `KeyService` (CLI path) | Direct async call, no AppState, no embedding | CLI path constructs only `Arc<Connection>` + `KeyService` |
| `KeyService` ↔ SQLite | `conn.call()` closures, same pattern as all existing services | Same `Arc<Connection>` shared with MemoryService in server mode |

---

## Scoped Keys and Existing `agent_id` Filtering

The existing `agent_id` filtering in `MemoryService` is a client-provided filter — clients can request any agent's memories by supplying any `agent_id`. Scoped API keys enforce namespace isolation by overriding the client-supplied `agent_id` at the handler layer.

**Enforcement point: handler, not service.** `MemoryService` and `CompactionService` remain unmodified. The override happens in each handler before calling the service:

```
auth.allowed_agent_id = Some("agent-x")
body.agent_id = "agent-y"  (from client request)

→ handler overrides: body.agent_id = "agent-x"
→ MemoryService sees agent_id = "agent-x" — namespace enforced
```

**Wildcard keys** (`agent_id IS NULL` in DB → `allowed_agent_id = None` in `AuthContext`) do not override the client-supplied `agent_id`. All namespaces are accessible. Use for multi-agent orchestrators or admin tooling.

**Compaction scoping:** `POST /memories/compact` body contains `agent_id`. The same override logic applies — if the key is scoped to `agent-x`, the compact request is forced to `agent_id = "agent-x"`. A scoped key cannot trigger compaction across namespaces.

**This means no service-layer changes are needed.** The scoping is entirely enforced at the handler layer via `AuthContext` injection.

---

## Recommended Project Structure (v1.2 delta)

Flat module structure preserved. Only one new file and one modified file of substance.

```
src/
├── main.rs          # MODIFIED: add clap parse + dual-mode dispatch
├── config.rs        # No change (auth is keycount-driven, not config-driven)
├── server.rs        # MODIFIED: AppState + key_service, route_layer for auth
├── service.rs       # No change
├── embedding.rs     # No change
├── db.rs            # MODIFIED: add api_keys DDL block
├── error.rs         # MODIFIED: add Unauthorized variant to ApiError
├── lib.rs           # MODIFIED: add pub mod auth
├── compaction.rs    # No change
├── summarization.rs # No change
│
└── auth.rs          # NEW: KeyService, ApiKey, AuthContext, auth_middleware
                     #      generate_raw_token(), sha256_hex()
                     #      has_active_keys(), validate(), create(), list(), revoke()
```

**Rationale:** All auth functionality is cohesive and belongs in one module (`auth.rs`). The middleware function can live in `auth.rs` or `server.rs` — prefer `auth.rs` so that `server.rs` has no knowledge of the cryptographic details.

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Storing Raw Tokens in the Database

**What:** Storing `mnk_abc1...` directly in the `hashed_key` column.

**Why it's wrong:** A DB leak exposes all tokens immediately, giving an attacker access to every client. The entire point of hashing is that a DB leak does not give usable credentials.

**Do this instead:** Store `sha256(raw_token)` as a hex string. The raw token is shown to the user once (at creation) and never persisted anywhere. On validation, compute `sha256(incoming_token)` and compare against the stored hash.

### Anti-Pattern 2: Using `bcrypt` or `argon2` for API Key Hashing

**What:** Applying password-hashing algorithms to API key validation.

**Why it's wrong:** bcrypt/argon2 are designed for low-entropy secrets (passwords) and deliberately take 100ms+ per operation. API keys are 32 bytes of random entropy — brute-forcing the hash is computationally infeasible regardless of algorithm. Using bcrypt for API keys adds 100ms+ to every authenticated request for no security benefit.

**Do this instead:** SHA-256. Fast, deterministic, sufficient for high-entropy tokens. Add the `sha2` crate.

### Anti-Pattern 3: Using `layer()` Instead of `route_layer()` for Auth Middleware

**What:** Applying auth middleware with `.layer(middleware::from_fn_with_state(...))` instead of `.route_layer(...)`.

**Why it's wrong:** `layer()` runs the middleware on ALL requests including those that match no route. A request to `/nonexistent` would return 401 instead of 404, leaking information about auth configuration.

**Do this instead:** `route_layer()` runs middleware only when a route is matched. Non-matching routes correctly return 404 without ever reaching auth validation.

### Anti-Pattern 4: Placing Auth Logic in `MemoryService` or `CompactionService`

**What:** Adding auth validation inside `MemoryService::create_memory()` or similar service methods.

**Why it's wrong:** Services would need to know about `AuthContext`, creating a coupling between business logic and auth concerns. The service layer is currently database-layer pure — it takes typed request structs and returns typed results. Mixing auth into services makes them harder to test and violates the separation already established.

**Do this instead:** Auth enforcement lives entirely in the middleware and handler layer. Services remain unaware of authentication.

### Anti-Pattern 5: Loading the Embedding Model for CLI Key Commands

**What:** Running the full v1.1 startup path (including `tokio::task::spawn_blocking` for model load) when `mnemonic keys create` is called.

**Why it's wrong:** The all-MiniLM-L6-v2 model takes ~2 seconds to load. A CLI command that should take 100ms becomes a 2-second operation. Worse, it loads a ~22MB model from disk for no purpose.

**Do this instead:** The `Commands::Keys` branch in `main()` opens only the DB, constructs only `KeyService`, runs the command, and exits. The embedding engine and LLM engine are never initialized.

### Anti-Pattern 6: Storing `AuthContext` in `AppState` Instead of Request Extensions

**What:** Attempting to share the per-request auth context via `AppState`.

**Why it's wrong:** `AppState` is shared across all concurrent requests — it is a single instance, not per-request. Storing per-request data in shared state requires a mutex and would serialize concurrent requests.

**Do this instead:** Use `req.extensions_mut().insert(AuthContext { ... })` in the middleware and `Extension::<AuthContext>` in handlers. Extensions are per-request.

---

## Build Order (v1.2 phases, considering dependencies)

```
1. db.rs             — Add api_keys DDL block + indexes
                       (no new dependencies; all other auth work depends
                        on the table existing)

2. error.rs          — Add Unauthorized variant to ApiError
                       (depends only on existing axum IntoResponse pattern;
                        auth.rs middleware depends on this)

3. auth.rs (KeyService + types)
                     — ApiKey struct, AuthContext struct, KeyService::new()
                       create() / list() / revoke() / validate() / has_active_keys()
                       generate_raw_token(), sha256_hex()
                       (depends on db.rs schema; no HTTP yet)

4. auth.rs (middleware)
                     — auth_middleware fn using from_fn_with_state
                       (depends on AuthContext, KeyService, ApiError::Unauthorized)

5. server.rs         — Add key_service to AppState; add route_layer to build_router();
                       restructure /health exclusion; add Extension(auth) to handlers
                       (depends on auth.rs being complete)

6. main.rs           — Add clap; split startup into server and CLI paths;
                       wire KeyService into both paths
                       (depends on all above; last change)
```

**Phase recommendation:**

- **Phase A (foundation):** steps 1-2 together — schema + error variants. Testable immediately: verify `api_keys` table appears in DB.
- **Phase B (KeyService):** step 3 in isolation. Unit-testable with a test DB: create key, list, revoke, validate. No HTTP involved yet.
- **Phase C (middleware):** step 4. Integration-testable with a test router and test keys in DB.
- **Phase D (HTTP wiring):** step 5. Add middleware to router; verify existing tests still pass; add auth-specific integration tests.
- **Phase E (CLI):** step 6. Add clap; verify `mnemonic keys create` works end-to-end; verify `mnemonic serve` (or no subcommand) still works exactly as before.

---

## Scaling Considerations

| Scale | Auth Behavior |
|-------|---------------|
| Local dev (no keys) | Open mode — zero auth overhead |
| Small deployment (1-10 keys) | SHA-256 lookup: < 1ms; indexed `api_keys` table with < 100 rows |
| Large deployment (1000s of requests/sec) | Add in-memory cache of `hashed_key → AuthContext` with 1-minute TTL; invalidate on revoke |

The cache optimization is not needed for v1.2. The `SELECT ... WHERE hashed_key = ?` query on an indexed 100-row table is sub-millisecond and SQLite WAL mode means reads never block writes.

---

## Sources

- [axum middleware docs — `from_fn_with_state`](https://docs.rs/axum/latest/axum/middleware/fn.from_fn_with_state.html) — HIGH confidence (official docs)
- [axum middleware docs — patterns overview](https://docs.rs/axum/latest/axum/middleware/index.html) — HIGH confidence (official docs)
- [axum `route_layer` vs `layer`](https://github.com/tokio-rs/axum/discussions/2878) — HIGH confidence (official project discussion)
- [clap derive tutorial — subcommands](https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html) — HIGH confidence (official docs)
- [API key authentication best practices — Zuplo](https://zuplo.com/blog/2022/12/01/api-key-authentication) — MEDIUM confidence (industry reference; SHA-256 recommendation confirmed by multiple sources)
- [axum Extension extractor](https://docs.rs/axum/latest/axum/struct.Extension.html) — HIGH confidence (official docs)
- Existing v1.1 source code (`src/*.rs`) — HIGH confidence (direct inspection)

---

*Architecture research for: Mnemonic v1.2 — API key authentication integration*
*Researched: 2026-03-20*
