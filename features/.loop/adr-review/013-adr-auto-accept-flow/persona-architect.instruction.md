# Role: ADR architect (過去 ADR との整合 / 設計階層の安定性で refute せよ)

## Task
以下の ADR draft を adversarial に review してください。
REFUTE を default とし、明確に refute できなければ approved を返してください。
理由が浅い (1 文以下、根拠なし) refute は失敗判定とし approved を返してください。

## 出力フォーマット (必須)
最終行に必ず以下のいずれかを記載:
  verdict: approved
  verdict: refuted

refuted の場合、その直前に 200 字以上の refute 理由を記載してください。

## ADR draft

# ADR-013: ADR 自動 accept フロー (Phase 8〜20 完全自律)

**Date**: 2026-06-17
**Status**: Accepted
**Related**: ADR-002 (ロードマップ・ラベル運用), ADR-006 (Issue 粒度), ADR-012 (tmux ランタイム), ADR-014 (Component RefPlane 隔離), plan: `~/.claude/plans/milestone-issue-3ailoopskill-ui-majestic-kahan.md`

---

## Context

`/3ailoop` を **Phase 8 から Phase 20 (自律実装期完了) まで人間ノータッチで** 自走させる方針が決定された (plan: milestone-issue-3ailoopskill-ui-majestic-kahan.md)。Phase 21-24 (UI 期) は人間レビュー前提。

現状の `/3ailoop` には ADR pause 機構 (L-5.6 `loop-adr-pause-detector`) があり、サイクル中に `/3ai` が新規 ADR draft を起こすと `gate:adr-review` Issue を起票して loop が pause する。**人間が draft を読み、accepted/rejected の判定をした上で gate ラベルを削除するまで loop は再開しない**。

Phase 8-20 で想定される ADR は約 10-12 件 (Phase 8 ref_planes 継承 / Phase 9 履歴 CRUD + CLI 命名 / Phase 11 拘束ソルバ / Phase 14 sketch→solid / Phase 15 NURBS / Phase 17 Mate / Phase 19 STEP I/O / Phase 12/16/20 ベスプラ Pass)。**毎 ADR で人間介入を要求する現状は完全自律方針と矛盾する**。

一方、無防備に `gate:adr-review` を取り払う ("auto-accept everything") とリスクが大きい:

- ADR 1 件の誤判定が後 Phase に伝播する (Phase 8 ref_planes 誤選択 → Phase 17 Mate データモデルで露呈、深い手戻り)
- Codex draft 単独で「思いついた option を採用」する判断は構造的に reviewable でない
- 既知 race (memory: `project_3ailoop_known_races`) と組み合わさると loop 自体の不整合発生時の検知が遅れる

**多重防御で品質を担保しつつ pause を取り払う** のが本 ADR のスコープ。

---

## Decision

**`gate:adr-review` pause を自動 accept フローに置き換える**。具体的には以下 4 重防御を `loop-adr-pause-detector` に組み込む。

### 設計

#### 1. Decision Matrix Lint (mechanical)

新規スクリプト `.claude/skills/3ailoop/scripts/loop-adr-decision-matrix-lint.ts` で ADR draft markdown を読み、以下必須セクションの存在を機械判定する:

- `## Decision` 直下に **Options A/B/C (最低 3 つ)** の列挙
- 各 option の **Trade-off** (賛否両面)
- **採用 option** とその選択理由
- **採用前提崩壊 trigger** (= この前提が崩れたら ADR 見直しという閾値条件、最低 1 件)
- **既存 ADR との関係** (引用 or 競合のリスト、無関係なら "Independent" と明記)

満たさない draft は `lint-fail` で reject → draft 再生成 (cap 内)。Codex が「1 案のみ書いて採用」とする運用を構造的に封じる。

#### 2. Multi-LLM Adversarial Review

Decision Matrix Lint を通った draft を **GLM 3 ペルソナで adversarial verify**:

- **architect**: 過去 ADR との整合性、設計階層の安定性
- **contrarian**: 採用 option を refute、却下 option の利点を強調
- **migration**: 既存実装からの移行容易性、後方互換性

各ペルソナは **「refute デフォルト」プロンプト** で動かす:

> 本 ADR draft の採用 option を refute せよ。refute できなければ approved を返せ。理由が浅ければ refute 失敗と判定し approved とせよ。

3 ペルソナ全員が `approved` を返した場合のみ **auto-accept** (`gate:adr-review` 削除 + Issue close)。
1 ペルソナでも `refute_succeeded` を返したら draft 再生成 (Codex に refute 内容を feed して書き直させる)。

`dispatch-glm.ts` に `--mode adr-review` を追加し、3 ペルソナ並列実行する。

#### 3. 暴走防止 cap

無限ループ・コスト暴走を防ぐ:

- **再生成 cap = 3 回**: draft 再生成が 3 回を超えたら該当 ADR Issue に `needs-human` を付与して退避。loop 全体は止めず、batch-select は次 Issue へ進む。
- **1 ADR あたり token 上限 = 200k**: draft + review の累積 token が 200k を超えたら同じく `needs-human` 退避。
- **token-meter 累積 100M pause**: 既存機構 (`loop-token-meter`) はそのまま残す。loop 全体の token 暴走に対する最終 line of defense。

カウンタは `features/.loop/adr-regen-count/<adr-slug>.json` に永続化:

```json
{
  "adr": "013-adr-auto-accept-flow",
  "regen_count": 2,
  "token_used": 134000,
  "started_at": "2026-06-17T12:00:00Z",
  "last_updated_at": "2026-06-17T12:30:00Z"
}
```

#### 4. Fable 5 全体監査 (3 Phase ごと)

memory: `feedback_fable5_strategy` で「2 倍コストの主な投入先は『3 Phase ごと全体監査』」と決めている。本 ADR ではこれを **Phase 11 / 14 / 17 / 20 完了時に自動起票** する:

- `loop-phase-close-check.ts apply` が Phase N 完了処理を実行する直後 (N ∈ {11, 14, 17, 20}) に、Fable 5 監査 Issue を 1 件起票
- Body: 「直近 3 Phase の ADR + 実装乖離 + 過去 ADR 矛盾を Fable 5 で全体審査」
- Labels: `type: foundation`, `batch:kernel` (Fable 5 が消化可能、batch-select で次 cycle に pick される)
- Findings は監査 Issue 消化中に Codex review の `split_proposal` で別 Issue に派生 → loop が順次消化

これにより 3 Phase 内で設計ドリフトが検知され、Phase 20 ゲート (UI 期前) に到達した時点では監査済みの ADR 群となる。

### 採用前提崩壊 trigger

以下のいずれかが観測されたら本 ADR を見直す:

- ADR draft が 3 ADR 連続で `needs-human` 退避 (= 多重防御が正常 ADR を refute し続ける = 判定基準が厳しすぎ)
- Multi-LLM Review の 3 ペルソナが同一 refute 理由を出すパターンが 5 ADR 連続 (= ペルソナ多様性が崩れている)
- Fable 5 監査で 3 期連続 critical findings 0 (= 監査が形骸化、本当に問題なしか判定基準が緩いか不明)
- ADR auto-accept 後の Phase で同一 ADR を引用するコードが 3 件以上 needs-human 退避 (= ADR 採用判断が誤っていた事後証拠)

trigger 発火時は loop 一時停止 + 本 ADR の見直し Issue を起票。

### 既存 ADR との関係

- **ADR-002 (ロードマップ・ラベル運用)**: `gate:adr-review` ラベル自体は廃止せず残す。auto-accept フローが何らかの理由で動かなくなった fallback として使う (例: GLM API outage 時は手動 accept)。
- **ADR-006 (Issue 粒度)**: 「1 ADR = 1 cycle で扱える粒度」は維持。再生成 cap 3 と token 上限 200k はこれを担保する。
- **ADR-012 (tmux ランタイム)**: L-5.6 の挙動を本 ADR が上書きする。SKILL.md を併せて更新する。

---

## Alternatives Considered

### (a) 現状維持 (gate:adr-review pause を残す)

ADR 品質は最高。しかし完全自律方針と矛盾。Phase 8-20 で 10+ 回の人間介入が必須となり、UI 期入口到達まで数ヶ月レベルで遅延する。

### (b) 無防備 auto-accept

`gate:adr-review` 機構を取り払い Codex draft をそのまま採用。コストは最小だが ADR 品質が暴落、後 Phase で深い手戻りが発生する確率が高い。Phase 20 ゲート (CLI 安定化レビュー + E2E シナリオ) で全部露呈し、UI 期突入が逆に遅れる。

### (c) 人間 ADR 判定を別 Claude セッションが代行

別 Claude セッションを常駐させ ADR draft を読んで accepted/rejected ラベル付け。実質「人間役を Claude が演じる」だけで Multi-LLM Review (本 ADR) と同義。ペルソナ多様性を明示しない分、本 ADR (b) よりリスクが高い。

---

## Consequences

### 影響範囲

| 項目 | 影響 |
|---|---|
| `loop-adr-pause-detector.ts` | 改修 (gate 起票 → 自動 review chain → pass で gate 削除 / fail で再生成 or needs-human) |
| `loop-adr-decision-matrix-lint.ts` | 新規 |
| `loop-phase-close-check.ts` | Phase 11/14/17/20 完了時の Fable 5 監査 Issue 自動起票を追加 |
| `dispatch-glm.ts` | `--mode adr-review` 追加 (3 ペルソナ並列・adversarial refute プロンプト) |
| `features/.loop/adr-regen-count/` | 新規状態ディレクトリ (1 ADR 1 json) |
| `.claude/skills/3ailoop/SKILL.md` | L-5.6 の挙動説明書き換え |
| `docs/3ailoop-runbook.md` (存在すれば) | ADR 自動 accept フロー §追加 |
| ADR-002 | ラベル運用に "fallback 用途" 追記 |

### 残置リスク

- **Multi-LLM Review の判定基準ずれ**: 3 ペルソナが「approved」と全員返したのに実装段階で問題発生するケース → trigger で検知して人間介入に戻す
- **Fable 5 監査コスト**: Phase 11/14/17/20 で計 4 回の Fable 5 起動、各 ~500k token 想定 = 約 $30-50 × 4 ≈ $150 程度 (Phase 8-20 全体コストの数% 程度)
- **再生成 cap 3 の妥当性**: 実運用で 3 回 retry の中で accept されるのが大半か、それとも頻繁に needs-human 退避するか観測が必要 → 最初の 5 ADR で再評価
- **GLM API outage 時の挙動**: dispatch-glm が fail したら auto-accept フロー全体が止まり、loop が pause する。fallback として gate:adr-review ラベルを残し、手動 accept 経路を維持

### コスト試算

- ADR 1 件: draft 30k + review (3 × 30k) + 再生成平均 2 回 = ~150k token (~$2-3)
- Phase 8-20 全体 12 件 = ~$25-40
- Fable 5 監査 4 回 = ~$150
- **合計 ~$200 程度** (token-meter 100M 上限の数% 以内、許容範囲)

### 移行手順

本 ADR で決定した方針を 4 Issue に分割して実装する (ADR-006 §1 粒度ガードに沿う):

1. **#190**: `loop-adr-pause-detector.ts` 改修 + auto-accept フロー骨格 (`type: foundation` / `batch:skill`)
2. **#191**: `loop-adr-decision-matrix-lint.ts` 新規 + テスト (`type: feature` / `batch:skill`)
3. **#192**: 再生成 cap 3 + token 200k 上限実装 (`type: foundation` / `batch:skill`)
4. **#193**: `loop-phase-close-check.ts` Fable 5 トリガー追加 (`type: feature` / `batch:skill`)

並列化はしない。直列実行で 1 PR 1 Issue 1 cycle 原則を維持する。
