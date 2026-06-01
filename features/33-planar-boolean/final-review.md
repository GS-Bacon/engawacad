issues:
  - id: F01
    severity: critical
    file: "crates/mycad-kernel/src/booleans/assemble.rs"
    line_hint: 138
    finding: "boolean 組み立てが各ポリゴン辺ごとに新しい Edge を作り HalfEdge を 1 本しかぶら下げていません。共有辺の再利用も反対向き HalfEdge の生成もないため、空でない boolean 結果は必ず `validate_manifold()` で `edge must have exactly 2 half-edges` になり、成功結果を返せません。"
    suggestion: "無向辺キーで Edge を共有し、各共有辺に反対向きの HalfEdge を 2 本そろえてから loop/face を張る実装にしてください。`reverse_face_orientation()` 側の再生成も同じ規約に合わせる必要があります。"
  - id: F02
    severity: high
    file: "crates/mycad-build/src/lib.rs"
    line_hint: 32
    finding: "boolean 後に operand を `consume()` しても、`all()`/`len()`/`is_empty()` は `bodies` 全体をそのまま返します。CLI/API は `bodies.all()` をそのまま tessellate しているため、`Fuse/Cut/Intersect` を含むモデルで消費済みの入力ボディまで一緒に出力され、最終形状が重複します。"
    suggestion: "`all()`/`len()`/`is_empty()` を live body ベースにするか、少なくとも CLI/API の呼び出し側を `live()` に切り替えてください。"
  - id: F03
    severity: high
    file: "crates/mycad-kernel/src/booleans/partition.rs"
    line_hint: 420
    finding: "`clip_line_to_polygon_2d()` が `t_min` を 0 に丸め、`clip_line_to_polygon_2d_given_points()` も元の segment 長で `t_max` を制限していません。結果として helper は『直線/線分の clip』ではなく『ray の clip』になっており、`intersect_planes()` が置いた原点次第で有効な交差区間を切り落としたり逆に延長したりします。PSLG に誤った分割線が入るため、正しい boolean 分割になりません。"
    suggestion: "無限直線を clip する場合は負の t を保持し、既知の線分を clip する場合は `[0, len]` の範囲に制限してください。"
  - id: F04
    severity: medium
    file: "crates/mycad-kernel/src/booleans/assemble.rs"
    line_hint: 355
    finding: "`find_connected_shells()` が `HashMap` で連結成分を集約し、そのまま `into_values()` を返しています。outer shell と void shell が複数ある結果では shell 順序と shell ID 付番がプロセスごとに変わり得るため、決定性要件を満たしません。"
    suggestion: "連結成分は face index 順でソートして返すか、`BTreeMap`/`IndexMap` を使って順序を固定してください。"

verdict: fail