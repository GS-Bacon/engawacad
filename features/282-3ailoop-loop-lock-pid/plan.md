## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `acquireLock` に pid liveness check (`process.kill(pid, 0)`) を追加し、dead owner の lock を 12h TTL 待たずに takeover する | retry 失敗時の observation log (Issue body 末尾の「併せて検討」項目、別 Issue) |
| 4 ケース (alive / dead / EPERM / no-pid meta) の unit test | stale takeover の CAS rename 化 (`project-3ailoop-known-races` の別 race) |
| `LOCK_DIR` / `META_PATH` を env var `LOOP_LOCK_DIR` で override 可能にする (テスト用) | pid recycling 対策 (kernel が pid を再割り当てた場合の検出。Linux で 4M pid 周期、12h TTL safety net で受容) |

## Non-Goals

- observation log (cycle 64-65 の forensic 用)
- stale takeover CAS rename
- pid recycling 対策
- 既存 `isStaleByMeta` (12h TTL) の動作変更

## 実装対象

- `.claude/skills/3ailoop/scripts/loop-lock.ts` — `isOwnerDead` 追加 + `acquireLock` 内で先行 check + LOCK_DIR/META_PATH を関数化して env override 化
- `.claude/skills/3ailoop/scripts/loop-lock.test.ts` (新規) — 4 ケース unit test

### 既存コード before/after (loop-lock.ts)

before (`acquireLock`, line 138-161):
```ts
if (result === "exists") {
  const meta = readMeta();
  if (!meta) {
    // meta.json 不在 / 破損 → dir mtime で短期 stale 判定
    ...
  }
  const { stale, reason } = isStaleByMeta(meta);
  if (stale) { rmSync(LOCK_DIR, ...); return acquireLock(owner); }
  return { ok: false, reason: `lock held by owner=...` };
}
```

after:
```ts
if (result === "exists") {
  const meta = readMeta();
  if (!meta) { /* unchanged */ }
  // 1. pid liveness check (新規) — TTL より先に
  const dead = isOwnerDead(meta);
  if (dead.dead) {
    rmSync(getLockDir(), { recursive: true, force: true });
    return acquireLock(owner);
  }
  // 2. existing TTL check
  const { stale, reason } = isStaleByMeta(meta);
  ...
}
```

新規関数:
```ts
function isOwnerDead(meta: LockMeta): { dead: boolean; reason: string } {
  if (typeof meta.pid !== "number") return { dead: false, reason: "no pid" };
  try {
    process.kill(meta.pid, 0);
    return { dead: false, reason: "alive" };
  } catch (e: unknown) {
    const code = (e as NodeJS.ErrnoException).code;
    if (code === "ESRCH") return { dead: true, reason: "ESRCH" };
    if (code === "EPERM") return { dead: false, reason: "EPERM (alive, other user)" };
    return { dead: false, reason: `unknown errno ${code}` };
  }
}
```

LOCK_DIR / META_PATH を関数化 (env override):
```ts
function getLockDir(): string {
  return process.env.LOOP_LOCK_DIR ?? "features/.batch/lock";
}
function getMetaPath(): string {
  return join(getLockDir(), "meta.json");
}
```
- 既存 `const LOCK_DIR` / `const META_PATH` は削除し、各参照箇所を関数呼び出しに置換 (内側 5 箇所)。
- 既存挙動への影響なし (env 未設定時は同じパス)。

## 設計方針

- **POSIX semantics**: `process.kill(pid, 0)` は実際に signal を送らず存在確認のみ。Node/Bun が標準でサポート。
- **保守判定**: EPERM / unknown errno は **alive 扱い** (誤って takeover しない方向に倒す)。これは Issue body の方針と一致。
- **TTL safety net**: pid recycling で誤検出した場合も既存 `isStaleByMeta` (12h) が最終的に拾うため、永久ブロックは発生しない。
- **後方互換**: meta.json schema 変更なし。外向き API (`acquire`/`release`/`renew`/`status`) 不変。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 正常系 (alive) | 自プロセス pid を meta に書き込んで acquire → 別 owner の acquire が LOCKED | `ok: false`, reason に `lock held by` |
| T02 | 正常系 (dead) | 存在しない pid (例: 2^22 など事実上 unused な大きい値) を meta に書き込んで acquire → 新規 token で OK | `ok: true`, `token` 取得 |
| T03 | エッジケース (EPERM proxy) | meta に PID 1 (init) を書き込み (= 通常 EPERM になる) → alive 扱いで LOCKED | `ok: false` (kill(1,0) は EPERM or 成功どちらでも alive 扱い) |
| T04_boundary | 境界 (no pid 互換) | meta から `pid` フィールドを欠落させると `readMeta` が null → 既存 dir mtime 経路へ fallback (本機能の責務外を侵さないこと) | `ok: false` reason に `meta.json missing/invalid` |
| T05_degen_takeover_round_trip | 退化→復旧 | dead pid で takeover → 新 token で release → 再度新規 acquire OK | 全 step `ok: true` |

退化/境界ケース ID: T04_boundary, T05_degen_takeover_round_trip ✓

## 幾何的不変条件チェックリスト

- N/A (本 Issue は B-rep トポロジー非関連、スキル script の race fix)
