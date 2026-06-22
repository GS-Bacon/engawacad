# ADR-017 抜粋 — #273 (Circle / Arc) に関連する Decision

> 元 ADR: `docs/decisions/017-phase10-sketch-curves-and-edits.md` (Status: Proposed)

## §1 採用: SketchElement enum 1 本 + tag = "kind" (Option A)

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SketchElement {
    Line { id: String, from: [f64; 2], to: [f64; 2] },
    Circle { id: String, center: [f64; 2], radius: f64 },
    Arc { id: String, center: [f64; 2], radius: f64, start_angle: f64, end_angle: f64 },
    // 後続 Issue で Ellipse / Conic / Rectangle / Polygon / Slot を追加
}
```

理由: 1 enum でまとめると全曲線を統一 `match` で処理できる。`Feature::CreateSketch.profile` の型を `Vec<SketchSegment>` → `Vec<SketchElement>` に変えるだけで既存履歴が拡張曲線対応になる。trait object は dyn 越境で Serialize/Deserialize と相性が悪い。

採用前提崩壊 trigger: variant 数が 30 超 (Phase 12 Refactor Pass 1 で再評価)。

## §2 退化判定 ε 値域 (Phase 10 関連)

| ε 名 | 値 | 用途 |
|------|----|----|
| `EPS_LENGTH` (= `ε_radius`) | `1e-9` | ADR-004 既定。半径・距離・最小辺長の退化判定 |
| `EPS_ANGLE` (= `ε_angle`) | `1e-9` | ADR-004 既定。角度差の退化判定 |

**#273 では `LENGTH_TOLERANCE` / `ANGLE_TOLERANCE` (既存 `geometry::math`) をそのまま流用** (新規定数なし)。
Phase 10 新規 ε (`ε_axis_ratio`, `ε_discriminant`) は #274 (Ellipse/Conic) で導入する。

退化検出時の挙動:
- `engawa-format` deserialize: **退化値はそのまま受理** (YAML は purely structural)
- `engawa-build` dispatch: **`BuildError::DegenerateSketchElement { element_id, reason }` で fail-fast**

格納先: `crates/engawa-kernel/src/error.rs` の `KernelError` enum に新 variant を追加 (`BuildError` という命名は ADR-017 本文の言い回しで、実装上は `KernelError`)。

## §4 既存 SketchSegment との互換性

採用: **`SketchElement::Line` に enum 内包 + custom Deserialize で legacy YAML 読み込み** (breaking なし)。

具体策:
- 既存 example YAML (`example/*.engawa`) の `profile:` 配列はそのまま動く (= `kind` 欠落時 Line にマップ)
- `from`/`to` のフィールド名はそのまま keep。golden YAML の表現を 1 字も変えない
- migration hook (ADR-015 §3) は不要。`schema_version` は v1 のまま据え置き

reject 戦略:
- YAML 内に `kind:` フィールドがあれば優先 (新形式)
- 無ければ legacy として Line にマップ
- 矛盾 (`kind: circle` だが `from`/`to` がある) は serde の Untagged 走査で wrap error として浮かぶ

## Migration Plan §1 (#273 の責務)

> #273 (Circle / Arc) — 最小の曲線 2 種を SketchElement enum に追加。SketchSegment → SketchElement::Line リネーム + serde default の実装も本 Issue で完了させる (= 後続 Issue は enum に variant を追加するだけ)

したがって #273 のスコープは:
1. `SketchElement` enum の骨格を立てる (Line / Circle / Arc)
2. legacy YAML 互換の Deserialize を実装する
3. `SketchSegment` を完全に廃止し `SketchElement::Line` に内包する
4. `Feature::CreateSketch.profile` の型を変える
5. tessellate API + 退化判定 + dispatcher 改修
6. Circle/Arc の単体テスト + golden YAML + smoke
