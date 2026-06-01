issues:
  - id: R01
    severity: high
    section: "アルゴリズム詳細 > Phase A / Phase B / Phase C"
    finding: "hole を生む閉ループ分割を `FaceFragment { polygon_2d: Vec<Point2D> }` と `outer_loop` 前提で扱っており、T18 のような『外周 - 内周』の annulus 片を表現できない。さらに coplanar overlap / ray-face 判定も `outer_loop` ベースのままなので、この Issue 自身が生成する concave・`inner_loops` 付き face を次の Boolean の入力にすると、穴領域を実体として誤認する。"
    suggestion: "fragment と face 入力を『outer loop + 0..n inner loops』を持つ trimmed planar region として表現し、coplanar overlap・point-in-polyhedron 用の face hit 判定も完全な境界集合で処理すること。少なくともこの対応を入れないなら、hole/concave 結果を後続 Boolean の入力にしないと明示してスコープを切ること。"
  - id: R02
    severity: medium
    section: "設計方針 > 決定性要件 / Phase A: Partition"
    finding: "同一 face 内 fragment の正規順が『重心 lex 順』と『outer_loop 起点の BFS 順』の 2 通りで規定されている。`traversal_index` は selector の seed であり、fragment の反復順は face/edge 組立と ID 発番にも波及するため、どちらを主順序に使うかが実装依存だと同一入力でも `EntityId` と derived name がずれうる。"
    suggestion: "fragment の canonical order を 1 本化し、その順序を fragment 出力順・`traversal_index`・entity 作成順のすべてで共有すること。BFS を採るなら重心 lex は tie-break のみに落とし、逆に重心 lex を採るなら selector も同順に揃えること。"
  - id: R03
    severity: medium
    section: "退化幾何の扱い > ケース対応表 / primitive の base name 付与"
    finding: "本 Issue では base name を cuboid/extrude にしか付けない一方、ケース表と T21 は `cylinder × box` を `NonPlanarBooleanInput` にする前提になっている。入力検証順が未規定のままだと、未命名の cylinder/sphere で `MissingEntityName` が先に発火し、宣言したエラー契約を満たせない。"
    suggestion: "boolean 入口の検証順を `validate_manifold` → 非平面 surface 検出 → name presence 検査に固定し、`MissingEntityName` は平面の unnamed solid を使って別テストに分離すること。"

verdict: fail