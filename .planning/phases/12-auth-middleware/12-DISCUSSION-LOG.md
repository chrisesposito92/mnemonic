# Phase 12: Auth Middleware - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-21
**Phase:** 12-auth-middleware
**Areas discussed:** Middleware placement, Open mode implementation, Health check exemption, Malformed header handling
**Mode:** Auto (all decisions auto-selected based on recommended defaults and prior phase decisions)

---

## Middleware Placement

| Option | Description | Selected |
|--------|-------------|----------|
| route_layer() on protected routes | Only applies auth to routes in the same Router segment — unmatched routes get 404, not 401 | ✓ |
| layer() on entire Router | Applies to all routes including unmatched — returns 401 before 404 | |
| Separate Router merge | Two Routers merged — public and protected | |

**User's choice:** route_layer() on protected routes (auto-selected — carried from STATE.md decision)
**Notes:** Prior decision in STATE.md explicitly chose `route_layer()` not `layer()` to prevent 401 on unmatched routes.

---

## Open Mode Implementation

| Option | Description | Selected |
|--------|-------------|----------|
| Per-request COUNT(*) query | Call count_active_keys() on every request — handles live key creation/revocation | ✓ |
| Cached count with invalidation | Cache key count, invalidate on create/revoke — fewer DB queries | |
| Startup flag | Check once at boot, require restart to change — simplest but inflexible | |

**User's choice:** Per-request COUNT(*) query (auto-selected — carried from STATE.md decision)
**Notes:** Prior decision in STATE.md explicitly chose per-request COUNT over startup flag. Already implemented as `count_active_keys()`.

---

## Health Check Exemption

| Option | Description | Selected |
|--------|-------------|----------|
| Separate route group | /health registered before route_layer() — structurally exempt | ✓ |
| Middleware skip logic | Check path inside middleware, skip auth for /health | |
| Custom extractor | Per-route opt-in via extractor — more flexible but more boilerplate | |

**User's choice:** Separate route group (auto-selected — recommended, cleanest with route_layer() approach)
**Notes:** axum's route_layer() only applies to routes in the same segment — registering /health before the layer means it never hits auth middleware.

---

## Malformed Header Handling

| Option | Description | Selected |
|--------|-------------|----------|
| 400 for malformed, 401 for missing/invalid | Distinguishes client format errors from auth failures | ✓ |
| 401 for all auth failures | Simpler, but conflates different error types | |
| 400 for malformed, ignore missing | Only enforces when header is present — doesn't match auth-active behavior | |

**User's choice:** 400 for malformed, 401 for missing/invalid (auto-selected — per success criteria #5)
**Notes:** Phase 12 success criteria #5 explicitly requires malformed headers to return 400, not panic or 500.

---

## Claude's Discretion

- Middleware function signature and internal control flow
- Header parsing helper extraction
- Test organization
- `#[allow(dead_code)]` cleanup timing

## Deferred Ideas

None.
