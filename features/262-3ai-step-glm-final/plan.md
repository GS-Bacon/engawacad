## 自律判断ログ

- Issue は `[自動起票]` `bug` `batch:kernel`、対象は `.claude/skills/3ai/scripts/dispatch-glm-review.ts`（TS）。
- 実装本体は **Claude 直接** が適切 (memory: `project_3ailoop_implementation_style` の精神 — TS 系はオーバースペック回避)。`crates/` を触らないため guard 抵触なし。
- STEP 7.5 (Codex 独立技術ゲート) は `keep_codex_gate: true` のため保持し、独立 review 軸を担保する。

## 根本原因 (#150 ci.log 必須調査)

元 Issue: #255 (`features/255-phase9-feature-crud-insert/`)。

1. `dispatch-glm-review.ts` は `claude -p ... --max-turns 5` で GLM (Z.AI) を spawn し、stdout を YAML として writeFileSync する。
2. GLM が max-turns を超過すると stdout は `Error: Reached max turns (15)` のような **エラー文字列だけ** になる。スクリプトはこれをそのまま `final-review.yaml` に書く。
3. `parseVerdict` は `verdict:` / `severity:` 行が見つからず `{verdict:"unknown", blocking:0}` を返す。
4. **`process.exit(0)` が無条件**なので、呼び元（STEP 7）は exit code から失敗を検知できない。
5. 上位フロー (`SKILL.md` STEP 7) は `blocking == 0 && verdict == "pass"` で pass 判定するが、`verdict: "unknown"` ではここを通らない。一方、`Critical/High があれば GLM 修正 dispatch` 経路 (`blocking >= 1`) にも入らないため、**どちらの分岐にも乗らない宙ぶらりん状態**。
6. 結果として「fix-dispatch 経路は無理にループするだけで成果なし、人手裁量で `final_review=passed` を書き込んで突破」というのが #255 で実際に取った迂回経路。`raise-issue-on-failure.ts` を呼んだことで本 Issue #262 が起票された。

ci.log 抜粋 (`features/255-phase9-feature-crud-insert/final-review.yaml`):
```
Error: Reached max turns (15)
```
verdict.json (`features/255-phase9-feature-crud-insert/final-review.verdict.json`):
```json
{"verdict":"unknown","severity_counts":{"critical":0,"high":0,"medium":0,"low":0},"blocking":0}
```

vacuous pass の **真の原因** は dispatch 層の silent fall-through であり、呼び元 markdown のロジックではない。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `dispatch-glm-review.ts` の **dispatch failure 検出** (max-turns / claude CLI non-zero exit / verdict 行不在) | GLM 側 (Z.AI API) の挙動チューニング、max-turns 自動 retry の最適値探索 |
| 検出時に **明示的な error verdict** (`verdict:"error"`, `dispatch_error:true`, `blocking:-1`) を `*.verdict.json` に書き出し | other dispatch script (`dispatch-glm.ts` / `dispatch-codex.ts`) の同様改修（別 Issue で扱う）|
| 検出時に **exit code 2** (引数エラー 1 と区別) | 既存の `final-review.yaml` ファイル / 過去成果物のバックフィル |
| `final` persona の default `--max-turns` を 5 → 30 に引き上げ（diff レビューは入力が大きい） | 全 persona の max-turns 統一見直し |
| `import.meta.main` ガード化（テスト import 可能にする） | 完全な単体テスト網羅（コア検出関数のみテストする）|
| `SKILL.md` STEP 7 に **dispatch_error 分岐** を追記（fix-dispatch せず `raise-issue-on-failure.ts` + escalate）| STEP 3 design review のエラーハンドリング (本 Issue は STEP 7 final review に限定)|

## Non-Goals

- max-turns を自動 retry する内部ループ実装: 「失敗を覆い隠す」方向に倒れやすいため明示的に避ける。最初から多めに振る (30 turns) ことで実用上の十分性を確保し、それでも失敗するなら error 経路に倒す。
- design review (`scope`/`invariant`/`ambig`/`numeric`) persona の挙動改修: 入力が plan.md のみで小さく、本 Issue で観測した max-turns 失敗の温床ではない。
- `dispatch-glm.ts` (実装系) や `dispatch-codex*.ts` の類似検証: 別 Issue 化が筋。本 Issue は #262 の発生源 = GLM final review に限定。
- 既存生成済み `final-review.yaml.verdict.json` のリトロアクティブ修正。

## 実装対象

- 影響ファイル:
  - `.claude/skills/3ai/scripts/dispatch-glm-review.ts` (本体改修)
  - `.claude/skills/3ai/scripts/__tests__/dispatch-glm-review.test.ts` (新規)
  - `.claude/skills/3ai/SKILL.md` STEP 7 (dispatch_error 分岐の追記)

### dispatch-glm-review.ts 改修内容

#### Before (該当箇所抜粋, 250-281 行)

```ts
const maxTurns = process.env.GLM_MAX_TURNS ?? "5";
const proc = Bun.spawn(
  ["claude", "-p", prompt, "--append-system-prompt-file", agentFile,
   "--allowedTools", "Read", "--max-turns", maxTurns, "--output-format", "text"],
  { env: glmEnv, stdout: "pipe", stderr: "pipe" }
);

const [rawOut, rawErr] = await Promise.all([
  new Response(proc.stdout).text(),
  new Response(proc.stderr).text(),
]);
await proc.exited;

if (rawErr.trim()) process.stderr.write(`[GLM stderr] ${rawErr.slice(0, 500)}\n`);

const yamlBlock = rawOut.match(/```yaml\r?\n([\s\S]*?)```/)?.[1] ?? rawOut;
const yamlText = yamlBlock.trim();

writeFileSync(resultFile, yamlText, "utf-8");

const verdict = parseVerdict(yamlText);
const verdictPath = resultFile.replace(/\.yaml$/, ".verdict.json");
writeFileSync(verdictPath, JSON.stringify(verdict, null, 2), "utf-8");

process.stderr.write(`  verdict=${verdict.verdict} blocking=${verdict.blocking} (C=${verdict.severity_counts.critical} H=${verdict.severity_counts.high})\n`);
process.exit(0);
```

#### After

```ts
const defaultMaxTurns = persona === "final" ? "30" : "5";
const maxTurns = process.env.GLM_MAX_TURNS ?? defaultMaxTurns;
const proc = Bun.spawn(
  ["claude", "-p", prompt, "--append-system-prompt-file", agentFile,
   "--allowedTools", "Read", "--max-turns", maxTurns, "--output-format", "text"],
  { env: glmEnv, stdout: "pipe", stderr: "pipe" }
);

const [rawOut, rawErr, exitCode] = await Promise.all([
  new Response(proc.stdout).text(),
  new Response(proc.stderr).text(),
  proc.exited,
]);

if (rawErr.trim()) process.stderr.write(`[GLM stderr] ${rawErr.slice(0, 500)}\n`);

const verdictPath = resultFile.replace(/\.yaml$/, ".verdict.json");

const yamlBlock = rawOut.match(/```yaml\r?\n([\s\S]*?)```/)?.[1] ?? rawOut;
const yamlText = yamlBlock.trim();

// dispatch 失敗検出: claude CLI が非ゼロ終了 / 'Error: Reached max turns' / verdict 行不在
const failure = detectDispatchFailure(rawOut, exitCode, yamlText);
if (failure.failed) {
  // 生の rawOut は result file に残して inspection 可能にする (上書きしてもよいが診断のため保持)
  writeFileSync(resultFile, rawOut, "utf-8");
  const errVerdict = makeErrorVerdict(failure.reason);
  writeFileSync(verdictPath, JSON.stringify(errVerdict, null, 2), "utf-8");
  process.stderr.write(`  verdict=error dispatch_error=true reason=${failure.reason} exit=${exitCode}\n`);
  process.exit(2);
}

writeFileSync(resultFile, yamlText, "utf-8");
const verdict = parseVerdict(yamlText);
writeFileSync(verdictPath, JSON.stringify(verdict, null, 2), "utf-8");
process.stderr.write(`  verdict=${verdict.verdict} blocking=${verdict.blocking} (C=${verdict.severity_counts.critical} H=${verdict.severity_counts.high})\n`);
process.exit(0);
```

#### 新規 export

```ts
export function parseVerdict(text: string): {...} // 既存ロジックを export 化
export function detectDispatchFailure(
  rawOut: string,
  exitCode: number,
  yamlText: string,
): { failed: boolean; reason: string } {
  if (exitCode !== 0) return { failed: true, reason: `claude-exit-${exitCode}` };
  if (/Error:\s*Reached\s+max\s+turns/i.test(rawOut)) return { failed: true, reason: "max-turns" };
  if (/^\s*Error:/m.test(yamlText) && !/^verdict:/m.test(yamlText)) {
    return { failed: true, reason: "claude-error" };
  }
  if (!/^verdict:\s*(pass|fail)/m.test(yamlText)) return { failed: true, reason: "no-verdict-line" };
  return { failed: false, reason: "" };
}
export function makeErrorVerdict(reason: string) {
  return {
    verdict: "error",
    dispatch_error: true,
    reason,
    severity_counts: { critical: 0, high: 0, medium: 0, low: 0 },
    blocking: -1,
  };
}
```

#### `import.meta.main` ガード化

`main()` を `if (import.meta.main) { ... }` ブロック内へ移して test import 時に副作用を発生させない。`dispatch-codex.ts` と同じパターン。

### SKILL.md STEP 7 改修内容

`final-review.yaml.verdict.json` を読む箇所に dispatch_error 分岐を追記:

```
verdict.json の `dispatch_error: true` または `verdict: "error"` の場合は **fix-dispatch ループに入らず** raise-issue-on-failure.ts で起票 → loop は当 Issue を needs-human に倒して別 Issue へ進む (Issue #262)。
```

## 設計方針

- **`blocking: -1` の意義**: 既存の `dispatch-codex.ts` が "result 読み込み失敗" 時に `{verdict:"unknown", blocking:-1}` を書いている前例に揃える。`blocking == 0` (= no findings) と区別が必要なため負数センチネルを採用。
- **`verdict: "error"` の意義**: 既存の `pass` / `fail` / `unknown` 集合に追加。`unknown` は parser に該当 token が無かった意味だが、`error` は **dispatch 層が明示的に失敗判定した** 意味で意味論を分ける。
- **exit code 2 の意義**: 引数エラー (`process.exit(1)`) と dispatch 失敗を区別。
- **default max-turns 5 → 30 (final persona のみ)**: design review は plan.md のみで小さいため 5 turn で足りるが、final review は diff 全文 + test summary + issue body の合算で大きく、構造化 YAML 出力までに 1〜3 tool call (Read) を挟むと 5 turn では足りない事例が #255 で発生した。30 turn は token コストを過剰にせず実用上の余裕を持つ。
- **dispatch_error 検出ロジックの順序**: ① CLI exit !=0 (最も強いシグナル) ② max-turns 文字列 ③ `Error:` プレフィックス + verdict 行欠落 ④ verdict 行欠落 (最後)。早期 return で最小コスト。
- **生 rawOut を `resultFile` に残す**: 検出時に書き戻すことで人間がデバッグ可能。verdict は別 file (`*.verdict.json`) に書くため衝突なし。

## エラーハンドリング

`detectDispatchFailure` は純関数 (副作用なし)。`makeErrorVerdict` も同様。`main()` 内のみ I/O。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 正常系 | `parseVerdict` に有効 YAML を渡す (`verdict: pass`, severity 各 1 件) | verdict=pass, blocking=2 (C+H) |
| T02 | 正常系 | `parseVerdict` に `verdict: fail` の YAML を渡す | verdict=fail |
| T03 | 退化 (boundary) | `parseVerdict` に空文字列を渡す | verdict=unknown, blocking=0, severity all 0 |
| T04_degen_max_turns | dispatch 失敗 (max-turns) | `detectDispatchFailure("Error: Reached max turns (15)\n", 0, "Error: Reached max turns (15)")` | failed=true, reason="max-turns" |
| T05_degen_non_zero_exit | dispatch 失敗 (CLI exit) | `detectDispatchFailure("anything", 137, "anything")` | failed=true, reason="claude-exit-137" |
| T06 | 正常系 (失敗なし) | 有効 YAML + exit 0 | failed=false |
| T07_boundary_error_prefix | dispatch 失敗 (Error: prefix + verdict 行不在) | `detectDispatchFailure("Error: api timeout\n", 0, "Error: api timeout")` | failed=true, reason="claude-error" |
| T08_boundary_no_verdict | dispatch 失敗 (verdict 行不在) | YAML 風だが verdict 行なし | failed=true, reason="no-verdict-line" |
| T09 | `makeErrorVerdict` 形状確認 | `makeErrorVerdict("max-turns")` | `{verdict:"error", dispatch_error:true, reason:"max-turns", blocking:-1, severity_counts:{...}}` |
| T10 | `makeErrorVerdict` の blocking は **負数** | `result.blocking < 0` | true |

T04/T05/T07/T08 が **退化/境界ケース** に該当 (検出ロジックの分岐ごと)。

## 幾何的不変条件チェックリスト

N/A (Boolean/Partition/Assemble 系ではない、TS スクリプト改修)。

- [N/A] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか
- [N/A] 各プリミティブの face ごとの outer_loop 2D 向き（CW/CCW）が文書化されているか
- [N/A] flip_normals / same_sense の意味論が明確か（頂点順を変えるか vs 法線だけ変えるか）
- [N/A] pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか
