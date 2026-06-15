# debug-spec round 9 (Issue #158 Codex r3: F01 ExtrudeCut offset + F02 build is_finite)

## 経緯

Codex r2 で指摘された F01 r2 (canonical fallback) + F02 (format is_finite) は r3 で確認済み。r3 で新たに以下 2 件が出た:

- **F01 (high)**: ExtrudeCut の `resolve_plane()` 統一化で **別 Issue (#161) スコープの「CreateSketch.offset 無視バグ」が意図せず修正** されており、`s04b_extrudecut_depth_just_inside_boundary` を `#[ignore]` で回避している。**plane_ref 無し経路では従来通り offset を無視する元挙動に戻し、s04b の #[ignore] を外す** べき。
- **F02 (medium)**: format 層の `validate_component` で `RefPlane.offset.is_finite()` を検証しているが、**build 公開 API (`build_bodies_from_features`) に直接渡される `&[RefPlane]` slice は未検証**。`resolve_plane()` の前段でも検証して typed error を返す。

## F01 修正: ExtrudeCut の plane_ref 無し経路で offset 無視を復活

### 修正対象 (`crates/engawa-build/src/lib.rs:211〜243` 付近の Feature::ExtrudeCut match arm)

**現状 (推定 — round 7 で書き換えた状態)**:
```rust
Feature::ExtrudeCut { id: _, sketch, depth, target } => {
    if *depth <= 0.0 { return Err(KernelError::InvalidParameter { kind: "depth" }); }
    let entry = sketches.get(sketch.as_str()).ok_or_else(...)?;
    let plane = resolve_plane(entry, ref_planes)?;  // ← Extrude と統一されている
    // ...
}
```

**after** (plane_ref 有り = 新挙動 / 無し = 元の _offset 抑制バグを温存):

```rust
Feature::ExtrudeCut { id: _, sketch, depth, target } => {
    if *depth <= 0.0 { return Err(KernelError::InvalidParameter { kind: "depth" }); }
    let entry = sketches.get(sketch.as_str()).ok_or_else(...)?;

    // ExtrudeCut は元来 CreateSketch.offset を無視する仕様バグがあった (#161 で別途修正予定)。
    // 本 Issue (#158) のスコープを守るため、plane_ref が無い経路では従来挙動を温存する。
    // plane_ref が Some なら新経路 (resolve_plane 経由) を通り、ref_planes から正しく解決する。
    let plane = if let Some(ref_id) = &entry.plane_ref {
        // 新経路: plane_ref を ref_planes から解決
        let rp = ref_planes.iter().find(|p| &p.id == ref_id)
            .ok_or_else(|| KernelError::UnknownRefPlane { id: ref_id.clone() })?;
        if !rp.offset.is_finite() {
            return Err(KernelError::InvalidRefPlaneOffset { id: rp.id.clone() });
        }
        let base = sketch_plane_to_plane(rp.plane);
        if rp.offset != 0.0 { base.translate(base.normal * rp.offset) } else { base }
    } else {
        // 旧経路: plane だけ使い、offset は **無視** (元仕様バグの温存、#161 で対応)
        sketch_plane_to_plane(entry.plane)
    };

    let profile_uv: Vec<(f64, f64)> = entry.profile.iter()
        .map(|s| (s.from[0], s.from[1]))
        .collect();
    let tool = make_extrusion(&plane, &profile_uv, *depth, gen)?;
    // ... 以下従来通り
}
```

### s04b の #[ignore] を外す

`crates/engawa-api/tests/e2e_api_scenarios.rs:s04b_extrudecut_depth_just_inside_boundary` の `#[ignore = "#158: ..."]` 属性行を **削除** し、テストを再有効化する。

これで「ExtrudeCut で offset 無視」が元通り機能し、s04b が `200 OK` を返すようになる。

## F02 修正: build 公開 API でも `RefPlane.offset.is_finite()` を検証

### 修正対象 (`crates/engawa-build/src/lib.rs` の `resolve_plane()` または build_bodies_from_features 前段)

**before** (resolve_plane の最初):
```rust
fn resolve_plane(entry: &SketchEntry, ref_planes: &[RefPlane]) -> Result<Plane, KernelError> {
    if let Some(ref_id) = &entry.plane_ref {
        let rp = ref_planes.iter().find(|p| &p.id == ref_id)
            .ok_or_else(|| KernelError::UnknownRefPlane { id: ref_id.clone() })?;
        let base = sketch_plane_to_plane(rp.plane);
        // ... rp.offset を使う
    } else {
        // ...
    }
}
```

**after** (resolve_plane の最初に検証を入れる):

```rust
fn resolve_plane(entry: &SketchEntry, ref_planes: &[RefPlane]) -> Result<Plane, KernelError> {
    if let Some(ref_id) = &entry.plane_ref {
        let rp = ref_planes.iter().find(|p| &p.id == ref_id)
            .ok_or_else(|| KernelError::UnknownRefPlane { id: ref_id.clone() })?;
        if !rp.offset.is_finite() {
            return Err(KernelError::InvalidRefPlaneOffset { id: rp.id.clone() });
        }
        let base = sketch_plane_to_plane(rp.plane);
        // ...
    } else {
        // 旧 plane+offset 経路でも offset を検証してよい (任意)。
        // ただし plan.md の Out-of-Scope に「ExtrudeCut の offset 無視バグ」が含まれるため、
        // Extrude/ExtrudeCut の旧経路の offset 検証は控えめにする。
        // is_finite() 検証を入れたいが既存テストへの副作用を最小化するため、本 Issue では plane_ref 経路のみ検証。
        // ...
    }
}
```

または、`build_bodies_from_features` の入り口で `ref_planes` slice 全体を検証:

```rust
pub fn build_bodies_from_features(
    features: &[Feature],
    ref_planes: &[RefPlane],
    gen: &mut IdGenerator,
) -> Result<BuiltBodies, KernelError> {
    // 公開 API のガード: ref_planes の offset 有限性検証
    for rp in ref_planes {
        if !rp.offset.is_finite() {
            return Err(KernelError::InvalidRefPlaneOffset { id: rp.id.clone() });
        }
    }
    // ... 以下従来通り
}
```

どちらでも良いが、Codex は `resolve_plane()` 前段を提案しているので **resolve_plane の plane_ref 経路でのみ検証** が最小修正。

### 新規エラー variant

`crates/engawa-build/src/error.rs` (または `crates/engawa-kernel/src/error.rs`) に `InvalidRefPlaneOffset { id: String }` を新設。**format 側の `FormatError::InvalidRefPlaneOffset` とは別**で、`KernelError::InvalidRefPlaneOffset` (または `BuildError::InvalidRefPlaneOffset`) として追加する。thiserror で:

```rust
#[error("RefPlane '{id}' has non-finite offset (NaN or Inf)")]
InvalidRefPlaneOffset { id: String },
```

### 新規テスト T18

`crates/engawa-build/tests/refplane_acceptance.rs` に追加:

```rust
#[test]
fn t18_degen_build_layer_refplane_offset_not_finite() {
    use engawa_build::build_bodies_from_features;
    use engawa_format::{Feature, RefPlane, SketchPlane, SketchSegment};
    use engawa_kernel::IdGenerator;

    let bad_plane = vec![RefPlane {
        id: "BadOffset".into(),
        plane: SketchPlane::Xy,
        offset: f64::NAN,  // raw build API に直接渡す
    }];
    let features = vec![
        Feature::CreateSketch {
            id: "sk".into(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            plane_ref: Some("BadOffset".into()),
            profile: vec![
                SketchSegment { id: "s1".into(), from: [0.0, 0.0], to: [1.0, 0.0] },
                SketchSegment { id: "s2".into(), from: [1.0, 0.0], to: [1.0, 1.0] },
                SketchSegment { id: "s3".into(), from: [1.0, 1.0], to: [0.0, 1.0] },
                SketchSegment { id: "s4".into(), from: [0.0, 1.0], to: [0.0, 0.0] },
            ],
        },
        Feature::Extrude { id: "ex".into(), sketch: "sk".into(), depth: 1.0, fuse_target: None },
    ];
    let mut gen = IdGenerator::new(0);
    let err = build_bodies_from_features(&features, &bad_plane, &mut gen).unwrap_err();
    // 期待: KernelError::InvalidRefPlaneOffset { id: "BadOffset" } (または BuildError::*)
    let s = format!("{err:?}");
    assert!(s.contains("BadOffset") && s.contains("non-finite"), "expected InvalidRefPlaneOffset error: {s}");
}
```

## 完了条件

- `cargo xtask ci` が green
- s04b_extrudecut_depth_just_inside_boundary が **再有効化** (= #[ignore] 削除) され、200 OK を返して pass
- F02 の T18 が pass
- 既存テスト全件維持

## 試した修正と結果

- Round 1〜6: CI green 達成
- Round 7: F01 修正 (空 child を親継承) → Codex r2 再指摘
- Round 8: F01 r2 修正 (canonical fallback) + F02 (format is_finite) → Codex r3 で新 F01 (ExtrudeCut スコープ違反) + 新 F02 (build is_finite)
- Round 9 (本ファイル): ExtrudeCut を plane_ref 有無で分岐 + build is_finite 追加

## 注意点

- ExtrudeCut の **plane_ref 無し経路** では `entry.offset` を **読まない** (= 元の `_offset` 抑制バグを温存)
- `plane_ref` 有り経路は新経路 (resolve_plane 統一) のまま
- `s04b` の `#[ignore]` 属性を **削除する**
- 副次起票した Issue #161 は **本 Issue で対応しない** (まだ生きている、Phase 8 等で別途処理)

## 追加で書いてほしいテスト

- 上記 T18 (build 層 is_finite)
- s04b の再有効化
