issues:
  - id: F01
    severity: high
    file: "crates/mycad-kernel/src/primitives/extrusion.rs"
    line_hint: 155
    finding: "profile 座標の有限性を検証していない。`(0,0) -> (inf,1) -> (0,2)` のような入力は現在の `<=` / `>` 判定をすり抜け、`Solid` に `inf` / `NaN` 頂点を作れてしまう。"
    suggestion: "各 `(u,v)` に `is_finite()` チェックを追加して `InvalidParameter { kind: \"profile\" }` を返し、build 側でも `SketchSegment.from/to` を同様に拒否する。"
  - id: F02
    severity: medium
    file: "crates/mycad-kernel/src/primitives/extrusion.rs"
    line_hint: 647
    finding: "T07 の合意内容は 3 平面 × CW/CCW だが、追加テストは `yz` の CCW しか固定していない。`Plane::yz()` で `winding < 0` になる分岐は未検証のまま。"
    suggestion: "`test_orientation_yz_plane_cw` を追加し、底面 `-X` / 上面 `+X` と manifold を明示的に固定する。"
  - id: F03
    severity: medium
    file: "crates/mycad-cli/tests/export.rs"
    line_hint: 61
    finding: "T13 の E2E 失敗系が未実装。凹多角形・自己交差 profile を `mycad export` に通したときに非ゼロ終了し、STL を出力しないことがテストされていない。"
    suggestion: "invalid profile の fixture を追加し、`export` が失敗することと output が生成されない/空であることを確認する。"

verdict: fail