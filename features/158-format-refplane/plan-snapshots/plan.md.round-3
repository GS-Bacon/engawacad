## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `engawa-format` に `RefPlane { id, plane, offset }` 型を追加 | UI (RefPlane 描画 / raycaster 選択) — #2 viewer-refplane-pick |
| `Component` に `ref_planes: Vec<RefPlane>` フィールド追加 | kernel の変更 (RefPlane は format/build に閉じる) |
| `Document::new()` で root_component.ref_planes に Front(Xy)/Top(Xz)/Right(Yz) 3 件を自動配置 | parse 経由 (`from_yaml` / `from_path`) での ref_planes 自動補填 — UI 側 (#2) が責任 |
| `Feature::CreateSketch` に `plane_ref: Option<String>` フィールド追加 (既存 `plane`/`offset` は残す) | `plane` フィールドの将来削除 (別 Phase 検討) |
| `engawa-build` の CreateSketch ディスパッチで `plane_ref` を `ref_planes` から解決し既存 base_plane 経路に乗せる | `plane_ref` と `plane`/`offset` の両方ある場合の警告ログ (plane_ref が Some なら plane を黙って捨てる) |
| `build_bodies_from_features` シグネチャ拡張: `ref_planes: &[RefPlane]` 引数追加 | Phase 8 用の任意平面追加 API / RefPlane の動的追加 |
| `examples/extruded_rect_refplane.engawa` 新規追加 + `examples_smoke.rs` エントリ追加 | `ExtrudeCut` の offset バグ修正 — #6 fix-extrudecut-offset (独立 Issue) |
| 新規 YAML golden test 2 種 (ref_planes 自動配置 / plane_ref を使う CreateSketch) | ts-rs 生成物 (`web/src/generated/RefPlane.ts`) の手書き調整 — 自動再生成のみ |
| 既存 `examples/*.engawa` の parse → build → kernel 互換維持テスト | フロント (`web/src/`) のコード変更 — 一切なし |

## Non-Goals

- UI 描画・クリック選択 (#2 担当)
- kernel への影響 (RefPlane は format/build に閉じる)
- 古い `.engawa` (ref_planes 無し) を parse した直後に root_component.ref_planes へ 3 件を自動補填する処理 — UI 側 (#2) で「空なら補填」する責任にする
- `plane_ref` と `plane`/`offset` 両方が指定された場合の警告/エラー (本 Issue では plane_ref が `Some` なら plane を黙って優先)
- 任意平面 (Phase 8) の追加 API
- `ExtrudeCut` の offset 無視バグ修正 (独立 Issue #6 fix-extrudecut-offset)
- フロントエンドへの変更
- format 層での `plane_ref` 文字列の妥当性検証 (空文字 / 不正 char) — build 層が「該当 ref_planes に見つからない」エラーで catch

## 実装対象

- Issue: #158
- 影響クレート/ファイル:
  - `crates/engawa-format/src/feature.rs`: `RefPlane` 型新規追加, `CreateSketch` に `plane_ref` 追加
  - `crates/engawa-format/src/component.rs`: `Component` に `ref_planes` フィールド追加
  - `crates/engawa-format/src/document.rs`: `Document::new` で `default_ref_planes()` を root_component に挿入。`Component::new` は空のまま (Document::new だけが補填責任を持つ)
  - `crates/engawa-format/src/lib.rs`: `pub use feature::RefPlane;` 追加
  - `crates/engawa-kernel/src/error.rs`: `KernelError::RefPlaneNotFound { id: String }` 追加
  - `crates/engawa-build/src/lib.rs`: `build_bodies_from_features` シグネチャ拡張 (`ref_planes: &[RefPlane]` 引数追加)、CreateSketch ディスパッチで `plane_ref` 解決ロジック追加、`build_component_tree` 内呼び出し修正
  - `crates/engawa-build/tests/*_acceptance.rs` (全 13 ファイル): 既存呼び出しに `&[]` または `&doc.root_component.ref_planes` を機械的に追加
  - `crates/engawa-build/tests/examples_smoke.rs`: 新サンプル `extruded_rect_refplane.engawa` smoke 関数追加
  - `examples/extruded_rect_refplane.engawa`: 新規 (Front プレーン経由で `extruded_rect.engawa` と同等形状)
  - **API crate / Viewer 側で `build_bodies_from_features` を直接呼んでいる場合**: 実装前に必ず `grep -rn "build_bodies_from_features" crates/ web/` で全呼び出し元を洗い出し、`&[]` または対応する ref_planes を渡すよう機械的修正

### 既存関数の修正 (before / after)

#### `crates/engawa-format/src/feature.rs` — Feature::CreateSketch

before:
```rust
#[serde(rename = "create_sketch")]
CreateSketch {
    id: String,
    plane: SketchPlane,
    #[serde(default, skip_serializing_if = "is_zero")]
    offset: f64,
    profile: Vec<SketchSegment>,
},
```

after:
```rust
#[serde(rename = "create_sketch")]
CreateSketch {
    id: String,
    plane: SketchPlane,
    #[serde(default, skip_serializing_if = "is_zero")]
    offset: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    plane_ref: Option<String>,
    profile: Vec<SketchSegment>,
},
```

新規型 (追加のみ):
```rust
/// A named reference plane available for sketch placement.
/// Auto-populated with Front/Top/Right when a Document is created via `Document::new`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct RefPlane {
    pub id: String,
    pub plane: SketchPlane,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub offset: f64,
}
```

#### `crates/engawa-format/src/component.rs` — Component

before:
```rust
pub struct Component {
    pub name: String,
    #[serde(default, skip_serializing_if = "is_default_transform")]
    pub transform: Transform,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "ref")]
    pub reference: Option<ComponentRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<Feature>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Component>,
}
```

after (フィールド追加のみ):
```rust
pub struct Component {
    pub name: String,
    #[serde(default, skip_serializing_if = "is_default_transform")]
    pub transform: Transform,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "ref")]
    pub reference: Option<ComponentRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<Feature>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Component>,
    /// Reference planes available for sketch placement.
    /// Populated only by `Document::new` (Front/Top/Right). Parse-loaded
    /// components without ref_planes deserialize as empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ref_planes: Vec<RefPlane>,
}
```

`Component::new(name)` は `ref_planes: Vec::new()` (補填しない)。Document::new だけが補填責任を持つ。

#### `crates/engawa-format/src/document.rs` — Document::new

before:
```rust
impl Document {
    pub fn new(name: &str) -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            version: env!("CARGO_PKG_VERSION").to_string(),
            root_component: Component::new(name),
        }
    }
}
```

after:
```rust
impl Document {
    pub fn new(name: &str) -> Self {
        let mut root_component = Component::new(name);
        root_component.ref_planes = default_ref_planes();
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            version: env!("CARGO_PKG_VERSION").to_string(),
            root_component,
        }
    }
}

/// Front/Top/Right の 3 枚を決定論的に返す。順序固定 (Front, Top, Right)。
fn default_ref_planes() -> Vec<RefPlane> {
    vec![
        RefPlane { id: "Front".to_string(), plane: SketchPlane::Xy, offset: 0.0 },
        RefPlane { id: "Top".to_string(),   plane: SketchPlane::Xz, offset: 0.0 },
        RefPlane { id: "Right".to_string(), plane: SketchPlane::Yz, offset: 0.0 },
    ]
}
```

`default_ref_planes()` は document.rs 内 module-private (UI 側 #2 も将来同じ補填が必要になるので、必要なら次 Issue で `pub fn` 化 — 本 Issue ではスコープ外)。

#### `crates/engawa-build/src/lib.rs` — build_bodies_from_features

before:
```rust
pub fn build_bodies_from_features(
    features: &[Feature],
    gen: &mut IdGenerator,
) -> Result<BuiltBodies, KernelError> {
    ...
    Feature::CreateSketch { id: _, plane, offset, profile } => {
        validate_sketch_segment_ids(profile)?;
        validate_profile_closed(profile)?;
        if sketches.insert(id, (plane, profile.as_slice(), *offset)).is_some() {
            return Err(KernelError::DuplicateFeatureId { id: id.to_string() });
        }
    }
    ...
}
```

after:
```rust
pub fn build_bodies_from_features(
    features: &[Feature],
    ref_planes: &[engawa_format::RefPlane],
    gen: &mut IdGenerator,
) -> Result<BuiltBodies, KernelError> {
    ...
    Feature::CreateSketch { id: _, plane, offset, plane_ref, profile } => {
        validate_sketch_segment_ids(profile)?;
        validate_profile_closed(profile)?;
        let (resolved_plane, resolved_offset): (&engawa_format::SketchPlane, f64) =
            if let Some(rp_id) = plane_ref {
                let rp = ref_planes.iter().find(|p| &p.id == rp_id)
                    .ok_or_else(|| KernelError::RefPlaneNotFound { id: rp_id.clone() })?;
                (&rp.plane, rp.offset)
            } else {
                (plane, *offset)
            };
        if sketches.insert(id, (resolved_plane, profile.as_slice(), resolved_offset)).is_some() {
            return Err(KernelError::DuplicateFeatureId { id: id.to_string() });
        }
    }
    ...
}
```

`build_component_tree` 内呼び出しも `&component.ref_planes` を渡すよう修正。

## 設計方針

- **決定性要件**: `default_ref_planes()` は常に `[Front(Xy,0.0), Top(Xz,0.0), Right(Yz,0.0)]` を同じ順序で返す。100 回 `Document::new("X")` を実行しても ref_planes の中身は完全一致。`build_bodies_from_features` の plane_ref 解決は線形検索 (find) で順序非依存だが、ref_planes の挿入順が決定的なら結果も決定的。
- **B-rep トポロジー妥当性**: 本 Issue は format/build 層に閉じ、kernel (`make_extrusion`) には変更なし。Euler-Poincaré (V-E+F=2) は既存テストでカバー済み。
- **退化幾何の扱い**:
  - SketchSegment の閉ループ判定 (`LENGTH_TOLERANCE = 1e-9`) は変更なし
  - RefPlane の offset 値は f64 そのまま保持。**本 Issue では finite チェック (NaN/Inf 弾き) を実施しない** (Phase 7 で配置される全 RefPlane は offset = 0.0 固定、ユーザー任意入力経路がまだ無いため)。Phase 8 で UI 経由の任意平面追加を導入する際に format 層 validation を別 Issue で追加する
  - `plane_ref` が空文字 `""` の場合 → `find` で一致しないので `RefPlaneNotFound` エラーになる (validation 不要)
  - ref_planes 内に同じ id が複数ある場合 → `find` が最初の 1 件を返す (= 挿入順で最初の RefPlane が優先)。**本 Issue では重複検証を行わない** (Phase 7 では Document::new が固定 3 件を返すため発生せず、parse 経由は UI 側責任)。警告ログも出さない。Phase 8 で「ユーザー定義 RefPlane」を導入する際に重複 id 検証を別 Issue で追加する
- **derive 規約**: `RefPlane` に `Debug, Clone, Serialize, Deserialize, JsonSchema, TS` を全て付与 (Document/Component と同等)。ts-rs により `web/src/generated/RefPlane.ts` が自動生成される。
- **エラーハンドリング**: `thiserror` で `KernelError::RefPlaneNotFound { id: String }` を追加。format 層では `plane_ref` 文字列の妥当性検証はしない (空文字や invalid char は build 時に「該当なし」で弾かれる)。
- **workspace.dependencies**: 新規依存なし。既存の serde/serde_yaml/schemars/ts_rs だけで完結。

### 数値モデル

本 Issue は形式変更のみで数値判断 (ε, tolerance) を持たない。N/A。

- RefPlane.offset は f64 そのまま保持。Phase 7 は全て 0.0
- build 層の `base_plane.translate(normal * offset)` 判定は既存通り `if *offset != 0.0` (直接 0.0 比較)
- ε 値の導入は不要

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | `Document::new("X")` を 100 回作成し、root_component.ref_planes が常に `[Front(Xy,0.0), Top(Xz,0.0), Right(Yz,0.0)]` の同一順序・同一値 | 100 回とも byte-identical |
| T02 | 正常系/format | `RefPlane` 単体の YAML serialize → deserialize round-trip | 完全一致 |
| T03 | 正常系/format | `Document::new("Test")` 直後の root_component.ref_planes が Front/Top/Right の 3 件で id/plane/offset すべて assert | 3 件、各 plane が Xy/Xz/Yz、offset=0.0 |
| T04 | 正常系/format | `Component::new("X")` (Document::new を介さず) は ref_planes が空 vec! | `is_empty()` |
| T05 | golden/format | `Document::new("Test")` の to_yaml() に `ref_planes` セクションが Front, Top, Right の順で出力される | byte-identical golden |
| T06 | golden/format | `plane_ref: Some("Front")` を持つ CreateSketch の to_yaml() で `plane_ref: Front` が出力される | byte-identical golden |
| T07 | 互換/format | 既存 `examples/*.engawa` 全件で `from_path` → `to_yaml` → `from_yaml` → `to_yaml` が byte-identical (`test_ts_derive_backward_compat` 拡張) | 全ファイル round-trip 一致 |
| T08 | 互換/format | 既存 `extruded_rect.engawa` を parse して CreateSketch の `plane_ref` が `None`、`plane` が `Xy` で読まれる | `plane_ref == None && plane == Xy` |
| T09 | 正常系/build | `plane_ref: Some("Front")` を含む CreateSketch + Extrude を `build_bodies_from_features(..., &default_ref_planes(), ...)` で組み立て、xy 平面の押出ソリッドになる | F=6 の cuboid 状ソリッド |
| T10 | 正常系/build | `plane_ref: Some("Top")` → xz 平面 (base_plane normal = +Y, profile を Y+ 方向に押出)、`plane_ref: Some("Right")` → yz 平面 (base_plane normal = +X, profile を X+ 方向に押出) で押出成功 | 各押出ソリッドの全面法線が ±{X,Y,Z} 軸方向に揃い、Front 経由 (base_plane normal = +Z, Z+ 方向押出) の押出結果と V/E/F 数が完全一致 |
| T11 | 正常系/build | `plane_ref: Some("Front")` + `plane: Yz` (両方ある) → plane_ref 優先で xy 平面に解決 | Xy 平面の押出 |
| T12 | 正常系/build | `plane_ref: None` + `plane: Xy` → 従来通り xy 平面 (既存 plane-only 経路) | 既存 extrude と同等 |
| T13_boundary | 境界/build | `plane_ref: Some("UnknownId")` → `KernelError::RefPlaneNotFound { id: "UnknownId" }` | typed error |
| T14_degen | 退化/build | `ref_planes: &[]` (空) + `plane_ref: Some("Front")` → `RefPlaneNotFound` (補填が無い環境での挙動を明示) | typed error |
| T15_boundary | 境界/build+kernel | `examples/extruded_rect_refplane.engawa` (Front 経由) を parse → `build_bodies_from_features(&features, &root.ref_planes, ...)` → 既存 `extruded_rect.engawa` と V/E/F 数が完全一致 | 形状等価 |
| T16 | smoke | `examples_smoke.rs` に `extruded_rect_refplane()` を追加し、CI で build エラーなく通る | smoke OK |
| T17 | 決定性/build | T15 と同じ build を 100 回繰り返し、生成 Solid の頂点座標・edge・face 列が完全一致 (IdGenerator(0) 固定) | 100 回 byte-identical |

退化/境界 ID: `T13_boundary`, `T14_degen`, `T15_boundary` (STEP 5.5 で `_degen_/_boundary_` grep 対象)

## 幾何的不変条件チェックリスト

本 Issue は format/build 層に閉じ、kernel の Boolean/Partition/Assemble は触らない。すべて **N/A**。

- [N/A] partition 出力の polygon 頂点順と assemble の normal 処理 — kernel 未変更
- [N/A] 各プリミティブの face ごとの outer_loop 2D 向き — 未変更
- [N/A] flip_normals / same_sense の意味論 — 未変更
- [N/A] pslg_subdivide の出力向き — 未変更
