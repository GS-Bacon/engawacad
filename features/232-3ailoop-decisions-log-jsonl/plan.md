# Plan: refactor(3ailoop) decisions.log JSONL 併記

## 自律判断ログ (Claude 直接実装方針)

`/3ailoop` 系スクリプトの改訂につき memory `project_3ailoop_implementation_style` に基づき **Claude 直接実装**。GLM dispatch は使わず、Claude が TS を直接編集する。`crates/**` 変更なし → guard-crates 対象外。STEP 7/7.5 は B-6 Codex cross-cut に集約 (light flow `keep_codex_gate: false`)。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `loop-decision-log.ts append` に追加引数 (`--cycle`, `--root-issue`, `--time-min`, `--token-claude`, `--token-glm`, `--token-codex`, `--blocked-on`) | 過去 decision の遡及 migration (今後の append から JSONL 化) |
| 新規 `features/.loop/decisions.log.jsonl` への JSONL 併記 (md は人間可読のまま) | 可視化 dashboard (本 Issue は CLI 集計まで) |
| 新規 `stats` subcommand (kind 分布 / 平均 time_min / pause root cause top) | alert (連続 pause 通知) — 別 Issue で I-4 と組合せ |
| kind enum 拡張: 既存 (adr-draft / issue-split / needs-human / phase-transition / other) + 新規 (failure-escape / phase-close / policy-update) を union で受容 | 既存 md 行の自動 JSONL 変換 (前方互換のみ、過去ログは markdown のまま残す) |
| 単体テスト: 各 kind 1 件 append + stats が正しく集計 | I-3 (token delta) との結合テスト (token field は I-3 完了済前提で渡される値をそのまま記録) |
| `cargo xtask ci` green + bun test 全 green | dashboard 側の JSONL 表示 (本 Issue では追加しない、md 抜粋表示は維持) |

## Non-Goals

- 過去 markdown 行の retroactive JSONL 変換
- alert / 通知ロジック (別 Issue)
- dashboard 表示変更
- JSONL ファイルの世代管理 (rotation / archive)
- 既存 callers の即時改修 — JSONL 拡張は opt-in (新引数なしの append は backward-compat で md と最小 jsonl 行を書く)

## 実装対象

### A. loop-decision-log.ts 改訂

**変更 1**: kind enum 拡張 (union of 既存 + Issue spec)

before:
```typescript
const VALID_KINDS = new Set([
  "adr-draft",
  "issue-split",
  "needs-human",
  "phase-transition",
  "other",
]);
```

after:
```typescript
const VALID_KINDS = new Set([
  "adr-draft",
  "issue-split",
  "failure-escape",
  "phase-close",
  "policy-update",
  "other",
  "needs-human",        // backward compat
  "phase-transition",   // backward compat
]);
```

**変更 2**: JSONL path 追加 + `appendEntry` を opts 受取に拡張

```typescript
export const JSONL_PATH = "features/.loop/decisions.log.jsonl";

export type AppendOpts = {
  kind: string;
  message: string;
  cycle?: number | null;
  rootIssue?: number | null;
  timeMin?: number | null;
  tokenDelta?: { claude?: number; glm?: number; codex?: number } | null;
  blockedOn?: string[];
};

export function appendEntry(opts: AppendOpts): void {
  // md (既存形式) + jsonl (新形式) 両方を書く
}
```

JSONL 行スキーマ:
```jsonc
{
  "at": "2026-06-18T02:50:00.000Z",
  "cycle": 26,
  "kind": "adr-draft",
  "root_issue": 207,
  "time_min": null,
  "token_delta": { "claude": 0, "glm": 50000, "codex": 12000 },
  "blocked_on": [],
  "message": "ADR-014 draft 作成 → gate:adr-review 起票"
}
```

**変更 3**: CLI args parser を拡張

```bash
bun loop-decision-log.ts append \
  --kind <k> --message <text> \
  [--cycle N] [--root-issue N] [--time-min N] \
  [--token-claude N] [--token-glm N] [--token-codex N] \
  [--blocked-on "207,gate:adr-review"]
```

**変更 4**: 新 subcommand `stats`

```bash
bun loop-decision-log.ts stats [--last-cycles N] [--json]
```

出力 (markdown 形式 デフォルト):
- kind 分布
- average / total time_min
- pause root cause top (root_issue 別 count)
- token delta sum (claude/glm/codex)

`--json` 指定時は同じ集計を JSON で出力。

### B. テスト

`.claude/skills/3ailoop/scripts/__tests__/loop-decision-log.test.ts` (新規):

- T01 append basic
- T02 append full
- T03 append backward-compat (needs-human, phase-transition)
- T04 invalid kind throws
- T05 stats kind dist
- T06 stats time avg
- T07 stats root cause top
- T08 stats token sum
- T09_degen_empty
- T10_boundary_last_cycles

## 設計方針

- **既存 md 形式は完全互換維持**: 既存 callers は変わらず md に追記される。jsonl は追加で書かれる
- **kind enum は union で寛容化**: 既存 5 + 新規 3 = 8 kind 受容。canonical 化は別 Issue
- **JSONL append atomicity**: 各行 `appendFileSync` で 1 行ずつ追記 (caller 側 flock 前提)
- **stats アルゴリズム**: jsonl 全行 parse → cycle filter → 集計 (linear scan で十分)
- **数値型の安全パース**: parseInt 失敗 → null として skip
- **blocked_on の文字列分割**: CLI で csv 受け取り、jsonl では `string[]`
- **エラーハンドリング**: jsonl 書き込み失敗時は warn のみ、md 書き込みを優先

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | append basic | kind=adr-draft + message のみ | md と jsonl 両方に行追加、jsonl の null フィールドは null/[] で記録 |
| T02 | append full | 全 field 渡し | jsonl 行に全 field 反映 |
| T03 | backward-compat | 旧 kind (needs-human, phase-transition) | エラーなし、両方に書かれる |
| T04 | invalid kind | 未知 kind | throw |
| T05 | stats kind dist | 異なる kind 5 件 → kind distribution 正解 | failure-escape=2, adr-draft=1, ... 合計が一致 |
| T06 | stats time avg | time_min 付き 3 件 (10, 20, 30) | average=20, sum=60 |
| T07 | stats root cause | 同 root_issue 重複 (#220×3, #206×2) | top に正しい順序で並ぶ |
| T08 | stats token sum | token_delta 3 件 | claude/glm/codex の合算正解 |
| T09_degen_empty | 退化 | jsonl 不在 | "(no data)" 出力、exit 0 |
| T10_boundary_last_cycles | 境界 | cycle 1-5 のうち --last-cycles=2 | cycle 4-5 のみ集計 |

## 幾何的不変条件チェックリスト

- 該当なし (TS スクリプト変更のみ)

## 実装順序

1. plan.md 確定 ✅
2. loop-decision-log.ts 改訂
3. `__tests__/loop-decision-log.test.ts` 実装
4. `bun test` 全 green 確認
5. `cargo xtask ci` workspace baseline green 確認
6. commit (Closes #232) + main へ直 push
