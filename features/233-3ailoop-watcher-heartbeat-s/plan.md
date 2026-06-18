# Plan: refactor(3ailoop) watcher heartbeat 削減 (30s + ring buffer + event-only log)

## 自律判断ログ (Claude 直接実装方針)

`/3ailoop` 系スクリプトの改訂につき memory `project_3ailoop_implementation_style` に基づき **Claude 直接実装**。GLM dispatch は使わず、Claude が TS を直接編集する。`crates/**` 変更なし → guard-crates 対象外。STEP 7/7.5 は B-6 Codex cross-cut に集約 (light flow `keep_codex_gate: false`)。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `loop-tmux-watcher.ts` の `POLL_SEC` default を 10 → 30 に変更 (env で従来値に override 可能) | log rotation (ring buffer で十分、外部 rotation 不要) |
| 新規 ring buffer ファイル `features/.loop/tmux/watcher.ring.log` を追加 (heartbeat 専用、最大 60 件 = 30 min @ 30s) | watcher の event-driven 化 (現状 polling、本 Issue は polling のまま interval 緩和のみ) |
| `heartbeat:` ログを ring buffer に分離し、`watcher.log` は event-only (start/exit/cycle/stuck/error/init-baseline/tmux-query-failed/state-corrupt/worker-gone) | structured log (JSONL) 化 (別 Issue で I-7 / #232 と統合検討) |
| `trimRingBuffer(lines, maxEntries)` を pure 関数として export (テスト容易性) | 既存 `watcher.log` の retroactive 切り戻し (新規 event-only は本 Issue 以降に書かれるログのみ) |
| stuck 検知 (45min idle) が 30s polling でも正しく動作することを確認 (decideAction は既に時間ベース → 変更不要、回帰テスト追加) | heartbeat 専用 ring buffer の人間向け formatter (生 log 行を timestamp 順に保持) |
| 単体テスト追加: ring buffer trim (T01-T04) / 30s interval (T05) / event-only log 分離 (T06) | watcher 起動時 ring buffer の初期化 (既存ファイルがあれば trim、なければ touch) — 副作用は最小化 |
| `cargo xtask ci` green + bun test 全 green | watcher.log の既存検索 grep workflow 変更 (event-only に絞られて見つけやすくなる副次効果は許容) |

## Non-Goals

- log rotation / archival (ring buffer で十分)
- event-driven watcher (poll のまま)
- JSONL log 化
- 過去 watcher.log のクリーンアップ
- ring buffer の世代管理 / 圧縮
- per-event log 行の formatter 変更 (既存 `[timestamp] msg` 形式維持)

## 実装対象

### A. loop-tmux-watcher.ts 改訂

**変更 1**: POLL_SEC default 10 → 30

before:
```typescript
const POLL_SEC = parseInt(process.env.LOOP_TMUX_POLL_SEC ?? "10", 10);
```

after:
```typescript
const POLL_SEC = parseInt(process.env.LOOP_TMUX_POLL_SEC ?? "30", 10);
```

(env override は変更なし。テストで `LOOP_TMUX_POLL_SEC=1` 等を渡せる仕様は維持)

**変更 2**: ring buffer path + 定数

```typescript
const RING_LOG_PATH = join(TMUX_DIR, "watcher.ring.log");
export const RING_BUFFER_MAX_ENTRIES = 60; // 30 min @ 30s
```

**変更 3**: pure 関数 `trimRingBuffer` を export

```typescript
/** 既存行 + 新規行を結合し、末尾 maxEntries 件のみ保持。空行は除外。 */
export function trimRingBuffer(existingLines: string[], newLine: string, maxEntries: number): string[] {
  const all = existingLines.concat([newLine]).filter(l => l.length > 0);
  return all.slice(Math.max(0, all.length - maxEntries));
}
```

**変更 4**: heartbeat 専用 logger `ringLog()`

```typescript
function ringLog(msg: string): void {
  const line = `[${nowIso()}] ${msg}`;
  try {
    mkdirSync(dirname(RING_LOG_PATH), { recursive: true });
    const existing = existsSync(RING_LOG_PATH)
      ? readFileSync(RING_LOG_PATH, "utf-8").split("\n").filter(l => l.length > 0)
      : [];
    const next = trimRingBuffer(existing, line, RING_BUFFER_MAX_ENTRIES);
    writeFileSync(RING_LOG_PATH, next.join("\n") + "\n", "utf-8");
  } catch {
    // ring buffer 失敗で daemon は止めない
  }
  // stderr へのミラー出力なし (ノイズ削減)
}
```

**変更 5**: 既存 `log("heartbeat: ...")` 呼び出しを `ringLog(...)` に置換 (line 512)

before:
```typescript
log(`heartbeat: pane_cmd=${paneCmd ?? "(null)"} active=${active} idle=${idleMin.toFixed(1)}min`);
```

after:
```typescript
ringLog(`heartbeat: pane_cmd=${paneCmd ?? "(null)"} active=${active} idle=${idleMin.toFixed(1)}min`);
```

これにより `watcher.log` は event-only (start/exit/cycle/stuck/error/...) に絞られ、heartbeat は `watcher.ring.log` に隔離。grep workflow が大幅に改善する。

### B. テスト

`.claude/skills/3ailoop/scripts/loop-tmux-watcher.heartbeat.test.ts` (新規):

- T01 trim basic: existing=[1..50] + 1 new → 全 51 件残る (maxEntries=60)
- T02 trim overflow: existing=[1..60] + 1 new → 末尾 60 件のみ (最古 drop)
- T03 trim large overflow: existing=[1..100] + 1 new → 末尾 60 件
- T04 trim empty existing: existing=[] + 1 new → 1 件
- T05 default POLL_SEC: env 未設定で POLL_SEC default を 30 とみなす (constants 確認)
- T06 ring constant: RING_BUFFER_MAX_ENTRIES === 60

(stuck 検知の回帰テストは既存 `loop-tmux-watcher.test.ts` の time-based 計算で既にカバー済み。本 Issue で新規追加するのは ring buffer まわりのみ)

## 設計方針

- **interval 30s の根拠**: 4069 行 watcher.log のうち 98.9% (4023 行) が heartbeat noise。30s に緩和すれば 3 倍削減 = 1356 件相当 (ring buffer 60 件と合わせれば実質ファイルサイズは限りなくゼロ近く)
- **ring buffer 60 件 = 30 min 分**: stuck threshold が 45min なので、stuck 発火 15min 前までの heartbeat が ring buffer に残る = 事後 debug に十分。stuck 発火時に ring buffer をスナップショットする運用は別 Issue
- **stuck 検知の時間ベース性**: 既存 `decideAction` の `minutesSinceLastUpdate >= stuckThresholdMin` は時間ベース。POLL_SEC を 10→30 にしても、stuck 発火 (45min 経過) のタイミングは最大 30s 遅延するだけ (= 過剰検出側ではなく detection-delay 側で安全)。Issue spec 「polling 回数依存→経過時間依存に書き換え」は誤読の可能性、現状コードは既に時間ベースで正しい。回帰テスト追加で安全性確認
- **ring buffer 書き込みは sequential**: heartbeat は単一 watcher process 内でのみ発生 (他 process は ring buffer に書き込まない) → file lock 不要、シンプルに read-merge-write atomic rename も使わず writeFileSync で十分 (失敗時は次 heartbeat で復旧)
- **ring buffer 失敗の扱い**: read/write 失敗時も daemon は止めない (heartbeat は monitoring 補助、loop 本体には影響しない)
- **既存 LOG_PATH 互換**: heartbeat 以外の event ログは引き続き `log(msg)` で `watcher.log` に append される。watcher 起動時の `watcher start pid=... poll=30s stuck=45min` も `log()` で書かれるので「heartbeat 30s に切替済み」の確認が現行 grep workflow で可能
- **テスト戦略**: pure 関数 `trimRingBuffer` をテスト中心とし、副作用層 (`ringLog`) は file I/O ラッパなので bun test 既存パターン (tmp dir + 環境変数 override) と同様に容易にテストできる

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | trim basic | existing=50 行 + 1 行 → 全 51 行残る (max=60) | `result.length === 51` |
| T02 | trim overflow | existing=60 行 + 1 行 → 末尾 60 行 (最古 drop) | `result[0] === existing[1]`、`result.length === 60` |
| T03 | trim large overflow | existing=100 行 + 1 行 → 末尾 60 行 | `result.length === 60` |
| T04_boundary_empty | 境界 | existing=[] + 1 行 → 1 行 | `result === [newLine]` |
| T05_degen_zero_max | 退化 | maxEntries=0 → 結果は [] | `result.length === 0` |
| T06 | POLL_SEC default | env 未設定で POLL_SEC === 30 | constants 確認 |
| T07 | RING_BUFFER_MAX_ENTRIES | === 60 | constants 確認 |

T01-T04 が ring buffer のメインロジック、T05 が退化境界、T06-T07 が定数の正しさ確認。

## 幾何的不変条件チェックリスト

- 該当なし (TS スクリプト変更のみ)

## 実装順序

1. plan.md 確定 ✅
2. loop-tmux-watcher.ts: POLL_SEC 30 + RING_LOG_PATH + trimRingBuffer + ringLog + line 512 置換
3. `loop-tmux-watcher.heartbeat.test.ts` 実装 (T01-T07)
4. `bun test` 全 green 確認 (既存 watcher テストも回帰なし確認)
5. `cargo xtask ci` workspace baseline green 確認
6. commit (Closes #233) + main へ直 push
