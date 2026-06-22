## 自律判断ログ (B-3 / B-5)

- 本 Issue は `.claude/skills/3ai/**` の prompt 拡張のみ。CAD コア (`crates/`) は触らない。
- 着手時点で 3 ファイルの「Issue #280 完全対応」差分が **既にローカルで uncommitted** だった
  (SKILL.md / glm-implementer.md / glm-reviewer-final.md)。差分内容は Issue #280 本文の「やること」
  3 項目 + #251/#266/#271 の文脈と一致しているため、**既存編集を採用** する判断を取る。
- light flow を以下のように簡略化する:
  - STEP 5 (branch 作成): CLAUDE.md 規範「branch・PR 不要、main 直 push」に従い skip
  - STEP 5.5 (acceptance skeleton): Rust 単体テスト対象外 (skill prompt のみ) のため skip
  - STEP 6 / 6.5 / 6.6 (GLM core 実装 + test 実装): 既存実装採用のため dispatch なし。state は shim で passed に倒す
  - STEP 6.7 (Claude self-review): 本 Issue で **追加した STEP 6.7 自体** を自己適用する。3 観点で diff を見て `claude-self-review.md` を作る (= dogfood)
  - STEP 7 (GLM final review): skill prompt 変更でも品質ゲートとして実行する
  - STEP 7.5 (Codex 独立 gate): `keep_codex_gate=false` (batch:skill) のため skip、state shim
  - STEP 8: squash commit + finalize-feature.ts

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `SKILL.md` に STEP 6.7 (Claude self-review) セクション追加 | STEP 6.7 を呼ぶ 3ailoop 側 orchestrator 改修 |
| `agents/glm-implementer.md` に「完了直前 self-review.md 出力」指示追加 | self-review.md の自動 parse / 自動 escalation |
| `agents/glm-reviewer-final.md` に Codex 3 persona checklist embed | Codex 7.5 そのものの prompt 改修 |
| 既存編集の dogfood として `claude-self-review.md` を出力 | 効果計測 (Codex round 2 突入率) — フォロー Issue 検討 |

## Non-Goals

- Codex 3 persona dispatch の挙動変更 (`dispatch-codex-3persona.ts`) — prompt 側だけで shift-left する
- GLM core implementer の Rust テスト追加 (本 Issue は skill prompt のみ)
- `escalate-glm-adversarial.ts` / `loop-adr-auto-accept.ts` の改修

## 実装対象

<!-- Issue: #280 -->
影響ファイル (skill prompt のみ、crates/ は変更なし):

- `.claude/skills/3ai/SKILL.md` — STEP 6.7 (Claude self-review) を STEP 6.6 と STEP 7 の間に追加
- `.claude/skills/3ai/agents/glm-implementer.md` — 「完了直前: GLM self-review.md 出力 (#280)」セクション追加 (3 観点: architect / contrarian / migration)
- `.claude/skills/3ai/agents/glm-reviewer-final.md` — 「0. Codex 3 persona checklist (#280 — shift-left review)」セクションを「## レビュー観点」直下に embed

各差分の意図:

1. GLM core 実装側 (glm-implementer.md) — 実装完了直前に `glm-self-review.md` を 3 観点で書く義務化。 = GLM が「Codex に指摘されそうな箇所」を**事前防御**する shift-left。
2. GLM final review 側 (glm-reviewer-final.md) — Codex 3 persona と**同じ観点**を GLM final で先取り適用。 = Codex round 2 (retry) 不要化。
3. SKILL.md STEP 6.7 — Claude も自分で diff を 3 観点で確認し `claude-self-review.md` を出力。 critical/high 検出時は GLM 再実装ループに戻すゲートを設定。

## 設計方針

- prompt 拡張のみで決定性 / トポロジー要件は影響なし
- `glm-implementer.md` の改修は **CI green 達成後** に出力させる ( = result JSON 書き出しより前)。result JSON だけ書いて self-review を書き忘れる事故を防ぐ
- `glm-reviewer-final.md` の Codex 3 persona checklist は既存 review 観点と **重複扱いしない** (severity を付けて issues 配列に普通に積む)。重複 finding 排除は STEP 7 集約側で既に行っている (#231 で導入済み)
- SKILL.md STEP 6.7 の弱点判定基準は「critical/high が 1 件以上 → STEP 6.x 戻し」「medium のみ → 記録のみ進行」「弱点なし → 進行 + 注意喚起」の 3 段
- 効果計測 (round 2 突入率) はスコープ外。フォロー Issue を立てて状況見守る

## テスト計画 (ID 付き)

skill prompt 変更のため Rust 単体テストはない。テスト相当の検証は以下:

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 構造 | `SKILL.md` に新セクション `## STEP 6.7: Claude self-review (#280 — Codex 往復削減 shift-left)` が存在する | `grep -q '^## STEP 6.7' SKILL.md` exit 0 |
| T02 | 構造 | `glm-implementer.md` に `## 完了直前: GLM self-review.md 出力 (#280)` が存在する | `grep -q '完了直前: GLM self-review.md 出力 (#280)' glm-implementer.md` exit 0 |
| T03 | 構造 | `glm-reviewer-final.md` に `### 0. Codex 3 persona checklist (#280 — shift-left review)` が存在する | `grep -q 'Codex 3 persona checklist (#280' glm-reviewer-final.md` exit 0 |
| T04_boundary | dogfood | 本 Issue 自身で `claude-self-review.md` を 3 観点で出力できる | `features/280-3ai-self-review-codex/claude-self-review.md` が architect / contrarian / migration の 3 セクションを持つ |
| T05 | 既存非破壊 | `cargo xtask ci` green (skill 変更で Rust ビルド・テストに影響しないことを確認) | exit 0 |

退化/境界 ID は T04 (dogfood = 本 Issue の最終境界条件) でカバー。

## 幾何的不変条件チェックリスト

非該当 Issue (Boolean/Partition/Assemble 系ではない):

- [N/A] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか
- [N/A] 各プリミティブの face ごとの outer_loop 2D 向き (CW/CCW) が文書化されているか
- [N/A] flip_normals / same_sense の意味論が明確か (頂点順を変えるか vs 法線だけ変えるか)
- [N/A] pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか
