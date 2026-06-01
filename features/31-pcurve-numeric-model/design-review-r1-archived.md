issues:
  - id: R01
    severity: high
    section: "設計方針 > 6. 退化幾何の扱い"
    finding: "計画は `zero-area face` の退化検出追加を掲げているが、詳細設計で実際に検出するのは `bbox_diag <= LENGTH_TOLERANCE` の1点潰れだけである。一直線上の頂点列や自己相殺 loop による面積ゼロ Face は `validate_manifold` を通過し、B-rep の退化面を明示エラーにできない。"
    suggestion: "`DegenerateFace` の責務を『1点退化 sentinel』に狭めて成果物記述とテスト名を合わせるか、`validate_manifold` に面積ベースの判定を追加して zero-area face を実際に拒否すること。"
  - id: R02
    severity: medium
    section: "実装順序 > STEP 0c2 / テスト計画 > T14b"
    finding: "`register_validated` を `pub(crate)` としつつ、`crates/mycad-build/tests` の fault-injection test から直接呼ぶ前提になっている。integration test から `pub(crate)` メソッドは参照できないため、safe path の回帰テストを計画どおり実装できない。"
    suggestion: "T14b を `src/lib.rs` 内の unit test に移すか、`register_validated` の公開範囲をテスト可能な形に再設計すること。"
  - id: R03
    severity: medium
    section: "実装対象 / 実装順序 > STEP 0a"
    finding: "`face_scale` の配置が未解決のまま二重定義されている。前半では `geometry::math::face_scale` を public API に含めている一方、後半では `brep -> geometry` 依存方向に反するとして `brep/topology.rs` または `brep/scale.rs` へ移す案に切り替わっており、アーキテクチャ方針が一貫していない。"
    suggestion: "`face_scale` の所有モジュールを1つに確定し、影響ファイル一覧・シグネチャ一覧・実装順序・テスト記述をその決定に合わせて統一すること。"
  - id: R04
    severity: medium
    section: "設計方針 > 2. 交線 pcurve"
    finding: "`Surface::tangent_at_uv` を Sphere/Cone にも導入する設計だが、球の極や円錐 apex のような Jacobian 退化点での扱いが定義されていない。ここで zero/NaN tangent が出ると、`validate_manifold` の同一直線性チェックが正当な pcurve を `PcurveMismatch` と誤判定しうる。"
    suggestion: "Jacobian が rank-deficient なサンプルでは tangent 比較を明示的にスキップするか、非特異点へサンプルをずらす方針を仕様化すること。"

verdict: fail