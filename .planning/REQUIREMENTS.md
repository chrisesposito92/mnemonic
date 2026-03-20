# Requirements: Mnemonic

**Defined:** 2026-03-20
**Core Value:** Any AI agent can store and semantically search memories out of the box with zero configuration — just download and run

## v1.2 Requirements

Requirements for API key authentication milestone. Each maps to roadmap phases.

### Key Management

- [ ] **KEY-01**: Admin can create an API key with optional name and optional agent_id scope, receiving the raw key (mnk_...) exactly once
- [ ] **KEY-02**: Admin can list all API keys showing name, prefix, scope, and creation date — never the full key
- [ ] **KEY-03**: Admin can revoke a key, immediately preventing its use on subsequent requests
- [ ] **KEY-04**: API key can be scoped to a specific agent_id, restricting access to only that agent's memories

### Authentication

- [ ] **AUTH-01**: Requests with a valid Bearer token in the Authorization header are authenticated
- [ ] **AUTH-02**: Requests with an invalid or revoked token receive 401 Unauthorized
- [ ] **AUTH-03**: When no API keys exist in the database, all requests are allowed (open mode)
- [ ] **AUTH-04**: A scoped key's agent_id overrides the client-supplied agent_id, preventing cross-agent access
- [ ] **AUTH-05**: GET /health is accessible without authentication regardless of auth mode

### Infrastructure

- [x] **INFRA-01**: api_keys table is created via idempotent SQLite migration on startup
- [ ] **INFRA-02**: Key hashes use BLAKE3 with constant-time comparison to prevent timing attacks
- [x] **INFRA-03**: Server startup log announces whether running in open or authenticated mode

### CLI

- [ ] **CLI-01**: `mnemonic keys create` creates an API key and displays the raw key
- [ ] **CLI-02**: `mnemonic keys list` displays all keys with metadata
- [ ] **CLI-03**: `mnemonic keys revoke` invalidates a key by ID or prefix

## Future Requirements

Deferred to future release. Tracked but not in current roadmap.

### Key Management

- **KEY-05**: Dual-mode binary — keys subcommand opens DB only, skips model loading for instant CLI response
- **KEY-06**: Time-based weighting parameter for age-aware compaction aggressiveness (carried from v1.1)

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| JWT / OAuth tokens | Overkill for single-binary tool; API keys are sufficient |
| Rate limiting | Separate concern; can be added later without auth changes |
| Key rotation (automatic) | Manual revoke+create is sufficient for v1.2 |
| Argon2/bcrypt key hashing | API keys are high-entropy random; BLAKE3/SHA-256 is correct and fast |
| User accounts / RBAC | API keys with agent scoping covers the access model |
| Admin key vs agent key distinction | Single key type with optional scope is simpler; admin = unscoped key |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| KEY-01 | Phase 11 | Pending |
| KEY-02 | Phase 11 | Pending |
| KEY-03 | Phase 11 | Pending |
| KEY-04 | Phase 11 | Pending |
| AUTH-01 | Phase 12 | Pending |
| AUTH-02 | Phase 12 | Pending |
| AUTH-03 | Phase 12 | Pending |
| AUTH-04 | Phase 13 | Pending |
| AUTH-05 | Phase 12 | Pending |
| INFRA-01 | Phase 10 | Complete |
| INFRA-02 | Phase 11 | Pending |
| INFRA-03 | Phase 10 | Complete |
| CLI-01 | Phase 14 | Pending |
| CLI-02 | Phase 14 | Pending |
| CLI-03 | Phase 14 | Pending |

**Coverage:**
- v1.2 requirements: 15 total
- Mapped to phases: 15
- Unmapped: 0

---
*Requirements defined: 2026-03-20*
*Last updated: 2026-03-20 after roadmap creation*
