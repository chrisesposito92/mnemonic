---
phase: 32
reviewers: [gemini, codex]
reviewed_at: 2026-03-22T00:00:00Z
plans_reviewed: [32-01-PLAN.md, 32-02-PLAN.md]
---

# Cross-AI Plan Review — Phase 32

## Gemini Review

### Summary
The implementation plans for Phase 32 are high-quality, technically sound, and align strictly with the established architectural patterns of the Mnemonic project. The transition from backend capability (GET /memories/{id}) to the 7-state UI machine in the dashboard follows a logical progression. The use of a mandatory dry-run flow with tree-drawing previews provides a professional, "CLI-native" feel within the web interface. The plans successfully address the core requirement of safe, user-verified memory compaction.

### Strengths
- **Security Consistency**: Plan 32-01 explicitly clones the scope enforcement pattern from delete_memory_handler, ensuring that scoped API keys cannot leak memory content from other agents via the new GET endpoint.
- **Robust State Management**: The 7-state machine (idle through error) for CompactTab is well-conceived and covers all async transitions, including the ability to discard/abort mid-fetch.
- **Resource Management**: The use of AbortController for parallel memory fetches is an excellent detail that prevents race conditions and stale state if a user quickly navigates away or discards a preview.
- **Visual Polish**: The decision to use Unicode box-drawing characters for the cluster tree maintains the project's "terminal-native" aesthetic even in a browser context.
- **Atomic Updates**: The strategy in Plan 32-02 Task 2 to update the routing, types, and UI in a single atomic wave prevents intermediate broken builds.

### Concerns
- **MEDIUM — Network Congestion**: D-08 requires fetching each source memory by ID for the preview. If a dry-run identifies 50+ memories to be compacted, the dashboard will fire 50+ concurrent HTTP requests. While AbortController handles the cleanup, this could cause temporary UI stutter or hit browser connection limits.
- **LOW — Empty State Requirement Gap**: The Roadmap requires all dashboard views to be verified for empty states. While the Research (Finding 3) claims an audit found no gaps, the implementation plans focus almost exclusively on the new CompactTab. There is no explicit task to "verify/fix" the other tabs.
- **LOW — Input Validation**: parseFloat() is mentioned for the threshold, but the plan doesn't explicitly define behavior for NaN or values outside the valid 0.0-1.0 range before the API call is made.

### Suggestions
- For the parallel fetches in handleDryRun, consider using a simple concurrency limit (e.g., fetching in chunks of 5-10) or add a "Batch Get" endpoint.
- Add a "Verification Step" at the end of Plan 32-02 to specifically test the "Empty State" requirement for Memories, Agents, and Search tabs.
- Add a simple UI validation check in CompactTab to disable the "Run Dry Run" button if the threshold is not a valid number between 0 and 1.
- Ensure the 80-character truncation happens client-side for ClusterPreview component performance.

### Risk Assessment
**LOW** — The technical complexity is low because it builds on existing, proven patterns. The primary risks are performance-related (parallel fetches) and minor requirement omissions.

**Verdict**: PASS — Proceed with execution.

---

## Codex Review

### Plan 32-01

#### Summary
This is a good, low-surface-area backend change. The route belongs where the plan puts it, and the service/backend already have the right primitive for it. The main weakness is not the implementation shape, but the validation strategy: the plan under-specifies auth coverage and puts core API tests in a dashboard-only test target.

#### Strengths
- Reuses the existing get_by_id backend capability rather than inventing a second retrieval path.
- Follows the existing ownership-check pattern already used for delete, which keeps scope enforcement consistent.
- Adds the exact client wrappers Phase 32 needs, without pulling in extra packages.

#### Concerns
- **HIGH — Tests in Wrong Suite**: /memories/{id} is a core API route in the main router, not a dashboard-only feature, so testing it only under tests/dashboard_integration.rs leaves the default build under-covered.
- **MEDIUM — Incomplete Security Coverage**: The plan only mentions 200 and 404, but this endpoint needs the same scoped-key 403 and scoped-owner 200 coverage already present for delete.
- **MEDIUM — UnauthorizedError Propagation**: fetchMemoryById will inherit the current 401/403 -> UnauthorizedError behavior, which may be wrong for the Compact tab's inline-error flow where individual memory fetches failing should degrade gracefully.

#### Suggestions
- Move the new endpoint tests into tests/integration.rs, and leave tests/dashboard_integration.rs for /ui-specific coverage.
- Add scoped-key forbidden and scoped-key own-memory tests alongside the existing delete auth cases.
- Split 403 from 401 in the dashboard client before wiring fetchMemoryById into the preview flow.

#### Risk Assessment
**MEDIUM** — The code change itself is straightforward, but the plan is light on the auth cases that matter most.

### Plan 32-02

#### Summary
This plan is directionally correct and matches the project's additive UI style, but it is missing several correctness details around the dry-run/confirm contract. As written, it could ship a UI that compiles and mostly works, while still misleading users about what will actually be compacted.

#### Strengths
- Correctly depends on 32-01 instead of duplicating backend/API work.
- Keeps the change mostly additive: new components plus minimal tab/router wiring.
- Uses the established mount-per-tab pattern, which already satisfies the "refresh on next navigation" decision.
- Calls out abort handling explicitly, which is important for this flow.

#### Concerns
- **HIGH — Preview Not Frozen on Input Change**: The plan does not freeze or invalidate the preview when agent or threshold changes. Since execute is a second POST that recomputes compaction server-side, users can confirm a different operation than the one they reviewed.
- **HIGH — Blank agent_id Ambiguity**: Blank agent_id is a real stored value today, but a normal select uses '' as "unselected/all". That makes the (none) namespace ambiguous or unselectable for compaction.
- **MEDIUM — Summary Line Field Mismatch**: In dry-run mode, memories_created is always 0, so "K compacted" cannot come directly from that field. Must derive from clusters_found and id_mapping.length.
- **MEDIUM — Promise.all Fragility**: "Fallback to raw memory ID if fetch failed" requires partial-failure handling. If implementation uses Promise.all, one failed memory fetch will collapse the whole preview instead of degrading gracefully.
- **MEDIUM — Truncated Flag Ignored**: The plan ignores the truncated field, even though the preview can be partial and max_candidates is intentionally hidden. That makes the dry-run diff potentially misleading.

#### Suggestions
- Lock agent and threshold once a preview exists, or automatically discard the preview when either input changes.
- Use a sentinel value for (none) in the dropdown instead of raw '', and translate it explicitly before the API call.
- Build preview hydration with Promise.allSettled and preserve partial previews when individual memory fetches fail.
- Derive the dry-run summary from clusters_found and id_mapping.length, not memories_created.
- Surface a visible warning when truncated === true.
- Explicitly document that D-12 overrides the roadmap's "error boundary" wording.

#### Risk Assessment
**HIGH** — The structure is fine, but the current plan leaves too much ambiguity around preview correctness, and that is the core requirement of the phase.

---

## Consensus Summary

### Agreed Strengths
- **Consistent security patterns**: Both reviewers praised the scope enforcement mirroring the existing delete handler pattern (Gemini: "Security Consistency", Codex: "ownership-check pattern").
- **AbortController for lifecycle management**: Both highlighted the explicit abort handling for parallel fetches and state cleanup as a strong design choice.
- **Additive, pattern-following architecture**: Both noted the plans correctly build on established patterns rather than inventing new abstractions.

### Agreed Concerns
- **Parallel fetch performance/resilience** (MEDIUM): Gemini flagged network congestion for 50+ concurrent requests; Codex flagged Promise.all fragility where one failed fetch collapses the preview. Both agree the parallel fetch strategy needs hardening — either concurrency limiting (Gemini) or Promise.allSettled (Codex).
- **Input validation gaps** (LOW-MEDIUM): Gemini flagged threshold validation (NaN, out-of-range); Codex flagged the blank agent_id ambiguity. Both agree form inputs need tighter guards.

### Divergent Views
- **Test placement**: Codex rated the test suite location as HIGH concern (core API tests in dashboard-only suite), while Gemini did not flag this. Worth investigating — Codex's point about default build coverage is valid.
- **Preview correctness**: Codex raised two HIGH concerns (preview not frozen on input change, blank agent_id ambiguity) that Gemini did not address. These are the sharpest findings in the review and deserve attention before execution.
- **Overall risk**: Gemini assessed LOW overall risk and recommended proceeding. Codex assessed MEDIUM for Plan 01 and HIGH for Plan 02, specifically around preview correctness. The divergence suggests Plan 02 needs targeted amendments before execution.
- **Summary line accuracy**: Codex identified that memories_created is 0 in dry-run mode, making the planned summary text misleading. Gemini did not catch this. This is a factual correctness issue that must be verified against the actual API response.
- **Truncated flag**: Codex flagged that ignoring truncated could mislead users when max_candidates caps the preview. Gemini did not mention this. Worth adding a truncation warning to the UI.
