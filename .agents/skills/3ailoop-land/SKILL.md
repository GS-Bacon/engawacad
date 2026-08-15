---
name: 3ailoop-land
description: Serially land verified work. Serialized by a file lock so only one land runs at a time. Before any default-branch mutation it revalidates receipt, CAS generation, Issue-closed state, worker ownership, scope, the complete diff, and tests. Conflicts or CI/test failures return a bounded explicit state — never destructive recovery.
allowed-tools: Read, Bash, Glob, Grep
---

# $3ailoop-land — serial landing

検証済みの work を **直列** に landing する。default-branch 変更前に全面再検証。

## 手順

```bash
bun .agents/skills/3ailoop/scripts/land.ts \
  --receipt <worktree>/.worker/receipt.json \
  --worktree <path> --worker-id worker-1 \
  --issue <N> --generation <gen>
```

- exit 0: success (CAS issue → closed, worker → idle)
- exit 1: bounded fail (outcome 参照)

## 再検証 (lib/land-serial.ts `revalidate`, default-branch 変更前)

1. receipt.status == completed + commit_sha 存在 (でないと `receipt-invalid`)
2. CAS: closed でない (`closed-issue`) / generation 一致 (`stale-generation`) / worker 所有一致
3. scope: worktree 変更が Issue scope 内 (`scope-drift`)
4. completeness: receipt.changed_files が完全 diff と完全一致 (`completeness-fail`, #330)
5. rebase → CI → push --force-with-lease

## bounded outcomes (破壊的 recovery しない)

`rebase-conflict` / `ci-red` / `push-failed` / `scope-drift` / `stale-generation` /
`receipt-invalid` / `closed-issue` / `completeness-fail` / `internal-error`。
いずれも明示的 state に戻し、worker を idle に正規化する。**破壊的 recovery はしない**。

## 直列化

land-serial file lock (`features/.loop/land-serial.lock-target`) で同時 1 件のみ。
N=3 fan-out でも landing だけは直列 (旧 Phase D-1 merge dispatcher と同じ方針)。

詳細は [../3ailoop/references/runtime.md](../3ailoop/references/runtime.md) の Serialized landing 節。
