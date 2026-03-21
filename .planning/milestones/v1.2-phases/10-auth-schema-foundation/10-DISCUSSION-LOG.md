# Phase 10: Auth Schema Foundation - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-20
**Phase:** 10-auth-schema-foundation
**Areas discussed:** Schema columns, 401 error response, Startup auth log, Module skeleton scope

---

## Schema Columns

### Hash algorithm
| Option | Description | Selected |
|--------|-------------|----------|
| BLAKE3 (Recommended) | Faster, pure Rust via blake3 crate, 32-byte output. Aligns with STATE.md decision. | ✓ |
| SHA-256 | Industry standard, familiar. sha2 crate. Architecture research already has examples. | |

**User's choice:** BLAKE3
**Notes:** STATE.md already decided BLAKE3; ARCHITECTURE.md references needed updating.

### Display ID
| Option | Description | Selected |
|--------|-------------|----------|
| Hash-derived (Recommended) | First 8 hex chars of BLAKE3(key). No part of raw key stored. Matches Stripe/GitHub. | ✓ |
| Separate random ID | Generate short random string independent of key. Zero correlation. | |
| UUID v7 only | Use UUID primary key as only identifier. No short display ID column. | |

**User's choice:** Hash-derived
**Notes:** Aligns with STATE.md blocker note about Auth Pitfall 7.

### Scope NULL semantics
| Option | Description | Selected |
|--------|-------------|----------|
| NULL (Recommended) | NULL = wildcard. Matches SQL conventions. Clean and explicit. | ✓ |
| Sentinel '*' | Store '*' for wildcard. Simpler WHERE clause but introduces magic string. | |

**User's choice:** NULL

### Revocation strategy
| Option | Description | Selected |
|--------|-------------|----------|
| revoked_at (Recommended) | Soft delete via timestamp. Preserves audit trail. Idempotent. | ✓ |
| Hard delete | DELETE FROM api_keys. Simpler schema. No audit trail. | |
| status column | TEXT 'active'/'revoked'. More explicit than NULL checks. | |

**User's choice:** revoked_at

---

## 401 Error Response

### Response body detail
| Option | Description | Selected |
|--------|-------------|----------|
| Detailed with hint (Recommended) | { "error", "auth_mode", "hint" }. Helps debugging. Agents can parse. | ✓ |
| Minimal | { "error": "unauthorized" }. No info leakage. | |
| Error code + message | { "error", "error_code" }. Machine-parseable, no auth_mode hint. | |

**User's choice:** Detailed with hint

### Error enum design
| Option | Description | Selected |
|--------|-------------|----------|
| Separate variant (Recommended) | ApiError::Unauthorized(String). Clean match arm. | ✓ |
| Two variants: Unauthorized + Forbidden | Add both now for 401 and 403. | |
| Single Unauthorized only | Only 401 now. 403 in Phase 13. | |

**User's choice:** Separate variant (Unauthorized only now)

### Trace ID
| Option | Description | Selected |
|--------|-------------|----------|
| No trace ID (Recommended) | Keep simple. Consistent with existing errors. | ✓ |
| Include trace ID | X-Request-Id header + body field. | |

**User's choice:** No trace ID

---

## Startup Auth Log

### Log verbosity
| Option | Description | Selected |
|--------|-------------|----------|
| Mode + actionable hint (Recommended) | "Auth: OPEN (no keys) — run 'mnemonic keys create' to enable". One line. | ✓ |
| Just the mode | "Auth mode: open". Minimal. | |
| Verbose with details | Full status dump including scoped agents. | |

**User's choice:** Mode + actionable hint

### Log level
| Option | Description | Selected |
|--------|-------------|----------|
| INFO (Recommended) | Always visible. Matches existing startup messages. | ✓ |
| WARN for open, INFO for active | Open mode draws attention via WARN. | |

**User's choice:** INFO for both

### Check timing
| Option | Description | Selected |
|--------|-------------|----------|
| Startup (Recommended) | Query key count during init, log once. | ✓ |
| First request | Defer until first HTTP request. | |

**User's choice:** Startup

---

## Module Skeleton Scope

### Skeleton depth
| Option | Description | Selected |
|--------|-------------|----------|
| Types + stubs (Recommended) | AuthContext, ApiKey, KeyService with todo!() method signatures. | ✓ |
| Types only | Just structs. No KeyService. | |
| Full signatures + error types | All structs, AuthError, auth_middleware signature. Maximum scaffolding. | |

**User's choice:** Types + stubs

### Crypto helpers
| Option | Description | Selected |
|--------|-------------|----------|
| Leave to Phase 11 (Recommended) | Phase 10 is foundation, not implementation. | ✓ |
| Include as stubs | fn blake3_hex() { todo!() } etc. | |

**User's choice:** Leave to Phase 11

### AppState change
| Option | Description | Selected |
|--------|-------------|----------|
| Add to AppState now (Recommended) | key_service: Arc<KeyService>. Avoids later breakage. | ✓ |
| Defer to Phase 12 | AppState unchanged until middleware phase. | |

**User's choice:** Add now

### Count function
| Option | Description | Selected |
|--------|-------------|----------|
| Real count function (Recommended) | KeyService::count_active_keys() queries DB. Used by startup log. | ✓ |
| Hardcode for now | Just log 'OPEN' since no keys can exist yet. | |

**User's choice:** Real count function

---

## Claude's Discretion

- Exact column ordering in DDL
- Whether to add `last_used_at` now or defer
- Index strategy beyond `hashed_key`
- Exact wording of startup log messages

## Deferred Ideas

None — discussion stayed within phase scope.
