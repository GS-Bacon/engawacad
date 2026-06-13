# Issue #144 — 隣接面の共有境界サンプル直接比較 acceptance テスト追加

## Context

ADR-009 で「Boolean 交線円を周期エッジで保持し、両側面はエッジ駆動で境界サンプルを共有する」(案 A) を決定したが、実装本体は Phase 4 再訪タイミングで起票予定。それまでは現状の「両側が独立に同じサンプル列 (`k * 2π / N`) を再導出する」規約に依存する状態が続く。

この規約違反 (両側のサンプル列がずれる) を**構造的に検出**する手段が現状ない。テッセレーション結果が watertight でなくなった瞬間に既存の naked-edge アサーション (`bool_naked_edge_acceptance.rs` 等) は失敗するが、failure mode が「naked edge カウント」のレイヤーで上がるため、どのエッジ・どの index でずれたかが分からず Phase 7 シナリオで再発時の調査コストが高い。

本 Issue は ADR-009 §Implementation Outline Phase 3 即時実施分の最終ピース (もう片方の「`adj_is_sphere` 死に分岐削除」は #143 / commit `2fc87fc` で完了)。`type: foundation` / Phase 7 milestone への差し込み作業として ADR-002 §「差し込み作業は奉仕する Phase の Milestone に入れる」運用に沿う。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| 新規 acceptance テスト 1 ファイル `crates/mycad-kernel/tests/shared_boundary_acceptance.rs` | 既存テストの修正・削除 |
| 既存 Boolean fixture (Cut / Intersect / Fuse) の流用 | 新規 Boolean fixture の作成 |
| 失敗時診断メッセージ整備 (edge idx, vertex idx, t_a / t_b, p_a / p_b, distance mm) | tessellation / partition の実装変更 |
| テスト側 `Face.name` ユニーク化ヘルパー (`assign_unique_face_names`) | カーネル側に per-face 公開 API を追加 |
| 比較対象は `Curve::Circle` edge のみ | Line edge の比較 (直線は両端 2 頂点のみで規約破綻リスクなし、Issue 焦点外) |

## Non-Goals

- 共有境界のずれを修正すること (= ADR-009 実装本体 = 別 Issue / Phase 4 再訪時)
- 規約依存の解消 (同上)
- ADR-009 Implementation Outline Phase 1 / Phase 2 の作業
- Line edge の比較
- 新規 Boolean fixture の作成

## 実装対象

<!-- Issue: #144 -->
<!-- 影響クレート/ファイル: crates/mycad-kernel/tests/shared_boundary_acceptance.rs (新規) -->

**新規ファイル 1 つだけ** (既存関数の編集はゼロ):

```
crates/mycad-kernel/tests/shared_boundary_acceptance.rs
```

### 新規シグネチャ

| 種別 | シグネチャ | 役割 |
|---|---|---|
| ヘルパー | `fn assign_unique_face_names(solid: &mut Solid)` | テスト側で `Face.name` を `EntityRef::Named { feature_id: "shared_boundary_test", kind: Face, role: "f{i}" }` に上書き |
| ヘルパー | `fn face_id_canonical(i: usize) -> String` | canonical_name 形式 `N(shared_boundary_test;F:f{i})` を返す |
| ヘルパー | `fn collect_face_vertex_set(mesh: &TriangleMesh, face_id: &str) -> HashSet<usize>` | `mesh.face_ids[tri_idx]` を照合し per-face 頂点 idx 集合を構築 |
| ヘルパー | `fn build_he_to_face(solid: &Solid) -> HashMap<usize, usize>` | outer_loop + inner_loops を走査し half_edge_idx → face_idx の写像を構築 |
| ヘルパー | `fn project_to_circle(p, center, normal, radius, t_start) -> (f64, f64)` | 点を Circle に投影し `(t in [t_start, t_start+2π), distance)` を返す |
| ヘルパー | `fn vertices_on_circle_sorted(mesh, candidates, center, normal, radius, t_start) -> Vec<(f64, [f64;3])>` | candidates から円上 (≤ LENGTH_TOLERANCE) の頂点を抽出し t でソート + 隣接 seam 重複を位置 dedup |
| コア | `fn assert_shared_boundary(solid: &Solid, label: &str) -> usize` | Circle edge の両側面サンプル列を直接比較。検査した Circle edge 数を返す |
| Fixture | `fn make_box_cut_cyl_through() -> Solid` | `box(10³) − cyl(r=2, h=15, c=(0,0,-7.5))` (through-hole, 上下に cross-face Circle 境界) |
| Fixture | `fn make_cyl_intersect_sphere() -> Solid` | `cyl(r=3,h=20) ∩ sphere(r=4, c=(0,0,10))` |
| Fixture | `fn make_box_fuse_cyl() -> Solid` | `box(10³) ∪ cyl(r=2, h=15, c=(0,0,-7.5))` |
| Test | `t01_determinism` 他 5 件 (T02-T06) | 後述「テスト計画」参照 |

### Fixture (既存流用元)

| BooleanOp | Fixture 関数 | 流用元 | 備考 |
|---|---|---|---|
| Cut | `make_box_cut_cyl_through` | `boundary_align_acceptance::build_intersect_box_cyl` の Cut バリアント (反対方向 = 円柱が箱を貫通) | 本ファイル内に独自定義。`box(10³) − sphere(r=3, origin)` (= `bool_naked_edge_acceptance::make_box_minus_sphere`) は sphere 完全内包の void shell で cross-face Circle 境界を持たないため Cut の正常系 fixture として不適切 (Codex review #144-F02) |
| Intersect | `make_cyl_intersect_sphere` | `bool_naked_edge_acceptance::make_intersect_cyl_sphere` | 同上 |
| Fuse | `make_box_fuse_cyl` | `boundary_align_acceptance::build_fuse_box_cyl` | 同上 |

(Out-of-Scope: 既存テスト修正なし。本ファイル内で独自に再定義する)

## 設計方針

### `Face.name` 上書きトリック (カーネル変更ゼロ)

`tessellate_solid` の `face_ids` は無名 face では空文字列になり、merged mesh から per-face 三角形を逆引きできない。テスト側で `solid.clone()` 後に各 `Face.name = Some(EntityRef::try_named("shared_boundary_test", Face, format!("f{i}")))` を割り当てれば、`mesh.face_ids[tri_idx]` に canonical name (`N(shared_boundary_test;F:f{i})`) が入り face_idx を逆引きできる。

- `Face.name: Option<EntityRef>` はトポロジー検証 (Euler-Poincaré、HalfEdge 2:1 等) では参照されず、tessellation の `face_id` 文字列生成のみで使われる (確認: `crates/mycad-kernel/src/tessellation/mod.rs:119-124`)
- `EntityRef::try_named` は `[a-zA-Z0-9_-]` 識別子で valid (`crates/mycad-format/src/feature.rs::validate_identifier`)
- カーネル不変条件は破られない

### コア比較ロジック (`assert_shared_boundary`)

1. `solid.clone()` → `assign_unique_face_names`
2. `tessellate_solid(&named)` で merged mesh を取得
3. `face_idx → HashSet<vertex_idx>` を構築 (`mesh.face_ids` 照合)
4. `he_idx → face_idx` 写像を構築 (outer_loop + inner_loops 走査)
5. 各 `Curve::Circle` edge について:
   - 2 つの half_edge を `solid.half_edges` から線形探索 (`he.edge == edge_idx`)
   - 両 half_edge から `he_to_face` で face_a / face_b を特定
   - `face_a == face_b` (seam / self-adjacent) は skip (Issue 焦点外)
   - 各 face の頂点集合から「円から `LENGTH_TOLERANCE` 内」のものを `project_to_circle` でフィルタ
   - `atan2(cy, cx)` を `[t_start, t_start + 2π)` 正規化してソート
   - 両側リストを `assert_eq!` (長さ)、各 index で位置を `LENGTH_TOLERANCE` 内で比較
6. 検査した Circle edge 数を返す

### 失敗時診断メッセージ仕様

```text
{label}: edge {edge_idx} sample count differs: face {face_a_idx} has {n_a} samples, face {face_b_idx} has {n_b} samples
{label}: edge {edge_idx} sample {i}: t_a={t_a:.6}, t_b={t_b:.6}, p_a=({:.10},{:.10},{:.10}), p_b=({:.10},{:.10},{:.10}), distance={dist:.3e} mm > LENGTH_TOLERANCE ({:.0e})
```

これにより `エッジ index / 頂点 index / 期待値 vs 実測値 / 距離 mm` の Issue 要件を満たす。

### 決定性要件

- `IdGenerator::new(0)` 固定種でフィクスチャを生成 (既存テスト同様)
- T01 で 2 回ビルドして positions / normals / indices が byte-identical を assert

### エラーハンドリング

- `boolean(...).expect(...)` / `tessellate_solid(...).expect(...)` で test 内 panic は許容 (acceptance テスト規約)
- `EntityRef::try_named(...).expect("identifier")` も同様

### derive 規約 / workspace.dependencies

- 新規型なし → derive 規約は不要
- 新規 dependency なし → workspace.dependencies 変更不要

### 数値モデル

| 項目 | 値 | 根拠 |
|---|---|---|
| 比較公差 | `LENGTH_TOLERANCE = 1e-9 mm` (`mycad_kernel::geometry::LENGTH_TOLERANCE`) | Issue 指定 / ADR-004 既存定数を流用 |
| 退化判定 | 退化 fixture は本テスト対象外 (T05 の seam skip 経路のみ)、既存テストに委ねる | Non-Goals に明記 |
| `atan2` 不連続 (±π) | `[t_start, t_start + 2π)` 正規化で吸収 | 円の周期性を 1 つの canonical t に集約 |
| ADR-004 準拠方針 | 既存 `LENGTH_TOLERANCE` をそのまま使用 (tolerant モデル) | 規約遵守時は両側が `Curve::evaluate(t_k)` を同一 t_k で呼ぶため bitwise 一致が期待され、1e-9 は十分マージン |

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| `t01_determinism` | 決定性 | `make_box_cut_cyl_through` を `IdGenerator::new(0)` で 2 回ビルド → mesh の positions/normals/indices が byte-identical | `assert_eq!` 3 件 |
| `t02_cut_box_cyl_through_shared_boundary` | 正常系 | `make_box_cut_cyl_through` → `assert_shared_boundary` → 戻り値 (検査済 Circle edge 数) > 0 | 戻り値 > 0 で assert pass |
| `t03_intersect_cyl_sphere_shared_boundary` | 正常系 | `make_cyl_intersect_sphere` → 同上 | 戻り値 > 0 で assert pass |
| `t04_fuse_box_cyl_shared_boundary` | 正常系 | `make_box_fuse_cyl` → 同上 | 戻り値 > 0 で assert pass |
| `t05_degen_self_adjacent_seam_no_panic` | degen | `make_sphere(3.0, origin)` 単体 (seam edge = self-adjacent のみ) → `assert_shared_boundary` を呼んでも panic なし、戻り値 = 0 | 戻り値 == 0 で assert pass |
| `t06_boundary_pure_cuboid_no_circle_edges` | boundary | `make_cuboid(10,10,10)` 単体 (Line edge のみ、Circle edge 0 件) → vacuous pass、戻り値 = 0 | 戻り値 == 0 で assert pass |

T05/T06 で `_degen_` / `_boundary_` ID 必須要件 (≥ 1 件) を充足。

## 幾何的不変条件チェックリスト

- [x] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか — **N/A** (partition/assemble の実装を触らない。既存出力をテスト側で観測するのみ)
- [x] 各プリミティブの face ごとの outer_loop 2D 向き（CW/CCW）が文書化されているか — **N/A** (既存規約を変更しない)
- [x] flip_normals / same_sense の意味論が明確か（頂点順を変えるか vs 法線だけ変えるか）— **N/A** (本テストは tessellate 後の position のみ参照、normal 処理は触らない)
- [x] pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか — **N/A** (subdivide は触らない)

追加項目 (本テスト固有):

- [x] **`Face.name` 上書きが Solid 不変条件に影響しないか** — `Face.name` は tessellation の `face_id` 文字列生成のみ参照。Euler-Poincaré、Edge ↔ HalfEdge 2:1 等の検証では参照されない (確認済み)
- [x] **`face_ids` の per-triangle 整合性** — `tessellation/mod.rs::push_triangle` が triangle 追加ごとに `face_id.to_string()` を push する実装で、`triangle_count() == face_ids.len()` 不変条件は保証済み
- [x] **周期 edge (`t_range = [0, 2π]`) の取り扱い** — `[t_start, t_start + 2π)` 正規化により seam 頂点 (t=0 vs t=2π) が同一 t に収束し、両側のソート順が一致

## 既存関数を編集する場合

新規ファイル追加のみで完結する Issue のため、既存関数の編集はなし (before/after スニペット不要)。

## 主要リスクと対応

| リスク | 対応 |
|---|---|
| `Face.name` 上書きが kernel 不変条件に影響 | tessellation の `face_id` 文字列生成のみ参照 (確認済み)。トポロジー検証で `face.name` は使用されない |
| `face_ids` の per-triangle 整合性 | `push_triangle` 内で triangle ごとに `face_id.to_string()` を push する実装で `triangle_count() == face_ids.len()` 不変条件は保証済み |
| 円周期 `t_range = [0, 2π]` で seam 頂点が両側で 0 と 2π に分かれる | `[t_start, t_start + 2π)` 正規化で同一 t に収束 |
| UV グリッド面 (cylinder lateral 等) が seam vertex を 2 回ストアし、cap 面 (loop walk) は 1 回しかストアしないため、ナイーブ比較すると 65 vs 64 で長さ不一致 | `vertices_on_circle_sorted` 末尾で隣接位置 dedup (LENGTH_TOLERANCE 内)。両方の seam 表現は `Curve::evaluate(t_start)` で bitwise 一致するため位置 dedup で安全に統合できる。論理サンプル列で比較する |
| `LENGTH_TOLERANCE = 1e-9` が緩すぎ/厳しすぎ | 現規約遵守時 bitwise 一致が期待されるため 1e-9 で十分余裕。緩めると微小なずれを見逃すリスク (これがまさに Issue が検出したい現象) |
| 検査した Circle edge = 0 で気付けない (false negative) | 各 fixture テスト (T02-T04) で `> 0` を assert。T05/T06 (vacuous pass 用) のみ `= 0` を許容 |
| `solid.half_edges` を線形探索 (O(N*E)) で性能劣化 | 各 fixture は小規模 (face 数 < 30, edge 数 < 100) のため許容範囲。`HashMap<edge_idx, Vec<he_idx>>` の事前構築は YAGNI |

## 完了条件

- `cargo xtask ci` green (fmt → clippy → test → build)
- 新ファイル `crates/mycad-kernel/tests/shared_boundary_acceptance.rs` 追加
- 最低 2 fixture (本 plan では T02-T04 で 3 fixture) pass
- 既存テスト群に回帰なし

## 関連

- ADR-009 §Implementation Outline Phase 3 即時実施分の最終ピース (もう片方 = #143 で `2fc87fc` 完了済)
- codex-review #129-F02 (本提案の出典)
- ADR-004 §LENGTH_TOLERANCE
- #129 / #130 / #131 (連鎖バグの症状)
