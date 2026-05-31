issues:
  - id: R01
    severity: high
    section: "実装順序 > STEP 3 — partition.rs の Option<PlaneData> 化"
    finding: "`PlaneData` を `Option` 化した後の残り参照箇所について「`.as_ref().unwrap()` または guard」としており、unwrap を許容したままです。sphere face を含む A3 で通る経路に 1 箇所でも残ると、今回の主目的である panic 回避が設計上保証されません。"
    suggestion: "`PlaneData` の参照は `Some(plane)` が証明できる分岐に限定し、非平面 face での unwrap を禁止してください。plane 投影が必要な処理は `Surface::Plane` / `Some(plane)` 前提の helper に閉じ込めるべきです。"
  - id: R02
    severity: medium
    section: "テスト計画 > A3"
    finding: "Issue Context の受け入れ条件は `Cut sphere (radius=3)` ですが、plan の A3/T22/T32 はすべて `radius=2` に差し替わっています。合意済み Acceptance をそのまま検証していません。"
    suggestion: "A3 の Acceptance テストは issue 本文どおり `radius=3` に戻してください。必要なら `radius=2` は補助ケースとして追加します。"
  - id: R03
    severity: medium
    section: "変更する型・関数のシグネチャ > `validate_planar_face_outer_loop_basic`"
    finding: "convex チェック撤廃後の検証が `n >= 3` と 1HE `Curve::Circle` 例外だけなので、3 点 collinear、面積ゼロ polygon、ゼロ長辺、半径ほぼ 0 の analytic Circle を弾けません。退化入力を `Ok(())` で通す設計になっています。"
    suggestion: "`LENGTH_TOLERANCE` を使って隣接頂点距離と outer loop 面積の下限を明記し、analytic Circle も `radius > LENGTH_TOLERANCE` を必須にしてください。退化時は明示的に `KernelError` を返すべきです。"
  - id: R04
    severity: medium
    section: "変更する型・関数のシグネチャ > `validate_boolean_input` の `cyl×sph` gate"
    finding: "`cyl×sph` gate を入力全体の「Cylinder と Sphere が同時に存在するか」で判定すると、実際には円柱面と球面が交差しない valid な boolean まで拒否します。Non-Goal は『cyl×sph partition』であって、混在入力全体の全面禁止ではありません。"
    suggestion: "gate は surface 種別の存在判定ではなく、実際に partition 対象になる face pair まで絞ってください。少なくとも『Cylinder と Sphere が両方ある』だけでは reject しない設計にするべきです。"

verdict: fail