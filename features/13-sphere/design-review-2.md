issues:
  - id: R01
    severity: critical
    section: "影響ファイル"
    finding: "`CreateSphere` を `mycad-build` で有効化すると、現行 `crates/mycad-api/tests/mesh_api.rs` の T07 はまだ `create_sphere` を 422 unsupported と期待しているため、設計書の影響ファイル一覧のままでは `cargo xtask ci` を満たせない。"
    suggestion: "`crates/mycad-api/tests/mesh_api.rs` を影響範囲に追加し、`create_sphere.mycad` を成功ケースへ更新すること。少なくとも sphere 対応後の期待 status/body に差し替えること。"
  - id: R02
    severity: high
    section: "テッセレーション > canonical 球 face の検証"
    finding: "canonical 判定条件が弱く、`Curve::Circle` であることと極頂点であることだけでは、壊れた loop や非 canonical seam を十分に弾けない。現行 `HalfEdge` は `start_vertex` を独立保持するため、`forward`/`start_vertex`/`edge.vertices` の不整合や loop 非閉包でも `TrimmedFaceUnsupported` にならず full sphere を silent に生成し得る。"
    suggestion: "canonical 判定で `edge.vertices == [v_south, v_north]`、2 HalfEdge が同一 edge の正逆であること、各 `start_vertex` が `forward` と整合すること、loop が閉じること、さらに seam の `Circle` が sphere の center/radius と一致し `t_range` も期待形に入ることまで検証すること。満たさない場合は必ず `TrimmedFaceUnsupported` を返すこと。"
  - id: R03
    severity: medium
    section: "テッセレーション > same_sense / テスト計画"
    finding: "`same_sense` の扱いが『法線反転』までしか書かれていないが、ASCII STL の facet normal は `to_ascii_stl` が三角形 winding から再計算する。北極/南極 fan の頂点順や `same_sense=false` が誤っても、T07/T09/T10 の facet 数・watertight だけでは inside-out を検出できない。"
    suggestion: "`!same_sense` では三角形 index 順も反転する設計にし、球でも円柱と同様の outward-normal 検証テストを追加すること。"
  - id: R04
    severity: medium
    section: "設計方針 > トポロジー (標準的な UV 球)"
    finding: "1-face sphere は 1 本の seam edge に 2 HalfEdge を載せる self-adjacent periodic face だが、現行カーネルの説明は『各 edge は adjacent face ごとに HalfEdge を持つ』という前提で、設計書はこの例外を明示していない。T02/T04 も HE 数と向きしか見ないため、この自己隣接表現をカーネル不変条件として正式に許容するのかが曖昧。"
    suggestion: "『full sphere は self-adjacent periodic face を許容する』ことを不変条件として明記し、専用の妥当性テストを追加すること。もし自己隣接を許容しない方針なら、別表現へ設計を改めること。"

verdict: fail