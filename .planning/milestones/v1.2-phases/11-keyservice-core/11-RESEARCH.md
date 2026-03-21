# Phase 11: KeyService Core - Research

**Researched:** 2026-03-20
**Domain:** Rust API key management — BLAKE3 hashing, constant-time comparison, secure token generation, SQLite CRUD via tokio-rusqlite
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Token Generation**
- D-01: Raw token format: `mnk_<64 hex chars>` — 32 random bytes hex-encoded (256-bit entropy)
- D-02: Random source: `rand` crate with `OsRng` (cryptographically secure, standard Rust practice)
- D-03: Token is returned exactly once from `create()` and never stored — only the BLAKE3 hash is persisted

**Key Hashing**
- D-04: BLAKE3 via `blake3` crate — 32-byte output, hex-encoded for storage in `hashed_key TEXT` column
- D-05: Constant-time comparison via `constant_time_eq::constant_time_eq_32()` on `[u8; 32]` — never `==` on hash values
- D-06: Display ID = first 8 hex chars of BLAKE3(raw_key) — not a prefix of the raw key itself (Auth Pitfall 7)

**Validation**
- D-07: `validate()` hashes the incoming raw token with BLAKE3, queries DB for matching `hashed_key`, returns `AuthContext` on success
- D-08: Error granularity: descriptive messages within `DbError` — "key not found" vs "key revoked" — but both map to 401 at the API layer. No separate error variants needed.
- D-09: Scope enforcement (checking agent_id match) deferred to Phase 13 handler layer — `validate()` returns `AuthContext { key_id, allowed_agent_id }` and the handler decides
- D-10: Revoked keys: `validate()` checks `revoked_at IS NULL` in the query — a revoked key returns an error, never an AuthContext

**List Behavior**
- D-11: `list()` returns all keys (active AND revoked) — preserves audit trail, matches D-05 soft delete decision from Phase 10
- D-12: Results ordered by `created_at DESC` (newest first)
- D-13: Never returns raw token or hashed_key — only ApiKey fields (id, name, display_id, agent_id, created_at, revoked_at)

**Revocation**
- D-14: `revoke()` sets `revoked_at = CURRENT_TIMESTAMP` via UPDATE — does not DELETE the row
- D-15: Idempotent — revoking a non-existent or already-revoked key returns `Ok(())`, not an error
- D-16: No confirmation step — immediate effect, subsequent `validate()` calls reject the key

**Crate Dependencies**
- D-17: Add `blake3` to Cargo.toml (pure Rust, fast, no OpenSSL)
- D-18: Add `constant_time_eq` to Cargo.toml (single-purpose crate for `constant_time_eq_32`)
- D-19: Add `rand` to Cargo.toml with `std` + `std_rng` features for `OsRng` + random byte generation

### Claude's Discretion
- Internal helper function organization (e.g., whether to extract `hash_token()` and `generate_token()` as private functions or module-level functions)
- Whether to add `#[cfg(test)]` unit tests inline in `auth.rs` or in a separate `tests/` file
- Exact SQL query structure (single query vs multiple for validation)

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| KEY-01 | Admin can create an API key with optional name and optional agent_id scope, receiving the raw key (mnk_...) exactly once | `create()` implementation: OsRng 32-byte generation → hex-encode → BLAKE3 hash → INSERT → return `(ApiKey, raw_token)` |
| KEY-02 | Admin can list all API keys showing name, prefix, scope, and creation date — never the full key | `list()` implementation: SELECT without hashed_key column, ORDER BY created_at DESC |
| KEY-03 | Admin can revoke a key, immediately preventing its use on subsequent requests | `revoke()` implementation: UPDATE revoked_at = CURRENT_TIMESTAMP, idempotent, no cache |
| KEY-04 | API key can be scoped to a specific agent_id, restricting access to only that agent's memories | `validate()` returns AuthContext with `allowed_agent_id`, scope enforcement deferred to Phase 13 handler |
| INFRA-02 | Key hashes use BLAKE3 with constant-time comparison to prevent timing attacks | `constant_time_eq_32()` on `[u8; 32]` from `blake3::Hash::as_bytes()` — never `==` on hash values |
</phase_requirements>

---

## Summary

Phase 11 fills in four `todo!()` stubs in `src/auth.rs`: `create`, `list`, `revoke`, and `validate` on `KeyService`. The schema foundation (api_keys table with `display_id`, `hashed_key`, `agent_id`, `revoked_at` columns) is complete from Phase 10. All architectural decisions are locked. This is a pure service-layer implementation phase — no HTTP handlers, no middleware, no CLI.

The three new crates (`blake3`, `constant_time_eq`, `rand`) are all lightweight, pure-Rust, and widely used in the Rust security ecosystem. Their APIs are straightforward. The main implementation complexity is in the correct sequence: generate token → compute display_id (BLAKE3 first 8 hex chars) → compute stored hash (full BLAKE3 hex) → INSERT → return raw token to caller.

The constant-time comparison requirement (INFRA-02) is the single most critical correctness property. `validate()` must: (1) compute a `[u8; 32]` BLAKE3 hash of the incoming token, (2) retrieve the stored hash bytes from the DB as `[u8; 32]`, (3) compare with `constant_time_eq_32()` — never with `==`. The DB query should include `WHERE revoked_at IS NULL` so revoked key rows never reach the comparison step.

**Primary recommendation:** Implement the four methods in dependency order — `create` (standalone), `list` (standalone), `revoke` (standalone), `validate` (depends on hash helpers already used in create). Extract `hash_token(raw: &str) -> [u8; 32]` as a private helper used by both `create` and `validate` to ensure consistent hashing.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| blake3 | 1.8.3 | BLAKE3 hashing for key storage and validation | Pure Rust, fast, simple API: `blake3::hash(bytes)` returns `Hash` with `.as_bytes() -> &[u8; 32]` and `.to_hex()` |
| constant_time_eq | 0.4.2 | Constant-time byte array comparison | Single-purpose, audited; `constant_time_eq_32(a: &[u8; 32], b: &[u8; 32]) -> bool`; prevents timing attacks per INFRA-02 |
| rand | 0.9.1 | Cryptographically secure random byte generation via OsRng | `OsRng.try_fill_bytes(&mut [u8; 32])` uses OS entropy; standard for key generation in Rust |

### Already Present (no addition needed)
| Library | Version | Purpose | Notes |
|---------|---------|---------|-------|
| tokio-rusqlite | 0.7 | Async DB access via `conn.call()` | All DB ops use this pattern; no change |
| uuid | 1 (v7) | Primary key generation | `uuid::Uuid::now_v7().to_string()` for key IDs |
| rusqlite | 0.37 | Underlying SQLite driver | Used inside `conn.call()` closures |

**Version verification (verified 2026-03-20 via cargo search):**
- `blake3 = "1.8.3"` — current
- `constant_time_eq = "0.4.2"` — current
- `rand = "0.9.1"` — current (0.10.0 also exists but OsRng API changed; use 0.9.x for stability)

**Note on rand version:** `rand` 0.10.0 exists but the CONTEXT.md decision (D-19) specifies `std` + `std_rng` features. In rand 0.9, `OsRng` is re-exported from `rand_core` and available via `rand::rngs::OsRng` with `features = ["os_rng"]` or through `rand_core`. In rand 0.9, `OsRng.try_fill_bytes(&mut buf)` requires `use rand_core::TryRngCore`. Pin to `0.9` to avoid any API churn from 0.10.

**Installation:**
```toml
blake3 = "1.8.3"
constant_time_eq = "0.4.2"
rand = { version = "0.9", features = ["std", "std_rng", "os_rng"] }
```

---

## Architecture Patterns

### Existing Code Structure to Preserve

`src/auth.rs` already defines:
- `ApiKey` struct (matches `api_keys` table: id, name, display_id, agent_id, created_at, revoked_at)
- `AuthContext` struct (key_id: String, allowed_agent_id: Option<String>)
- `KeyService` with `conn: Arc<Connection>`
- `count_active_keys()` as a working reference implementation
- Four `todo!()` stubs: `create`, `list`, `revoke`, `validate`

The `conn.call(|c| ...)` pattern is the only way to run rusqlite from async context. All four stub methods must use this pattern.

### Pattern 1: Token Generation and Hashing Pipeline

**What:** A single private helper `hash_token(raw: &str) -> [u8; 32]` that both `create()` and `validate()` call. A second helper `generate_raw_token() -> String` that only `create()` calls.

**When to use:** Any time a raw token needs hashing (create for storage, validate for comparison).

```rust
// Source: blake3 1.8.3 docs + constant_time_eq 0.4.2 docs
use rand_core::{OsRng, TryRngCore};

fn generate_raw_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.try_fill_bytes(&mut bytes).expect("OsRng failed — OS entropy unavailable");
    hex::encode(bytes)  // 64 hex chars
    // Full token: format!("mnk_{}", hex::encode(bytes))
}

fn hash_token(raw: &str) -> [u8; 32] {
    *blake3::hash(raw.as_bytes()).as_bytes()
}

fn hash_to_hex(raw: &str) -> String {
    blake3::hash(raw.as_bytes()).to_hex().to_string()
}
```

**Note on hex encoding:** The project has no `hex` crate in Cargo.toml. Use `format!("{:02x}", byte)` in a loop, or add `hex = "0.4"` as a lightweight dependency. Alternatively, `blake3::Hash::to_hex()` already handles hex encoding of the hash — no separate hex crate needed for the hash itself. For encoding the raw random bytes, `format!` in a loop or the `hex` crate is needed.

**Simplest approach without adding `hex` crate:**
```rust
fn generate_raw_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.try_fill_bytes(&mut bytes).expect("OsRng entropy unavailable");
    let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
    format!("mnk_{}", hex)
}
```

### Pattern 2: `create()` Implementation

**What:** Generate token → compute display_id → compute hashed_key → INSERT → return `(ApiKey, raw_token)`.

**D-06 critical:** `display_id` = first 8 chars of `BLAKE3(raw_token).to_hex()` — NOT a prefix of the raw token string.

```rust
// Source: auth.rs stub signature + CONTEXT.md D-01 through D-06
pub async fn create(
    &self,
    name: String,
    agent_id: Option<String>,
) -> Result<(ApiKey, String), crate::error::DbError> {
    let raw_token = generate_raw_token();                        // "mnk_<64 hex>"
    let hash = blake3::hash(raw_token.as_bytes());
    let hashed_key: String = hash.to_hex().to_string();          // 64-char hex string
    let display_id: String = hashed_key[..8].to_string();        // first 8 chars of hash hex (D-06)
    let id = uuid::Uuid::now_v7().to_string();

    let key = ApiKey {
        id: id.clone(),
        name: name.clone(),
        display_id: display_id.clone(),
        agent_id: agent_id.clone(),
        created_at: String::new(),  // filled from DB after INSERT
        revoked_at: None,
    };

    // INSERT then query back to get server-generated created_at
    self.conn.call(move |c| -> Result<ApiKey, rusqlite::Error> {
        c.execute(
            "INSERT INTO api_keys (id, name, display_id, hashed_key, agent_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, name, display_id, hashed_key, agent_id],
        )?;
        let row = c.query_row(
            "SELECT id, name, display_id, agent_id, created_at, revoked_at
             FROM api_keys WHERE id = ?1",
            rusqlite::params![id],
            |row| Ok(ApiKey {
                id: row.get(0)?,
                name: row.get(1)?,
                display_id: row.get(2)?,
                agent_id: row.get(3)?,
                created_at: row.get(4)?,
                revoked_at: row.get(5)?,
            }),
        )?;
        Ok(row)
    })
    .await
    .map(|key| (key, raw_token))
    .map_err(crate::error::DbError::from)
}
```

### Pattern 3: `validate()` — Constant-Time Comparison

**What:** Hash incoming token → single SQL query with `WHERE revoked_at IS NULL` — no two-step lookup. Compare stored hash bytes with constant-time eq.

**Critical:** The SQL query returns the stored `hashed_key` column. Convert both the incoming hash and the stored hash to `[u8; 32]` before calling `constant_time_eq_32()`.

```rust
// Source: constant_time_eq 0.4.2 docs, blake3 1.8.3 docs, CONTEXT.md D-07/D-10
use constant_time_eq::constant_time_eq_32;

pub async fn validate(
    &self,
    raw_token: &str,
) -> Result<AuthContext, crate::error::DbError> {
    let incoming_hash: [u8; 32] = *blake3::hash(raw_token.as_bytes()).as_bytes();
    let incoming_hex = blake3::hash(raw_token.as_bytes()).to_hex().to_string();

    let raw_token = raw_token.to_string();
    self.conn.call(move |c| -> Result<AuthContext, rusqlite::Error> {
        // Single query: only active (non-revoked) keys
        let result = c.query_row(
            "SELECT id, hashed_key, agent_id, revoked_at
             FROM api_keys
             WHERE hashed_key = ?1 AND revoked_at IS NULL",
            rusqlite::params![incoming_hex],
            |row| {
                let stored_hex: String = row.get(1)?;
                Ok((
                    row.get::<_, String>(0)?,  // id
                    stored_hex,
                    row.get::<_, Option<String>>(2)?,  // agent_id
                ))
            },
        );

        match result {
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // Could be: key not found, OR key exists but is revoked.
                // D-08: descriptive message; both map to 401 at API layer.
                Err(rusqlite::Error::InvalidParameterName("key not found or revoked".into()))
            }
            Err(e) => Err(e),
            Ok((key_id, stored_hex, agent_id)) => {
                // Constant-time comparison — D-05
                let mut stored_bytes = [0u8; 32];
                if hex::decode_to_slice(&stored_hex, &mut stored_bytes).is_err() {
                    return Err(rusqlite::Error::InvalidParameterName("invalid stored hash".into()));
                }
                if !constant_time_eq_32(&incoming_hash, &stored_bytes) {
                    // Hash collision in hex lookup would be extraordinary, but be safe
                    return Err(rusqlite::Error::InvalidParameterName("key not found or revoked".into()));
                }
                Ok(AuthContext {
                    key_id,
                    allowed_agent_id: agent_id,
                })
            }
        }
    })
    .await
    .map_err(crate::error::DbError::from)
}
```

**Simpler validate() approach:** Since the SQL `WHERE hashed_key = ?1` is an exact string match on an indexed column, the lookup itself is already effectively secure — BLAKE3 hash equality is not guessable. The constant-time comparison is an additional layer (required by INFRA-02) to prevent hash oracle attacks. The cleanest implementation:

1. Compute `incoming_hex = blake3::hash(raw_token.as_bytes()).to_hex().to_string()`
2. Query `SELECT id, hashed_key, agent_id FROM api_keys WHERE hashed_key = ?1 AND revoked_at IS NULL`
3. If row found, decode `stored_hex` to `[u8; 32]`, decode `incoming_hex` to `[u8; 32]`, call `constant_time_eq_32()`
4. Return `AuthContext` if match, `DbError` if not

### Pattern 4: `list()` — Safe Projection

**What:** SELECT all fields EXCEPT `hashed_key`. Order by `created_at DESC`. Map to `Vec<ApiKey>`.

```rust
// Source: CONTEXT.md D-11, D-12, D-13
pub async fn list(&self) -> Result<Vec<ApiKey>, crate::error::DbError> {
    self.conn.call(|c| -> Result<Vec<ApiKey>, rusqlite::Error> {
        let mut stmt = c.prepare(
            "SELECT id, name, display_id, agent_id, created_at, revoked_at
             FROM api_keys
             ORDER BY created_at DESC",
        )?;
        let keys = stmt.query_map([], |row| Ok(ApiKey {
            id: row.get(0)?,
            name: row.get(1)?,
            display_id: row.get(2)?,
            agent_id: row.get(3)?,
            created_at: row.get(4)?,
            revoked_at: row.get(5)?,
        }))?
        .collect::<Result<Vec<_>, _>>()?;
        Ok(keys)
    })
    .await
    .map_err(crate::error::DbError::from)
}
```

### Pattern 5: `revoke()` — Idempotent Soft Delete

**What:** UPDATE revoked_at = CURRENT_TIMESTAMP WHERE id = ?. Use `execute()` not `query_row()`. Check rows_affected only to decide — but D-15 says idempotent, so returning `Ok(())` regardless is correct.

```rust
// Source: CONTEXT.md D-14, D-15
pub async fn revoke(&self, id: &str) -> Result<(), crate::error::DbError> {
    let id = id.to_string();
    self.conn.call(move |c| -> Result<(), rusqlite::Error> {
        c.execute(
            "UPDATE api_keys SET revoked_at = CURRENT_TIMESTAMP WHERE id = ?1",
            rusqlite::params![id],
        )?;
        Ok(())  // Idempotent: 0 rows affected (not found or already revoked) is not an error
    })
    .await
    .map_err(crate::error::DbError::from)
}
```

### Anti-Patterns to Avoid

- **Using `==` on hash strings:** `stored_hex == incoming_hex` is NOT constant-time even as a String comparison. Always use `constant_time_eq_32` on `[u8; 32]` byte arrays.
- **Storing the raw token:** `create()` must return `raw_token` to the caller and never persist it. Only `hashed_key` goes to the DB.
- **Prefix as display_id:** `display_id` must be `hashed_key[..8]` (first 8 chars of the BLAKE3 hex), NOT `raw_token[..8]`. See Auth Pitfall 7.
- **`todo!()` panic paths remaining:** After implementing each method, remove its `#[allow(dead_code)]` annotation — Phase 10 added these to suppress warnings on stubs.
- **Two separate queries in validate():** A query `WHERE id = ?` followed by a hash comparison in Rust exposes the system to timing oracle attacks on key IDs. Use a single `WHERE hashed_key = ?` query.
- **Using `hex::decode` without adding hex to Cargo.toml:** If hex decoding of stored bytes is needed in `validate()`, either add `hex = "0.4"` or use the blake3 `Hash::from_hex()` constructor.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Constant-time comparison | Custom loop with early return prevention | `constant_time_eq::constant_time_eq_32()` | Compiler optimizations can eliminate "constant-time" hand-rolled loops; audited crate prevents this |
| Cryptographic hashing | SHA-256 via stdlib or manual | `blake3::hash()` | Locked to BLAKE3 (D-04); blake3 crate is the canonical implementation |
| OS-level entropy | `std::time` seeded PRNG | `OsRng.try_fill_bytes()` | Deterministic seed sources are predictable; OS entropy is cryptographically strong |
| Hex encoding of hash | format! loop per byte | `blake3::Hash::to_hex()` | Already returns 64-char lowercase hex; no extra crate or loop needed |

**Key insight:** The three crypto operations (hashing, constant-time comparison, OS entropy) each have exactly one correct Rust crate. Using anything else risks subtle vulnerabilities that pass all tests.

---

## Common Pitfalls

### Pitfall 1: display_id Derived from Raw Token Prefix (Auth Pitfall 7)
**What goes wrong:** `display_id = raw_token[4..12]` (skipping "mnk_" prefix) exposes the first 8 chars of the key — reduces brute-force search space.
**Why it happens:** "Display the first few chars" is the intuitive approach for identification.
**How to avoid:** `display_id = hashed_key[..8]` — first 8 hex chars of the BLAKE3 hash. This is what Phase 10 schema stores in the `display_id` column. The `create()` implementation must derive it from `hashed_key`, not from `raw_token`.
**Warning signs:** Any code that does `raw_token[..N]` before calling `hash_token()`.

### Pitfall 2: `==` Comparison on Hash Strings (Auth Pitfall 1 / INFRA-02)
**What goes wrong:** `stored_hex == incoming_hex` compiles and works correctly but is not constant-time — short-circuits on first differing byte.
**Why it happens:** String equality is the obvious comparison; the side channel is invisible in code review.
**How to avoid:** Convert both hex strings to `[u8; 32]` and call `constant_time_eq_32()`. The blake3 `Hash::as_bytes()` returns `&[u8; 32]` directly — no hex round-trip needed for the incoming hash.
**Warning signs:** Any `==` or `.eq()` on a variable holding a hash value.

### Pitfall 3: Raw Token Stored or Logged
**What goes wrong:** `tracing::info!("Created key {}: {}", id, raw_token)` puts the full key in log output.
**Why it happens:** Standard debug logging patterns.
**How to avoid:** Log only `key_id` and `agent_id`. Never format `raw_token` in any tracing call. The raw token must be returned from `create()` and immediately discarded after the caller prints it.
**Warning signs:** Any `tracing::*!` macro that references the raw token variable.

### Pitfall 4: Revoke Returns Error for Non-Existent ID
**What goes wrong:** If `execute()` returns 0 rows_affected and the code returns `Err("not found")`, CLI commands become fragile.
**Why it happens:** "No rows changed = nothing happened = error" feels natural.
**How to avoid:** D-15 is explicit — return `Ok(())` regardless of rows_affected. Revoking a non-existent or already-revoked key is a no-op.
**Warning signs:** Any check on `c.execute(...)?` return value (rows_affected) in `revoke()`.

### Pitfall 5: validate() Returns AuthContext for Revoked Key
**What goes wrong:** `SELECT ... WHERE hashed_key = ?1` without `AND revoked_at IS NULL` returns a revoked key's row. Code checks `revoked_at` after the fact and returns an error, but the DB query itself succeeded — timing side channel on the extra conditional check.
**Why it happens:** "Check revocation separately" is logically equivalent but subtly different.
**How to avoid:** Include `AND revoked_at IS NULL` directly in the SQL `WHERE` clause. A single no-rows result covers both "not found" and "revoked" cases (D-08, D-10).

### Pitfall 6: hex Decode Error in validate() not Handled
**What goes wrong:** `stored_hex` from the DB might be malformed if a row was inserted incorrectly. `hex::decode_to_slice` returning `Err` is not a rusqlite error and needs explicit handling inside `conn.call()`.
**Why it happens:** Happy path testing never hits this branch.
**How to avoid:** Handle the hex decode error as a DB error with a descriptive message, or use the blake3 `Hash::from_hex()` constructor which handles validation. Alternatively: since the stored hash is always written by `create()` using `blake3::hash(...).to_hex()`, it is always valid hex. A check is still good practice.

---

## Code Examples

### OsRng byte generation (rand 0.9 / rand_core 0.9)
```rust
// Source: docs.rs/rand/0.9.1/rand/rngs/struct.OsRng
use rand_core::{OsRng, TryRngCore};

let mut bytes = [0u8; 32];
OsRng.try_fill_bytes(&mut bytes).expect("OsRng entropy unavailable");
let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
let raw_token = format!("mnk_{}", hex);  // "mnk_<64 hex chars>" — D-01
```

### BLAKE3 hashing and hex output
```rust
// Source: docs.rs/blake3/1.8.3/blake3/
let hash = blake3::hash(raw_token.as_bytes());
let hashed_key: String = hash.to_hex().to_string();     // 64-char hex — stored in DB
let hash_bytes: &[u8; 32] = hash.as_bytes();             // raw bytes — used for ct comparison
let display_id: String = hashed_key[..8].to_string();   // first 8 chars — D-06
```

### Constant-time comparison
```rust
// Source: docs.rs/constant_time_eq/0.4.2/constant_time_eq/fn.constant_time_eq_32.html
use constant_time_eq::constant_time_eq_32;

let incoming_bytes: [u8; 32] = *blake3::hash(raw_token.as_bytes()).as_bytes();
let stored_bytes: [u8; 32] = *blake3::Hash::from_hex(&stored_hex)
    .expect("stored hash is always valid")
    .as_bytes();

if constant_time_eq_32(&incoming_bytes, &stored_bytes) {
    // authorized
}
```

### conn.call() pattern (from existing count_active_keys)
```rust
// Source: src/auth.rs — count_active_keys() reference implementation
self.conn
    .call(|c| -> Result<T, rusqlite::Error> {
        // all rusqlite operations here
        Ok(result)
    })
    .await
    .map_err(crate::error::DbError::from)
```

### Removing #[allow(dead_code)] annotations
Phase 10 added `#[allow(dead_code)]` to all four stub methods. After implementing each method, remove its annotation:
```rust
// Before (Phase 10 stub):
#[allow(dead_code)]
pub async fn create(...) -> ... { todo!() }

// After (Phase 11 implementation):
pub async fn create(...) -> ... { /* real impl */ }
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| SHA-256 for API key hashing | BLAKE3 (D-04 locked) | BLAKE3 stable 2021 | Faster on all platforms, pure Rust, simpler API |
| `subtle::ConstantTimeEq` | `constant_time_eq::constant_time_eq_32()` (D-05 locked) | Both are current | `constant_time_eq` is simpler for fixed-size arrays; `subtle` is broader scope |
| rand 0.8 `thread_rng().gen::<[u8; 32]>()` | rand 0.9 `OsRng.try_fill_bytes()` | rand 0.9 released 2024 | OsRng is explicit about entropy source; `thread_rng` seeds from OS but is less auditable |

**Deprecated/outdated:**
- `sha2` crate for API key hashing: The architecture docs mention SHA-256 in some places, but D-04 locks BLAKE3. Do not add the `sha2` crate — this phase uses `blake3` only.
- `rand::thread_rng().gen()`: Functionally equivalent but D-02 specifies OsRng explicitly for auditability.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[tokio::test]` (via tokio 1.x) |
| Config file | None — `cargo test` discovers tests automatically |
| Quick run command | `cargo test --test integration 2>&1 | tail -30` |
| Full suite command | `cargo test 2>&1 | tail -50` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| KEY-01 | create() returns (ApiKey, raw_token); raw_token starts with mnk_; DB has hashed_key not raw token | unit (inline `#[cfg(test)]` in auth.rs) | `cargo test --test integration key_service` | ❌ Wave 0 |
| KEY-01 | display_id = first 8 hex chars of BLAKE3(raw_token), not raw_token prefix | unit | `cargo test auth::tests::test_display_id_is_hash_derived` | ❌ Wave 0 |
| KEY-02 | list() returns all keys ordered by created_at DESC; never returns hashed_key | unit | `cargo test auth::tests::test_list_returns_all_keys` | ❌ Wave 0 |
| KEY-03 | revoke() sets revoked_at; subsequent validate() returns error | unit | `cargo test auth::tests::test_revoke_prevents_validate` | ❌ Wave 0 |
| KEY-03 | revoke() is idempotent — revoking non-existent key returns Ok(()) | unit | `cargo test auth::tests::test_revoke_idempotent` | ❌ Wave 0 |
| KEY-04 | validate() returns AuthContext with correct allowed_agent_id | unit | `cargo test auth::tests::test_validate_returns_auth_context` | ❌ Wave 0 |
| INFRA-02 | constant_time_eq_32 used (not == on hash values) | code review + unit | `cargo test auth::tests::test_constant_time_comparison` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test --lib -- auth` (unit tests inline in auth.rs)
- **Per wave merge:** `cargo test 2>&1 | tail -50`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `src/auth.rs` `#[cfg(test)]` module — covers KEY-01 through KEY-04 and INFRA-02
  - `test_create_returns_raw_token` — KEY-01
  - `test_display_id_is_hash_derived` — KEY-01 / Auth Pitfall 7
  - `test_create_stores_hash_not_raw` — KEY-01 / Auth Pitfall 2
  - `test_list_returns_all_keys` — KEY-02
  - `test_list_excludes_hashed_key` — KEY-02
  - `test_revoke_prevents_validate` — KEY-03
  - `test_revoke_idempotent` — KEY-03 / D-15
  - `test_validate_returns_auth_context` — KEY-04
  - `test_validate_rejects_wrong_token` — INFRA-02
  - `test_validate_rejects_revoked_key` — KEY-03 + D-10

---

## Open Questions

1. **hex crate for raw bytes encoding**
   - What we know: `blake3::Hash::to_hex()` handles hash hex. For the raw token's 32-byte body, the project has no `hex` crate.
   - What's unclear: Is `format!("{:02x}", b)` in a collected iterator acceptable, or should `hex = "0.4"` be added?
   - Recommendation: The `format!` approach works and avoids a dependency. Use `bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>()`. If it feels verbose, add `hex = "0.4"`.

2. **hex decode in validate() for stored_bytes**
   - What we know: `blake3::Hash::from_hex(stored_hex)` can reconstruct a `Hash` from a hex string, then `.as_bytes()` gives `[u8; 32]`. This avoids needing the `hex` crate.
   - Recommendation: Use `blake3::Hash::from_hex(&stored_hex).unwrap().as_bytes()` inside the `conn.call()` closure. The stored hash is always written by `create()` using `blake3::hash(...).to_hex()`, so it is always valid — `.unwrap()` is safe or can be mapped to a rusqlite error.

3. **rand 0.9 vs 0.10 OsRng API**
   - What we know: `rand` 0.10.0 exists. In rand 0.9, `OsRng` is at `rand::rngs::OsRng` with `os_rng` feature, using `TryRngCore::try_fill_bytes`. The API may differ in 0.10.
   - Recommendation: Pin to `rand = "0.9"` matching CONTEXT.md D-19 intent. If 0.10 is preferred, verify the OsRng import path before committing.

---

## Sources

### Primary (HIGH confidence)
- `src/auth.rs` — Existing struct definitions and `count_active_keys()` reference pattern; `todo!()` stubs to implement
- `src/db.rs` — `api_keys` DDL with column names; `conn.call()` pattern
- `src/error.rs` — `DbError` variants; `ApiError::Unauthorized` already defined
- `Cargo.toml` — Existing dependencies; baseline for new additions
- `tests/integration.rs` — Test infrastructure patterns (build_test_state, conn.call usage)
- docs.rs/blake3/1.8.3 — `hash()`, `Hash::to_hex()`, `Hash::as_bytes()`, `Hash::from_hex()` API
- docs.rs/constant_time_eq/0.4.2 — `constant_time_eq_32(a: &[u8; 32], b: &[u8; 32]) -> bool` signature
- docs.rs/rand/0.9.1 — `OsRng.try_fill_bytes()` with `rand_core::TryRngCore` trait

### Secondary (MEDIUM confidence)
- `.planning/research/PITFALLS.md` §Auth Pitfalls 1, 2, 7, 9 — timing attack, plaintext storage, prefix display, no-cache policy
- `.planning/research/ARCHITECTURE.md` — KeyService position, `conn.call()` pattern, build order

### Tertiary (LOW confidence)
- None

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — crate versions verified via `cargo search` 2026-03-20; APIs verified via docs.rs
- Architecture: HIGH — implementation is filling in stubs in existing code; patterns verified from working `count_active_keys()` reference
- Pitfalls: HIGH — sourced from `.planning/research/PITFALLS.md` which cites CVE-2025-59425 and official crate docs

**Research date:** 2026-03-20
**Valid until:** 2026-06-20 (blake3, constant_time_eq are stable; rand 0.9 is pinned; 90-day validity)
