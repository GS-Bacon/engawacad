issues:
  - id: F01
    severity: high
    file: "crates/mycad-kernel/src/tessellation/mod.rs"
    line_hint: 382
    finding: "canonical sphere 判定の `curve.evaluate(t_range[..])` 照合が絶対誤差 `1e-9` 固定のため、半径が大きい正当な球を `TrimmedFaceUnsupported` に誤判定する。`r=1e10` では `sin(π) * r ≈ 1.2e-6` となり、`make_sphere` は成功しても tessellation で落ちる。"
    suggestion: "端点照合は `radius` に比例した相対許容差へ変えるか、端点の位相整合性を index/parameter 契約で判定する。大半径 sphere の tessellation 回帰テストも追加する。"
  - id: F02
    severity: high
    file: "crates/mycad-kernel/src/brep/topology.rs"
    line_hint: 198
    finding: "`validate_manifold()` が `self.half_edges` から得た `edge_he_count` の entry しか検査していないため、0 HalfEdge の孤立 Edge が `Ok(())` で通る。helper のコメントにある『each edge has exactly 2 HEs』を満たしておらず、T16 の多様体検証に偽陽性を出す。"
    suggestion: "`0..self.edges.len()` を全走査して各 edge の参照数を必ず検証し、孤立/片側 edge を `Err` にする負例テストを追加する。"
  - id: F03
    severity: medium
    file: "crates/mycad-kernel/src/brep/topology.rs"
    line_hint: 213
    finding: "`validate_manifold()` は `inner_loops` を境界チェックするだけで閉包を見ておらず、outer loop の接続判定も頂点 index ではなく座標 epsilon 比較に依存している。壊れた hole loop や coincident vertices を使った位相断裂を偽陽性で通せる。"
    suggestion: "outer/inner の全 loop について HalfEdge index と頂点 index を検証し、接続は `end_v == next.start_vertex` の index 一致で判定する。coincident-vertex と broken inner-loop の負例を追加する。"

verdict: fail