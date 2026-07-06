# ADR-018: LENGTH_TOLERANCE 比較規約 — `<=` で退化判定 standard

## Status

Accepted

## Context

kernel 内に `LENGTH_TOLERANCE` を用いた退化判定の比較演算子に 2 種類の流派が共存していた:

- `<` strict (厳密): `value < LENGTH_TOLERANCE` で退化と判定
- `<=` inclusive (包含): `value <= LENGTH_TOLERANCE` で退化と判定

具体例:

| モジュール | 比較演算子 | テスト |
|-----------|-----------|------|
| `tessellation/sketch.rs` (Circle/Arc/Ellipse) | `<` | `T_EDGE_length_tolerance_boundary: radius = LENGTH_TOLERANCE passes` |
| `primitives/extrusion.rs` (depth/edge) | `<=` | — |
| `booleans/mod.rs` (analytic circle) | `<=` | TX6: `radius = LENGTH_TOLERANCE` rejected |

この不一致は #275 codex-7.5 で発覚した。

また、`geometry::math::length_near(a, b)` は `(a - b).abs() <= LENGTH_TOLERANCE` 規約を持つ。これは「TOL 以下の差は等しい」という kernel の根本判定関数であり、`value <= TOL` ↔ `value == 0` (退化) と意味的に等価。

## Options 比較

| Option | 概要 | 採用 | 主な trade-off |
|---|---|---|---|
| A: `<=` inclusive 統一 (採用) | `value <= LENGTH_TOLERANCE` を退化と判定 | ✅ | `length_near()` 規約と一致。extrusion / booleans の既存 `<=` を変更不要 (修正範囲: sketch のみ)。既存 `T_EDGE_length_tolerance_boundary` の assertion 反転が必要 (破壊的) |
| B: `<` strict 統一 (棄却) | `value < LENGTH_TOLERANCE` を退化と判定 (#275 codex-7.5 r5 M-F01 提案) | — | `length_near()` の `<=` 規約 (= 「TOL 以下の差は等しい」) と矛盾。extrusion / booleans / TX6 既存テスト + #275 で取り込み済の Rectangle/Polygon/Slot golden を全て変更する必要があり修正範囲が大きい |
| C: 現状維持 (棄却) | 2 流派共存のまま | — | #275 codex-7.5 で発覚したように boundary 動作が予測不能。新規 sketch curve 追加時 (Phase 10 後続) に同じ問題が繰り返す |

## Decision

Option A を採用する。全 `LENGTH_TOLERANCE` 退化判定を `<=` inclusive に統一する。

### 変更対象

- `tessellation/sketch.rs`: Circle/Arc/Ellipse の radius/major/minor 判定を `<` → `<=` に変更
- 既存テスト `T_EDGE_length_tolerance_boundary` の assertion を反転 (pass → reject)
- 新規境界テスト追加: `T_BOUNDARY_above_tolerance_*` (pass), `T_BOUNDARY_exact_tolerance_*` (reject)

### 変更不要

- `primitives/extrusion.rs`: 既に `<=` 使用
- `booleans/mod.rs`: 既に `<=` 使用、TX6 テストも整合

### Issue body M-F01 (Codex r5 migration `<` 統一提案) を逆方向で決着した理由

#275 codex-7.5 r5 M-F01 は kernel 全体で `<` strict 統一を推奨していた。本 ADR はこの提案を逆方向 (Option B 棄却 → Option A 採用) に決着している。理由は以下:

- M-F01 は `length_near()` の `<= TOL` 規約を見落としており、その規約と整合性を取ると `<=` inclusive が一意に決まる
- #275 で Rectangle/Polygon/Slot を既に `<=` で取り込んだ後の状態を起点とすると、`<` 統一は extrusion / booleans / sketch 多数を変更する必要があり修正範囲が大幅に拡大する
- 「`value <= TOL` ↔ `value == 0` (退化)」は意味的に自然な定義であり、tolerance の日常的解釈 (「許容差以下は同じ」) とも一致

## Rationale

`<=` inclusive 採用の根拠:

1. **`length_near()` 規約との整合**: `length_near(a, b) := (a - b).abs() <= LENGTH_TOLERANCE` は「TOL 以下の差は等しい」を意味し、`value <= TOL` ならば `value == 0` (退化) と等価
2. **多数派整合**: `extrusion` / `booleans` が既に `<=` 採用
3. **意味的自然**: 「TOL 以下は無視する」という inclusive 解釈は、tolerance (許容差) の日常的な意味論に即している

### Trade-off (破壊的変更)

既存 `T_EDGE_length_tolerance_boundary` テストの意味が反転する (pass → reject)。これは破壊的変更だが、以下の理由で許容:

- Rectangle/Polygon/Slot (#275) が既に `<=` で導入され golden に取り込まれている
- Circle/Arc/Ellipse を `<=` に揃える方が修正範囲が小さい
- 本変更は Phase 10 内で吸収する一過性の破壊的変更

## Related

- ADR-004 (tolerance 規約): トレラント方式採用、global LENGTH_TOLERANCE 規定
- ADR-017 (Phase 10 sketch): sketch 曲線の退化判定

## Migration

1. `tessellation/sketch.rs` の比較演算子を `<` → `<=` に変更
2. 既存テストの assertion 反転
3. 境界テスト追加
4. 本 ADR は ADR-013 auto-accept チェーンで承認待ち

### Wire-format / golden への影響

なし。本変更は kernel 内部の退化判定境界のみで、`.engawa` YAML schema (engawa-format)・既存 golden round-trip テスト・public API シグネチャを一切変更しない。境界 `radius == LENGTH_TOLERANCE` (= 1e-9 mm = 1 pm) を渡すユーザコードは現実的に存在しないため、外部影響は無視できる。

### Error 文言 (`KernelError::to_string()`) の互換性

本 ADR は `tessellation/sketch.rs` の `DegenerateSketchElement::reason` 文字列を `"radius < ε_radius"` → `"radius <= ε_radius"` 等に変更する (8 件)。これは `KernelError::to_string()` を経由した外部表示文言の変更である。

公式契約:

- error reason 文字列は **public API contract の対象外** とする。表示用文言として扱い、`KernelError` 列挙体の variant 名・field 構造のみが安定保証対象
- 外部システムが reason 文字列を parse する運用は想定外。pattern 判定は variant (`DegenerateSketchElement`) match で行うこと
- Phase 10 内で導入された Issue (#275 / #289) で reason 文字列の変更が複数回入るのは設計上織り込み済 (kernel 安定化前の調整期間)
