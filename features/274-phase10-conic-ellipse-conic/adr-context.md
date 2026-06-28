
各 variant の必須パラメータ:

| 曲線 | 必須パラメータ |
|------|--------------|
| Line     | `id: String`, `from: [f64; 2]`, `to: [f64; 2]` |
| Circle   | `id`, `center: [f64; 2]`, `radius: f64` |
| Arc      | `id`, `center`, `radius`, `start_angle: f64`, `end_angle: f64` |
| Ellipse  | `id`, `center`, `major: f64`, `minor: f64`, `rotation: f64` |
| Conic    | `id`, `coeffs: [f64; 5]` (Ax² + Bxy + Cy² + Dx + Ey + F = 0、F = -1 で正規化) |
| Rectangle | `id`, `corner_min: [f64; 2]`, `corner_max: [f64; 2]` |
| Polygon  | `id`, `center`, `vertex_count: u32`, `circumradius: f64`, `rotation: f64` |
| Slot     | `id`, `center_a: [f64; 2]`, `center_b: [f64; 2]`, `radius: f64` |

理由:
- 1 enum でまとめると **パターンマッチで全曲線を一括処理** できる (`SketchElement::tessellate(&self) -> Vec<Point2>` のような統一 API)
- `Feature::CreateSketch.profile` の型を `Vec<SketchSegment>` → `Vec<SketchElement>` に変えるだけで既存履歴が拡張曲線対応になる
- trait object (`Box<dyn SketchElementTrait>`) は dyn 越境で型情報が失われ、Serialize/Deserialize と相性が悪い
- 別 type 並列 (`SketchSegment` + 新規 `SketchCurve`) は profile 内の順序保証が難しい (= 線と円が混在する scratch 順を表現するのに 2 つの Vec を ID で同期する必要)

### 2. 退化判定基準 (ε 値域)

採用: **ADR-004 既存値を流用し、Phase 10 で新規 ε を 2 つ追加する**。

| ε 名前 | 値 | 用途 |
|--------|----|----|
| `EPS_LENGTH` (= `ε_radius`) | `1e-9` | ADR-004 既定。半径・距離・最小辺長の退化判定 |
| `EPS_ANGLE` (= `ε_angle`) | `1e-9` | ADR-004 既定。角度差の退化判定 |
| `ε_axis_ratio` | `1e-6` | **Phase 10 新規**。Ellipse の `minor / major < ε_axis_ratio` で near-line 退化 |
| `ε_discriminant` | `1e-9` | **Phase 10 新規**。Conic の `\|B² - 4AC\| < ε_discriminant` で退化 (退化 conic = 直線対 / 1 点) |
| `ε_polygon_min_edge` | `1e-9` (= `EPS_LENGTH`) | Polygon の隣接頂点間距離下限 (可読性のため別名で再 export) |

退化検出時の挙動:
- `engawa-format` の deserialize: 退化値は **そのまま受理** (`from_yaml` は purely structural)
- `engawa-build` の dispatch: **`BuildError::DegenerateSketchElement { element_id, reason }` で fail-fast**
- これにより YAML 自体は人間が書ける (= 退化形を一時的に保持できる) が、build には乗らない。ADR-001 「Feature history = source of truth」を保ちつつ、build 出力の妥当性は保証する

格納先: `crates/engawa-kernel/src/geometry/tolerances.rs` に追加 (ADR-004 既定 ε と同 module)。

### 3. スケッチ編集 7 種の API 抽象

採用: **各編集オペレーションを独立した `Feature` enum variant として履歴に残す (純関数モデル)**。

| 編集 op | feature variant (`Feature::*`) | 主要パラメータ |
|--------|-----------------------------|--------------|
| Trim   | `SketchTrim` | `sketch_ref: EntityRef`, `element_id`, `trim_point: [f64; 2]` |
| Extend | `SketchExtend` | `sketch_ref`, `element_id`, `extend_to: ExtendTarget` (`EntityRef` または `Point`) |
| Offset | `SketchOffset` | `sketch_ref`, `selection: Vec<element_id>`, `distance: f64` |
| Sketch Fillet | `SketchFillet` | `sketch_ref`, `vertex_ref: (e1_id, e2_id)`, `radius: f64` |
| Sketch Chamfer | `SketchChamfer` | `sketch_ref`, `vertex_ref`, `distance_a: f64`, `distance_b: f64` |
| Mirror | `SketchMirror` | `sketch_ref`, `mirror_line: MirrorLine` (`EntityRef` または `LineEq { p1, p2 }`), `selection: Vec<element_id>` |
| Pattern | `SketchPattern` | `sketch_ref`, `kind: PatternKind` (`Rect` / `Polar`), `count_u`, `count_v`, `spacing` |

ID 安定性:
- 編集後も既存 element ID は **保持** する
- 分割される場合 (例: Trim で element a が 2 つに分かれる) は `{a}_split_{n}` 派生 ID を決定的に割り当てる
- これは ADR-005 Topological Naming の「編集後も意味的に同一であれば同 ID を保つ」原則と整合

参照解決失敗時のエラー:
- `BuildError::SketchRefNotFound { sketch_id, element_id }` を導入
- `engawa-build` の `Feature::apply()` 内で **fail-fast**
- 履歴ロールバック (engawa-build の rollback API) で削除済み element を参照する古い編集 op を消化する場合のみエラーが発生する想定

dispatch 順序:
- 編集 op は `engawa-build` で先頭から線形に適用する
- in-place mutation ではなく純関数 (= 各 op が新しい `Vec<SketchElement>` を返す)
- 中間状態は `engawa-kernel` の `SketchModel` に保持 (Phase 11 拘束ソルバが状態を読みやすいよう)

### 4. 既存 SketchSegment との互換性 (schema_version v1 → v2 バンプ + migration hook)

採用: **`schema_version` を v1 → v2 にバンプし、Document loader に v1 → v2 migration hook を追加する。`SketchElement::Line` に enum 内包 + serde default によって legacy v1 YAML はそのまま v2 にマップされる (reader 互換維持)**。

設計判断の経緯: 当初 draft では「v1 据え置き + breaking なし」としていたが、auto-accept 3 ペルソナが「`SketchSegment` → `SketchElement` への型差し替えは `Feature::CreateSketch.profile` の wire format を変えるため定義上 breaking であり、v1 のまま据え置くと `engawa-format` の `CURRENT_SCHEMA_VERSION = 1` ガードが意味を失う」と指摘 (architect/contrarian/migration 全員)。指摘は正当で、v2 バンプ + migration hook を採用する。

serde 表現:

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SketchElement {
    Line { id: String, from: [f64; 2], to: [f64; 2] },

--- ADR-017 §2 (退化判定 ε) ---
| Mirror | `SketchMirror` | `sketch_ref`, `mirror_line: MirrorLine` (`EntityRef` または `LineEq { p1, p2 }`), `selection: Vec<element_id>` |
| Pattern | `SketchPattern` | `sketch_ref`, `kind: PatternKind` (`Rect` / `Polar`), `count_u`, `count_v`, `spacing` |

ID 安定性:
- 編集後も既存 element ID は **保持** する
- 分割される場合 (例: Trim で element a が 2 つに分かれる) は `{a}_split_{n}` 派生 ID を決定的に割り当てる
- これは ADR-005 Topological Naming の「編集後も意味的に同一であれば同 ID を保つ」原則と整合

参照解決失敗時のエラー:
- `BuildError::SketchRefNotFound { sketch_id, element_id }` を導入
- `engawa-build` の `Feature::apply()` 内で **fail-fast**
- 履歴ロールバック (engawa-build の rollback API) で削除済み element を参照する古い編集 op を消化する場合のみエラーが発生する想定

dispatch 順序:
- 編集 op は `engawa-build` で先頭から線形に適用する
- in-place mutation ではなく純関数 (= 各 op が新しい `Vec<SketchElement>` を返す)
- 中間状態は `engawa-kernel` の `SketchModel` に保持 (Phase 11 拘束ソルバが状態を読みやすいよう)

### 4. 既存 SketchSegment との互換性 (schema_version v1 → v2 バンプ + migration hook)

採用: **`schema_version` を v1 → v2 にバンプし、Document loader に v1 → v2 migration hook を追加する。`SketchElement::Line` に enum 内包 + serde default によって legacy v1 YAML はそのまま v2 にマップされる (reader 互換維持)**。

設計判断の経緯: 当初 draft では「v1 据え置き + breaking なし」としていたが、auto-accept 3 ペルソナが「`SketchSegment` → `SketchElement` への型差し替えは `Feature::CreateSketch.profile` の wire format を変えるため定義上 breaking であり、v1 のまま据え置くと `engawa-format` の `CURRENT_SCHEMA_VERSION = 1` ガードが意味を失う」と指摘 (architect/contrarian/migration 全員)。指摘は正当で、v2 バンプ + migration hook を採用する。

serde 表現:

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SketchElement {
    Line { id: String, from: [f64; 2], to: [f64; 2] },
    Circle { id: String, center: [f64; 2], radius: f64 },
    // ...
}

// v1 YAML 互換: kind 欠如時に Line にマップ (内部実装は untagged fallback)
```

具体策:

- `engawa-format` の `CURRENT_SCHEMA_VERSION` を `1` → `2` に更新
- Document loader (`Document::from_yaml`) で `schema_version` を読み、`1` の場合のみ **migration hook (`migrate_v1_to_v2`)** を起動
- migration hook は `profile: Vec<{id, from, to}>` を `profile: Vec<SketchElement::Line {id, from, to}>` に inline 変換する pure function
- 既存 example YAML (`example/*.engawa`) は **書き換えなしで読める** (= loader 経由で透過的に v2 表現に持ち上がる)。書き戻し時は v2 形式 (`kind: line` 明示) で保存される
- `schema_version: 2` を持つ YAML はそのまま v2 として読まれる (= migration skip)
- writer は常に v2 で出力 (= v1 への down-grade はしない)

reject 戦略:

- 既存 YAML 内に `kind:` フィールドがあれば優先 (= v2 として扱う、`schema_version` 不問)
- `kind:` 無し + `schema_version: 1` なら migration hook で Line にマップ
