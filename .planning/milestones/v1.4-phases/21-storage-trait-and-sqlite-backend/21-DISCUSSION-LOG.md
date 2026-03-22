# Phase 21: Storage Trait and SQLite Backend - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md -- this log preserves the alternatives considered.

**Date:** 2026-03-21
**Phase:** 21-storage-trait-and-sqlite-backend
**Areas discussed:** Trait method surface, Module organization, Compact audit log placement
**Mode:** --auto (all decisions auto-selected with recommended defaults)

---

## Trait Method Surface

| Option | Description | Selected |
|--------|-------------|----------|
| One method per REST operation | Mirror existing MemoryService/CompactionService method boundaries | Yes |
| Coarse-grained (CRUD only) | Fewer methods, compaction uses raw CRUD | |
| Fine-grained (SQL-level) | Expose query builders, too leaky | |

**User's choice:** [auto] One method per REST operation (recommended default)
**Notes:** Mirrors existing code structure, minimizes refactoring surface, and gives backends enough granularity to optimize (e.g., Qdrant can use native scroll API in fetch_candidates)

---

## Module Organization

| Option | Description | Selected |
|--------|-------------|----------|
| src/storage/ module tree | mod.rs for trait, sqlite.rs for backend, future files for other backends | Yes |
| Single src/storage.rs file | All trait + SqliteBackend in one file | |
| Inline in existing files | Add trait to service.rs, keep SQLite code in place | |

**User's choice:** [auto] src/storage/ module tree (recommended default)
**Notes:** Follows Rust convention for trait + multiple implementations. Phases 23 and 24 will add qdrant.rs and postgres.rs to this same directory.

---

## Compact Audit Log Placement

| Option | Description | Selected |
|--------|-------------|----------|
| Separate from StorageBackend | CompactionService keeps own SQLite connection for compact_runs | Yes |
| Part of StorageBackend trait | Every backend must implement audit log methods | |
| No audit log in trait, optional companion | Backends optionally provide audit, SQLite does it natively | |

**User's choice:** [auto] Separate from StorageBackend (recommended default)
**Notes:** Per STATE.md, Qdrant backend will use a companion SQLite file for audit logs. Making audit part of the trait would force non-relational backends to implement SQL-like tables. Keeping it separate is cleaner.

---

## Claude's Discretion

- Exact method signatures and error types
- Dual-table insert/delete handling inside SqliteBackend
- StorageError type design
- Test refactoring approach
- Method naming conventions

## Deferred Ideas

None.
