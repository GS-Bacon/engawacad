## 自律判断ログ (B-3 intent-check aligned:no への対応)

**判断 (2026-06-17, Cycle #18 batch)**: Codex intent-check が #217 を `aligned: no` と判定した。理由は「完了条件 (engawa build で正常に押出形状が生成される) が CI で何を観測して合格とするか計測不能」。Phase 8 スコープ自体には整合しているため、自律モード方針に従い以下のスコープ調整で続行する。

採用した調整:
1. **acceptance test ID 表** (T01〜T05_) を計測可能 assertion で plan に明記
2. **「engawa build で正常」を「`build_assembly` が `Ok` を返し、`extrude_1` body が cuboid (box_1) と独立に live で残り、頂点 z 範囲が [top_z, top_z + depth] に乗ること」に具体化** (#216 の T12 で similar contract 確立済 → #217 では top face 法線方向への押出 contract を pin)
3. **「ADR-005 命名による決定性」を「cuboid サイズを 10×10×5 → 20×20×8 に変更しても同じ EntityRef (`f_z_pos`) が解決され、extrude_1 の z 範囲が [4.0, 6.0] に乗る」に具体化** (回帰検出可能)
4. **「押出方向が Face の法線と一致」を「extrude_1 頂点の z 値が dz/2 (top face) より小さくならない」(押出が +Z 方向に進む) に具体化**

---

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| 新規 example `examples/sketch_extrude_pillar.engawa` — cuboid (10×10×5) + 上面 Face EntityRef → 8 角形 sketch → Extrude depth=2 (柱が生える) | ExtrudeCut → #218 |
| `crates/engawa-build/tests/face_sketch_extrude_acceptance.rs` (新規 integration test) — T01〜T05_degen の 5 テスト | 押出方向の反転 (Face 裏側押出) → Phase 9 以降 |
| `crates/engawa-build/tests/examples_smoke.rs` に新 example smoke 追加 | Draft / Taper → Phase 14 |
| Phase 3 既存 Extrude examples (`examples/extruded_rect.engawa`) non-regression smoke (既存 smoke で十分なことを確認 + T テストで再 build) | 複数 Sketch の同時押出 → Phase 9 以降 |
| cuboid サイズ変更の命名安定性 (10×10×5 → 20×20×8 で同 EntityRef が解決される) | Face 上の Sketch にさらに別の Sketch 重ねる経路 → Phase 9 以降 |
| | 新規 kernel API の追加 (既存 Extrude path のみ使用) |

## Non-Goals

- ExtrudeCut 経路の検証 (#218 に委譲)
- 押出方向の反転 (negative depth) の動作検証 (Phase 9 以降)
- Draft / Taper 機能 (Phase 14)
- 複数 Sketch 重ね合わせ (Phase 9 以降)
- 新規 KernelError variant の追加

## 実装対象

- **新規ファイル**:
  - `examples/sketch_extrude_pillar.engawa` — cuboid(width=10, height=10, depth=5) + 上面 Face EntityRef + 8 角形 sketch (中心 (0,0) plane u-v、半径 2) + Extrude depth=2 (柱が cuboid 上面から z=2.5 → z=4.5 へ生える)
  - `crates/engawa-build/tests/face_sketch_extrude_acceptance.rs` — Acceptance test skeleton + 実装テスト 5 件 (T01〜T05_degen_zero_depth)
- **既存ファイル変更**:
  - `crates/engawa-build/tests/examples_smoke.rs` — `sketch_extrude_pillar` smoke エントリ追加
- **新規 kernel API は追加しない** — Extrude path (`crates/engawa-build/src/lib.rs:222-256`) はすでに plane_ref 経路を resolve_plane 経由で処理しているため、本 Issue は **既存実装の挙動を契約として pin** する

## 設計方針

### 決定性

- T01 は #216 の T01 と同じ方式: `build_assembly + tessellate + to_ascii_stl` を 3 回呼んで STL byte 列が完全一致することを assert
- `IdGenerator::new(0)` 固定 seed
- ADR-005 命名 (`feature_id_kind_role`) 安定性: cuboid サイズ変更後の同 EntityRef 解決を T04 で検証

### B-rep / Plane 幾何整合

- Extrude path (`crates/engawa-build/src/lib.rs:222-256`) は resolve_plane (#216 で検証済) で plane_ref を解決 → make_extrusion (`crates/engawa-kernel/src/primitives/extrusion.rs`) で押出
- 押出方向: plane.normal の方向 (+Z for f_z_pos)
- 退化幾何: 半径 0 polygon は make_extrusion 内で `InvalidParameter { kind: "profile" }` で reject される (#216 で確認済)。本 Issue では depth=0 を退化境界として採用

### derive 規約

新規型なし。

### エラーハンドリング

既存 `KernelError` variant のみ使用。新規 variant 追加なし。`InvalidParameter { kind: "depth" }` (extrusion.rs:146) を T05 で観測。

### workspace.dependencies

変更なし。

### 数値モデル (Phase 8 必須)

- **tolerance: ε_snap = 1e-9** — Plane / Solid vertex 座標比較。z 範囲検証に `assert!((z - expected).abs() < 1e-9)` を使用
- **tolerance: ε_len = 1e-9** — 8 角形 polygon 各辺非ゼロ (半径 2 で十分大きい)
- **tolerance: ε_area = N/A** — 面積比較なし (頂点 z 範囲のみで挙動を pin)
- **ADR-004 準拠方針: tolerant** — `< 1e-9` 比較

## テスト計画 (ID 付き)

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | `examples/sketch_extrude_pillar.engawa` を in-process で build → tessellate → to_ascii_stl を 3 回実行し STL byte 列比較。`let mut gen = IdGenerator::new(0);` 固定 seed | `assert_eq!(a, b); assert_eq!(b, c);` |
| T02 | 正常系 / e2e | example を `build_assembly` で build し、box_1 と extrude_1 の両方が live (`bodies.len() == 2`)、extrude_1 の頂点 z 範囲が `[2.5, 4.5]` (top face z=2.5 + depth=2)、**`extrude_1.solid.euler_poincare() == 0`** (IN01 採用: B-rep 構造妥当性を明示) | `bodies.len() == 2 && extrude_z_min ≈ 2.5 && extrude_z_max ≈ 4.5 && extrude_1.solid.euler_poincare() == 0` |
| T03 | 押出方向 = Face 法線 | extrude_1 の全頂点 z 値が `>= 2.5 - 1e-9` (top face 以下に頂点が存在しない = +Z 方向に押し出されている) | `extrude_1.vertices.iter().all(|v| v.point.z >= 2.5 - 1e-9)` |
| T04 | 命名安定性 (resize regression) | 同じ feature 列だが cuboid を 20×20×8 に変更した別 doc を build → 同 EntityRef (`feature_id=box_1, kind=Face, role=f_z_pos`) が解決され extrude_1 z 範囲が `[4.0, 6.0]` (top z=4 + depth=2) | `build OK && extrude_z_min ≈ 4.0 && extrude_z_max ≈ 6.0` |
| T05_degen_zero_depth | 退化境界 | extrude depth=0 を含む doc を build → `Err(KernelError::InvalidParameter { kind: "depth" })` (`extrusion.rs:146`) | `matches!(err, KernelError::InvalidParameter { kind } if *kind == "depth")` |

**退化/境界ケース ID**: `T05_degen_zero_depth` が _degen_ 命名規約を満たす。

## 幾何的不変条件チェックリスト

- [x] N/A — partition / assemble / boolean は本 Issue のスコープ外
- [x] N/A — Phase 3 既存 Extrude path を流用 (cap face の outer_loop 向きは make_extrusion 内部で確立済)
- [x] N/A — flip_normals / same_sense は make_extrusion 内部処理、本 Issue で触らない
- [x] N/A — pslg_subdivide は呼ばれない

## 実装順序

1. `examples/sketch_extrude_pillar.engawa` 新規作成 (cuboid + 8 角形 sketch + Extrude depth=2)
2. `crates/engawa-build/tests/face_sketch_extrude_acceptance.rs` 新規作成 — skeleton (T01〜T05 を `#[ignore]` で先行配置) (STEP 5.5)
3. `examples_smoke.rs` に `sketch_extrude_pillar` smoke エントリ追加
4. T01〜T05 を実装 (STEP 6)
5. `cargo test --workspace` green 確認

## 既知の制約

- 「engawa build で正常」を vertex z 範囲で観測する: `tessellate_solid_with` 経由でも観測可能だが頂点配列を直接読む方が変動要素が少ない
- cuboid resize テスト (T04) は同 feature_id を使うため、ADR-005 命名 (`feature_id_kind_role`) が崩れない限り Face 解決は安定する想定
- 8 角形は #216 と同じ near-circle 近似 (中心 (0,0)、半径 2、各点 `r*cos(k*PI/4)`/`r*sin(k*PI/4)`)。真の Circle/Arc primitive は Phase 10
