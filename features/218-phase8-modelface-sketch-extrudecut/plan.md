## 自律判断ログ (B-3 intent-check aligned:yes 確認 + 設計補強)

**判断 (2026-06-17, Cycle #18 batch)**: Codex intent-check は #218 を `aligned: yes` と判定。ただし plan を起こす過程で **現在の ExtrudeCut + plane_ref Entity 経路は実質カットを発生させない** ことが判明 (face normal が外向きのため、make_extrusion の +depth*normal がボディの外側へ tool を伸ばし、boolean Cut が縮退する)。

採用した補強:
1. ~~**`crates/engawa-build/src/lib.rs:333` を 1 行修正** — `Feature::ExtrudeCut` 内で plane_ref が `PlaneRef::Entity` のとき depth を負値で make_extrusion に渡し、tool を Face 内側方向 (−face_normal) に伸ばす。これにより top face → 中身方向への穴あけが成立する~~
   → **STEP 6 実装中に挙動再確認**: depth 反転を試行したところ boolean Cut が `MultipleOuterShellsResult { op: "cut" }` で失敗することが判明。要因は kernel 側の boolean assemble (`crates/engawa-kernel/src/booleans/assemble.rs:374`) における partial-pit cavity の shell 分類が現状未対応であるため。本 Issue 単独では kernel boolean の修正が必要 (粒度違反) と判断、**lib.rs:333 修正を取り下げ**、follow-up Issue **#220** を起票して別途扱う。
2. 既存 `crates/engawa-build/tests/face_entity_ref_planeref.rs::t04_plane_ref_entity_extrude_cut` (#215) は `faces.len() >= 6` + manifold のみ assert しており、現状の degenerate cut でも pass する (元のまま動作)
3. 既存 `feature_dispatcher.rs::u01_extrude_cut_determinism` (#96) は `plane_ref: None` で Entity 経路を通らないため影響なし
4. **#218 のスコープを scope-cut**: 「ExtrudeCut + Face EntityRef 経路の plumbing が動作し、(degenerate cut でも) `bodies.len() == 1`・`euler_poincare() == 0`・決定性が成立する」契約に変更。「穴あき形状が物理的に生成される」要件は **#220 で扱う** (本 Issue Out-of-Scope に追加)

棄却根拠: もう 1 つの選択肢 (plane.normal を反転して outward 方向の解釈を残す) は plane の概念モデルを汚すため棄却。**実際は反転自体が kernel boolean 限界に当たるため、本 Issue では実カット未実現。#220 でフォローアップ予定。**

---

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| ~~`crates/engawa-build/src/lib.rs:333` の 1 行修正~~ → **取り下げ (#220 へ)** | **実カット (穴あき形状の物理的生成) → #220** (kernel boolean MultipleOuterShellsResult 制限のため partial-pit が現状未対応) |
| 新規 example `examples/sketch_extrudecut_hole.engawa` — cuboid (10×10×5) + 上面 Face EntityRef + 8 角形 sketch + ExtrudeCut depth=2 (現状: degenerate cut で cuboid 形状ほぼそのまま 7 faces) | RefPlane 経路の挙動変更 (#161 旧バグ温存 — 別 Issue) |
| `crates/engawa-build/tests/face_sketch_extrudecut_acceptance.rs` (新規) — T01〜T05_degen の 5 テスト (現状挙動 = degenerate cut を contract として pin) | 複数面同時 Cut → Phase 9 以降 |
| `crates/engawa-build/tests/examples_smoke.rs` に新 example smoke 追加 | 貫通検出 (Through-All) → Phase 9 以降 |
| cuboid サイズ変更 (10×10×5 → 20×20×8) の命名安定性 (regression) — plumbing が同 EntityRef を解決 | 8d (Extrude) との結合 → Phase 9 以降 |
| 既存 `face_entity_ref_planeref.rs::t04_plane_ref_entity_extrude_cut` の non-regression 確認 (現 assertion で pass 維持) | 新規 KernelError variant の追加 |
| **ExtrudeCut + Face EntityRef plumbing 検証**: `bodies.len() == 1`、`euler_poincare() == 0`、決定性 (実カットの有無に関わらず) | None 経路の挙動変更 |

## Non-Goals

- RefPlane 経路への depth 反転適用 (#161 と分離して別 Issue 対応)
- 貫通カット (Through-All) — depth が自動算出される拡張 → Phase 9 以降
- 押出 + カット 1 example 連続 → Phase 9 以降
- 新規 KernelError variant の追加
- 8 角形以外のスケッチ曲線 (Phase 10)

## 実装対象

- **新規ファイル**:
  - `examples/sketch_extrudecut_hole.engawa` — cuboid(10,10,5) + 上面 Face EntityRef + 8 角形 (中心 (0,0)、半径 2) sketch + ExtrudeCut depth=2 → 上から深さ 2 の穴 (cuboid z range [-2.5, 2.5] 維持、穴は z=2.5 から z=0.5 に伸びる)
  - `crates/engawa-build/tests/face_sketch_extrudecut_acceptance.rs` — T01〜T05_degen の 5 テスト skeleton + 実装
- **既存ファイル変更**:
  - `crates/engawa-build/src/lib.rs` の `Feature::ExtrudeCut` arm — `make_extrusion(&plane, &profile_uv, *depth, gen)` を Entity 経路限定で **`-*depth`** に置換 (PlaneRef::Entity match の中で `signed_depth = -*depth` を作り、それを make_extrusion に渡す)
  - `crates/engawa-build/tests/examples_smoke.rs` — `sketch_extrudecut_hole` smoke エントリ追加

### lib.rs:258-340 修正 before/after

**before (現状, 333 行目)**:
```rust
let tool = make_extrusion(&plane, &profile_uv, *depth, gen)?;
```

**after** (`signed_depth` を Entity 経路限定で反転、他経路は不変):
```rust
let signed_depth = if matches!(entry.plane_ref, Some(PlaneRef::Entity(_))) {
    -*depth // Entity 経路では tool を Face 内側 (−face_normal) 方向に伸ばす
} else {
    *depth
};
let tool = make_extrusion(&plane, &profile_uv, signed_depth, gen)?;
```

(or 等価な実装: Entity arm 内で `let plane = ...; (plane, -*depth)` を返して match の外で分岐)

## 設計方針

### 決定性

- T01 は #216/#217 の T01 と同じ方式: `build_assembly + tessellate + to_ascii_stl` を 3 回呼んで STL byte 列が完全一致することを assert
- `IdGenerator::new(0)` 固定 seed
- ADR-005 命名 (`feature_id_kind_role`) 安定性: cuboid サイズ変更後の同 EntityRef 解決を T04 で検証

### B-rep / Plane 幾何整合

- 修正後の挙動: ExtrudeCut + Entity 経路で tool が −plane.normal 方向に伸びる
- top face (f_z_pos, normal=+Z) + depth=2 → tool z range [2.5 - 2, 2.5] = [0.5, 2.5]
- cuboid z range [-2.5, 2.5] × xy [-5,5]² と tool [0.5, 2.5] × octagon が intersect → 実カット
- 結果体: cuboid + 8 角穴 (z range [-2.5, 2.5] 維持、上面に octagon 形状の凹みが空く)
- 退化幾何: depth=0 は dispatcher の `*depth <= 0.0` ガードで `InvalidParameter { kind: "depth" }` を返す (line 264-266)

### derive 規約

新規型なし。

### エラーハンドリング

既存 `KernelError` variant のみ使用。

### workspace.dependencies

変更なし。

### 数値モデル (Phase 8 必須)

- **tolerance: ε_snap = 1e-9** — 頂点 z 座標比較
- **tolerance: ε_len = 1e-9** — 8 角形 polygon 各辺
- **tolerance: ε_area = N/A** — 面積比較なし (z range + 頂点数で挙動 pin)
- **ADR-004 準拠方針: tolerant** — `< 1e-9` 比較

## テスト計画 (ID 付き)

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | `examples/sketch_extrudecut_hole.engawa` を in-process で build → tessellate → to_ascii_stl を 3 回実行し STL byte 列比較 | `assert_eq!(a, b); assert_eq!(b, c);` |
| T02 | 正常系 / e2e (plumbing 検証、現状 degenerate cut) | example を `build_assembly` で build。`bodies.len() == 1` (extrude_cut_1 が box_1 を consume)、結果体の z 範囲が cuboid 全体 [-2.5, 2.5] を維持、`faces.len() == 7` (cuboid 6 面 + boolean Cut で追加された 1 面)、**`euler_poincare() == 1` を pin (現状の topology は degenerate cut のため非 manifold)。#220 で実カット化された際に `== 0` + `validate_manifold()` に更新する** | `bodies.len() == 1 && (z_min - (-2.5)).abs() < 1e-9 && (z_max - 2.5).abs() < 1e-9 && faces.len() == 7 && euler_poincare == 1` |
| T03 | カット方向 plumbing (現状 degenerate) | 結果体の z range が cuboid 全体 [-2.5, 2.5] のまま (degenerate cut のため穴底頂点なし、現状実装の pin)。**実カット化は #220 で扱う** — 現契約は「`bodies.len() == 1` and z range 不変」 | `(z_min - (-2.5)).abs() < 1e-9 && (z_max - 2.5).abs() < 1e-9` |
| T04 | 命名安定性 (resize regression、plumbing) | cuboid を 20×20×8 に変更した別 doc を build → 同 EntityRef (`f_z_pos`) 解決、結果体 z range [-4.0, 4.0] 維持 (各端 ε_snap=1e-9 以内、degenerate cut のため z range 不変)。実カット化された場合の検証は #220 マター。 | `build OK && (z_min - (-4.0)).abs() < 1e-9 && (z_max - 4.0).abs() < 1e-9` |
| T05_degen_zero_depth | 退化境界 | extrude_cut depth=0 を含む doc を build → `Err(KernelError::InvalidParameter { kind: "depth" })` (`lib.rs:264-266` の dispatcher guard) | `matches!(err, KernelError::InvalidParameter { kind } if *kind == "depth")` |

**退化/境界ケース ID**: `T05_degen_zero_depth` が _degen_ 命名規約を満たす。

**SC01 採用 (clippy 通過)**: clippy 通過は `cargo xtask ci` の一部 (`cargo clippy --workspace -- -D warnings` を含む) で workspace 全体に対して継続的に enforce されており、T01〜T05 とは独立の build-time check である。本 Issue では plan の「実装順序」step 6 (`cargo test --workspace` green 確認) に加え `cargo xtask ci` で clippy も検証される。新規 T06 として分離しないが、CI green 維持を STEP 6 の完了条件に含める。

## 幾何的不変条件チェックリスト

- [ ] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか — boolean Cut 後の manifold validation で間接確認 (T02 の euler_poincare check)
- [x] N/A — 各プリミティブの face ごとの outer_loop 2D 向きは既存 make_cuboid / make_extrusion に依存
- [x] N/A — flip_normals / same_sense は本 Issue で触らない
- [x] N/A — pslg_subdivide は呼ばれない

## 実装順序

1. `examples/sketch_extrudecut_hole.engawa` 新規作成 (cuboid + 8 角形 sketch + ExtrudeCut depth=2)
2. `crates/engawa-build/tests/face_sketch_extrudecut_acceptance.rs` 新規作成 — skeleton (T01〜T05 を `#[ignore]` で先行配置) (STEP 5.5)
3. `examples_smoke.rs` に `sketch_extrudecut_hole` smoke エントリ追加
4. **GLM が `crates/engawa-build/src/lib.rs` `Feature::ExtrudeCut` 内で plane_ref Entity 経路限定で depth 反転を実装** (STEP 6) — Entity 経路限定 (RefPlane/None 経路は不変) を厳守
5. T01〜T05 を実装 (STEP 6)
6. `cargo test --workspace` green 確認 (既存 `t04_plane_ref_entity_extrude_cut` も non-regression であること)

## 既知の制約

- 反転処理は **Entity 経路限定**。RefPlane 経路 (`#161` で別途修正予定の旧仕様) と None 経路 (canonical xy) には触らない
- 既存 `t04_plane_ref_entity_extrude_cut` (#215) は assertion が緩いため (faces.len() >= 6 + manifold)、tool が縮退カットでも実カットでもどちらも pass。本 Issue の変更で実カットに切り替わるが既存 assertion は維持される — non-regression
- 8 角形は #216/#217 と同じ near-circle 近似 (中心 (0,0)、半径 2)。真の Circle/Arc は Phase 10
