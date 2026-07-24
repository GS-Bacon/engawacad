# ADR context for #295

## ADR-017 §3 スケッチ編集 API 抽象 (excerpt)
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

## ADR-017 Migration Plan (excerpt, #275-#278 順序)

子 Issue 実装順:

1. **#273 (Circle / Arc)** — closed (cycle 47 で実装完了)。`SketchSegment` → `SketchElement::Line` リネーム + serde default は本 Issue で完了済み
2. **#274 (Ellipse / Conic)** — 解析幾何系。`ε_axis_ratio` / `ε_discriminant` を `tolerances` に追加。**併せて schema_version v1 → v2 バンプ + Document loader の `migrate_v1_to_v2` hook も本 Issue で実装** (= 既存 v1 example YAML が透過的に v2 表現に持ち上がることを担保する。後続 #275-#278 は v2 前提で書ける)
3. **#275 (Rectangle / Polygon / Slot)** — 合成曲線系。内部的には Line/Arc の組み合わせで描画するが、parameter は enum variant として保持
4. **#276 (Trim / Extend)** — 編集の入口。`SketchTrim` / `SketchExtend` を `Feature` enum に追加 + `BuildError::SketchRefNotFound`
5. **#277 (Offset / Sketch Fillet / Sketch Chamfer)** — Offset/Fillet/Chamfer。`Vec<element_id>` selection の表現を確立
6. **#278 (Mirror / Pattern)** — 複製系。`MirrorLine` / `PatternKind` enum を確立

各 Issue は本 ADR を参照しつつ、Decision Matrix の採用済み Option を前提として plan.md を書く。

---

## Notes

- Issue #272 の本文では "ADR-016" と記載されているが、既存 `docs/decisions/016-engawa-cli-naming.md` と番号衝突するため、本 ADR は **ADR-017** として起票する
- 本 ADR は `Status: Proposed` で起票し、ADR-013 auto-accept フローの Decision Matrix lint + Codex 3 ペルソナレビューで approved になれば `Status: Accepted` に昇格する設計だった (`/3ailoop` の L-1.5 / L-5.6 自動チェーンで処理)
- 2026-06-27: auto-accept 3 ペルソナ全 refute (regen_count=1, token=278k) で滞留したため、人間判断で §4 互換性を「v1 据え置き + breaking なし」→「schema_version v2 バンプ + migration hook」に書き直し、`Status: Accepted` に昇格。refute の指摘は §4 だけで他 §1-3 は妥当判定だったため、その他構造は draft 維持


## ADR-018 LENGTH_TOLERANCE 比較規約 (excerpt)
# ADR-018: LENGTH_TOLERANCE 比較規約 — `<=` で退化判定 standard

## Status

Proposed

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


## ADR-005 EntityRef 対象 (excerpt)
# ADR-005: トポロジカル・ネーミング方式の決定

## Status

Accepted

## Context

### 問題: 生 index 参照の崩れシナリオ

現状、B-rep トポロジーエンティティ間の参照は `Solid` のフラット配列への生 `usize` インデックスで行われている
(`crates/mycad-kernel/src/brep/topology.rs`)。`EntityId`(u64)は各エンティティに付くが参照キーとしては
未使用であり、`IdGenerator` は 0 始まりの単調カウンタ(生成順=インデックスと同じ脆さ)になっている。

Feature 間でエンティティを生 index / `EntityId` で参照した場合、以下のシナリオで崩壊する:

```
F1: CreateBox        → faces[0..5] (例: 上面 = faces[5])
F2: <下流 Feature>   → "faces[5]" を参照
─ F1 の上流に F0 を挿入、または F1 を CreateCylinder に変更 ─
→ faces 配列の中身・長さが変わり "faces[5]" は別物か範囲外
```

これは FreeCAD が長年抱えた「トポロジカル・ネーミング問題」と同根である。

### 現状の資産

- `EntityRef { feature_id, role }` が `crates/mycad-format/src/feature.rs:6-12` に定義済みだが未使用。
- Feature 間は既に文字列 `id` で参照している(`Extrude { sketch: String }` 等)。
- ADR-001/004 の原則: **Feature history が真実の源で、B-rep は Feature から再生成される**。
  `.mycad` は Feature 列のみ永続化 → ID スキーム変更にファイル移行不要。

### 堅牢性の対象範囲（スコープ明示）

本 ADR が守る不変性は「Feature **再生成・再番号付け**に対する参照の安定性」である。
ユーザによる明示的な `feature_id` の rename は「文書全体の協調的リファクタ」(全参照を同時に書き換える
明示編集)であり、本 ADR のスコープ外とする。

また、参照スコープは**現在の文書の単一 Component 内**に限定する。Component 階層越し・同一部品の
複数 occurrence を跨ぐ参照は **Phase 5(アセンブリ/部品参照)** で `(component/occurrence path,
feature_id, kind, role)` へ前方拡張する(既存の基底名は書き換えない、path を prefix として加算)。

### Extrude (#22) との関係

Phase 3 の `Extrude { sketch, depth }` 最小実装は既存ソリッドの面を参照しない(スケッチ + 深さの
スカラーのみ)。面選択の実需は Phase 4(Boolean / フィレット / 面上スケッチ)で初めて発生する。
ただし本 ADR を #22 の前提として先に確定し、命名規約を Extrude の実装に反映させる。

## Decision


## ADR-004 freeform / tolerance 方針 (excerpt)
# ADR-004: 自由曲面・自由曲線を確定要件として扱う

## Status

Accepted

## Context

現在の幾何カーネルは解析曲面・解析曲線のみを持つ:

- `Surface` (`crates/mycad-kernel/src/geometry/surface.rs`): `Plane` / `Cylinder` / `Sphere` / `Cone`
- `Curve` (`crates/mycad-kernel/src/geometry/curve.rs`): `Line` / `Circle`

「解析曲面だけで進められないか」を検討したが、ロードマップ上の操作を踏むと自由曲面・自由曲線は構造的に避けられないと結論した:

- **Boolean 演算 (Phase 4)**: 曲面同士の交線は一般に円・直線にならない。例として円筒 ∩ 球の交わりは 4 次の空間曲線であり、`Curve::Line` / `Curve::Circle` では表せない。→ 自由**曲線** (交線・スプライン) が必須。
- **フィレット・面取り (ADR-001 が動機として挙げた操作)**: 一般のフィレット面や掃引面は解析式に乗らない。→ 自由**曲面** (NURBS 等) が必須。

一方、次の理由から「今すぐ実装する必要はない」:

- `Surface` / `Curve` は Rust の enum であり、variant 追加は加算的。`evaluate` / `normal_at` / tessellation などの `match` はコンパイラが網羅性を強制するため、追加漏れは検出される。
- ADR-001 のとおり **Feature history が真実の源で、B-rep は Feature から再生成される**。`.mycad` に永続化されるのは Feature 列であり B-rep ではないため、`Surface` / `Curve` / `Edge` の内部表現は後で改修してもファイル移行が不要。後付けコストが構造的に低い。

## Decision

自由曲面・自由曲線を「いつか検討する」ものではなく **確定要件** として扱う。ただし NURBS 等の実装は今は行わず (現在 Phase 1)、Boolean が要求する範囲から漸進的に導入する。これに伴い、以降の設計で次を守る:

1. **`Surface` / `Curve` enum を唯一の幾何拡張点**とする。`Nurbs` / スプライン variant の追加が加算的であり続けるよう、アルゴリズムは variant 集合を仮定しない。
2. **アルゴリズムは曲面・曲線の型に非依存**であること。平面・直線前提をアルゴリズムへ埋め込まない (例: 面法線は面ごとに 1 回でなく、点ごとに `Surface::normal_at_point` で評価する)。
3. **数値モデル: トレラント方式採用 (Decision 3 — Phase 4 (#31) で確定)**。詳細は [下の節](#数値モデル-トレラント方式採用-decision-3--phase-4-31-で確定) を参照。
