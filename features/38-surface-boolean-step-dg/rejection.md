## Round 2

- R02: 「`IntersectionSegment.arc_provenance` だけでは pcurve 整合性を満たせない」を **棄却** — A1 (intersection を伴う Cut) を別 Issue #39 へ分離したため、#38 は intersection が起きない A3 (内包球 void shell) のみが Acceptance。`reconstruct_intersection_curve` / `attach_pcurves_for_trimmed_faces` / `ArcProvenance` / trim tessellation は #38 plan から全削除し Non-Goals (別 Issue #39 で対応) へ移動
- R03: 「PSLG / `assemble.add_face` が multi-loop face (annulus) をサポートしない」を **棄却** — 同上、A1 (box 上面の inner_loop) は別 Issue #39 のスコープ。A3 は intersection なしで multi-loop face を生成しないため #38 では対象外
- R04: 「seam-crossing arc (`...31,0,1...`) の cyclic merge が必要」を **棄却** — 同上、A1 (cylinder lateral × box top の全周交線) は別 Issue #39 のスコープ。A3 は intersection なしのため arc 再構成自体が不要
