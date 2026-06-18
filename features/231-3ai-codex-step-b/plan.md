# Plan: refactor(3ai) Codex 3 ペルソナ STEP 7.5-B 並列化

## 自律判断ログ (Claude 直接実装方針)

本 Issue は `.claude/skills/3ai/scripts/dispatch-codex.ts` および新規 wrapper を追加する **3ai dispatch スクリプト自身の改訂**。
通常 light flow は STEP 6 で GLM dispatch を使うが、本 Issue は **dispatch 経路そのものを書き換える** ため、`project_3ailoop_implementation_style` と同じ循環リスクが発生する:

1. GLM が 7.5-B dispatch スクリプトを実装中にバグを混入 → 次 Issue で STEP 7.5 が即座に壊れる
2. GLM が自分自身を呼ぶ dispatch を書き換える self-modification

#225 で確定した「3ai dispatch 系の改訂は Claude 直接実装」原則を本 Issue にも適用。STEP 6/6.6 の GLM dispatch を Claude 直接実装に置き換える (`crates/**` 変更なし → guard-crates 対象外)。STEP 6.5 (test-spec) / STEP 7 (GLM final review) / B-6 (Codex cross-cut) は通常通り実行 (= 第三者 review は保持)。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `dispatch-codex.ts` に `--persona <name>` 追加 (`single` (legacy default) / `architect` / `contrarian` / `migration`) | persona 追加 (本 Issue は 3 ペルソナ固定、追加は別 Issue) |
| persona 指定時に `buildPrefix` 内で persona scope hint (役割と refute 方針) を prepend | persona 間の adversarial 投票ロジック変更 (現行 voting 維持) |
| `dispatchCodex` の戻り値型を `Promise<void>` → `Promise<number>` (exit code) に変更し、`process.exit` を CLI wrapper に集約 (parallel 呼び出し可能化) | `escalate-glm-adversarial.ts` の再 refactor (#225 で並列実装済、共通化は別 Issue) |
| 新規 `dispatch-codex-3persona.ts`: 3 ペルソナを `Promise.all` で並列 spawn、結果を merge して `<result>.yaml` + `<result>.yaml.verdict.json` を生成 | dispatch-codex-intent.ts / dispatch-codex-auto.ts の挙動変更 (post-await dead code が動くようになる副次効果は許容) |
| `SKILL.md` STEP 7.5-B を `dispatch-codex-3persona.ts` 呼び出しに置換 | dispatch-codex の `--mode design` 経路への persona 適用 |
| 単体テスト: 3 spawn 並列 (全 success / 1 failure / 全 failure) のマージ、persona injection、refactor 後 dispatchCodex の戻り値 | dispatch-codex.ts の token 計測 (推定値で代替) |
| `cargo xtask ci` green + bun test 全 green | loop 1 cycle 実走の wall time 計測 (期待 -35-40% は理論値で記録のみ) |

## Non-Goals

- ペルソナ追加 (本 Issue は 3 ペルソナ固定)
- adversarial 投票ロジック変更
- escalate-glm-adversarial.ts の共通化
- dispatch-codex の `--mode design` 経路への persona 適用
- 既存 `codex-final.yaml` 構造変更 (merge 結果は単一 yaml に concat、各 issue id に `[persona-prefix]` を付ける後方互換形式)
- per-persona codex-final-reviewer.md の派生 agent ファイル新設 (persona scope hint は `dispatch-codex.ts` 内で動的注入し、agent file は 1 つ維持)

## 実装対象

### A. dispatch-codex.ts 改訂

**変更 1**: persona 用 scope hint 追加

```typescript
// 新規定義 (ファイル冒頭近く)
const PERSONA_HINTS: Record<string, string> = {
  single: "",  // 既定 = 注入なし (legacy)
  architect: "# Persona: architect\n既存 invariant / API 契約 / B-rep トポロジー保証の観点で refute せよ。\n決定性 / Euler-Poincaré / HalfEdge twin 整合性に焦点。",
  contrarian: "# Persona: contrarian\n採用された修正案を refute し、棄却案の利点を強調せよ。\nスコープ逸脱 / 過度な抽象化 / 代替実装の見落としに焦点。",
  migration: "# Persona: migration\n既存テスト互換性 / 後方互換性で refute せよ。\nテスト充足性 / golden YAML / API 破壊変更に焦点。",
};
```

**変更 2**: `DispatchCodexOpts` に `persona?: keyof typeof PERSONA_HINTS` を追加

**変更 3**: `buildPrefix` を拡張 (persona scope hint を最先頭に挿入):

```typescript
function buildPrefix(scopeHint: string, extraInputFile: string, persona: string = "single"): string {
  let prefix = "";
  const personaHint = PERSONA_HINTS[persona] ?? "";
  if (personaHint) {
    prefix += `===== PERSONA SCOPE =====\n${personaHint}\n===== END PERSONA SCOPE =====\n\n`;
  }
  if (scopeHint) {
    prefix += `===== SCOPE PROFILE =====\n${scopeHint}\n===== END SCOPE PROFILE =====\n\n`;
  }
  if (extraInputFile) {
    try {
      prefix += readFileSync(extraInputFile, "utf-8") + "\n\n";
    } catch {}
  }
  return prefix;
}
```

**変更 4**: `dispatchCodex` 戻り値を `Promise<void>` → `Promise<number>` (exit code)、末尾の `process.exit(exitCode)` を `return exitCode` に変更。CLI wrapper 末尾で `process.exit(await dispatchCodex(...))`。

**変更 5**: CLI args parser に `--persona <name>` 追加。

### B. 新規スクリプト dispatch-codex-3persona.ts

API:
```typescript
export interface Persona3Opts {
  instructionFile: string;
  resultFile: string;           // 例: features/N-slug/codex-final.yaml
  extraInputFile?: string;
  scopeHint?: string;
  baseBranch?: string;
  mockMode?: "pass" | "fail" | "mixed";  // テスト用
}

export type PersonaResult = {
  persona: "architect" | "contrarian" | "migration";
  exitCode: number;
  yamlPath: string;
  verdict: "pass" | "fail" | "unknown";
  severity_counts: { critical: number; high: number; medium: number; low: number };
  blocking: number;
};

export type MergedVerdict = {
  verdict: "pass" | "fail";
  severity_counts: { critical: number; high: number; medium: number; low: number };
  blocking: number;
  per_persona: PersonaResult[];
};

export function mergePersonaResults(results: PersonaResult[]): MergedVerdict;
export async function dispatchCodex3Persona(opts: Persona3Opts): Promise<MergedVerdict>;
```

実装:
- 3 ペルソナ (`architect` / `contrarian` / `migration`) を `Promise.all` で並列 spawn
- 各 persona の result path = `<resultFile-without-ext>-<persona>.yaml` (例: `codex-final-architect.yaml`)
- spawn 完了後、各 yaml の verdict.json を読んで `PersonaResult[]` に変換
- `mergePersonaResults`: 各 severity を sum、`blocking = sum(blocking)`、`verdict = blocking > 0 ? "fail" : "pass"`
- 統合 yaml を `<resultFile>` に書く (各 persona の issues を `id` プレフィックス付きで concat、最終行に統合 verdict)
- 統合 verdict.json を `<resultFile>.verdict.json` に書く (既存 schema 後方互換: `{verdict, severity_counts, blocking}` + 拡張 `per_persona`)
- mockMode はテスト用 (CODEX_CLI 呼び出しを bypass し pre-set verdict を返す)

CLI:
```bash
bun dispatch-codex-3persona.ts \
  --instruction <path> \
  --result <path> \
  [--extra-input <path>] [--scope-hint <text>] [--base <branch>] [--mock-mode pass|fail|mixed]
```

exit code: merged blocking > 0 なら 1、それ以外 0。

### C. SKILL.md STEP 7.5-B 改訂

before (L600-606):
```bash
bun .claude/skills/3ai/scripts/dispatch-codex.ts \
  --mode review \
  --instruction .claude/skills/3ai/agents/codex-final-reviewer.md \
  --result features/$ISSUE_NUM-$ISSUE_SLUG/codex-final.yaml \
  --extra-input features/$ISSUE_NUM-$ISSUE_SLUG/codex-input.md
```

after:
```bash
bun .claude/skills/3ai/scripts/dispatch-codex-3persona.ts \
  --instruction .claude/skills/3ai/agents/codex-final-reviewer.md \
  --result features/$ISSUE_NUM-$ISSUE_SLUG/codex-final.yaml \
  --extra-input features/$ISSUE_NUM-$ISSUE_SLUG/codex-input.md
```

説明文 (L608) の追記: 「3 ペルソナ (architect / contrarian / migration) を並列実行し、各 persona の指摘を統合した `codex-final.yaml` と `codex-final.yaml.verdict.json` を生成する (#231)。1 つでも persona が critical/high を返せば `blocking >= 1` となる」。

### D. テスト

`.claude/skills/3ai/scripts/__tests__/dispatch-codex.test.ts` (新規):
- T01 (persona injection): `CODEX_DRY_RUN=1` + `dispatchCodex({persona: "architect", ...})` → stdout に `PERSONA SCOPE` ブロック + `architect` 役割が含まれる
- T02 (persona default = single): `dispatchCodex({...})` (persona 未指定) → stdout に `PERSONA SCOPE` ブロックなし (legacy 維持)
- T03 (exit code 返却): `CODEX_DRY_RUN=1` 経路で `dispatchCodex(...)` の戻り値が `0`

`.claude/skills/3ai/scripts/__tests__/dispatch-codex-3persona.test.ts` (新規):
- T04 (merge all pass): 3 persona 全 verdict=pass → merged verdict=pass, blocking=0
- T05 (merge 1 fail): 1 persona が critical 1 件 → merged verdict=fail, blocking=1
- T06 (merge all fail): 3 persona 全 fail (critical 各 1) → merged verdict=fail, blocking=3
- T07_boundary_empty_issues: 各 persona issues=[] → merged verdict=pass, blocking=0
- T08 (parallel spawn): mockMode=pass で 3 spawn → 3 個の `codex-final-<persona>.yaml` が生成され、統合 yaml/verdict.json が書き出される

## 設計方針

- **persona scope hint の挿入位置**: prefix の最先頭。Codex は instruction (codex-final-reviewer.md) + prefix + git diff の順で stdin を受ける。persona hint を最先頭にすることで「これからのレビューはこの lens で行う」と明示
- **後方互換性**: `--persona` 省略時は `single` = 現行挙動と完全一致 (PERSONA SCOPE ブロックなし)。既存呼び出し (intent.ts / auto.ts) は無改変で動く
- **dispatchCodex 戻り値変更の副次効果**: `dispatch-codex-intent.ts` lines 98-107 (process.exit 後の dead code) が動くようになる = intent.ts の本来意図通り (`aligned: yes/no` 判定が後段に伝わる)。これは既存バグ修正であり望ましい挙動
- **マージ yaml の構造**: 後方互換のため、各 persona の `issues:` を id プレフィックス (`A-F01` / `C-F01` / `M-F01`) 付きで一本化。末尾に統合 `verdict: pass|fail`。codex-findings 記録 (medium/low) を読む既存スクリプトは変更不要
- **並列度**: 3 ペルソナ固定 (Codex は外部 API 呼び出しで stateless、同時 spawn OK)。Promise.all で 3 個 spawn は問題なし
- **エラーハンドリング**: 1 persona spawn が thrower (Codex CLI exit != 0) → その persona を `verdict: "unknown", blocking: 0` として扱い、他 2 persona の verdict を採用 (merge は継続)。全 3 persona が unknown のみ → merged verdict="fail" にエスカレート (silent fail 防止)
- **テスト用 mock**: `mockMode: "pass" | "fail" | "mixed"` は Codex CLI を呼ばずに pre-set verdict を返す。`CODEX_DRY_RUN=1` 環境変数も尊重 (dispatch-codex の DRY_RUN と共存)

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | persona injection | `CODEX_DRY_RUN=1` + persona=architect で stdin に PERSONA SCOPE ブロック | stdout に `architect` 役割を含む |
| T02 | legacy default | persona 未指定 (single) で stdin に PERSONA SCOPE ブロックなし | stdout に `PERSONA SCOPE` 文字列なし |
| T03 | exit code 返却 | dispatchCodex(CODEX_DRY_RUN=1) 戻り値 | `0` |
| T04 | merge all pass | 3 persona 全 verdict=pass | merged verdict=pass, blocking=0 |
| T05 | merge 1 fail | 1 persona critical=1 | merged verdict=fail, blocking=1 |
| T06 | merge all fail | 3 persona 全 critical=1 | merged verdict=fail, blocking=3 |
| T07_boundary_empty_issues | 境界 | 全 persona issues=[] | merged verdict=pass, blocking=0 |
| T08 | parallel spawn | mockMode=pass で 3 spawn | `codex-final-<persona>.yaml` 3 個 + 統合 yaml/verdict.json 生成 |
| T09_degen_all_unknown | 退化 | 全 persona spawn が thrower (exit!=0) | merged verdict=fail (silent fail 防止) |

T01/T02 が persona injection の正常系/legacy、T03 が refactor 副次効果のガード、T04-T07 が merge ロジック、T08 が並列 spawn 統合、T09 が退化エッジ。

## 幾何的不変条件チェックリスト

- 該当なし (TS スキル変更のみ、B-rep 幾何処理を含まない)

## 実装順序

1. plan.md (本ファイル) 確定 ✅
2. dispatch-codex.ts: PERSONA_HINTS 定数 + buildPrefix 拡張 + DispatchCodexOpts に persona フィールド + CLI arg + 戻り値 number 化
3. 新規 `.claude/skills/3ai/scripts/dispatch-codex-3persona.ts` 実装
4. `__tests__/dispatch-codex.test.ts` (T01-T03) + `__tests__/dispatch-codex-3persona.test.ts` (T04-T09) 実装
5. `bun test .claude/skills/3ai/scripts/__tests__/dispatch-codex*.test.ts` 全 green 確認
6. SKILL.md STEP 7.5-B 改訂 (dispatch-codex → dispatch-codex-3persona)
7. `cargo xtask ci` (workspace baseline) green 確認
8. commit (Closes #231) + main へ直 push
