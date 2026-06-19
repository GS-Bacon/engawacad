<!-- Round ごとに以下の形式で追記すること -->
<!-- ## Round N -->
<!-- - R0X: 「<指摘の要点>」を棄却 — <理由（scope 外、Non-Goals に記載済み、次 issue で対応 等）> -->

## Round 1

- IN01 (invariant, critical) を棄却 — 「clone & remove パターンに決定性脆弱性」は実コード未読の hallucination。理由 3 点:
  1. `engawa_format::Document::root_component.features` は `Vec<Feature>` (順序保証型) であり、`remove(idx)` は決定的な O(n) 左シフト操作。順序非保証コンテナの可能性に言及しているが事実誤認。
  2. `check_refs_resolve_before` / `check_no_downstream_break` の内部呼び先 `simulate_history` は `for (i, f) in features.iter().enumerate()` で `Vec` を順序走査しており、`HashMap` は `sketches_at` / `live_bodies_at` の存在判定のみで出力順序に伝播しない。既存 `insert` (#255) も同じパターンで決定性テスト T01 を pass している実証あり。
  3. T01 (同一入力を 2 回適用して `to_yaml()` byte-equal を assert) が決定性を実証する設計になっており、「3 段階それぞれの結合テストが不足」という指摘は冗長 (byte-equal が満たされれば内部段階の順序非依存性は自動的に担保される)。
- 結果: critical/high 0、scope/ambig も pass。次 round に進む (収束判定のため 2 round 連続 C/H=0 を要する)。

## Round 2

- SC01 (scope, critical) を棄却 — 「In-Scope / Out-of-Scope セクションが存在しない」は事実誤認 (hallucination)。plan.md L13-18 に `## In-Scope / Out-of-Scope` セクションが In-Scope 4 行 / Out-of-Scope 4 行で実在し、ADR-006 §plan.md 必須項目を満たしている。GLM が plan を読まずに虚偽の指摘を返したと判断。yaml も ```yaml code fence で wrap + verdict:pass と critical issue の矛盾を含む dispatch_error 状態 (script exit 2) で、scope 自身も内部矛盾している。
- AM01 (ambig, medium) を採用 — T03 の golden 比較方式が plan.md 内で曖昧 ("外部 golden file" or "テスト内 to_yaml() byte-equal" のどちらか不明)。テスト計画表に「テスト内で `Document::to_yaml()` を呼んで期待 YAML を組み立て、`read_to_string(out.engawa)` と byte-equal を assert (外部 golden ファイル不要)」と具体化済み。

## Round 3 (design_loops light=3 上限到達)

- SC03 (scope, critical, dispatch_error) を棄却 — 「In-Scope / Out-of-Scope セクションが存在しない」は r2 SC01 と同じ hallucination の繰り返し。plan.md L13-18 に明白に実在。
- IN01 (invariant, critical, dispatch_error) を棄却 — 「IdGenerator を使わず Uuid::new_v4() を使用する設計になっている」は plan 完全未読の hallucination。plan.md に `Uuid` も `IdGenerator` も一切言及なし (本 Issue は履歴 Document の純関数変換で、ID 生成は本 Issue 範囲外)。
- IN02 (invariant, high, dispatch_error) を棄却 — 「T01 決定性テストがテスト計画に存在しない」は plan 完全未読の hallucination。plan.md L100 のテスト計画 ID 表に T01「同一 (doc, feature_id, new_feature) を 2 回 `edit` → 結果 Document の `to_yaml()` byte-equal」と明白に記載。
- 3-E ループ上限超過判定: raw critical/high (棄却前) は ≥ 1 だが、3 round 連続で GLM が plan 内容を全く参照せず hallucination を返している実態 (scope は r2/r3 連続で「セクションがない」と虚偽報告 / invariant は r3 で 4 軸 hallucination)。**棄却後の critical/high = 0** で設計実害なし。
- **Claude 裁量で 3-F へ進める判定**: 設計は scope/ambig 観点で 3 round とも pass。invariant は r2 で pass、r1/r3 のみ hallucination。設計自体に問題はなく、各 round で出た critical/high はすべて GLM の plan 未読が原因。よって design_review passed を確定し STEP 5 に進む。


