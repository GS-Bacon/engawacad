# #147 Test Spec

STEP 6 (core impl) で `tessellate_sphere_face_trimmed` への validation 追加と退化テスト 2 件は完了。
本 spec は **STEP 6.6 GLM テスト実装フェーズ** で完遂すべき残テストをまとめる。

## 不足テスト (plan 計画分)

### T04_strict (再実装必須)

**plan の規定**:
> T04_strict: `boolean_cut_sphere_dimple` の trimmed sphere face と隣接 cyl lateral face の boundary ring 頂点列を twin で対応付け、index ごとに `LENGTH_TOLERANCE` 内一致を確認

**現状**: GLM core 実装では `t04_shared_boundary_with_cyl_lateral` が naked_edge ベースのまま (#137 から変更なし)、コメントで「strict 版は後続フェーズで実装」と先送り。

**やるべきこと**:

1. `t04_shared_boundary_with_cyl_lateral` を以下のロジックに置換する (既存テスト本体を strict 版に書き換える):

   - `boolean_cut_sphere_dimple` で Solid を生成し `tessellate_solid` でメッシュ化する
   - Solid から `Surface::Sphere` 型の trimmed face (`face.inner_loops.len() == 1` で `Surface::Sphere`) の `face_idx_sphere` を特定する
   - その `inner_loops[0].half_edges[0]` の twin から対面 face を辿り `face_idx_cyl` を特定する (twin の所属 loop → 所属 face)
   - 各 face の `face.name` (`Option<String>`) が `Some` の場合 mesh.face_ids でフィルタできる。`None` の場合は **face.name を明示的に set してから tessellate しなおす** か、Solid から face_idx → name を付与する補助関数を test 内ヘルパとして書く
   - mesh から両 face 別の triangle 集合を抽出し、各 face の **boundary edge** (1 triangle にしか含まれない edge) を quantize 量子化付きで集合化する
   - 両 face の boundary edge 集合を `LENGTH_TOLERANCE` 量子化で比較し、**完全一致** (位相一致) を assert する
   - 一致しない場合は「sphere 側 boundary edge: M 本、cyl 側 boundary edge: N 本、対称差 K 本、最初の不一致 edge の position は (...) と (...) で差は X mm」のように詳細を panic メッセージで出す

2. **microscopic ずれ検出**: 両 face の boundary 頂点を unique 集合に並べ、sphere 側の各頂点に対し cyl 側で最近傍頂点を探し、距離が `LENGTH_TOLERANCE` を超える頂点が 1 つでもあれば失敗させる。

3. **向きずれ検出**: boundary edges を polyline として並べ直し (始点・終点接続)、両 face で同じ ring を構築する。両 ring の頂点列を index で対応付け (どちらかが逆順なら reverse して揃える)、その上で全 index が一致するか確認する。

**実装メモ**:
- `TriangleMesh.face_ids` は per-triangle (`Vec<String>`) で `triangle_count` 長。triangle 3 indices で face_id を取れる。
- `Face.name` は `Option<String>`。`boolean_cut_sphere_dimple` 経由で生成される face に name が無い (Some(""))場合は、test 内で Solid を加工して name を付けてから tessellate する。
- 既存の `count_naked_edges` の量子化ロジック (1e-10 round) を共通ヘルパに抽出してよい。

### 不要セクション

- **不足テスト (plan 計画分)** 以外の「実装差分から追加すべきテスト」「エッジケース・退化入力」「数値境界」「決定性」は **追加不要**:
  - 退化入力 2 件 (`t_degen_offset_axis_circ_center_rejected`, `t_degen_mismatched_circ_radius_rejected`) は STEP 6 で既に実装済み
  - 決定性は既存 `t01_determinism_axis_z` が #137 から継続
  - 数値境界は既存 `t_boundary_*` 系で網羅
- GLM は **acceptance file の既存 T01〜T03, t_boundary_*, t_degen_*, t04 以外** のテスト関数を追加しないこと (skeleton 過剰拡張禁止、#151 案件と同じ趣旨)

## 完了条件

- `cargo test -p mycad-kernel --test trim_sphere_circ_normal_acceptance` で全テスト green (退化 2 件 + T04_strict 版含む)
- `cargo xtask ci` green (Playwright #145 既知 infrastructure 失敗を除く)
