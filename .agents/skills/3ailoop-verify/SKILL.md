---
name: 3ailoop-verify
description: Verify the worker receipt against the complete worktree delta. Validates the receipt schema, scans the safety audit for forbidden external operations (push/gh-write/scope/stash), and checks completeness across tracked + staged + committed + untracked files — a missing untracked file fails (#330) rather than yielding a false clean result.
allowed-tools: Read, Bash, Glob, Grep
---

# $3ailoop-verify — verify receipt + complete diff

worker の receipt を **完全 diff (untracked 含む)** と照合して検証する。

## 手順

```bash
bun .agents/skills/3ailoop/scripts/verify-receipt.ts \
  --receipt <worktree>/.worker/receipt.json \
  --worktree <path> \
  --audit <worktree>/.worker/audit.json \
  --expect-issue <N> --expect-worker <worker-id> \
  --expect-model <provider/model> --expect-session <session-id>
```

- exit 0: 検証 ok (stdout JSON に `receipt_valid / identity_ok / safety_violations / completeness / diff`)
- exit 1: 検証 fail (receipt invalid / identity mismatch / safety violation / completeness fail)

## 検証内容 (lib/verify-identity.ts, lib/git-diff.ts, lib/safety.ts, lib/receipt.ts)

1. **receipt schema** validation — 必須フィールド / 型 / status 別要件 (completed⇒commit_sha+changed_files,
   needs_supervisor⇒consultation) / forbidden_operations 非空は即 fail。
2. **lifecycle binding (identity)** — receipt の issue / worker_id / model / session_id を queued job の
   `--expect-*` と照合。1 つでも不一致なら fail (違う Issue/model/session の receipt が通るのを防ぐ)。
3. **safety audit** — worker が実行した command を監査 log から走査し、push/gh-write/default-branch-land/
   shared-stash/cross-worktree-edit/config-credential を検出。audit は **必須かつ fail-closed**: 指定がない /
   読めない / 壊れている audit は、worker が安全だったことを証明できず safety 違反 (clean receipt だけでは通さない)。
4. **complete diff** — tracked + staged + committed + **untracked** を計算し、receipt.changed_files と完全一致
   させる (#330: untracked 抜けは `completeness-fail`)。

検証 ok かつ `--expect-issue` / `--expect-worker` が与えられていれば、CAS を `verifying` に遷移
(lib/cas-state.ts `moveToVerifying`、stale-generation で冪等失敗は JSON に記録されるが verify 判定は反転しない)
し、`$3ailoop-land` へ。

詳細は [../3ailoop/references/runtime.md](../3ailoop/references/runtime.md) の Complete diff / Worker receipt 節。
