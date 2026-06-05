# test-spec.md — #55 Fuse(box+cyl) manifold fix

## 実装差分サマリ (STEP 6.5 作成時点)

### partition.rs: ring 内ループ CW/CCW 補正 (partition.rs:582-618)
`collect_ordered_circle_polygon` は atan2 昇順ソート(CCW)で内ループ点を生成するが、
3D 空間での向きが交線円 normal に対して CCW になる場合がある。
そのまま assemble に渡すと cylinder band の境界円(こちらは CW)と**逆向きで 2 本**の forward HE
が生まれ、manifold violation になる。

修正: `inner_poly_3d` を交線円の `Curve::Circle { normal }` で向き判定し、
CCW なら `reverse()` して CW に揃える。

新たに生じた分岐:
- A: `circle_normal_opt` が None (交線セグメントに Circle 曲線なし) → 補正なし
- B: `n_ip < 3` (内ループ点が 2 点以下、退化) → 補正なし
- C: `is_ccw = false` (すでに CW) → 補正なし・そのまま
- D: `is_ccw = true` (CCW を検出) → `reverse()` して CW に揃える ← **今回のバグ修正の本体**

### topology.rs: 計装 eprintln! が残存
STEP A の診断コード (`eprintln!("DIAG edge#...")`) が `topology.rs:239-249` に残っている。
**STEP 6.6 で GLM が必ず削除すること。**

### examples_smoke.rs: #[ignore] 解除
`boolean_fuse_box_cyl` テストの `#[ignore]` が外れ、smoke で通るようになった。

---

## 不足テスト（plan 計画分）

acceptance テスト T01〜T07 はすべてスケルトン (`todo!()` + `#[ignore]`) のまま。

| ID | 実装必要な内容 | 対応 acceptance 関数 |
|----|----------------|----------------------|
| T01 | 決定性: seed=0 で 2 回 build → 全 ID・座標 assert_eq! | `t01_determinism` |
| T02 | manifold: `validate_manifold()` が `is_ok()` | `t02_manifold` |
| T03 | Euler: V-E+F を数えて `== 2` | `t03_euler_poincare` |
| T04 | smoke build: `build_fuse_result(0)` が panic しない | `t04_smoke_build` |
| T05 | regression: `boolean_box_fuse.mycad` と `boolean_intersect_box_cyl.mycad` が smoke OK | `t05_regression_existing_boolean` |
| T06 | 片側貫通: cylinder を z=0..15(box の上面だけ交差)の Fuse で manifold OK | `t06_single_pierce_fuse_manifold` |
| T07 | 数値境界: cylinder origin の z を ±1e-8 ずらして交線がεに近い配置でも manifold OK | `t07_numerical_eps_boundary` |

---

## 実装差分から追加すべきテスト（plan 計画外）

| 追加ID | 種別 | 内容 | 期待結果 |
|--------|------|------|----------|
| T08 | 分岐C | is_ccw=false の配置(もともと CW の内ループ)で manifold OK かつ reverse されない | `is_ok()` |
| T09 | 計装削除確認 | topology.rs に "DIAG" が含まれないことをコードで assert (後述) | コンパイル後の文字列を grep しない |

> T09 は自動テストで書くより CI の `grep` チェックが適切。GLM には不要。

---

## エッジケース・退化入力

- `circle_segs` が空(交線なし): `circle_segs.is_empty()` で既に早期 return → テスト不要
- `n_ip == 0` または `n_ip == 1`: B 分岐でスキップ → T02 で間接カバー
- `n_ip == 2`: B 分岐でスキップ → 退化入力は現行の examples と異なる。T02 で正常系をカバー
- 完全に同じ方向(is_ccw が数値的に 0 近傍): cross product が小さすぎる場合は現行コードでは `dot < 0` で CW 判定される。T07 で数値境界を確認

---

## 数値境界

- ε_snap = 1e-9: 頂点マージの許容。T07 で交線円を z=±5 から ±1e-8 ずらして確認
- ε_angle = 1e-9: 今回の修正は角度ではなく法線の dot を使う。dot の閾値は 0.0(符号のみ判定)で数値的に安定

---

## 決定性

T01 で同一入力 2 回実行 → 全 EntityID・頂点座標が一致することを assert。
`reverse()` の呼び出しは同一入力に対して常に同じ → 決定性は保たれる。

---

## STEP 6.6 GLM への指示

1. `crates/mycad-kernel/src/brep/topology.rs` の `eprintln!("DIAG ...")` ブロック(13行)を**必ず削除**すること。
2. `crates/mycad-build/tests/fuse_box_cyl_acceptance.rs` の T01〜T08 を実装し、各関数の `#[ignore]` を外すこと。
3. T09(計装削除確認)は GLM が実装不要 — CI の fmt/clippy が eprintln! の残留をある程度カバーする。
4. `cargo xtask ci` を通してから完了とすること。
