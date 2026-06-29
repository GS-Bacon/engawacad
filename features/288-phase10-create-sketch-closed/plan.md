## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `Feature::Extrude` / `Feature::ExtrudeCut` 経路で、profile が「closed primitive を含む 2 要素以上」の場合に `KernelError::InvalidParameter { kind: "profile" }` を返す defensive guard を追加 | 複数 closed contour profile を実際にサポートする実装 (Phase 11+ で multi-contour `make_extrusion` 拡張時に対応) |
| `crates/engawa-build/src/lib.rs` の 2 箇所 (Extrude L262-274 / ExtrudeCut L361-374) に共通の `validate_sketch_profile_contours` ヘルパで guard 挿入 | `tessellate_sketch_element` の API 変更 / `make_extrusion` の signature 変更 |
| 退化テスト 1 件 (T03_degen) + 正常系 2 件 (T01 単一 Circle / T02 Line+Arc) + reject ガード 1 件 (T04_boundary_reject) | Conic の closed/open 判定の精密化 (現状は Conic も closed 扱いで保守的に reject) |

## Non-Goals

- 複数 closed primitive を含む profile の正常実装 (Phase 11+ multi-contour 拡張)
- `tessellate_sketch_element` 出力構造の変更 (現状の `Vec<[f64;2]>` 維持)
- `make_extrusion` の multi-contour 対応 (Phase 11+)
- Conic 係数を解析して closed/open を厳密判定するロジック (今回は保守的に Conic を closed 扱いで reject)

## 実装対象

- Issue: #288
- 影響クレート: `engawa-build`
- 影響ファイル:
  - `crates/engawa-build/src/lib.rs` (修正): Extrude / ExtrudeCut の 2 箇所で guard 呼び出し
  - `crates/engawa-build/tests/create_sketch_closed_acceptance.rs` (新規): T01-T04 の acceptance test

### 変更する型・関数

- 新規 free fn: `fn validate_sketch_profile_contours(profile: &[SketchElement]) -> Result<(), KernelError>`
  - profile が 2 要素以上 かつ いずれかが closed primitive (Circle / Ellipse / Conic) の場合に `Err(KernelError::InvalidParameter { kind: "profile" })` を返す
  - それ以外は `Ok(())`
- 新規 free fn: `fn is_closed_primitive(elem: &SketchElement) -> bool`
  - `Circle`, `Ellipse`, `Conic` → true、`Line`, `Arc` → false

### 既存関数 before / after

**before** (`crates/engawa-build/src/lib.rs` L262 付近、Extrude):

```rust
const BASE_SEGMENTS: usize = 32;
let mut profile_uv: Vec<(f64, f64)> = entry
    .profile
    .iter()
    .map(|elem| {
        engawa_kernel::tessellation::sketch::tessellate_sketch_element(
            elem,
            BASE_SEGMENTS,
        )
    })
    .collect::<Result<Vec<_>, _>>()?
    .into_iter()
    .flat_map(|poly| poly.into_iter().map(|p| (p[0], p[1])))
    .collect();
```

**after** (Extrude / ExtrudeCut 共通):

```rust
// Reject multi-element profiles that include a closed primitive (Phase 10 scope).
// Multi-contour profiles are deferred to Phase 11+ (#288).
validate_sketch_profile_contours(&entry.profile)?;

const BASE_SEGMENTS: usize = 32;
let mut profile_uv: Vec<(f64, f64)> = entry
    .profile
    .iter()
    .map(|elem| {
        engawa_kernel::tessellation::sketch::tessellate_sketch_element(
            elem,
            BASE_SEGMENTS,
        )
    })
    .collect::<Result<Vec<_>, _>>()?
    .into_iter()
    .flat_map(|poly| poly.into_iter().map(|p| (p[0], p[1])))
    .collect();
```

新規ヘルパ (file scope / private fn):

```rust
fn is_closed_primitive(elem: &engawa_format::SketchElement) -> bool {
    use engawa_format::SketchElement;
    matches!(
        elem,
        SketchElement::Circle { .. } | SketchElement::Ellipse { .. } | SketchElement::Conic { .. }
    )
}

fn validate_sketch_profile_contours(
    profile: &[engawa_format::SketchElement],
) -> Result<(), KernelError> {
    if profile.len() > 1 && profile.iter().any(is_closed_primitive) {
        return Err(KernelError::InvalidParameter { kind: "profile" });
    }
    Ok(())
}
```

## 設計方針

- 決定性要件: validate は純粋関数で副作用なし。同一入力→同一エラー / 同一通過
- B-rep トポロジー妥当性: N/A (guard レベルの defensive check、kernel まで到達しない)
- 退化幾何の扱い: profile.len() == 0 はここでは扱わない (make_extrusion 側の `n < 3` チェックで補足される既存挙動を維持)
- derive 規約: 既存型のみ使用、新規型なし
- エラーハンドリング: 既存の `KernelError::InvalidParameter { kind: "profile" }` を再利用 (新規 variant を追加しない)
- workspace.dependencies: 変更なし

### 自律判断ログ

- Issue body は対処方針として「A: 複数 closed primitive を contour ごとに保持」と「B: InvalidParameter で reject」の 2 案を提示。本 plan は **Option B** を採用。
- 理由: (1) Phase 10 は SketchElement 1 要素 / create_sketch を前提 (Issue body 明記)、(2) multi-contour 正規実装は `make_extrusion` signature 変更が必要で Phase 11+ scope、(3) defensive reject は将来の正規実装で削除/緩和可能。
- Conic を closed 扱いに含めた理由: Conic は理論上 ellipse / parabola / hyperbola を表現し、bounded か unbounded か判定するには係数解析が必要。保守的に「closed primitive 扱いで reject」とし、Phase 11+ で contour 化と同時に精密判定を導入する。

## テスト計画（ID 付き）

配置: `crates/engawa-build/tests/create_sketch_closed_acceptance.rs`

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 正常系 | Circle 1 要素のみの profile → Extrude OK | `build()` が `Ok` で 1 Solid 生成 |
| T02 | 正常系 | Line 4 本で Rectangle 形状の profile → Extrude OK (Phase 10 通常用法) | `build()` が `Ok` で 1 Solid 生成 |
| T03_degen | 退化系 | `[Circle, Circle]` (2 closed primitives) → reject | `Err(KernelError::InvalidParameter { kind: "profile" })` |
| T04_boundary_reject | 境界系 | `[Circle, Line]` (closed + open mix) → reject | `Err(KernelError::InvalidParameter { kind: "profile" })` |
| T05 | 正常系 | ExtrudeCut 経路でも同じ guard が効くこと (Circle + Line で reject) | `Err(KernelError::InvalidParameter { kind: "profile" })` |

## 幾何的不変条件チェックリスト

- [x] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか — **N/A** (guard レベル、kernel 不到達)
- [x] 各プリミティブの face ごとの outer_loop 2D 向き — **N/A**
- [x] flip_normals / same_sense の意味論 — **N/A**
- [x] pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか — **N/A**
