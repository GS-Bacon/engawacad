# Claude self-review for #280 (dogfood = STEP 6.7 を自己適用)

本 Issue 自身で新規 STEP 6.7 (Claude self-review) を初実行する。
対象 diff: `.claude/skills/3ai/SKILL.md` / `agents/glm-implementer.md` /
`agents/glm-reviewer-final.md` の 3 ファイル prompt 拡張。

## architect 観点 (既存 invariant / API 契約 / 構造保証)

何を満たしているか:

- 3 ファイルとも prompt 拡張のみ。Rust API / B-rep 不変条件 / 決定性要件には無関係 (CAD コア無影響)。
- SKILL.md の STEP 6.7 を **STEP 6.6 と STEP 7 の間** に挟む構造は既存 step 命名規約と整合 (5.5, 6.5, 6.6 と同じ「dot 付き挿入」パターン)。
- `glm-implementer.md` の「result JSON 書く前に self-review.md を書く」 = result 仕様への追加。glm-result.json schema 自体は変更なく、別ファイル並列出力で互換維持。
- `glm-reviewer-final.md` の Codex 3 persona checklist は既存 issues 配列に severity 付きで積む規約 (#231 で既に重複 finding 排除導入済み)。

弱点 / リスクが残る箇所:

- なし (architect 観点で critical/high なし)

## contrarian 観点 (採用方針の反論可能性)

代替案を意図的に却下した理由:

- 代替 1: 「Codex 7.5 だけで十分。shift-left 不要」 → 却下: cycle 47-60 で 12 連続 pause した実害 (Codex usage limit) があり、Codex 呼び出し回数自体を減らす必要があった。
- 代替 2: 「GLM final reviewer を強化するだけで十分、Claude self-review は冗長」 → 却下: GLM 系 (実装 GLM + final reviewer GLM) は同モデル系で self-bias を持つ。Claude が別軸で diff を見ることで部分的に独立性を確保。Codex (= 完全独立) の代替ではないが round 1 で潰せる finding は増える。
- 代替 3: 「self-review.md を required 化せず optional に」 → 却下: prompt で optional にすると GLM は書かない (Anthropic 系 LLM の経験則)。required かつ「CI green 後 result JSON 前」の順序強制が機械的に書かせる最低条件。

直前 Issue や同 Phase の defensive semantics を退化させていないか:

- **#251 (Codex usage limit → GLM fallback)** との関係: GLM fallback は API 不在時の回避策、self-review は API 消費自体を抑える方向。**機能が重複せず相補的**。 #251 fallback は本 Issue で無効化されない。
- **#266 (defensive semantics 退化検出)** との関係: 3 persona 内訳 (architect/contrarian/migration) を**そのまま shift-left に流用**することで、 #266 で観測した「last consumer 検出畳み込み bug」「#265 defensive semantics 退化」「transitive ref 漏れ」を実装段階で防御可能になる。 #266 が辛うじて catch した観点は本 Issue 後の round 1 で catch される設計。
- **#271 (phase-close + split parent auto-close)** との関係: 独立軸 (Phase 進行を止めない構造修正 vs Codex 呼び出し削減)、衝突なし。

弱点 / リスクが残る箇所:

- **C1 (medium)**: SKILL.md の STEP 6.7 で「弱点なし → 進行」を許す = **形骸化リスク**。注意喚起「最低 1 つは仮説を出せ」を embed したが強制力なし。本 Issue 自身が dogfood として「弱点なし」と書かないことを実証 (= 本 review)。実測ベースで運用 (フォロー Issue 検討)。
- **C2 (low)**: contrarian 観点で「#266 のような defensive semantics 退化」を例示するが、Claude/GLM が #266 を読みに行く明示指示がない (= コンテキスト窓依存)。実用上は plan.md または review-*.yaml に過去 Issue が引用されるはずだが、形式保証はない。

## migration 観点 (既存テスト互換 / 後方互換性)

触った public API / golden YAML の変更があれば列挙:

- skill prompt 変更のみ。public API なし、golden YAML なし、Rust テスト変更なし。
- `dispatch-glm.ts` / `dispatch-glm-review.ts` / `dispatch-codex-3persona.ts` の **スクリプト変更なし** (= 既存 dispatch 互換)。
- glm-result.json schema に **「self-review.md ファイル名」は新フィールドとして追加しない**。並列ファイルとして `features/$N-$SLUG/glm-self-review.md` を出力するだけで、result JSON consumers (resolve-issues.ts / check-dispatch-result.ts) は変更不要。
- SKILL.md の STEP 6.7 ゲート (`features/$N-$SLUG/glm-self-review.md` 存在) は新規。既存 feature dir には影響しない (state shim で skip 可能)。

既存 acceptance test を改変したならその理由:

- なし。Rust テスト無変更。

弱点 / リスクが残る箇所:

- **M1 (medium)**: GLM core 実装が CI green 達成できず failed で返した場合、self-review.md は書かれない (= 仕様通り)。STEP 6.7 ゲートは「ファイル存在チェック」なので、失敗ループ中は STEP 6.7 に進めず STEP 6-C/6-D の通常 escalation 経路を辿る。 → 仕様整合、退化なし。
- **M2 (low)**: STEP 6.7 を呼び出す自動化スクリプト (dispatch / orchestrator) は本 Issue では未追加。 SKILL.md の手順に従い Claude が手動実行する。 → 将来 loop 自動化時にゲートを機械検証する仕組みが必要 (フォロー Issue 候補)。
- **M3 (low)**: 既存 feature (state shim なし) が STEP 6.7 に到達すると「glm-self-review.md がない」で blocking する可能性。 → 後方互換のため、 STEP 6.7 ゲートに「ファイル不在なら警告のみで進行」フォールバックを将来追加する余地あり。本 Issue では強制ゲートで開始 (退化検出を優先)。

## 残課題 (scope-defer / 後続 Issue 候補)

- **F1 (medium)**: 効果計測 (Codex round 2 突入率) を実装後 5 Issue で測定 → 効果が薄ければ STEP 6.7 のチェックリスト強化 or 廃止判断。 → **フォロー Issue 候補** (本 Issue 完了条件にも明記済み)。
- **F2 (low)**: STEP 6.7 を機械化するための loop side orchestrator 改修 (= self-review.md ゲートの状態管理)。本 Issue では手動運用で開始。 → loop 側でゲート機械化が必要になれば次の self-review-orchestrator Issue を起票。
- **F3 (low)**: GLM core 実装が failed で返した場合に「何が詰まったか」の self-review.md を書かせる拡張。STEP 6-D escalation の精度向上に寄与可能。 → scope 外、 6-D adversarial と相補設計。

## 結論

弱点判定:

- **critical**: 0 件
- **high**: 0 件
- **medium**: 3 件 (C1 / M1 / F1)
- **low**: 4 件 (C2 / M2 / M3 / F2 / F3)

SKILL.md STEP 6.7 の判定基準より「**medium のみ → claude-self-review.md に記録、STEP 7 へ進む**」。STEP 7 (GLM final review) でも独立軸で見てもらう。
