## In-Scope / Out-of-Scope
<!-- ADR-006 §plan.md 必須セクション。GLM SCOPE ペルソナが存在を検証する。 -->
| In-Scope | Out-of-Scope |
|----------|--------------|
| T18: A2 結果 solid の `tessellate_solid` 成功・三角形数 > 0 | T20(manifold) / T33(Euler): #49 で実装済み (`t02_manifold` / `t04_euler_poincare`)。本 Issue では再実装しない |
| T34: `intersect_surfaces(cyl, sph)` 返り値の loop 数・center.z 昇順・normal=(0,0,1)・t_range=[0,2π] | A2/A4b 基本動作: #49 で land 済み |
| T22b: 再ビルド決定性 + `tessellate@8/@64` 成功 + `assert_solids_equal_with_names` | Cyl×Sph の Cut / Fuse: Phase 4 完了条件は他ペアでカバー済み |
| T23: `examples/boolean_intersect_cyl_sphere.mycad` の YAML round-trip byte-identical | 非軸整列 cyl×sph: MVP 外、reject のまま |
| `examples/boolean_intersect_cyl_sphere.mycad` の新規作成 (T23 fixture) | ビューア目視確認 (#35) |
| 幾何コア・partition ロジックの変更（#49 で完了済み） |

## Non-Goals
<!-- Out-of-Scope と同内容でも重複 OK。dispatch-codex-auto.ts の guard が参照する。 -->
- T20 (`validate_manifold`): #49 の `t02_manifold` で実装済み。重複実装しない。
- T33 (Euler-Poincaré): #49 の `t04_euler_poincare` で実装済み（seam 考慮で `1 or 2` 許容）。重複実装しない。
- Cyl×Sph の Cut / Fuse 演算: 本 Issue 対象外。
- 非軸整列 cyl×sph のサポート: MVP 外、reject 維持（#49 t06 でエラー伝播確認済み）。
- 幾何コア / partition / assemble の挙動変更: #49 で完了。本 Issue はテスト追加 + example fixture のみ。

## 実装対象
<!-- Issue: #43 -->
<!-- 影響クレート/ファイル: -->
- `crates/mycad-build/tests/cyl_sph_intersect_acceptance.rs` (既存拡張): **T18**, **T22b** を追加
- `crates/mycad-kernel/tests/cyl_sph_surface_intersect.rs` (新規): **T34**
- `examples/boolean_intersect_cyl_sphere.mycad` (新規): **T23** fixture
- `crates/mycad-build/tests/cyl_sph_intersect_acceptance.rs` に **T23** round-trip テストを追加（`Document` API 経由）

新規テスト・新規 fixture の追加のみ。**既存関数の変更なし**のため before/after スニペットは不要。

再利用する既存 API（推測でなく調査済み）:
- `build_bodies_from_features(&[Feature], &mut IdGenerator)` — `mycad-build`
- `tessellate_solid(&Solid)` (`tessellation/mod.rs:96`), `tessellate_solid_with(&Solid, &TessellationOptions)` (`:101`), `TriangleMesh::triangle_count()` (`:41`)
- `TessellationOptions::new(angular, axial)` (`tessellation/mod.rs:76`)
- `intersect_surfaces(&Surface, &Surface) -> Result<Vec<IntersectionLoop>, KernelError>` (`geometry/surface_intersect.rs:25`, pub)
- `Surface::Cylinder { origin, axis, radius }` / `Surface::Sphere { center, radius }` (`geometry/surface.rs:25`, pub variants)
- `IntersectionLoop { curve_3d, t_range, .. }` (`surface_intersect.rs:13`, pub fields); `Curve::Circle { center, normal, radius }` (`geometry/curve.rs:7`)
- `Document::from_path` / `to_yaml` / `from_yaml` — `mycad-format/src/document.rs`
- `assert_solids_equal_with_names` — テストローカルヘルパー（`position_params_acceptance.rs:68` / `feature_dispatcher.rs:63` から本ファイルへコピー。`cyl_sph_intersect_acceptance.rs` に未存在なら追加）
- `examples_dir()` パターン — `CARGO_MANIFEST_DIR/../../examples`（`position_params_acceptance.rs:141`）

## 設計方針
- **決定性**: T22b は `IdGenerator::new(0)` で同一 features を2回ビルド。`angular_segments` は `.mycad` schema に存在せず tessellation 時のみ作用するため、Solid トポロジーは解像度非依存。`tessellate_solid_with(8,1)` と `(64,1)` の両方が `Ok` になることを確認したうえで、2つの Solid を `assert_solids_equal_with_names`（id・座標・entity name 一致）で比較する。
- **B-rep 妥当性**: manifold (T20) / Euler (T33) は #49 でカバー済みのため本 Issue では再検証しない（Non-Goals）。
- **退化幾何**: 本 Issue は新規幾何生成なし。フィクスチャは `r_sq = 16 > 0` の正常交差のみ扱う。
- **derive 規約**: 新規公開型を追加しないため対象外。
- **エラーハンドリング**: テストは `expect`/`assert`。非軸エラー伝播は #49 `t06_non_coaxial_errors` でカバー済み。
- **workspace.dependencies**: 新規依存なし。

### 数値モデル
フィクスチャ（#49 と同一）: cyl `radius=3, height=20, origin=(0,0,-10), axis=(0,0,1)`、sph `radius=5, center=(0,0,0)`。
- `r_sq = 5² − 3² = 16` → `h_offset = 4` → 交差円: 中心 `z = ±4`、半径 `3`。
- **T34 期待値**: `loops.len() == 2`；両 `curve_3d` が `Curve::Circle`；両 `normal == Vec3(0,0,1)`；両 `t_range == [0.0, 2π]`；`loops[0].center.z (= -4) < loops[1].center.z (= +4)`（#49 が height 昇順ソート済み）。
- 浮動小数比較: `center.z` / `radius` / `normal` は `LENGTH_TOLERANCE` 近傍で近似 assert（既存 `t03` と同 tolerance）。`2π` は `2.0 * std::f64::consts::PI` を tolerance 付きで比較。
- ADR-004: tolerant 比較を踏襲。

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T18 | 正常系/tessellation | `build_intersect()` の solid を `tessellate_solid` | `Ok` かつ `triangle_count() > 0` |
| T34 | 幾何コア | `intersect_surfaces(cyl, sph)` の返り値検証 | 2 loop、両 normal≈(0,0,1)、両 t_range≈[0,2π]、center.z 昇順 |
| T22b | 決定性/解像度非依存 | 同 features を2回ビルド、`tessellate_solid_with(8,1)`/`(64,1)` が成功、Solid を比較 | 両 tessellate `Ok` かつ全 entity name 一致 |
| T23 | golden/round-trip | `boolean_intersect_cyl_sphere.mycad` を `from_path→to_yaml→from_yaml→to_yaml` | 再シリアライズ idempotent（byte-identical）。可能ならファイル内容とも一致 |

(T20/T33 は #49 実装済みのため本表から除外)

T23 補足: 新 example は `document.rs::test_ts_derive_backward_compat` の `read_dir` ループにも自動的に取り込まれ round-trip idempotency が検証される。byte-identical を満たすため、example ファイルはシリアライザ出力形（float は `3.0` 形式、`origin`/`center` のデフォルト `[0,0,0]` は省略）に揃える。実装時に round-trip テストで検証し、不一致なら serializer 出力で再生成する。

## 幾何的不変条件チェックリスト
<!-- Boolean/Partition/Assemble 系の Issue のみ記述。非該当は各項目を "N/A" に書き換えること。 -->
- N/A — partition 出力の polygon 頂点順と assemble の normal 処理（#49 で確定、本 Issue は変更なし）
- N/A — 各プリミティブの outer_loop 2D 向き（本 Issue で変更なし）
- N/A — flip_normals / same_sense の意味論（本 Issue で変更なし）
- N/A — pslg_subdivide の出力向き（本 Issue で変更なし）

本 Issue は **テスト追加 + example fixture のみ**で幾何生成ロジックを変更しないため、全項目 N/A。
