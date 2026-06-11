## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `mycad-build::build_component_tree` で `Component.transform.rotation` を読んで `Solid::rotate` を適用する配線 | rotation × Boolean Cut **球** の組合せテスト (#137 trimmed sphere tessellation が前提・本 Issue 着手時点で未解決) |
| `euler_to_matrix` のシグネチャを **rad 入力**に変更 (ADR-004 §「deg↔rad 境界」厳格化) | `euler_to_matrix_deg` ラッパ API の追加 (段階移行は採らず一括差し替え) |
| 配線地点での deg→rad 変換 (build 層が境界) | `Component.transform` 型自体の変更 (format 層は deg のまま据え置き) |
| 親→子の transform 合成: `total_rotation = parent_rotation * local_rotation` (行列積、ZYX 順) と `total_position` の rotation 適用後の合成 | examples/*.mycad に rotation 値を持つ新規 example を追加すること (assembly.mycad は rotation=[0,0,0] のまま) |
| 非有限値 (NaN/Inf) を rotation 配列に含む場合の `KernelError::InvalidParameter { kind: "transform.rotation" }` 拒否 | 跨ぎ Boolean (Phase 6+ 延期、ADR-007 §5) |
| `crates/mycad-kernel/tests/rotate_acceptance.rs` 既存呼出しの追従修正 (`.to_radians()` 付与) | tessellation 側の rotation 対応 (`tessellate_solid` は最終 `Solid` を消費するため自動で正しく動く想定) |
| `crates/mycad-build/tests/` に rotation 配線の Acceptance テスト | rotation × Cut sphere をテストすること (#137 が解決するまで `#[ignore = "blocked: #137"]` で保留) |

## Non-Goals

- **rotation × Boolean Cut Sphere の組合せテスト**: #136 / #137 が未解決のため、決定性とトポロジーの両方で不安定。本 Issue では `#[ignore = "blocked: #137"]` の skeleton のみ置く。
- **`euler_to_matrix_deg` ラッパの提供**: ADR-004 整合のためのシグネチャ変更は一括差し替え。既存テストの追従修正は本 Issue スコープ内。
- **format 層の `Transform` 型変更**: 既存 `.mycad` ファイル互換のため `rotation: [f64; 3]` (deg) を維持する。
- **deg/rad 識別を型レベルで保証する newtype 導入**: 別 Issue 候補 (ADR-004 補強). 本 Issue では関数境界のコメント明示のみ。
- **examples ファイルに新規 rotation 例を追加**: 本 Issue は配線とテスト追加に絞る。

## 実装対象

<!-- Issue: #135 -->

**影響クレート/ファイル**:
- `crates/mycad-kernel/src/geometry/transform.rs`: `euler_to_matrix` のシグネチャ変更 (deg → rad)
- `crates/mycad-kernel/src/geometry/transform.rs` の inline test (`tests` mod): 既存呼出しの `.to_radians()` 追従
- `crates/mycad-kernel/tests/rotate_acceptance.rs`: 既存 30+ 呼出しの追従修正
- `crates/mycad-build/src/lib.rs`: `build_component_tree` の rotation 配線
- `crates/mycad-build/tests/transform_rotation_acceptance.rs` (新規): Acceptance テスト
- `crates/mycad-kernel/src/error.rs`: 既存 `InvalidParameter { kind }` を流用、新規エラー variant は追加しない

### 変更箇所 1: `euler_to_matrix` シグネチャ変更

**before** (`crates/mycad-kernel/src/geometry/transform.rs:25-36`):
```rust
/// Euler angles (degrees) → 3×3 rotation matrix (ZYX order: R = Rx(rx) * Ry(ry) * Rz(rz)).
pub fn euler_to_matrix(rx_deg: f64, ry_deg: f64, rz_deg: f64) -> [[f64; 3]; 3] {
    let to_rad = std::f64::consts::PI / 180.0;
    let (sx, cx) = (snap((rx_deg * to_rad).sin()), snap((rx_deg * to_rad).cos()));
    let (sy, cy) = (snap((ry_deg * to_rad).sin()), snap((ry_deg * to_rad).cos()));
    let (sz, cz) = (snap((rz_deg * to_rad).sin()), snap((rz_deg * to_rad).cos()));
    [
        [cy * cz, -cy * sz, sy],
        [sx * sy * cz + cx * sz, -sx * sy * sz + cx * cz, -sx * cy],
        [-cx * sy * cz + sx * sz, cx * sy * sz + sx * cz, cx * cy],
    ]
}
```

**after**:
```rust
/// Euler angles (**radians**) → 3×3 rotation matrix (ZYX order: R = Rx(rx) * Ry(ry) * Rz(rz)).
///
/// ADR-004 §単位系: kernel は rad のみを扱う。deg→rad 変換は format/build 層の責務。
pub fn euler_to_matrix(rx: f64, ry: f64, rz: f64) -> [[f64; 3]; 3] {
    let (sx, cx) = (snap(rx.sin()), snap(rx.cos()));
    let (sy, cy) = (snap(ry.sin()), snap(ry.cos()));
    let (sz, cz) = (snap(rz.sin()), snap(rz.cos()));
    [
        [cy * cz, -cy * sz, sy],
        [sx * sy * cz + cx * sz, -sx * sy * sz + cx * cz, -sx * cy],
        [-cx * sy * cz + sx * sz, cx * sy * sz + sx * cz, cx * cy],
    ]
}
```

### 変更箇所 2: `build_component_tree` の rotation 配線

**before** (`crates/mycad-build/src/lib.rs:365-434`):
```rust
fn build_component_tree(
    component: &Component,
    base_dir: &Path,
    visiting: &mut Vec<PathBuf>,
    depth: usize,
    accumulated_offset: Vec3,
    gen: &mut IdGenerator,
    out: &mut Vec<Body>,
) -> Result<(), KernelError> {
    // Accumulate this component's position into the running offset (rotation ignored until #77)
    let p = &component.transform.position;
    if p.iter().any(|v| !v.is_finite()) {
        return Err(KernelError::InvalidParameter {
            kind: "transform.position",
        });
    }
    let local_offset = Vec3::new(p[0], p[1], p[2]);
    let total_offset = accumulated_offset + local_offset;

    // 1. Build this component's own features and apply accumulated translation
    if !component.features.is_empty() {
        let built = build_bodies_from_features(&component.features, gen)?;
        for mut body in built.live().cloned() {
            if total_offset != Vec3::zeros() {
                body.solid.translate(total_offset);
            }
            out.push(body);
        }
    }
    // (...reference 解決、children 再帰は同じ accumulated_offset を伝搬...)
}
```

**after** (rotation を伝搬):
```rust
// 累積 transform を「行列 × 平行移動ベクトル」のペアで持つ。
// 子の Body 配置は: p_world = parent_offset + parent_rotation * (local_offset + local_rotation * p_local)
// だが、ADR-007 §1 で B-rep を世界座標へ焼き込むため、再帰時に accumulated_rotation と
// accumulated_offset を「親 transform 適用後の座標系」で渡す方式を採る。

fn build_component_tree(
    component: &Component,
    base_dir: &Path,
    visiting: &mut Vec<PathBuf>,
    depth: usize,
    accumulated_offset: Vec3,
    accumulated_rotation: [[f64; 3]; 3], // NEW
    gen: &mut IdGenerator,
    out: &mut Vec<Body>,
) -> Result<(), KernelError> {
    // --- position ---
    let p = &component.transform.position;
    if p.iter().any(|v| !v.is_finite()) {
        return Err(KernelError::InvalidParameter { kind: "transform.position" });
    }
    let local_offset = Vec3::new(p[0], p[1], p[2]);

    // --- rotation (deg → rad → matrix) ---
    let r = &component.transform.rotation;
    if r.iter().any(|v| !v.is_finite()) {
        return Err(KernelError::InvalidParameter { kind: "transform.rotation" });
    }
    let local_rotation = euler_to_matrix(r[0].to_radians(), r[1].to_radians(), r[2].to_radians());

    // 累積:
    //   p_world(p_local) = accumulated_offset + accumulated_rotation * (local_offset + local_rotation * p_local)
    // を分配すると:
    //   total_offset   = accumulated_offset + accumulated_rotation * local_offset
    //   total_rotation = accumulated_rotation * local_rotation
    let rotated_local_offset = rotate_vec(local_offset, accumulated_rotation);
    let total_offset = accumulated_offset + rotated_local_offset;
    let total_rotation = matrix_mul3(accumulated_rotation, local_rotation);

    // 1. Build this component's own features and apply total transform
    if !component.features.is_empty() {
        let built = build_bodies_from_features(&component.features, gen)?;
        for mut body in built.live().cloned() {
            // 順序: rotate (around origin) → translate
            if total_rotation != IDENTITY3 {
                body.solid.rotate(total_rotation, Point::origin());
            }
            if total_offset != Vec3::zeros() {
                body.solid.translate(total_offset);
            }
            out.push(body);
        }
    }

    // 2. reference 解決と 3. children 再帰は total_offset/total_rotation を渡す
    // (... 既存ロジックの伝搬引数を accumulated_offset → total_offset,
    //  accumulated_rotation → total_rotation に置換 ...)
}
```

**追加ヘルパ** (`crates/mycad-build/src/lib.rs` 内 private):
- `const IDENTITY3: [[f64; 3]; 3] = [[1.0,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]];`
- `fn matrix_mul3(a: [[f64;3];3], b: [[f64;3];3]) -> [[f64;3];3]` — 3×3 行列積 (純関数、決定性)

**呼び出し元** (`build_assembly`):
```rust
// before
build_component_tree(&doc.root_component, base_dir, &mut visiting, 0, Vec3::zeros(), gen, &mut bodies)?;
// after
build_component_tree(&doc.root_component, base_dir, &mut visiting, 0, Vec3::zeros(), IDENTITY3, gen, &mut bodies)?;
```

### 変更箇所 3: 既存テスト追従

`euler_to_matrix` 呼出し全箇所を `euler_to_matrix(<deg>.to_radians(), ...)` に置換する。数値結果と assertion 値は不変。以下の **3 ファイル** が対象 (`grep -rn 'euler_to_matrix' crates/` で完全列挙すること):

1. `crates/mycad-kernel/src/geometry/transform.rs` の inline `#[cfg(test)] mod tests` (4 箇所程度: T04/T05/t_rotation_is_isometry/t_rotation_rows_orthogonal)
2. `crates/mycad-kernel/src/brep/topology.rs` の inline `#[cfg(test)] mod tests` (7 箇所: L1495/1513/1547/1638/1669/1700/1718 付近 — t02_rotate_90deg_face_normals / t08_boundary_180 含む)
3. `crates/mycad-kernel/tests/rotate_acceptance.rs` の全 30+ 箇所

**手順**: シグネチャ変更前に `grep -rn 'euler_to_matrix' crates/` を実行し、`src/` 配下を含む全呼出し箇所を網羅すること。`tests/` 配下のみ走査すると `brep/topology.rs` 等の inline test を漏らす。

## 設計方針

- **決定性**: `euler_to_matrix` は純関数。`build_component_tree` の rotation 適用は浮動小数演算順序が固定 (行列積の左→右、Component の DFS 順) のため決定的。`IdGenerator` は rotation 適用前に Solid を生成するため EntityID は不変 (ADR-007 §2)。
- **B-rep トポロジー妥当性**: `Solid::rotate` は座標を写すだけで `vertices/edges/faces/shells` のインデックス構造を保持する (`crates/mycad-kernel/src/brep/topology.rs:476-489`)。Euler-Poincaré V−E+F−2S=0 は invariant。
- **退化幾何**: `is_finite()` ガードで NaN/Inf を拒否。snap (1e-15) は `euler_to_matrix` 内で吸収。
- **derive 規約**: 既存型の derive は変更なし。新規定数 `IDENTITY3` は `[[f64;3];3]` のため不要。
- **エラーハンドリング**: 既存 `KernelError::InvalidParameter { kind: "transform.rotation" }` を流用 (新規 variant 追加なし)。
- **workspace.dependencies**: 新規依存追加なし。

### 数値モデル

- **deg↔rad 境界**: ADR-004 厳守 — `euler_to_matrix` 入力は rad、build 層が `f64::to_radians()` で変換。format 層 `Transform.rotation` は deg のまま。
- **snap 閾値**: `1e-15` (`crates/mycad-kernel/src/geometry/transform.rs:14` の既存 `snap` 関数)。90°/180°/270° で行列要素が exact 0.0/1.0/-1.0 になる。
- **行列積の精度**: ADR-007 §2「Cylinder: 変換後の `orthonormal_basis(new_axis)` が元の基準を回転したものと一致することを精度 `1e-12` で確認」。本 Issue では axis 整合の数値検査は T06 で 1e-12 を用いる。
- **回転順序**: ZYX 順 (R = Rx(rx) * Ry(ry) * Rz(rz))。`euler_to_matrix` 既存実装の順序を維持。
- **回転中心**: origin (0,0,0)。`Solid::rotate(matrix, Point::origin())` を呼ぶ。component-local origin 回転は将来の拡張点 (本 Issue 範囲外)。

## テスト計画 (ID 付き)

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | `assembly.mycad` 相当の Document を rotation=[30,45,60] で 2 回 build_assembly し、両 Body の全 vertex 座標 & EntityID が完全一致 | byte-equal (assert_eq! の Vec<Body>) |
| T02 | 正常系 | 単一 cuboid Component を rotation=[0,0,90] (Z 軸 90°) で build → vertex の X/Y が入れ替わり Z 不変、座標値は snap で exact (整数 ±1, 0) | assert_eq! exact |
| T03 | 正常系 | rotation=[0,0,0] のみの assembly → rotation 配線前と byte-equal な出力 (rotation 適用パスをスキップ) | assert_eq! |
| T04 | 階層合成 | 親 Component (rotation=[90,0,0]) + 子 Component (position=[1,0,0], rotation=[0,90,0]) → 子の vertex が「親回転後の座標系で配置 + 子自身も回転」になっているか | 期待値は手計算で `[[f64;3];3]` 行列積で導出 |
| T05 | manifold | rotation=[30,45,60] × Boolean Fuse (cuboid + cuboid)。結果 Solid が `is_manifold() == true`, `euler_poincare() == 0` | assertion |
| T06_boundary_seam | 境界 | Cylinder を rotation=[90,0,0] (axis: +Y → +Z) で回転後、Cylinder face の axis に対応する pcurve seam の整合を ADR-007 §2 の `orthonormal_basis(new_axis)` 精度 1e-12 で検証 | `(actual − expected).abs() < 1e-12` |
| T07_degen_zero_rotation | 退化/ガード | rotation=[0,0,0] が rotation=[1e-20, 0, 0] と同等の出力になる (snap で吸収)、かつ `accumulated_rotation == IDENTITY3` のとき `Solid::rotate` を呼ばない (byte-equal ガード) | assert_eq! Vec<Body> |
| T08_boundary_nonfinite | エラー系 | `transform.rotation = [f64::NAN, 0, 0]` → `KernelError::InvalidParameter { kind: "transform.rotation" }` | `matches!` |
| T09 | ADR 整合 | `euler_to_matrix(90.0_f64.to_radians(), 0.0, 0.0)` が `Rx(90°) = [[1,0,0],[0,0,-1],[0,1,0]]` を返す (シグネチャ変更後も既存 snap 規則が rad で動く) | assert_eq! exact |
| T10_blocked_cut_sphere | 保留 | rotation × Boolean Cut Sphere — `#[ignore = "blocked: #137 trimmed sphere tessellation"]` のみ配置 | (#137 解決後に通る) |

## 幾何的不変条件チェックリスト

- N/A: 本 Issue は partition / pslg_subdivide / 法線処理を直接触らない。`Solid::rotate` は座標写像のみで topology を保つ。
- T05 で boolean Fuse 後の `is_manifold()` / `euler_poincare()` を assertion することで間接保証する。
- 自己隣接周期面 (cylinder seam) の取扱: T06 で軸回転後の `orthonormal_basis` 整合 (1e-12) を assertion し、seam edge の正逆 HalfEdge 構造が崩れないことを担保する。
