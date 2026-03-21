# Phase 11: KeyService Core - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-20
**Phase:** 11-keyservice-core
**Areas discussed:** Token generation, Validation error semantics, List behavior, Revocation edge cases
**Mode:** Auto (--auto flag — recommended defaults selected automatically)

---

## Token Generation

| Option | Description | Selected |
|--------|-------------|----------|
| 32 bytes hex (mnk_ + 64 chars) | 256-bit entropy, standard practice, familiar format | ✓ |
| 24 bytes base64url | Shorter token, URL-safe encoding | |
| UUID v4 raw | Simpler but only 122-bit entropy | |

**User's choice:** [auto] 32 random bytes hex-encoded → `mnk_<64 hex chars>` (recommended default)
**Notes:** `rand::rngs::OsRng` for cryptographic randomness. 256-bit entropy is industry standard for API keys.

---

## Validation Error Semantics

| Option | Description | Selected |
|--------|-------------|----------|
| Descriptive messages, single error type | "key not found" / "key revoked" messages in DbError, both → 401 | ✓ |
| Separate error variants per cause | KeyNotFound, KeyRevoked, ScopeMismatch variants | |
| Opaque "unauthorized" for all cases | No information leak, but harder to debug | |

**User's choice:** [auto] Descriptive messages within DbError (recommended default)
**Notes:** Scope enforcement deferred to Phase 13. validate() returns AuthContext with allowed_agent_id for handler-layer scope checking.

---

## List Behavior

| Option | Description | Selected |
|--------|-------------|----------|
| All keys (active + revoked) | Preserves audit trail, matches soft-delete decision D-05 | ✓ |
| Active only | Simpler, but loses visibility into revoked keys | |

**User's choice:** [auto] All keys, ordered by created_at DESC (recommended default)
**Notes:** Consistent with Phase 10 decision D-05 (soft delete via revoked_at). Never returns raw token or hashed_key.

---

## Revocation Edge Cases

| Option | Description | Selected |
|--------|-------------|----------|
| Idempotent Ok(()) | Non-existent or already-revoked → Ok, no error | ✓ |
| Error on not-found | Return NotFound if key ID doesn't exist | |
| Error on already-revoked | Return specific error if already revoked | |

**User's choice:** [auto] Idempotent Ok(()) for all cases (recommended default)
**Notes:** Matches Phase 10 D-05 "idempotent if revoked twice". Simplest and most robust.

---

## Claude's Discretion

- Internal helper function organization
- Unit test placement (inline vs separate file)
- SQL query structure for validation

## Deferred Ideas

None — discussion stayed within phase scope.
