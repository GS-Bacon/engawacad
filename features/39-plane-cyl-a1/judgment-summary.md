<!-- Round ごとに以下の形式で追記すること -->
<!-- ## Round N -->
<!-- - R01: 採用 → plan の「...」節を修正 -->
<!-- - R02: 棄却 → Non-Goals に記載済みの蒸し返し -->
<!-- - R03: 部分採用 → epsilon 比較は採用。HalfEdge 循環チェックは後続 issue に委譲 -->

## Round 1

採用 (4件):
- R01 (Critical): FaceFragment に `inner_polygons_3d` + `inner_boundary_partners/curves` を追加。partition で interior 円弧ループを検出して inner_polygon として出力。assemble で Face `inner_loops` を構成する設計を plan に明記。
- R02 (High): IntersectionSegment に per-chord `curve_3d_t_range` + `pcurve_on_a/b` を追加。「再呼び出し方式」を廃止し、partition で収集した provenance を assemble まで運搬する。
- R03 (Medium): `IntersectionSegment` / `FaceFragment` を `pub(crate)` に変更。
- R04 (Medium): A1 結果の golden YAML roundtrip テスト (T16) を追加。

棄却 (0件)

## Round 2

採用 (3件):
- R01 (High): FaceFragment の outer boundary にも `boundary_t_ranges: Vec<[f64; 2]>`, `boundary_pcurves_a: Vec<Option<Curve2D>>`, `boundary_pcurves_b: Vec<Option<Curve2D>>` を追加。outer/inner 統一の pcurve attach 経路に揃える。
- R02 (High): T04 の Euler 式を修正。blind hole (h<box) は V-E+F=2 (genus-0)、thru-hole (h>box) は V-E+F=0 (genus-1) と 2 ケースに分けて明記。
- R03 (High): Cylinder trim の発火条件を「HE 数 > 4」から「outer_loop の pcurve から v_range を計算し v_max−v_min < height−ε」に修正。A1 の cylinder lateral は 4 HE だが v_range=[0,10] で height=12 なので正しく検出。

棄却 (0件)
