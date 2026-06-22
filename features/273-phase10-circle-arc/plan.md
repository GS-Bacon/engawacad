## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `SketchElement` enum を **`crates/engawa-format/src/feature.rs` に**新設 (variants: `Line`, `Circle`, `Arc`)。後続 Issue が variant 追加だけで済む骨格 | Ellipse / Conic / Rectangle / Polygon / Slot (= #274, #275) |
| `Feature::CreateSketch.profile` の型を `Vec<SketchSegment>` → `Vec<SketchElement>` に置換 | スケッチ編集 (Trim / Extend / Offset / Fillet / Mirror / Pattern = #276, #277, #278) |
| `SketchSegment` → `SketchElement::Line { id, from, to }` への内包移行 (型名は廃止、構造同型) | 制約ソルバ (Phase 11) |
| 既存 legacy YAML (`profile: [- id, from, to]`) のシリアライズ後方互換 (custom `Deserialize` で `kind` 欠落→Line) | engawa-viewer / web frontend 側の Circle/Arc 表示 (Phase 7+ で別 Issue) |
| `SketchElement::tessellate(&self, base_segments: usize) -> Result<Vec<[f64;2]>, KernelError>` 統一 API | Sketch 3D-position 計算 (= profile_uv → plane 上の 3D 座標化) — 既存 `make_extrusion` の責務 |
| Circle/Arc の polyline 化 (既存 `arc_segment_count` 再利用) | NURBS / 一般 parametric curve 表現 (Phase 15+) |
| 退化判定 (`InvalidRadius` / `InvalidArcRange`) + `KernelError::DegenerateSketchElement` 新設 | 多 Sketch 間の決定性 (= sketch id 単位の局所決定性のみ) |
| `engawa-build` dispatcher の SketchElement 対応 (Circle/Arc 含む profile_uv 抽出) | ExtrudeCut の SketchElement 対応 (= 同 Feature::Extrude 側のみ; ExtrudeCut は line-only sketch 前提で温存し別 Issue で揃える) |
| `examples/circle_arc.engawa` 追加 + `examples_smoke.rs` 登録 | 既存 example YAML の Circle/Arc 化 (= 後方互換ガードで `profile: [- id, from, to]` のままも動かす) |
| Unit/integration tests (T01〜T_BOUNDARY_full_circle、6 件) | property-based テスト (= 後続 Issue で導入) |

## Non-Goals

- 残 5 曲線 (Ellipse/Conic/Rectangle/Polygon/Slot) — #274, #275
- スケッチ編集 (Trim/Extend/Offset/Fillet/Chamfer/Mirror/Pattern) — #276, #277, #278
- ADR-017 を formally `Status: Accepted` に昇格させる作業 — auto-accept フロー (#251 / #281 経路)
- ExtrudeCut の Circle/Arc 対応 — Phase 8 完了条件を満たす line-only sketch 前提を維持。Circle/Arc 入りの ExtrudeCut は別 Issue 起票
- engawa-viewer (Three.js 側) の Circle/Arc rendering — Phase 7+ サブ Issue
- 既存 example の Circle/Arc 化 (`sketch_extrude_pillar.engawa` 等は line のままで動き続けることが goal)

## 実装対象

<!-- Issue: #273 -->

### 影響クレートとファイル

| クレート | ファイル | 変更種別 |
|---------|---------|---------|
| `engawa-format` | `src/feature.rs` | `SketchElement` enum 新設 / `SketchSegment` 廃止 (Line に置換) / `CreateSketch.profile` 型変更 / custom `Deserialize` (legacy compat) |
| `engawa-format` | `src/lib.rs` | `pub use` 更新 (`SketchSegment` → `SketchElement`) |
| `engawa-format` | (テスト) `src/feature.rs` 内 `#[cfg(test)] mod tests` | golden YAML test 追加 (Line tagged / Line legacy / Circle / Arc) |
| `engawa-kernel` | `src/error.rs` | `KernelError::DegenerateSketchElement { element_id, reason }` 新設 |
| `engawa-kernel` | `src/tessellation/mod.rs` (もしくは新 `src/tessellation/sketch.rs`) | `pub fn tessellate_sketch_element(elem: &SketchElement, base_segments: usize) -> Result<Vec<[f64;2]>, KernelError>` 新設 / `arc_segment_count` を `pub(crate)` から再 export または引数経由で渡す |
| `engawa-build` | `src/lib.rs` | `Feature::CreateSketch` arm を `SketchElement` 受領に変更 / `validate_sketch_segment_ids` を `validate_sketch_element_ids` に generalize / profile_uv 抽出経路で Circle/Arc を polyline 化 |
| `examples/` | `circle_arc.engawa` (新規) | cuboid + 上面 sketch に Line 1 + Circle 1 + Arc 1 を含む golden YAML |
| `engawa-build` | `tests/examples_smoke.rs` | `circle_arc` smoke test 追加 |
| `engawa-kernel` | `tests/sketch_element_acceptance.rs` (新規) | T01-T_BOUNDARY_full_circle (6 件) の integration test |

### `SketchElement` enum シグネチャ

```rust
// crates/engawa-format/src/feature.rs

#[derive(Debug, Clone, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SketchElement {
    Line {
        id: String,
        from: [f64; 2],
        to: [f64; 2],
    },
    Circle {
        id: String,
        center: [f64; 2],
        radius: f64,
    },
    Arc {
        id: String,
        center: [f64; 2],
        radius: f64,
        /// radian; CCW from +X axis
        start_angle: f64,
        /// radian; CCW from +X axis. end_angle - start_angle が掃引角
        end_angle: f64,
    },
}

// custom Deserialize: `kind` フィールド欠落時は Line にフォールバック (legacy YAML 互換)
impl<'de> Deserialize<'de> for SketchElement { /* tagged 試行 → 失敗時に Line 試行 */ }
```

### `Feature::CreateSketch` (before/after)

```rust
// before (crates/engawa-format/src/feature.rs:312-325)
#[serde(rename = "create_sketch")]
CreateSketch {
    id: String,
    plane: SketchPlane,
    #[serde(default, skip_serializing_if = "is_zero")]
    offset: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    variables: Vec<Variable>,
    profile: Vec<SketchSegment>,  // ← 型変更
    #[serde(default, skip_serializing_if = "Option::is_none")]
    plane_ref: Option<PlaneRef>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    suppressed: bool,
},

// after
profile: Vec<SketchElement>,  // SketchElement::Line で legacy YAML を吸収
```

### `engawa-build` dispatcher (before/after)

```rust
// before (crates/engawa-build/src/lib.rs:259-263)
let profile_uv: Vec<(f64, f64)> = entry
    .profile
    .iter()
    .map(|s| (s.from[0], s.from[1]))  // SketchSegment.from のみ参照
    .collect();

// after — Circle/Arc は polyline 化、Line はそのまま
let profile_uv: Vec<(f64, f64)> = entry
    .profile
    .iter()
    .map(|elem| tessellate_sketch_element(elem, BASE_SEGMENTS))
    .collect::<Result<Vec<_>, _>>()?
    .into_iter()
    .flat_map(|poly| poly.into_iter().map(|p| (p[0], p[1])))
    .collect();
```

`BASE_SEGMENTS = 32` を `engawa-build` 側で固定 (Phase 10 では runtime 設定なし、ADR-017 §6 「runtime config 化は Phase 11」と整合)。

### `tessellate_sketch_element`

```rust
// crates/engawa-kernel/src/tessellation/sketch.rs (新規) or mod.rs 内

pub fn tessellate_sketch_element(
    elem: &SketchElement,
    base_segments: usize,
) -> Result<Vec<[f64; 2]>, KernelError> {
    use crate::geometry::math::{ANGLE_TOLERANCE, LENGTH_TOLERANCE};
    match elem {
        SketchElement::Line { from, to, .. } => Ok(vec![*from]),  // 既存挙動と同型: from のみ列挙
        SketchElement::Circle { id, center, radius } => {
            if *radius < LENGTH_TOLERANCE {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: id.clone(),
                    reason: "radius < ε_radius",
                });
            }
            let n = arc_segment_count(0.0, std::f64::consts::TAU, base_segments);
            Ok((0..n).map(|i| {
                let t = (i as f64) * std::f64::consts::TAU / (n as f64);
                [center[0] + radius * t.cos(), center[1] + radius * t.sin()]
            }).collect())
        }
        SketchElement::Arc { id, center, radius, start_angle, end_angle } => {
            if *radius < LENGTH_TOLERANCE {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: id.clone(),
                    reason: "radius < ε_radius",
                });
            }
            let sweep = end_angle - start_angle;
            if sweep.abs() < ANGLE_TOLERANCE {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: id.clone(),
                    reason: "|end_angle - start_angle| < ε_angle",
                });
            }
            let n = arc_segment_count(*start_angle, *end_angle, base_segments);
            // start_angle..=end_angle を等間隔分割し最後の点を除外 (polyline の連結のため)
            Ok((0..n).map(|i| {
                let t = start_angle + (i as f64) * sweep / (n as f64);
                [center[0] + radius * t.cos(), center[1] + radius * t.sin()]
            }).collect())
        }
    }
}
```

注: `Line` の戻り値は **`from` 1 点のみ** (既存 dispatcher が Vec<SketchSegment> から `from[0], from[1]` だけ取って polyline 化していた挙動と同じ振る舞いに保つ)。Polyline の閉路化は呼び出し側 (`validate_profile_closed` + `make_extrusion`) の責務。

## 設計方針

### 決定性要件

- `SketchElement::tessellate` は副作用なしの純関数。同じ enum 入力に対し常に同じ `Vec<[f64;2]>` を返す
- `arc_segment_count(t_start, t_end, base)` は既に決定的 (`ceil(base * |t_end - t_start| / (2π))`)
- IdGenerator 使用箇所なし (Sketch element の id は YAML から渡された文字列をそのまま保持)

### B-rep トポロジー妥当性

- 本 Issue はスケッチ平面 (2D) の話で、3D トポロジーは触らない。Euler-Poincaré V-E+F=2 は `make_extrusion` 側 (既存) で保証され続ける
- profile が **閉路を成すこと** は `validate_profile_closed` の責務。Circle 単独 (= 1 個の SketchElement) で閉路を成すケースは polyline 化後の最終点と最初の点が ε 以内になるよう Circle の n+1 点目を return せず n 点だけ返す (= 既存 line-only profile と同じ「最後の点は次の SketchElement の最初の点と一致」モデル)
- 同様に Arc も `(0..n).map(\|i\| start + i*sweep/n)` で **n 点** を返し、`end_angle` の点 (= sweep 終端) は含めない (= 後段の SketchElement の `from` 点と連結する想定)。これにより Arc(0,0,r=1.0,start=0,end=2π) と Circle(0,0,r=1.0) の polyline が完全一致 (T_BOUNDARY_full_circle が期待する境界条件)

### 退化幾何の扱い

- Circle: `radius < LENGTH_TOLERANCE` (= 1e-9 mm) で `DegenerateSketchElement`
- Arc: `radius < LENGTH_TOLERANCE` または `|end_angle - start_angle| < ANGLE_TOLERANCE` (= 1e-9 rad) で `DegenerateSketchElement`
- `engawa-format` の deserialize は退化値も受理 (ADR-017 §決定 2 「YAML は purely structural」)
- 退化エラーは `engawa-build` dispatcher で fail-fast (`Result<Solid, KernelError>` で伝播)

### derive 規約

- `SketchElement` enum: `Debug, Clone, Serialize, Deserialize, JsonSchema, TS` (= 既存 SketchSegment と同型)
- ただし `Deserialize` のみ custom impl (legacy `kind` 欠落時 → Line フォールバック)

### エラーハンドリング

- `thiserror::Error` で `KernelError::DegenerateSketchElement { element_id: String, reason: &'static str }` を追加
- `reason` は `'static str` で確定理由を列挙: `"radius < ε_radius"` / `"|end_angle - start_angle| < ε_angle"`
- 既存 `KernelError::InvalidParameter { kind: &'static str }` は触らない (= ADR-017 でも separate error)

### workspace.dependencies

- 新規 dependency なし。既存の `serde`, `schemars`, `ts-rs`, `thiserror` を再利用

### 数値モデル

- tolerance: `ε_radius = LENGTH_TOLERANCE = 1e-9` — Circle/Arc の最小半径 (流用、新規定数なし)
- tolerance: `ε_angle = ANGLE_TOLERANCE = 1e-9` — Arc の最小掃引角 (流用、新規定数なし)
- ADR-004 準拠方針: tolerant (= absolute tolerance with `≤` 比較)
- 数値範囲: `radius >= 0`, `start_angle`/`end_angle` は radian 任意 (= ±∞ を `NaN` 除き許容; Arc の周回数は `(end - start) / 2π` で自然に決まる)
- 退化 cap: `radius < 1e-9` または `|sweep| < 1e-9` で退化判定。これ以上の幾何意味づけは別 ADR で

## テスト計画（ID 付き）

| ID | 種別 | 対象 | 内容 | 期待結果 |
|----|------|------|------|---------|
| T01 | 決定性 | `tessellate_sketch_element` | 同一 Circle(0,0,r=1.0) と Arc(0,0,r=1.0,start=0,end=π/2) をそれぞれ 2 回 tessellate (= 連続呼び出し) し、両回の戻り `Vec<[f64;2]>` を比較 | 1 回目と 2 回目の polyline が `assert_eq!` で完全一致 (= `f64::PartialEq` で bit-equal、浮動小数の中間計算順序を変えない) |
| T02_circle | 正常系 | `tessellate_sketch_element` | Circle(0,0,r=1.0) を tessellate して 32 点取得 | 各点が `\|p - center\| ≈ 1.0` (< ε_len)、x 範囲 ≈ [-1,1], y 範囲 ≈ [-1,1] |
| T02_arc | 正常系 | `tessellate_sketch_element` | Arc(0,0,r=1.0,start=0,end=π/2) を tessellate | 開始点 ≈ (1,0)、終了点 ≈ (0,1) (両端 < ε_len)、点数 = ceil(32·(π/2)/(2π)) = 8 |
| T_DEG_zero_radius | 退化 | `tessellate_sketch_element` | Circle(id="c", center=[0,0], radius=0.0) を tessellate | `Err(KernelError::DegenerateSketchElement { element_id: "c", reason: "radius < ε_radius" })` |
| T_DEG_zero_angle | 退化 | `tessellate_sketch_element` | Arc(id="a", center=[0,0], radius=1.0, start_angle=0.5, end_angle=0.5) を tessellate | `Err(KernelError::DegenerateSketchElement { element_id: "a", reason: "\|end_angle - start_angle\| < ε_angle" })` |
| T_BOUNDARY_full_circle | 境界 | `tessellate_sketch_element` | Arc(0,0,r=1.0,start=0,end=2π) と Circle(0,0,r=1.0) を tessellate (両方とも n 点・最後の周回点は除外する実装) | 両者の点数一致 (= `arc_pts.len() == circle_pts.len()`) + 全点 i で `\|arc_pts[i] - circle_pts[i]\| < ε_len`。実装は Arc の polyline を `(0..n).map(\|i\| start + i*sweep/n)` で生成し最後の "閉路点" を含めない (= 周回 Arc と Circle の頂点列が完全一致) |
| T_GOLDEN_legacy_compat | golden YAML | `engawa-format::feature` | legacy YAML (`{id, from, to}`、kind 無し) を deserialize | `SketchElement::Line { id, from, to }` として読める |
| T_GOLDEN_circle_yaml | golden YAML | `engawa-format::feature` | tagged YAML (`{kind: circle, id, center, radius}`) を round-trip | serialize → deserialize で完全一致 |
| T_GOLDEN_arc_yaml | golden YAML | `engawa-format::feature` | tagged YAML (`{kind: arc, id, center, radius, start_angle, end_angle}`) を round-trip | serialize → deserialize で完全一致 |
| T_SMOKE_circle_arc_engawa | smoke (E2E) | `engawa-build` | `examples/circle_arc.engawa` を build → Solid 生成 | エラーなく Solid が返り、tessellate して TriangleMesh も得られる |

### 類似ケース (確認 — `bug` ラベルなしのため新規追加 ID なし)

該当 bug 修正 Issue ではないため、既存類似コードパス調査は不要。

### examples/ smoke 登録

- `examples/circle_arc.engawa` 新規追加
- `crates/engawa-build/tests/examples_smoke.rs` に `fn circle_arc()` を追加 (パターンは既存の `fn sketch_extrude_pillar()` と同型)

## 幾何的不変条件チェックリスト

- N/A — 本 Issue は **スケッチ平面 (2D) の話**で、3D B-rep の partition/assemble は触らない
- partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか → N/A
- 各プリミティブの face ごとの outer_loop 2D 向き（CW/CCW）が文書化されているか → N/A (= Circle 単独 sketch の場合は CCW を ADR-017 で規定済み: t=0→2π = CCW from +X)
- flip_normals / same_sense の意味論が明確か → N/A
- pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか → N/A

## 関連

- Issue: #273
- ADR: ADR-017 (`docs/decisions/017-phase10-sketch-curves-and-edits.md`) — Decision §1 (SketchElement enum 採用) / §2 (ε 値域) / §4 (legacy 互換)
- 親 Issue: #195 (Phase 10 起点、blocked-by-split)
- 兄弟 Issue: #274 (Ellipse/Conic), #275 (Rectangle/Polygon/Slot), #276 (Trim/Extend), #277 (Offset/Fillet/Chamfer), #278 (Mirror/Pattern)
- 検証パス: #281 (dispatch-codex-3persona GLM fallback) — STEP 7.5 で Codex usage limit が再発したら fallback ルートに乗ることを確認
