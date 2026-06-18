You are an independent Codex reviewer for a small TypeScript refactor in the EngawaCAD `/3ailoop` self-hosting tooling.

## Issue context
- Issue #230: `refactor(3ailoop): loop-context-bootstrap 差分モード + gh issue list cache (token 20-30% 削減)`
- File: `.claude/skills/3ailoop/scripts/loop-context-bootstrap.ts` (+ companion `loop-context-bootstrap.test.ts`)
- The loop script runs at the start of every /3ailoop cycle and dumps ROADMAP / CLAUDE.md / Memory / dashboard / state.json / open-issue snapshot.
- New: cache layer at `features/.loop/issue-cache.json` (last gh fetch) and `features/.loop/context-cache.json` (per-file mtime+sha). Default mode emits diffs only; `--full` bypasses cache.

## What to evaluate

1. **Correctness of diff logic** (`diffIssues`, `checkFileChange`)
   - Are added / closed / labels-changed buckets disjoint and exhaustive?
   - Does label-change detection survive non-deterministic label order from `gh`?
   - Does sha collision risk matter for 12-hex first chars (the cache key)?
2. **Cache file safety**
   - What happens if `issue-cache.json` is corrupted? (Should fall back to cold-start.)
   - Concurrent writes from parallel loop runs — likely already handled by L-0 loop-lock but is anything in the script that would race?
3. **Backwards-compat / regressions**
   - Existing callers may pass no flag and expect the old full output. Verify that diff-mode degradation is acceptable: classification summary is still emitted, just compressed.
4. **Token-budget impact**
   - Smoke run shows 19.2KB → 0.9KB (≈ -95%). Is anything important truncated (e.g., gate breakdown disappears in diff mode if no changes)?
5. **Code quality** — naming, error handling, unnecessary abstractions.

Return verdict in this format:
```
verdict: pass|fail
severity: low|medium|high|critical
  - <one-line finding>
  - ...
notes:
  <any extra context>
```

Only emit `verdict: fail` if there is a high/critical correctness or safety issue. Style nits → low/medium with verdict: pass.
