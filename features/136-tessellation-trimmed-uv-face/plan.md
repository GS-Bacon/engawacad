## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `tessellate_trimmed_uv_face` 内側ループ u シフトを `round(Δ / 2π) * 2π` の周期保存シフトに変更 (`tessellation/mod.rs:474-489`) | 外周ループの unwrap 修正 (`mod.rs:443-450` は別経路で既に正しく動作) |
| シフト基準を「外周 u スパンの中央値 (= (u_min + u_max) / 2)」へ変更 (任意の `outer_uv.first()` 依存を排除) | earcut アルゴリズム自体の変更・差し替え |
| 穴位置を周方向に掃引するパラメタライズドテスト追加 (u ≈ 0, π/2, π, -π/2, -π を全網羅) | sphere の trimmed UV face 修正 (#137 別 Issue, sphere は φ-θ 二重周期で別アプローチ) |
| naked_edge = 0 / 三角形数の検証アサート | cone の trimmed UV (現状未実装) |
| 既存テスト群への回帰チェック (`#130` t01-t04 を含む) | `unwrap_periodic_uv` 関数本体の修正 |

## Non-Goals

- **#137 (trimmed sphere tessellation)**: 球は φ-θ 二重周期で、本 Issue の単一 u シフト修正では対応不可。別 Issue。
- **earcut アルゴリズム自体の見直し**: earcut への入力 (UV 座標) を正しく整えるのが本 Issue。earcut 自体の不具合 (穴と外周の交差等) は対象外。
- **外周ループ自体の unwrap 再設計**: L443-450 の外周 unwrap は既に動作しており、本 Issue 範囲外。
- **`tessellate_face_uv_grid` の path 分岐見直し**: L551-553 の早期 delegate は変更しない。

## 実装対象

<!-- Issue: #136 -->

**影響クレート/ファイル**:
- `crates/mycad-kernel/src/tessellation/mod.rs`: `tessellate_trimmed_uv_face` 内 L474-489 を修正
- `crates/mycad-kernel/tests/trim_surface_tessellation_acceptance.rs` (既存): 既存テスト維持、回帰チェック対象
- `crates/mycad-kernel/tests/trim_surface_uv_shift_acceptance.rs` (新規): 穴位置掃引テスト

### 変更箇所 1: シフト計算の周期保存化 + 基準を u スパン中央値へ

**before** (`crates/mycad-kernel/src/tessellation/mod.rs:474-489`):
```rust
{
    let mut u_list: Vec<f64> = il_uv.iter().map(|(u, _)| *u).collect();
    unwrap_periodic_uv(&mut u_list);
    if let Some(&(outer_u, _)) = outer_uv.first() {
        let avg_inner_u: f64 = u_list.iter().sum::<f64>() / u_list.len() as f64;
        let shift = outer_u - avg_inner_u;
        if shift.abs() > PI {
            for u in &mut u_list {
                *u += shift;
            }
        }
    }
    for (i, u) in u_list.into_iter().enumerate() {
        il_uv[i].0 = u;
    }
}
```

**after**:
```rust
{
    let mut u_list: Vec<f64> = il_uv.iter().map(|(u, _)| *u).collect();
    unwrap_periodic_uv(&mut u_list);

    // Reference: midpoint of outer loop's u-span (not an arbitrary "first" point).
    // Holes near the seam (u ≈ ±π) are correctly placed inside the outer u-range when
    // we snap by the *nearest 2π multiple*, preserving periodicity exactly.
    if !outer_uv.is_empty() {
        let (u_min, u_max) = outer_uv.iter().map(|(u, _)| *u).fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(lo, hi), u| (lo.min(u), hi.max(u)),
        );
        let outer_center = 0.5 * (u_min + u_max);
        let avg_inner_u: f64 = u_list.iter().sum::<f64>() / u_list.len() as f64;
        let raw_shift = outer_center - avg_inner_u;
        // Snap to nearest integer multiple of 2π (period-preserving).
        // For |raw_shift| < π this gives 0 (no shift). For |raw_shift| ≥ π this rounds
        // to ±2π, ±4π, etc., guaranteeing the hole lands within the outer u-span when
        // the hole is the same period winding as the outer loop.
        let two_pi = 2.0 * PI;
        let k = (raw_shift / two_pi).round();
        let shift = k * two_pi;
        if shift != 0.0 {
            for u in &mut u_list {
                *u += shift;
            }
        }
    }
    for (i, u) in u_list.into_iter().enumerate() {
        il_uv[i].0 = u;
    }
}
```

**変更点要約**:
1. **基準**: `outer_uv.first()` (任意の点) → `(u_min + u_max) / 2` (外周 u スパン中央値)
2. **シフト量**: `if shift.abs() > PI { shift }` (中途半端な閾値判定) → `round(raw_shift / 2π) * 2π` (周期保存)
3. **退化ガード**: `outer_uv.is_empty()` を保険として保持 (外周が 0 点なら呼出し元で早期 return 済み)

### 変更箇所 2: shift ロジックを抽出して単体テスト化

**重要な調査結果**: 現状の base カーネル (#130/#131 後の状態) では「cylinder の side face で inner_loop が出る」シナリオを Boolean Cut だけで作れない (`cylinder × cylinder Cut` / `cylinder × box 貫通 Cut` がいずれも `manifold validation failed: edge must have exactly 2 half-edges` でエラー)。Issue #134 は監査による「将来到達するリスク」起票であり、現行機能では integration 経路でバグを直接 trigger できない。

**対応**: shift ロジックを `pub(crate)` 関数として抽出し、その単体性質を inline test で直接検証する。

抽出する関数 (`crates/mycad-kernel/src/tessellation/mod.rs` 内 helper として追加):
```rust
/// 内側ループ u 列を、外側ループ u スパンの中央値に「2π の整数倍」だけシフトする。
///
/// 周期保存: `shift = round((outer_center - avg_inner) / 2π) * 2π`。
/// outer 配列が空のときは入力をそのまま返す (no-op、保険ガード)。
pub(crate) fn periodic_u_shift(outer_us: &[f64], inner_us: &mut Vec<f64>) {
    if outer_us.is_empty() || inner_us.is_empty() {
        return;
    }
    let (u_min, u_max) = outer_us.iter().fold(
        (f64::INFINITY, f64::NEG_INFINITY),
        |(lo, hi), &u| (lo.min(u), hi.max(u)),
    );
    let outer_center = 0.5 * (u_min + u_max);
    let avg_inner: f64 = inner_us.iter().sum::<f64>() / inner_us.len() as f64;
    let raw_shift = outer_center - avg_inner;
    let two_pi = 2.0 * PI;
    let k = (raw_shift / two_pi).round();
    let shift = k * two_pi;
    if shift != 0.0 {
        for u in inner_us.iter_mut() {
            *u += shift;
        }
    }
}
```

呼び出し側 (L474-489) を以下に差し替え:
```rust
{
    let mut u_list: Vec<f64> = il_uv.iter().map(|(u, _)| *u).collect();
    unwrap_periodic_uv(&mut u_list);
    let outer_us: Vec<f64> = outer_uv.iter().map(|(u, _)| *u).collect();
    periodic_u_shift(&outer_us, &mut u_list);
    for (i, u) in u_list.into_iter().enumerate() {
        il_uv[i].0 = u;
    }
}
```

### 変更箇所 3: 新規テストファイル (shift ロジック単体性質 + メッシュ健全性回帰)

`crates/mycad-kernel/tests/trim_surface_uv_shift_acceptance.rs` を新規作成。テスト構成:
- shift ロジック単体テスト (T01-T08): `periodic_u_shift` を直接呼び、入力/出力の対応を検証
- 既存 boolean Cut 経路の回帰 (T09): `box - cylinder Cut` (#130 t02 と同じ) が依然 tessellate 成功 + naked_edge=0

**単体テストの U 位置 sweep**:
- T02: outer u スパン [-0.1, 0.1] (中央 0), inner avg 0 → shift = 0
- T03: outer u スパン [π/2 - 0.1, π/2 + 0.1], inner avg = π/2 → shift = 0 (差なし)
- T04: outer u スパン [-π + 0.1, π - 0.1] (cylinder full minus seam epsilon), inner avg = 0.5 → shift = 0 (中央が 0)
- T05_boundary_seam_pos: outer 中央 = π/2, inner avg = -3π/2 (= +π/2 - 2π 周期表現) → shift = +2π → 一致
- T06_boundary_seam_neg: outer 中央 = -π/2, inner avg = +3π/2 → shift = -2π
- T07_degen_empty_outer: outer 空 → no-op
- T08_boundary_inner_centered: outer 中央 = inner avg → shift = 0 (exact 0.0)

## 設計方針

- **決定性**: shift 計算は浮動小数演算順序が固定 (sum → divide → divide → round → multiply)。同一入力で同一出力。
- **B-rep トポロジー妥当性**: 本 Issue は tessellation 段階の修正で B-rep 自体は変更しない。Euler-Poincaré は不変。
- **退化幾何**: 外周ループ 3 点未満は L437-439 で既に早期 return。内側ループも L468-470 で早期 continue 済み。
- **derive 規約**: 変更なし。
- **エラーハンドリング**: 既存 `TessellationError::NonManifoldLoop` を流用。新規 variant 追加なし。
- **workspace.dependencies**: 新規追加なし。

### 数値モデル

- **周期 (period)**: `2π = 6.283185307179586` rad。f64 で `2.0 * std::f64::consts::PI` を使用 (mod.rs:12 で `use std::f64::consts::PI` 済)。
- **`round` の挙動**: f64 `round()` は half-away-from-zero (Rust 標準)。`raw_shift / 2π == 0.5` のとき `round() == 1.0`、`== -0.5` のとき `-1.0`。境界 `|raw_shift| == π` (= ちょうど ±0.5 サイクル) のときは ±1 シフト。これは正しい (シーム境界を超えて回るほうが trimmed メッシュとして自然)。
- **精度**: 内側ループ unwrap 後の u は `unwrap_periodic_uv` で周期境界が解消済み。シフトは整数倍 2π なので加算による浮動小数ノイズは最小 (= 浮動小数加算の最後位の rounding error のみ)。naked_edge 判定の `LENGTH_TOLERANCE = 1e-9` で十分許容。
- **退化境界**: `outer_uv.is_empty()` ガードを保持 (外周 0 点は呼出し元で防いでいるが保険)。

## テスト計画 (ID 付き)

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | `periodic_u_shift` を同一入力で 2 回呼び、結果が完全一致 | assert_eq! exact |
| T02 | 正常系 | outer 中央 0, inner avg 0 → shift = 0 (no shift) | assert |
| T03 | 正常系 | outer 中央 π/2, inner avg π/2 → shift = 0 | assert |
| T04 | 正常系 | outer 中央 0, inner avg 0.5 → shift = 0 (`|raw| < π`) | assert |
| T05_boundary_seam_pos | シーム | outer 中央 π/2, inner avg = π/2 - 2π → shift = +2π → inner avg シフト後 = π/2 | assert_eq! |
| T06_boundary_seam_neg | シーム | outer 中央 -π/2, inner avg = -π/2 + 2π → shift = -2π | assert_eq! |
| T07_degen_empty_outer | 退化/ガード | outer 空 → no-op (inner unchanged) | assert_eq! |
| T08_boundary_inner_centered | ガード | outer 中央 = inner avg → shift exact 0.0 (浮動小数ノイズなし) | assert_eq! |
| T09 | 回帰 | #130 既存 boolean Cut 経路 (`box - cylinder Cut` の整数倍/シーム外位置) が依然 pass | 既存テスト全 green |

## 幾何的不変条件チェックリスト

- N/A: 本 Issue は tessellation のみで B-rep 構造 (partition / pslg_subdivide / 法線処理) を直接触らない。
- **watertight 性**: T02-T06 で `naked_edge_count == 0` を assert することで「穴が earcut で消されて塞がったメッシュ」のバグを検出。
- **自己隣接周期面 (cylinder seam)**: cylinder の outer loop は seam edge を含むが、本修正は inner loop の u シフトのみ。outer loop の取扱には影響しない。
- **face ごとの outer_loop 2D 向き (CW/CCW)**: 変更なし。本修正は座標値のシフトのみで向きを変えない。
