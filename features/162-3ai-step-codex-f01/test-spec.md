# test-spec.md — Issue #162

## 概要
STEP 6 で GLM が `build_component_tree` の dead パラメータ `_ref_planes` を削除し、3 箇所の呼び出し側を更新した。テスト面では `t17_degen_grandchild_canonical_fallback` がフル実装され ADR-014 の grandchild canonical fallback を regression として固定。`cargo xtask ci` が green。

## 不足テスト (plan 計画分)
plan のテスト計画 ID 表:
- T01 (決定性): 既存テスト `t01_determinism` が build_bodies_from_features 経由で root のみケースを検証。本 Issue はパラメータ除去のみで内部ロジックは不変。新規 T01 dispatch 不要。
- T15 / T16 (既存 regression): 変更なし pass を確認済み (パラメータ除去後も挙動 identical)。
- T17_degen_grandchild_canonical_fallback: GLM が STEP 6 で `crates/engawa-build/tests/refplane_acceptance.rs:451-` に full 実装 → 単体 run pass を確認 (`cargo test -p engawa-build --test refplane_acceptance t17`)。

→ **不足テストなし**。

## 実装差分から追加すべきテスト
diff (3 ファイル / +104 -10):
1. `crates/engawa-build/src/lib.rs`: signature 削減 + コメント書き換えのみ。新規分岐・ガードなし。
2. `crates/engawa-build/tests/refplane_acceptance.rs`: T17 追加のみ。
3. `docs/decisions/013-adr-auto-accept-flow.md`: Related 行 cross-link 追加のみ (テスト不要)。
4. `docs/decisions/014-component-refplane-isolation.md`: 新規 ADR (テスト不要)。

→ **追加テスト不要**。実装差分は signature 削減 + 既存 regression (T15/T16) の対象内であり、T17 が深ネスト境界を新規 cover。

## エッジケース・退化入力
- reference subtree (`ComponentRef::StdLib` / `ComponentRef::File`) の場合 (旧 caller 2): `crates/engawa-build/src/lib.rs:543-554` の呼び出し側は param drop のみ。`&ref_doc.root_component.ref_planes` を渡さなくなったが、`build_component_tree` 内で再帰先 component の `effective_ref_planes` を独自導出するため挙動 identical。既存 `examples_smoke` の `assembly.engawa` 系統で間接的に cover (parser 経由)。
- depth=16 (MAX_REFERENCE_DEPTH): パラメータ削除はガード不変、既存 `MaxDepthExceeded` テストがあれば cover (本 Issue では追加不要)。
- 親 non-empty + 中間 empty + 孫 empty の中間 fallback ケース: T17 は親 custom-only + 中間 custom-only + 孫 empty を cover。中間 empty + 孫 empty の二段 fallback は `t16_empty_child_falls_back_to_canonical_not_parent` が間接的に cover (root → 1段 child)。追加不要。

## 数値境界
- N/A (signature cleanup のみ、数値ロジック無変更)

## 決定性
- T01 (root のみ) は既存 pass。
- assembly 経由の決定性は `examples_smoke` の `assembly.engawa` が間接 cover。新規 grandchild 構成での決定性は ID 生成順序が変わらないため追加不要 (`IdGenerator` は組み立て順を保持)。

## 結論
**STEP 6.6 (GLM テスト実装) はスキップ可能**。STEP 6 で GLM が plan T17 を full 実装し、CI green を確認済み。state shim は不要 (STEP 6.6 の追加実装が無い)。

ただし state machine 上は `glm_impl` を passed にする必要があるため、明示的に状態遷移:

```bash
bun .claude/skills/3ai/scripts/state.ts set features/162-3ai-step-codex-f01/state.json glm_impl passed
```

期待値乖離: `check-spec-divergence.ts` が "数値段落なし" を返したのは本 Issue がテスト計画ID に数値段落を持たない設計 (T01/T15/T16/T17 すべて bool / Vec 比較ベース) のため。乖離なし。
