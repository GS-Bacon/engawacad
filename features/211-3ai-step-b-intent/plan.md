## 自律判断ログ

- **Issue 本文の表面的訴求**: `STEP B-3 intent-check failed` を bug として扱い直す
- **Claude が確定した根本原因**: `raise-issue-on-failure.ts` が「呼び出し側が STEP 名を渡せば必ず起票する」設計で、`STEP B-3 intent-check failed` のように **設計判断 (=人間判断項目) を扱う step まで誤起票してしまう**。intent-check の `aligned: no` は ADR-006 粒度違反を loop に通知する正規シグナルであり、runtime failure ではない。本件 #211 自体がその誤起票結果 (cycle #8 の Decision Log 参照)。
- **採用方針**: スクリプト側に `intent-check` ガードを追加。step name に `intent-check` (case-insensitive) を含めば skip + stderr WARN し、loop-notify / loop-intent-guard 経由の人間通知に委譲。これにより #211 系の再発を構造的に防ぐ。
- **棄却した別案**: (a) `#206` を 5 sub-issue に分割する自律実装 — Phase 8 着手 = ロードマップ進行判断であり Decision Log で人間項目と明記済みのため自律実行不可 / (b) `#211` を invalid 直接 close — 構造的修正にならず再発する。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `raise-issue-on-failure.ts` に `intent-check` ガード関数追加 | #206 の sub-issue 分割設計・着手 |
| pure function 化 + bun:test での unit test | `dispatch-codex-intent.ts` 本体の挙動変更 |
| ガード時 stderr に `loop-notify.ts` 推奨を出す | loop-intent-guard.ts への代替集約 (既に動作している) |

## Non-Goals

- Phase 8 スコープ分割 (#206) — ROADMAP 進行判断のため別途 intake 経由
- raise-issue-on-failure 全体のリファクタ
- 他 step name パターンの追加ガード (今回は `intent-check` のみ)

## 実装対象

- 影響ファイル:
  - `/.claude/skills/3ai/scripts/raise-issue-on-failure.ts` (修正)
  - `/.claude/skills/3ai/scripts/__tests__/raise-issue-on-failure.test.ts` (新規 / unit test)
- 既存関数の修正 (before/after):

**before** (`raise-issue-on-failure.ts` の引数 parse 直後):
```ts
if (!step || !featureDir || !errorSummary) {
  process.stderr.write("Usage: ...\n");
  process.exit(2);
}

// featureDir から issue 番号・slug を取得
```

**after**:
```ts
if (!step || !featureDir || !errorSummary) {
  process.stderr.write("Usage: ...\n");
  process.exit(2);
}

// intent-check 系は raise 対象外 (#211)
const skip = shouldSkipStep(step);
if (skip.skip) {
  process.stderr.write(
    `SKIP: step="${step}" は自動起票対象外 (${skip.reason}). ` +
      `人間への通知には loop-notify.ts / loop-intent-guard.ts を使ってください。\n`,
  );
  process.exit(0);
}

// featureDir から issue 番号・slug を取得
```

- 新規 export 関数 (同ファイル末尾、または top-level に近い位置):
```ts
export function shouldSkipStep(step: string): { skip: boolean; reason: string } {
  // intent-check の aligned:no は ADR-006 粒度違反シグナルで runtime failure ではない (#211)
  if (/intent[-_ ]?check/i.test(step)) {
    return { skip: true, reason: "intent-check 系は設計判断項目 — bug 起票対象外" };
  }
  return { skip: false, reason: "" };
}
```

## 設計方針

- **副作用分離**: ガード判定は pure function `shouldSkipStep(step)` として export。bun:test で stdin/stdout なしに ロジック検証できる。
- **正規表現の widening**: `intent-check` / `intent check` / `intent_check` / `INTENT-CHECK` を全部拾う。skill 文書中の `STEP B-3 intent-check failed` / `intent-check failed` を網羅。
- **exit code**: `skip` 時は **0** (= 正常終了)。呼び出し側 (Claude 手打ち / state.ts inc の auto raise) で「失敗」と扱われないように。
- **stderr メッセージ**: 何が起きたか + 代替経路 (`loop-notify.ts` / `loop-intent-guard.ts`) を必ず案内する。
- **derive 規約 / thiserror**: 対象は TS。N/A。
- **workspace.dependencies**: N/A (TS スクリプト)。

### 数値モデル

N/A (TS スクリプト修正、数値判断なし)。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | `shouldSkipStep("STEP B-3 intent-check failed")` を 2 回呼んで結果一致 | 同一 `{skip:true, ...}` |
| T02 | 正常系 (skip) | `STEP B-3 intent-check failed` | `skip:true` |
| T03 | 正常系 (skip) | `intent_check` (snake_case) | `skip:true` |
| T04 | 正常系 (skip) | `INTENT-CHECK aligned no` (大文字) | `skip:true` |
| T05 | 正常系 (pass) | `STEP 6-D` | `skip:false` |
| T06 | 正常系 (pass) | `STEP 7.5 codex_review` | `skip:false` |
| T07_boundary_empty | 退化/境界 | 空文字列 | `skip:false` (誤マッチ防止) |
| T08_boundary_intent_only | 退化/境界 | `"intent"` 単体 (check なし) | `skip:false` |
| T09_boundary_unrelated | 退化/境界 | `"check"` 単体 (intent なし) | `skip:false` |

## 幾何的不変条件チェックリスト

N/A (TS スクリプト、幾何処理なし)。

## 類似ケース（未カバー）

- `raise-issue-on-failure` には他にも「人間判断項目を自動起票してしまう」step が潜在的に存在しうる (例: `STEP 7.5-D critical 残存 docs-only` 系)。ただし現時点で再発実例なし。今回は intent-check のみガード追加し、将来の再発時に同関数へケース追加する形で拡張する想定。
