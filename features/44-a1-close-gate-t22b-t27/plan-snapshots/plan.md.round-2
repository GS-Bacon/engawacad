## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| T22b: A1 ソリッドを tessellate (angular=8 と 64) しても B-rep entity name が変わらないことを `assert_solids_equal_with_names` で確認 | Cylinder×Sphere Intersect 統合テスト (#43) |
| T27: A1 の mesh 体積 ≈ 1000 - 20π ≈ 937.168 (相対誤差 < 1%) の数値アサーション | ビューア目視確認 (#35) |
| `a1_mesh_volume_abs` private ヘルパー追加 (`feature_dispatcher.rs` 内) | ADR-004 への追記 |
| — | `ANGULAR_SEGMENTS_DEFAULT` の API parameterization |
| — | A1.1 (Fuse/Intersect) 系テストの追加 |

## Non-Goals

- Cylinder×Sphere 統合テスト — Issue #43 に委譲
- ビューア目視確認 — Issue #35 (Phase 4 close gate) に委譲
- ADR-004 追記 — #34 親 Issue の責務
- `ANGULAR_SEGMENTS_DEFAULT` の可変化 — Phase 4 スコープ外
- `signed_volume()` を Solid public API として expose — mesh 積分で十分

## 実装対象
<!-- Issue: #44 -->
<!-- 影響クレート/ファイル: crates/mycad-build/tests/feature_dispatcher.rs (テスト追加のみ) -->

**既存ファイル変更なし。テスト 2 本 + ヘルパー 1 本の追記のみ。**

配置先: `crates/mycad-build/tests/feature_dispatcher.rs` の A1 テストブロック末尾 (現在 line 1869 付近) に追記。

### 追加コード

```rust
fn a1_mesh_volume_abs(mesh: &mycad_kernel::tessellation::TriangleMesh) -> f64 {
    let mut vol = 0.0_f64;
    for tri in 0..mesh.triangle_count() {
        let i0 = mesh.indices[tri * 3] as usize;
        let i1 = mesh.indices[tri * 3 + 1] as usize;
        let i2 = mesh.indices[tri * 3 + 2] as usize;
        let p0 = &mesh.positions[i0];
        let p1 = &mesh.positions[i1];
        let p2 = &mesh.positions[i2];
        vol += (p0[0] * (p1[1] * p2[2] - p2[1] * p1[2])
            + p1[0] * (p2[1] * p0[2] - p0[1] * p2[2])
            + p2[0] * (p0[1] * p1[2] - p1[1] * p0[2]))
            / 6.0;
    }
    vol.abs()
}

#[test]
fn a1_solid_invariant_across_angular_segments() {
    use mycad_kernel::tessellation::{tessellate_solid_with, TessellationOptions};

    let mut g1 = IdGenerator::new(0);
    let b1 = build_bodies_from_features(&build_a1_input(), &mut g1).expect("build @8");
    let solid1 = b1.get("cut1").unwrap().solid.clone();
    let _mesh_low = tessellate_solid_with(&solid1, &TessellationOptions::new(8, 1))
        .expect("tess @8");

    let mut g2 = IdGenerator::new(0);
    let b2 = build_bodies_from_features(&build_a1_input(), &mut g2).expect("build @64");
    let solid2 = b2.get("cut1").unwrap().solid.clone();
    let _mesh_high = tessellate_solid_with(&solid2, &TessellationOptions::new(64, 1))
        .expect("tess @64");

    assert_solids_equal_with_names(&solid1, &solid2);
}

#[test]
fn a1_signed_volume_matches_theoretical() {
    use mycad_kernel::tessellation::tessellate_solid;

    let bodies = build_features(build_a1_input()).expect("A1 build");
    let solid = &bodies.get("cut1").unwrap().solid;
    let mesh = tessellate_solid(solid).expect("tessellate");

    let vol = a1_mesh_volume_abs(&mesh);
    let expected = 1000.0 - 20.0 * std::f64::consts::PI;
    let rel_err = (vol - expected).abs() / expected;
    assert!(
        rel_err < 0.01,
        "A1 volume: expected ~{expected:.3}, got {vol:.3} (rel_err={rel_err:.4})"
    );
}
```

## 設計方針

**決定性**: B-rep 側の構築は `ANGULAR_SEGMENTS_DEFAULT = 64` (固定定数) で決定的。`TessellationOptions` は mesh 生成にのみ作用し、Solid 構造を変更しない。T22b で「tessellation 呼び出しが Solid に副作用を持たない」ことを両方向 (angular=8/64) から保証する。

**T22b の意義**: `tessellate_solid_with` は `&Solid` 受け取り → Solid は変更不可。現状はキャッシュなし設計だが、将来 Solid にキャッシュや memoization が追加されても本テストが検知できる。

**T27 の数値根拠**:
- box: 10×10×10 = 1000
- cylinder 内部分 (box と重なる部分): r=2, h_overlap=5 (cylinder h=6, box top z=5 → overlap = 5)
- 理論値: 1000 - π × 2² × 5 = 1000 - 20π ≈ 937.168
- angular=32 (デフォルト) の chord 近似誤差: 円周の ~0.4% 過小 → 体積も過小 ~0.4% → 閾値 1% に余裕あり

**既存関数の編集**: なし。新規テスト + private ヘルパー追加のみ。

### 数値モデル

- 体積積分: tessellation mesh の signed tetrahedral volume (スカラー三重積 / 6) の abs 総和
- 合格閾値: 相対誤差 < 1% (Issue #44 文面)
- 参照ε: ADR-004 准拠 (tolerant 方式継続、#31 で確立済み)

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T22b | 不変条件 | A1 を angular=8 と angular=64 で tessellate 後に Solid を `assert_solids_equal_with_names` 比較 | V/E/F 数・entity name が完全一致 |
| T27 | 数値正常系 | A1 mesh 体積を三重積積分で計算、理論値 1000-20π と比較 | 相対誤差 < 1% |

## 幾何的不変条件チェックリスト

- N/A: partition の polygon 向きと assemble の normal 整合 — 本 Issue はテスト追加のみ、partition/assemble 変更なし
- N/A: face outer_loop 2D 向きの文書化 — テスト追加のみ
- N/A: flip_normals / same_sense 意味論 — テスト追加のみ
- N/A: pslg_subdivide 出力向き整合 — テスト追加のみ
