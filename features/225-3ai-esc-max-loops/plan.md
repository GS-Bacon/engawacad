# Plan: refactor(3ai) ESC_MAX_LOOPS と debug-spec 規約を自律方針に整合

## 自律判断ログ (Claude 直接実装方針)

本 Issue は `.claude/skills/3ai/SKILL.md` と新規 escalation スクリプトを追加する **3ai スキル自身の改訂**。
通常 light flow は STEP 6 で GLM dispatch を使うが、本 Issue は **GLM の dispatch 経路そのものを書き換える**ため、GLM 経由で実装すると以下の循環リスクが発生:

1. GLM が `escalate-glm-adversarial.ts` を実装中にバグを混入 → 次の Issue で STEP 6 が新スクリプトに依存していた場合、escalation が即座に壊れる
2. GLM が `SKILL.md` を書き換えて自分自身の動作仕様を変える = 一度の dispatch で「実装 + 規約改訂」が両方走る = 後で挙動を追えなくなる

memory `project_3ailoop_implementation_style` (「`/3ailoop` 実装は Claude 直接 + Codex 独立レビュー」) の原則は本 Issue (3ai スキル中核の改訂) にも一般化される。
よって STEP 6 (core)/6.6 (test) の GLM dispatch を **Claude 直接実装**に置き換える。STEP 6.5 (test-spec) と STEP 7 (GLM final review) と B-6 (Codex cross-cut) は通常通り実行する (= 第三者 review は保持)。

採用根拠:
- Issue body: 「dispatch スクリプトに上記制約を実装」「単体テスト ... 2 ケース」 → 実装 + テストはどちらも TS 側で、`crates/**` 変更なし → guard-crates 対象外、Claude 直接編集可
- リスク回避: GLM が自分の dispatch を書き換える self-modification を回避

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `.claude/skills/3ai/SKILL.md` L406: `ESC_MAX_LOOPS=1` → `ESC_MAX_LOOPS=3` | `dispatch-glm.ts` の token 直接計測 (推定値で代替) |
| SKILL.md L442-450 (STEP 6-C/6-D): 自律方針整合 (debug-spec 無制限追記、Codex adversarial、token cap 200k) を明文化 | #220 partial-pit detect 自体の完成 |
| 新規 `.claude/skills/3ai/scripts/escalate-glm-adversarial.ts` (Codex 3 ペルソナ adversarial + per-Issue tracker) | STEP B-3 (intent-check) / STEP 5.5 (baseline) の escalation 規約変更 |
| 新規 `.claude/skills/3ai/scripts/__tests__/escalate-glm-adversarial.test.ts` (approved / refute / token cap の 3 ケース) | ESC_MAX_LOOPS の per-Issue 動的調整 |
| SKILL.md L688 (エラー検知章) の起票タイミング更新 (新 escalation 経路追加) | dispatch-glm.ts 本体への ABI 変更 (新 escalation は外部 caller 化、dispatch-glm はそのまま再利用) |

## Non-Goals

- dispatch-glm.ts 本体の token 計測 API 化 (推定値 30k/persona × 3 = 90k/round を tracker に記録するのみ)
- 既存 `loop-adr-regen-tracker.ts` / `loop-adr-auto-accept.ts` のリファクタ (ROOT 差異のため独立コピーで実装、共通化は別 Issue)
- needs-human ラベル自動付与の loop 側との二重管理 (本 escalation script は `--add-label needs-human` を呼ぶのみ、後段は既存 `failure-tracker` と独立)
- #220 ワーキングツリー復旧 (本 Issue 完了後、別途 stash 解除して再開する手順は #220 で扱う)

## 実装対象

### A. SKILL.md 改訂 (`.claude/skills/3ai/SKILL.md`)

**L406 (ループ定数)** — before:
```
**ループ定数**: `GLM_MAX_LOOPS=3`（通常試行上限）、`ESC_MAX_LOOPS=1`（debug-spec 付き再 dispatch 上限）
```
after:
```
**ループ定数**: `GLM_MAX_LOOPS=3`（通常試行上限）、`ESC_MAX_LOOPS=3`（debug-spec 付き再 dispatch 上限）、`ESCALATION_TOKEN_CAP=200_000`（per-Issue 累積 token cap）
```

**L442-450 (STEP 6-C / 6-D)** — 自律方針整合のため全面改稿:
- STEP 6-C: Claude が `debug-spec.md` を **追記** (上限 ESC_MAX_LOOPS=3)。自律モードでは Claude 判断で無制限に追記可
- STEP 6-D (新): ESC_MAX_LOOPS 到達 → `escalate-glm-adversarial.ts` で Codex 3 ペルソナ adversarial review
  - 全 approved → debug-spec をさらに追記して再 dispatch (ESC ループ続行可、ただし token cap 200k 越えで停止)
  - 1 つでも refute → `needs-human` 自動退避
  - token cap 越え → `needs-human` 自動退避

**L688 (エラー検知章)** — STEP 6-D 起票タイミングを「Codex adversarial で refute or token cap」に修正

### B. 新規スクリプト `.claude/skills/3ai/scripts/escalate-glm-adversarial.ts`

API:
```typescript
export type EscalationOutcome =
  | { kind: "continue"; verdicts: PersonaVerdict[]; regen_count: number; token_used: number }
  | { kind: "needs_human"; reason: "refute" | "token_cap"; details: string[]; regen_count: number; token_used: number };

export async function escalate(opts: {
  issueNum: number;
  featureDir: string;
  dryRun?: boolean;
  mockMode?: "pass" | "refute";
  estimatedReviewTokens?: number;
}): Promise<EscalationOutcome>;
```

CLI:
```bash
bun escalate-glm-adversarial.ts \
  --issue <N> \
  --feature-dir features/<N>-<slug> \
  [--dry-run] [--mock-mode pass|refute]
```

exit code: 0=continue, 3=needs_human

内部 tracker: `features/.loop/glm-escalation/<issue>.json` に `{regen_count, token_used, started_at, last_updated_at, retired?}` を atomic rename で永続化。

Codex 3 ペルソナ (architect / contrarian / migration) は `loop-adr-auto-accept.ts` の `runPersonaReview` をモデルにして、対象を「ADR draft」ではなく「ci.log 抜粋 + debug-spec.md」に差し替え。

### C. テスト `.claude/skills/3ai/scripts/__tests__/escalate-glm-adversarial.test.ts`

bun test で実行。`features/.loop/glm-escalation/` は test 用 tmp dir に差し替え可能にする (`ESC_TRACKER_ROOT` env var で root を override する仕様)。

## 設計方針

- **Codex 3 ペルソナの選定**: ADR-013 を流用 (architect=設計階層整合 / contrarian=採用案 refute / migration=後方互換)。本 Issue では「実装の妥当性」judged so personas を CAD 実装向けに微調整 (architect=既存 invariant との整合 / contrarian=実装案を refute / migration=テスト互換性)
- **token cap 推定**: 1 ペルソナあたり 30k token (CAD 系 ci.log + debug-spec ペイロード)、3 ペルソナで 90k/round → 200k cap = 約 2 round 分で限界
- **エラーハンドリング**: `gh` 呼び出し失敗は warn ログ + 続行 (tracker は更新済み)
- **冪等性**: 同一 issue で 2 回 escalate() を呼んだ場合、tracker の `regen_count` は inc される (= 2 round 目として扱う)。test では tmp dir で隔離
- **mock モード**: `--mock-mode pass|refute` または env `ESC_GLM_ADV_MOCK=pass|refute` で Codex 呼び出しを bypass → test と dry-run で使用

## テスト計画 (ID 付き)

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | 同一 mock 入力で 2 回 escalate() 呼び出し → tracker state が deterministic に inc される | regen_count=1 → regen_count=2、kind 判定一致 |
| T02 | 正常系 (approved) | `--mock-mode pass`、tracker 初期化済 → kind=continue、verdicts 全 approved | exit 0、`needs-human` ラベル付与なし |
| T03 | 正常系 (refute) | `--mock-mode refute` → kind=needs_human, reason=refute | exit 3 |
| T04_boundary_token_cap | 境界 | token_used を 199k に初期化 → estimatedReviewTokens=2k で escalate → token cap 越え | kind=needs_human, reason=token_cap、exit 3 |
| T05_degen_missing_dir | 退化 | feature-dir が存在しない場合 | エラーで非 0 exit |

T02 / T03 / T04_boundary_token_cap の 3 ケースが Issue 要件「approved/refute 判定の 2 ケース」+「token cap」をカバー。T01 は state.ts 等と同様の決定性要件をカバー。

## 幾何的不変条件チェックリスト

- 該当なし (TS スキル変更のみ、B-rep 幾何処理を含まない)

## 実装順序

1. plan.md (本ファイル) 確定
2. SKILL.md 改訂
3. `escalate-glm-adversarial.ts` 実装
4. `__tests__/escalate-glm-adversarial.test.ts` 実装
5. `bun test .claude/skills/3ai/scripts/__tests__/escalate-glm-adversarial.test.ts` で green 確認
6. `cargo xtask ci` (workspace baseline) green 確認
7. commit (Closes #225) + main へ直 push
