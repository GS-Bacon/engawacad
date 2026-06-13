# Debug Spec for #147 (Codex final review round 1)

Codex STEP 7.5 で 1 high + 1 medium の指摘が出た。両方を修正する。

## F01 (high): T04_strict が face 単位 mesh 境界を実際には比較していない

**ファイル**: `crates/mycad-kernel/tests/trim_sphere_circ_normal_acceptance.rs` (`fn t04_shared_boundary_with_cyl_lateral`, line ~874)

**指摘**:
> `sphere_boundary` / `_adj_boundary` は非空確認だけで捨てられ、その後の microscopic check は `mesh.positions` 全体を走査して別 face の頂点も拾い、向き比較も `result.vertices` 由来です。結果として、片側 face の tessellation だけが共有円からずれても他方 face の頂点を拾って通過できます。

**修正方針 (Codex 提案)**:
> 各 face の `tri_indices` から boundary polyline を再構成し、その face 自身の mesh 頂点列どうしを比較してください。B-rep の `result.edges` / `result.vertices` や全体 `mesh.positions` 走査には戻らない形にするべきです。

**やること (acceptance tests を書き換える)**:

1. `mesh.face_ids` と `mesh.indices` から `sphere_face_id` / `adj_face_id` ごとの **triangle 集合** を抽出する。
2. 各 face の triangle 集合内で **boundary edge** (`1 triangle にしか属さない (a,b) edge`) を抽出する。各 edge の vertex index ペアを **その face 自身の mesh 頂点 (mesh.positions[i])** で量子化キーに変換し集合化する。
3. **位相一致**: sphere 側 boundary edge 集合と adj 側 boundary edge 集合が `LENGTH_TOLERANCE` 量子化下で **完全一致** (`HashSet` 等しさ) することを assert する。一致しない場合は「sphere 側 M 本、adj 側 N 本、対称差 K 本」を panic で出す。
4. **microscopic check (face 内のみ)**: 上で構築した sphere 側 boundary 頂点集合 (各 edge の両端) と adj 側 boundary 頂点集合をそれぞれ `mesh.positions` から **その face の triangle が参照している index のみ** で抽出する。`result.vertices` や全 `mesh.positions` は使わない。集合 cardinality が等しいこと、量子化キーで集合一致することを assert する。
5. **向きずれ検出**: 両 face の boundary edges から **その face の triangles のみを使って** polyline ring を構築し、各 ring の頂点列を index で対応付ける。逆順なら明示的に失敗させるか reverse 補正して一致を確認する。`result.vertices` 由来の頂点や mesh.positions 全体を走査するロジックは禁止。

**禁止事項**:
- `mesh.positions` 全体を走査して B-rep vertex に最も近い mesh 頂点を探すロジック (現状の microscopic check) は削除する
- `result.vertices` / `result.edges` の頂点座標を直接 mesh 比較に使わない (B-rep と mesh の対応付けは face_id 経由のみ)

## F02 (medium): trim circle validation が NaN を拒否できない

**ファイル**: `crates/mycad-kernel/src/tessellation/mod.rs` (line ~1040, `tessellate_sphere_face_trimmed`)

**指摘**:
> `circ_radius = NaN` などの非有限値を拒否できません。`abs()` や `>` は NaN に対して false になるため `InvalidTrimCircle` にならず、後段で NaN 座標のメッシュを生成し得ます。

**修正方針 (Codex 提案)**:
> `circ_radius` と `circ_center` の各成分、`signed_offset`、`expected_radius_sq` に対して `is_finite()` を明示チェックし、非有限なら即 `InvalidTrimCircle` を返してください。

**やること**:

1. 既存の `signed_offset.abs() > radius + LENGTH_TOLERANCE` ガード **より前** に、以下の非有限チェックを追加する:
   - `circ_radius.is_finite() == false` → `InvalidTrimCircle`
   - `circ_center.x.is_finite() && circ_center.y.is_finite() && circ_center.z.is_finite()` が false → `InvalidTrimCircle`
   - `signed_offset` 計算後に `signed_offset.is_finite() == false` → `InvalidTrimCircle`
   - `expected_radius_sq.is_finite() == false` → `InvalidTrimCircle`
2. 新規退化テスト 2 件を `crates/mycad-kernel/tests/trim_sphere_circ_normal_acceptance.rs` に追加:
   - `t_degen_nan_circ_radius_rejected`: `circ_radius = f64::NAN` → `InvalidTrimCircle`
   - `t_degen_nan_circ_center_rejected`: `circ_center = (NaN, 0, 0)` → `InvalidTrimCircle`

## 試した修正と結果

- (初回ループのため空)

## 次にやること

1. 上記 F02 を `tessellate_sphere_face_trimmed` に追加 (4 つの is_finite チェック)
2. F01 に従い `t04_shared_boundary_with_cyl_lateral` を face 単位 mesh 境界比較に書き換え
3. F02 の退化テスト 2 件を追加
4. `cargo xtask ci` で全テスト green を確認

## 追加で書いてほしいテスト

- `t_degen_nan_circ_radius_rejected`
- `t_degen_nan_circ_center_rejected`
