# Requirements: Mnemonic

**Defined:** 2026-03-22
**Core Value:** Any AI agent can store and semantically search memories out of the box with zero configuration — just download and run

## v1.6 Requirements

Requirements for the embedded web dashboard milestone. Each maps to roadmap phases.

### Build Infrastructure

- [ ] **BUILD-01**: Dashboard assets embedded into binary at compile time via rust-embed, served at `/ui` via axum-embed with SPA fallback
- [ ] **BUILD-02**: Dashboard feature-gated behind `dashboard` Cargo feature with zero impact on default binary
- [ ] **BUILD-03**: CI release workflow updated with Node.js build step before cargo build; separate job verifies default binary still passes all tests

### Data Browsing

- [ ] **BROWSE-01**: User can view a paginated list of memories showing content preview, agent_id, session_id, tags, and created_at
- [ ] **BROWSE-02**: User can filter memory list by agent_id, session_id, and tag
- [ ] **BROWSE-03**: User can perform semantic search from the dashboard and see ranked results with distance scores
- [ ] **BROWSE-04**: User can expand a memory row to see full content and metadata
- [ ] **BROWSE-05**: User can view per-agent memory counts and last-active timestamps via agent breakdown table (requires new `GET /stats` endpoint)

### Operations

- [ ] **OPS-01**: Dashboard header shows health indicator with active storage backend name from `GET /health`
- [ ] **OPS-02**: User can trigger compaction with dry-run preview showing before/after memory mapping, then confirm to execute

### Auth & Security

- [ ] **AUTH-01**: Dashboard detects auth mode via 401 response, prompts for `mnk_...` bearer token, stores in-memory only (never localStorage)
- [ ] **AUTH-02**: All `/ui/` responses include Content-Security-Policy header to prevent XSS

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Dashboard Enhancements

- **DASH-01**: User can delete individual memories from the dashboard with confirmation modal
- **DASH-02**: User can view 2D vector visualization (UMAP projection) of memory embeddings
- **DASH-03**: User can toggle between dark and light theme
- **DASH-04**: User can bulk delete memories with selection checkboxes

## Out of Scope

| Feature | Reason |
|---------|--------|
| Memory edit form | High corruption risk; CLI/API are the correct write paths |
| Bulk delete without undo | Dangerous; individual delete via CLI + compaction covers the use case |
| Separate frontend process | Violates single-binary constraint; embedded-at-compile-time only |
| Server-side rendering | Unnecessary for operational dashboard; SPA is sufficient |
| Real-time WebSocket updates | Polling or manual refresh sufficient for operational tool |
| Cross-backend migration | Deferred from v1.4; all backends must be stable first |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| BUILD-01 | — | Pending |
| BUILD-02 | — | Pending |
| BUILD-03 | — | Pending |
| BROWSE-01 | — | Pending |
| BROWSE-02 | — | Pending |
| BROWSE-03 | — | Pending |
| BROWSE-04 | — | Pending |
| BROWSE-05 | — | Pending |
| OPS-01 | — | Pending |
| OPS-02 | — | Pending |
| AUTH-01 | — | Pending |
| AUTH-02 | — | Pending |

**Coverage:**
- v1.6 requirements: 12 total
- Mapped to phases: 0
- Unmapped: 12

---
*Requirements defined: 2026-03-22*
*Last updated: 2026-03-22 after initial definition*
