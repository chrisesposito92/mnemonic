# Phase 29: StorageBackend Routing Fix - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-22
**Phase:** 29-storagebackend-routing-fix
**Areas discussed:** Init tier, Signature refactor, Total count, Test strategy
**Mode:** --auto (all decisions auto-selected)

---

## Init tier for recall

| Option | Description | Selected |
|--------|-------------|----------|
| init_db() + create_backend() | Create backend after DB init, no embedding model | :white_check_mark: |
| Full init_db_and_embedding() | Load embedding model too (MemoryService) | |
| Lazy backend creation | Only create backend on first call | |

**User's choice:** [auto] init_db() + create_backend() (recommended default)
**Notes:** Recall needs backend but not embedding. For SQLite this is virtually free. For remote backends it adds a connection but that's fundamental to making recall work across backends.

---

## Signature refactor approach

| Option | Description | Selected |
|--------|-------------|----------|
| Arc<dyn StorageBackend> directly | Minimal interface — list + get_by_id only | :white_check_mark: |
| MemoryService | Full service with embedding — overkill for recall | |
| Generic over backend type | Compile-time dispatch — unnecessary complexity | |

**User's choice:** [auto] Arc<dyn StorageBackend> directly (recommended default)
**Notes:** Recall doesn't do embedding or search. Arc<dyn StorageBackend> is the minimal interface. MemoryService would add 2-3s startup for unused embedding model.

---

## Total count for list

| Option | Description | Selected |
|--------|-------------|----------|
| Use ListResponse.total from trait | Every backend already computes total | :white_check_mark: |
| Separate count query on backend | Extra round-trip, duplicates trait work | |
| Remove footer count entirely | Loses useful "Showing X of Y" info | |

**User's choice:** [auto] Use ListResponse.total from trait (recommended default)
**Notes:** ListResponse already includes total from every backend. No separate query needed.

---

## Test strategy

| Option | Description | Selected |
|--------|-------------|----------|
| SQLite-only with SqliteBackend | Verify CLI delegates to trait; backend routing tested elsewhere | :white_check_mark: |
| Multi-backend parameterized tests | Test recall against all 3 backends | |
| Mock StorageBackend | Verify trait method calls directly | |

**User's choice:** [auto] SQLite-only with SqliteBackend (recommended default)
**Notes:** StorageBackend routing is already proven by phases 21-24 tests. Phase 29 only needs to verify recall correctly delegates to the trait. SQLite as test backend keeps tests fast with zero external deps.

---

## Claude's Discretion

- Whether to create a dedicated `init_recall()` helper or inline backend creation in main.rs
- Exact parameter mapping from RecallArgs to ListParams
- Internal file organization of refactored functions

## Deferred Ideas

None — discussion stayed within phase scope.
