# test-spec.md — Issue #54 cyl×sphere intersect 上側 cap 欠落修正

## 不足テスト（plan 計画分）

plan の T08 / T09 は GLM がコアモードで実装済み (`#[ignore]` 解除済み)。
plan の T01 / T05 / T18 / T22b / T34 は既存テストで継続 pass。
→ **plan 計画のテストは全て実装済み。追加不要。**

## 実装差分から追加すべきテスト

変更差分は以下の 2 箇所のみ:
1. `tessellation/mod.rs:756`: `trim_lower = circ_normal.z > 0.0` → `trim_lower = center_z < center.coords.z`
2. `tessellation/mod.rs:842`: `let _ = (circ_radius, circ_normal, base_idx);`（clippy suppress）

コード変更から生じる追加ケース:
- **球中心より上の交線円 (center_z > sph_center.z)**: 新判定で `trim_lower=false` → 北極方向サンプル。T08/T09 が直接カバー。
- **球中心より下の交線円 (center_z < sph_center.z)**: `trim_lower=true` → 従来どおり。T08 の `min_z≈-5` がカバー。
- **same_sense = false の cap**: 実装内 `same_sense` 分岐は trim_lower 変更と直交。T22b（解像度テスト）が副次的にカバー。

**追加実装を要するテストなし**（T08/T09 が差分を十分カバー）。

## エッジケース・退化入力

| ケース | 対処状況 | 補足 |
|--------|----------|------|
| 接線交差 (cyl_r = sph_r) | 既存 T07 pass | cap 生成なし、trim 到達しない |
| 非軸整列 (cyl 軸 ≠ Z) | 既存 T06 pass | 本修正は軸方向に依存しない（`center_z < center.coords.z` は `circ_center.coords.z < sph_center.coords.z` と等価） |
| 多重 inner loop | `TrimmedFaceUnsupported` エラーで早期リターン。変更なし | 本 Issue out-of-scope |

## 数値境界

- `center_z < center.coords.z` は厳密比較。交線は h_center ± h_offset で h_offset>0 なので等号は生じない（既存 T07 が境界を検証）。
- T08 の tol=1e-3 は `evaluate(0, ±π/2)` が解析的に ±radius を返すため十分（浮動小数誤差 < 1e-12）。

## 決定性

- T01: B-rep 決定性（既存、変更なし）
- T22b: tessellation の解像度非依存決定性（既存、変更なし）
- 新修正は純粋な比較演算のみで副作用なし。決定性に影響なし。

## 結論

test-spec 上の追加 GLM 作業は不要。STEP 6.6 (GLM test mode) はスキップ可能だが、
フロー遵守のため dispatch して test-only 空振り確認を行う。
