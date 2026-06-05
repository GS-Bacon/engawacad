# Issue #48: CreateCylinder origin + CreateSphere center を YAML から指定可能にする

## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| `Feature::CreateCylinder` に optional `origin: [f64;3]` 追加 | `axis` パラメータ (Cylinder 軸の非 +Z 化) — 別 Issue |
| `Feature::CreateSphere` に optional `center: [f64;3]` 追加 | `Feature::Translate { target, dx, dy, dz }` 新設 — 別 Issue |
| serde `#[serde(default, skip_serializing_if = "is_origin")]` で後方互換 | `Component.transform` の build 適用 — Phase 5 |
| `make_cylinder` に `origin: Point` 引数追加 + 全呼び出し更新 | 回転 (rotation) パラメータ — 別 Issue |
| `make_sphere` に `center: Point` 引数追加 + 全呼び出し更新 | ADR-005 §7 追記 — #47 で実施済み (closed) |
| `mycad-build` ディスパッチで origin/center を kernel に伝搬 | |
| NaN/Inf 値検証 → `FormatError` (format 層 `Document::validate`) | |
| 新 example `cylinder_offset.mycad` / `sphere_offset.mycad` | |
| golden (TS / sphere YAML / examples roundtrip) の同期更新 | |

## Non-Goals
- axis パラメータ (Cylinder 軸変更): 別 Issue
- Translate feature 新設: 別 Issue
- Component.transform の build 適用: Phase 5、別 Issue
- 回転 (rotation): 別 Issue
- ADR-005 §7 への追記: #47 で実施済み (closed)、本 Issue では行わない

## 実装対象
<!-- Issue: #48 -->
影響クレート/ファイル:
- `crates/mycad-format/src/feature.rs` — enum 2 バリアント + ヘルパ
- `crates/mycad-format/src/error.rs` — FormatError 新バリアント
- `crates/mycad-format/src/document.rs` — validate に NaN/Inf チェック
- `crates/mycad-kernel/src/primitives/cylinder.rs` — make_cylinder シグネチャ + 本体
- `crates/mycad-kernel/src/primitives/sphere.rs` — make_sphere シグネチャ + 本体
- `crates/mycad-build/src/lib.rs` — ディスパッチ 2 箇所
- 全 `make_cylinder(`/`make_sphere(` テスト呼び出し箇所 (下記)
- `examples/cylinder_offset.mycad`, `examples/sphere_offset.mycad` (新規)
- `crates/xtask/src/main.rs` — TS FEATURE_GOLDEN
- `web/src/generated/` — TS 再生成 (drift チェック)

### 1. feature.rs — enum バリアント (before/after)
before (276-284):
```rust
CreateCylinder {
    id: String,
    radius: f64,
    height: f64,
},
// ...
CreateSphere { id: String, radius: f64 },
```
after:
```rust
CreateCylinder {
    id: String,
    radius: f64,
    height: f64,
    #[serde(default, skip_serializing_if = "is_origin")]
    origin: [f64; 3],
},
// ...
CreateSphere {
    id: String,
    radius: f64,
    #[serde(default, skip_serializing_if = "is_origin")]
    center: [f64; 3],
},
```
新規ヘルパ (feature.rs 末尾、`is_default_transform` (component.rs:141) を先行例とする):
```rust
fn is_origin(p: &[f64; 3]) -> bool {
    *p == [0.0, 0.0, 0.0]
}
```
注: `Feature::id()` の match arm は `{ id, .. }` で `..` を使うため変更不要 (feature.rs:331-333 確認済み)。

### 2. error.rs — FormatError 新バリアント
`InvalidReference`/`InvalidName` の `{ value, reason }` 構造に倣う:
```rust
#[error("non-finite position in feature {id:?}: {reason}")]
InvalidPosition { id: String, reason: &'static str },
```

### 3. document.rs — validate_component の feature ループに検証追加
`validate_identifier(id, ...)` 後に position の有限性を検証:
```rust
match feature {
    Feature::CreateCylinder { origin, .. } => check_finite_position(id, origin)?,
    Feature::CreateSphere { center, .. } => check_finite_position(id, center)?,
    _ => {}
}
```
ヘルパ (document.rs 内、private):
```rust
fn check_finite_position(id: &str, p: &[f64; 3]) -> Result<(), FormatError> {
    if p.iter().any(|v| !v.is_finite()) {
        return Err(FormatError::InvalidPosition {
            id: id.to_string(),
            reason: "position components must be finite (no NaN/Inf)",
        });
    }
    Ok(())
}
```

### 4. cylinder.rs — make_cylinder (before/after)
before (12-16):
```rust
pub fn make_cylinder(
    radius: f64,
    height: f64,
    id_gen: &mut IdGenerator,
) -> Result<Solid, KernelError> {
```
after:
```rust
pub fn make_cylinder(
    radius: f64,
    height: f64,
    origin: Point,
    id_gen: &mut IdGenerator,
) -> Result<Solid, KernelError> {
```
本体: 現状ハードコードの原点起点を `origin` だけ平行移動する。`origin.x/y/z` の有限性は `radius`/`height` と同様 (cylinder.rs:17-22) に冒頭で `is_finite` チェックして `KernelError` (防御的; format 層が一次検証)。
- 底 seam 頂点: `Point::new(0.0, radius, 0.0)` → `origin + Vec3::new(0.0, radius, 0.0)`
- 頂 seam 頂点: `Point::new(0.0, radius, height)` → `origin + Vec3::new(0.0, radius, height)`
- 底円 `Curve::Circle { center: Point::origin(), .. }` → `center: origin`
- 頂円 `center: Point::new(0.0,0.0,height)` → `center: origin + Vec3::new(0.0,0.0,height)`
- seam 線 `Curve::Line { origin: Point::new(0.0,radius,0.0), direction: Vec3::new(0,0,height) }` → origin を `origin + Vec3::new(0,radius,0)` に (direction は不変)
- 底cap `Surface::Plane { origin: Point::origin(), normal:-Z }` → `origin: origin`
- 頂cap `Surface::Plane { origin: Point::new(0,0,height), .. }` → `origin: origin + Vec3::new(0,0,height)`
- 側面 `Surface::Cylinder { origin: Point::origin(), axis:+Z, radius }` → `origin: origin`
- 軸 (axis=+Z)・法線・loop 巻き方向・seam 原点方位は不変 → role 名不変 (ADR-005 §7)

### 5. sphere.rs — make_sphere (before/after)
before (13):
```rust
pub fn make_sphere(radius: f64, id_gen: &mut IdGenerator) -> Result<Solid, KernelError> {
```
after:
```rust
pub fn make_sphere(radius: f64, center: Point, id_gen: &mut IdGenerator) -> Result<Solid, KernelError> {
```
本体: 21 行目 `let origin = Point::origin();` → `let origin = center;` を基本とし、極座標を center 起点に平行移動:
- 南極 `Point::new(0.0,0.0,-radius)` → `center + Vec3::new(0.0,0.0,-radius)`
- 北極 `Point::new(0.0,0.0,radius)` → `center + Vec3::new(0.0,0.0,radius)`
- seam `Curve::Circle { center: origin, normal:-Y, radius }` は既に `origin` 変数を使うため自動追従
- `Surface::Sphere { center: origin, radius }` も `origin` 経由で自動追従
- 冒頭で center の有限性 `is_finite` チェック → `KernelError`
- seam 原点方位 (+X 子午線)・loop 巻き方向・極 role は不変 → role 名不変

### 6. build/src/lib.rs — ディスパッチ (before/after)
import に Point 追加:
```rust
use mycad_kernel::geometry::Point;
```
CreateCylinder (before 128-135):
```rust
Feature::CreateCylinder { id:_, radius, height } => {
    let solid = make_cylinder(*radius, *height, gen)?;
    built.register(id.to_string(), solid);
}
```
after:
```rust
Feature::CreateCylinder { id:_, radius, height, origin } => {
    let solid = make_cylinder(*radius, *height, Point::new(origin[0], origin[1], origin[2]), gen)?;
    built.register(id.to_string(), solid);
}
```
CreateSphere (before 136-139):
```rust
Feature::CreateSphere { id:_, radius } => {
    let solid = make_sphere(*radius, gen)?;
    built.register(id.to_string(), solid);
}
```
after:
```rust
Feature::CreateSphere { id:_, radius, center } => {
    let solid = make_sphere(*radius, Point::new(center[0], center[1], center[2]), gen)?;
    built.register(id.to_string(), solid);
}
```

### 7. 全テスト呼び出し箇所の更新 (機械的に `Point::origin()` を挿入)
`make_cylinder(...)` 呼び出し: cylinder.rs (L152,153,179,198,218,270,312-377), tessellation/mod.rs (L927,948,949,993,1047,1137), booleans/mod.rs (L280), booleans/partition.rs (L2056,2103)
`make_sphere(...)` 呼び出し: sphere.rs (L78,79,158,... 多数), surface_boolean_a2_acceptance.rs (L54,252,284,313,338), booleans/mod.rs (L402,411), tessellation/mod.rs (多数), booleans/partition.rs (L2017,2034,2059)
→ いずれも id_gen 引数の直前に `Point::origin()` を追加するだけ。

### 8. 新 example
`examples/cylinder_offset.mycad`:
```yaml
schema_version: 1
version: "0.1.0"
root_component:
  name: "Offset Cylinder"
  features:
    - type: create_cylinder
      id: cyl_1
      radius: 5.0
      height: 20.0
      origin: [0.0, 0.0, -10.0]
```
`examples/sphere_offset.mycad`:
```yaml
schema_version: 1
version: "0.1.0"
root_component:
  name: "Offset Sphere"
  features:
    - type: create_sphere
      id: sphere_1
      radius: 5.0
      center: [2.0, 0.0, 0.0]
```

### 9. golden 同期
- `xtask/src/main.rs` FEATURE_GOLDEN (L318): ts-rs が `#[serde(default)]` を optional field として出力 → 生成結果に合わせて golden 文字列更新。GLM は `cargo xtask` で TS 再生成し `web/src/generated/` drift を解消。
- feature.rs `test_create_sphere_yaml_golden` (L365-381): struct literal に `center: [0.0,0.0,0.0]` を追加。skip_serializing_if=is_origin により期待 YAML 文字列は不変。
- document.rs examples 一括 roundtrip (L274-300): 新 example は read_dir で自動対象。byte-identical roundtrip を満たすこと。
- JsonSchema golden (feature.rs:653, `schema_for!(EntityRef)`): EntityRef 不変につき影響なし。

## 設計方針
- **決定性**: 全 EntityID は `IdGenerator` で生成。生成順は origin/center に依存しない (座標値のみ平行移動)。`IdGenerator::new(0)` 2 回 → 完全一致。
- **B-rep トポロジー妥当性**: 平行移動はトポロジー (V/E/F 数・接続) を変えない。Euler-Poincaré は既存値を維持 (cylinder: 自己整合な既存値、sphere: periodic face)。
- **退化幾何**: 位置オフセットは退化を生まない (radius/height が正なら不変)。退化判定は該当なし。
- **derive 規約**: `Feature` の derive (Debug/Clone/Serialize/Deserialize/JsonSchema/TS) は enum 全体に付与済み、フィールド追加で維持される。`[f64;3]` は Default 実装あり (serde default が `[0.0,0.0,0.0]` を供給)。
- **エラーハンドリング**: format 層は `thiserror` の `FormatError::InvalidPosition` を新設。kernel 層は既存 `KernelError` の is_finite パターンを踏襲。
- **workspace.dependencies**: 新規依存なし。

### 数値モデル
- `origin: [f64;3]` / `center: [f64;3]` の値検証: NaN/Inf を `Document::validate` で検出 → `FormatError::InvalidPosition`。
- ADR-004 準拠: tolerant 方式。`assert_solids_equal` の座標比較許容は既存どおり `< 1e-12` (feature_dispatcher.rs:14-20)。`LENGTH_TOLERANCE = 1e-9` は本 Issue では新規 tolerance を導入しない (位置は exact に座標へ反映)。
- 退化判定基準: 該当なし (位置オフセットそのものは退化を生まない)。

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | `cylinder_offset.mycad` を `IdGenerator::new(0)` で 2 回 build | `assert_solids_equal_with_names` 一致 |
| T02 | 正常系(origin) | `origin:[0,0,-10]` で build した cylinder | `Surface::Cylinder.origin == Point::new(0,0,-10)`、底面 Vertex z == -10 |
| T03 | 後方互換 | `origin` 省略 cylinder と既存 `cylinder.mycad` | byte-identical Solid (`assert_solids_equal`)、YAML roundtrip 不変 |
| T04 | 正常系(center) | `center:[2,0,0]` で build した sphere | `Surface::Sphere.center == Point::new(2,0,0)`、極/seam 座標が center 周り |
| T05 | 派生名不変 | 原点版 vs offset 版で全 face/edge/vertex name 比較 | role 名 (lateral/cap_top/seam/surface 等) が完全一致 |
| T06 | YAML roundtrip | `cylinder_offset.mycad`/`sphere_offset.mycad` | byte-identical roundtrip (既存 examples ループでパス) |
| T07 | 検証 | `origin:[NaN,0,0]` の YAML を from_yaml | `FormatError::InvalidPosition` を返す |

配置: integration test は `crates/mycad-build/tests/position_params_acceptance.rs` (T01-T06)。検証 T07 は mycad-format inline/test。

## 幾何的不変条件チェックリスト
- N/A (Boolean/Partition/Assemble 系ではない。位置パラメータ追加のみ)
- ただし派生名不変 (T05) と byte-identical 後方互換 (T03) を不変条件として検証する。
