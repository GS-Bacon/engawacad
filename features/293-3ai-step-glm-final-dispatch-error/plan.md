## 自律判断ログ

- light flow (bug, batch:data, flow=light, keep_codex_gate=false) で自律モード起動。本 Issue は crates/** ではなく `.claude/skills/3ai/scripts/dispatch-glm-review.ts` 配下の TS 修正のため、`project_3ailoop_implementation_style` の流儀に倣い **GLM dispatch ではなく Claude 直接実装**で進める。STEP 7.5 は `keep_codex_gate=false` なので state shim で `codex_review=passed` に倒す。

## 自動起票 Issue の前回 ci.log 必須調査 (STEP 1-A)

- feature-dir: `features/290-format-phase-example-golden/`
- `ci.log` は **全テスト pass** (Rust workspace / 3ai shell / 3ailoop bun + viewer playwright 全部 green)。CI 失敗は無し。
- `final-review.yaml` を読むと:
  - `verdict: pass`
  - `issues[0]`: severity=high / finding="In-Scope #275 の golden round-trip テストが未実装" / suggestion="**#275 が close して rect_polygon_slot.engawa が examples/ に作成された後に、別 Issue または follow-up commit で golden test を追加**"
- `final-review.verdict.json`:
  - `verdict: error` / `dispatch_error: true` / `reason: pass-with-blockers` / `blocking: -1`
- **根本原因**: `dispatch-glm-review.ts:107` の `if (verdict === "pass" && blocking > 0) return { failed: true, reason: "pass-with-blockers" }` が、GLM が verdict は pass にしつつ defensive note (defer 系) を high で出してきたケースを内部矛盾として dispatch_error 扱い → STEP 7 dispatch_error 分岐で `raise-issue-on-failure.ts` 起票 → #293 自動起票。
- 実態: GLM 自身が `suggestion: "別 Issue または follow-up commit で追加"` と明記しており、blocker 意図はない。severity を誤って high に置いた defensive note。
- 該当 Issue (#290) は STEP 7 dispatch_error 分岐の "state shim で final_review=passed に倒す" 経路で正常に Closes #290 でマージ済 (commit 3741a1b)。問題はフローが進んだことではなく **false-positive な auto-raise Issue が量産される** こと。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `dispatch-glm-review.ts` の `detectDispatchFailure` の `pass-with-blockers` 分岐を「dispatch_error 化」から「issues を medium にデモート + blocking=0 で verdict.json を整合化 + warning ログ」に変更 | `fail-without-blockers` 分岐の挙動変更 (こちらは本物の矛盾なので dispatch_error のまま) |
| `detectDispatchFailure` 戻り値の semantics 拡張: 失敗以外に "demote" モードを追加 | GLM プロンプト側の「pass のとき high を出すな」改善 (本件の即時 fix とは独立、別 Issue で扱える) |
| `dispatch-glm-review.test.ts` を新規追加し再現テストを書く (T01 = 当該ケースで失敗しなくなる、T02 = 既存の fail-without-blockers は失敗のまま、T03 = 既存の正常 pass は変化なし) | `dispatch-codex.ts` / `dispatch-codex-3persona.ts` 側の同等ロジック (本 Issue では touch しない) |

## Non-Goals

- GLM の review プロンプト改善 (pass のとき high を出すなと指示する)
- `dispatch-codex*.ts` 側の同等ロジック修正
- 既に起票済の #293 自体の自動 close (STEP 8 で `Closes #293` を commit message に入れる)
- `fail-without-blockers` の挙動緩和

## 実装対象

- 影響ファイル:
  - `/home/bacon/engawacad/.claude/skills/3ai/scripts/dispatch-glm-review.ts`
  - `/home/bacon/engawacad/.claude/skills/3ai/scripts/dispatch-glm-review.test.ts` (新規)
- 変更する型・関数:
  - `detectDispatchFailure` の戻り値型に `demoted?: boolean` を追加 (export 型を後方互換維持)。または別関数 `classifyVerdict` を切り出し、戻り値を `{ kind: "ok" | "demote" | "fail"; reason: string }` の 3 値にする
  - `pass-with-blockers` 検出時の caller 側処理: verdict.json を error にせず、issues 配列を `severity=medium` に強制デモートして書き出す + console.warn

### before/after スニペット

**before** (dispatch-glm-review.ts:94-116):
```ts
export function detectDispatchFailure(
  rawOut: string,
  exitCode: number,
  yamlText: string,
): { failed: boolean; reason: string } {
  if (exitCode !== 0) return { failed: true, reason: `claude-exit-${exitCode}` };
  if (isValidReviewYaml(yamlText)) {
    const { verdict, blocking } = parseVerdict(yamlText);
    if (verdict === "fail" && blocking === 0) return { failed: true, reason: "fail-without-blockers" };
    if (verdict === "pass" && blocking > 0) return { failed: true, reason: "pass-with-blockers" };
    return { failed: false, reason: "" };
  }
  ...
}
```

**after**:
```ts
export type DispatchClassification =
  | { kind: "ok"; reason: "" }
  | { kind: "demote"; reason: "pass-with-blockers" }  // 新規: verdict=pass を尊重しつつ issues を medium にデモート
  | { kind: "fail"; reason: string };

export function classifyDispatch(
  rawOut: string,
  exitCode: number,
  yamlText: string,
): DispatchClassification {
  if (exitCode !== 0) return { kind: "fail", reason: `claude-exit-${exitCode}` };
  if (isValidReviewYaml(yamlText)) {
    const { verdict, blocking } = parseVerdict(yamlText);
    if (verdict === "fail" && blocking === 0) return { kind: "fail", reason: "fail-without-blockers" };
    if (verdict === "pass" && blocking > 0) return { kind: "demote", reason: "pass-with-blockers" };
    return { kind: "ok", reason: "" };
  }
  if (/Error:\s*Reached\s+max\s+turns/i.test(rawOut)) return { kind: "fail", reason: "max-turns" };
  if (/^\s*Error:/m.test(yamlText)) return { kind: "fail", reason: "claude-error" };
  if (!/^verdict:\s*(pass|fail)\b/m.test(yamlText)) return { kind: "fail", reason: "no-verdict-line" };
  return { kind: "fail", reason: "missing-issues" };
}

// 後方互換 wrapper (caller が外部スクリプトから直接 import している可能性)
export function detectDispatchFailure(
  rawOut: string, exitCode: number, yamlText: string,
): { failed: boolean; reason: string } {
  const c = classifyDispatch(rawOut, exitCode, yamlText);
  return { failed: c.kind === "fail", reason: c.reason };
}
```

- caller 側 (`writeVerdictJson` 周辺): `kind: "demote"` のとき:
  1. issues 配列の各 entry の `severity` を `medium` に上書き
  2. verdict.json には `verdict: pass`, `blocking: 0`, `severity_counts: {critical:0, high:0, medium:<count>, low:0}`, `demoted: true`, `demote_reason: "pass-with-blockers"` を書く
  3. `console.warn` で警告を出力

## 設計方針

- 決定性要件: 入力 yaml が同一なら verdict.json の出力も同一 (severity デモートは決定的に medium に倒す、issues の順序は変えない)
- derive 規約: TS なので N/A
- エラーハンドリング: classifyDispatch は副作用なし。caller 側で `kind: "demote"` を console.warn する

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 再現 (バグ確認) | `verdict: pass` + issues に high が 1 件ある yaml を `classifyDispatch` に渡す | 修正前: `{ kind: "fail", reason: "pass-with-blockers" }` (= 修正前は dispatch_error)、**修正後: `{ kind: "demote", reason: "pass-with-blockers" }`** |
| T02_boundary_pass_no_issues | 正常系 | `verdict: pass` + issues 空 | `{ kind: "ok", reason: "" }` |
| T03_boundary_fail_with_blockers | 正常系 | `verdict: fail` + issues に high 1 件 | `{ kind: "fail", reason: "" }` ではなく — verdict=fail かつ blocking>=1 は本物の fail (実際は kind=ok を返す = caller 側で fail-handling、別 reason 不要)。**実体は L116 `return { kind: "ok", reason: "" }` を通って caller の verdict=fail 分岐に乗る**。 |
| T04_degen_fail_without_blockers | エッジケース | `verdict: fail` + issues 空 | `{ kind: "fail", reason: "fail-without-blockers" }` |
| T05_degen_max_turns | エッジケース | rawOut に `Error: Reached max turns`、yaml は不完全 | `{ kind: "fail", reason: "max-turns" }` |
| T06_degen_exit_nonzero | エッジケース | exitCode=2、yaml は valid だが exit 非ゼロ | `{ kind: "fail", reason: "claude-exit-2" }` |
| T07_determinism | 決定性 | 同一入力で 2 回呼び出して同一結果 | assert.deepStrictEqual |

## 幾何的不変条件チェックリスト

- N/A (本 Issue は 3ai スクリプト修正で B-rep に触れない)
