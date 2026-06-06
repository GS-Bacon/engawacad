# ADR コンテキスト (Issue #72 平行移動 transform)

## ADR-007 抜粋: transform をB-rep本体に焼き込む (案A)

決定1: transform (位置・回転) を、ソリッドを構成する幾何型
(Point/Plane/Cylinder/Sphere/Cone/Line/Circle および pcurve) に直接適用する。
テッセレーション後のメッシュだけを変換する案 (案B) は採用しない。
理由: 「厳密B-repが真実の源」原則 (ADR-004) に忠実。

決定2: 平行移動を先に、回転は独立 Issue。
- examples/assembly.mycad の回転は全て [0,0,0] (平行移動のみ)。
- 平行移動は「点・原点・中心を一律ずらす」だけで pcurve・軸・半径は無傷。

ASSEMBLY 観点 (transform の検証ポイント):
- ③平行移動・回転で EntityID が変わっていないか (決定性)
- ④pcurve と曲面パラメータの整合が transform 後も保たれるか

決定性テスト要求: 変換後のソリッドをシリアライズして 100 回同一出力を確認。

影響ファイル: geometry/transform.rs (新規), surface.rs, curve.rs, brep/topology.rs。

## ADR-004 抜粋: 数値公差

| 定数 | 値 | 用途 |
|---|---|---|
| LENGTH_TOLERANCE | 1e-9 (mm) | 距離・座標の絶対比較 |
| ANGLE_TOLERANCE | 1e-9 (rad) | 角度・パラメータの絶対比較 |
| RELATIVE_TOLERANCE | 1e-9 (無次元) | スケール比例の相対比較 |

- deg↔rad 変換は format/build 層の責務、カーネルは rad のみ扱う (本Issueは平行移動のみで角度を扱わない)。
- 比較 helper: length_near / angle_near / point_near / point_near_scaled は geometry::math に集約。
