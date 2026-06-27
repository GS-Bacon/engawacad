# ADR-017: Phase 10 スケッチ基本曲線拡張 + スケッチ編集の方針

**Date**: 2026-06-22
**Status**: Accepted (2026-06-27: 人間判断で accept。auto-accept 3 ペルソナ全 refute (schema_version v1 据え置きが breaking なし主張と矛盾) は §4 を schema_version v2 バンプ + migration hook に書き直して解消。ADR-015/016 は Phase 9 完了で Withdrawn 化したため、本 ADR では参照を ADR-010 (Sketch input model 互換維持の先例) に差し替え)
**Related**: ADR-001 (B-rep), ADR-004 (tolerance 規約), ADR-005 (Topological Naming), ADR-006 (Issue 粒度), ADR-010 (Sketch input model — serde default + 互換維持の先例), ADR-013 (ADR 自動 accept フロー)
**Resolves**: Issue #272 (Issue 本文の "ADR-016" は番号衝突 — 次の空き番号 ADR-017 を採用)

---

## Context

Phase 10 (スケッチ基本曲線拡張 + スケッチ編集) の設計基盤を 1 つの ADR にまとめる。本 ADR の決定は親 #195 split-child である #273 / #274 / #275 / #276 / #277 / #278 の各実装 Issue の前提となる。実装に着手する前に方針を確定しておかないと、各 Issue が個別に判断を持ち寄って後段の Phase で矛盾する (ADR-006 §1 「ADR 決定と実装を混在させない」運用)。

具体的に固める 4 項目:

1. **基本曲線 7 種 (Circle / Arc / Ellipse / Conic / Rectangle / Polygon / Slot) のパラメータ表現**
2. **退化判定基準** (`ε_radius` / `ε_angle` / `ε_axis_ratio` / 多角形最小辺長 / conic discriminant 境界)
3. **スケッチ編集 7 種 (Trim / Extend / Offset / Sketch Fillet / Sketch Chamfer / Mirror / Pattern) の API 抽象**
4. **既存 Line ベース sketch (`SketchSegment`) との互換性** — `SketchElement` enum 拡張 vs 別 type の選択

現状 `engawa-format` の `Feature::CreateSketch` は `profile: Vec<SketchSegment>` で、`SketchSegment { id, from, to }` 直線のみを保持する。Phase 10 で曲線種別が一気に 7 種類増えるため、データモデルの拡張方針を最初に固める必要がある。

---

## Decision

本 ADR では Phase 10 設計基盤の 4 項目 (基本曲線 7 種のパラメータ表現 / 退化判定基準 / スケッチ編集 7 種の API 抽象 / 既存 Line ベース sketch との互換性) を決定する。代表項目 (基本曲線データモデル) の Options 要約をここで明示し、残り 3 項目は §1〜§4 のサブセクション + `## Decision Matrix` 表 (本 ADR では §1 で表形式を兼ねる) を参照のこと。

### Options 要約 (基本曲線データモデル / SketchSegment との互換性)

- A. 新規 `SketchElement` enum を導入し既存 `SketchSegment` を `SketchElement::Line` に内包 (採用) — Pros: パターンマッチで全曲線を一括処理 (`SketchElement::tessellate` 統一 API)、`Feature::CreateSketch.profile` の型を 1 行で差し替えるだけで既存履歴が拡張対応。Cons: `SketchSegment` を `Line` 配下に追い込むため deserialize 互換層が必要 (`#[serde(untagged)]` で吸収)
- B. trait object (`Box<dyn SketchElementTrait>`) — Pros: 第三者 crate からの曲線追加が容易。Cons: dyn 越境で型情報が欠落し Serialize/Deserialize と相性が悪い、決定性検証が dyn のため難しい
- C. 別 type 並列 (`SketchSegment` + 新規 `SketchCurve`) — Pros: 既存 `SketchSegment` を破壊しない。Cons: profile 内の順序保証 (= 線と円が混在する scratch 順を表現) が難しい、2 つの Vec を ID で同期する必要

**Trade-off**: Option A は deserialize 互換層のコストを払う代わりに、profile 順序の保証と Serialize 安定性で Option B/C より優位 (A 採用)。

### 1. 基本曲線 7 種のパラメータ表現

採用: **新規 `SketchElement` enum (`#[serde(tag = "kind", rename_all = "snake_case")]`) を導入し、既存 `SketchSegment` を `SketchElement::Line` に内包する**。

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
- `kind:` 無し + `schema_version: 2` (or 未指定) なら `LoadError::AmbiguousSchema` で fail-fast (= v2 宣言なのに legacy 表現)
- 矛盾 (`kind: circle` だが `from`/`to` がある) は serde が wrap error として浮かす

---

## Decision Matrix

| 項目 | Option A | Option B | Option C | 採用 | Trade-off | 採用前提崩壊 trigger |
|------|----------|----------|----------|------|-----------|--------------------|
| 曲線 enum 構造 | `SketchElement` enum (Line/Circle/Arc/...) 1 本で profile を保持 | 既存 `SketchSegment` を残し、新規 `SketchCurve` enum を別フィールドで持つ (2 並列) | trait object (`Box<dyn SketchElementTrait>`) | **Option A (enum 1 本)** | enum は variant 追加が breaking、trait object は dyn 越境で型情報失われる。Option A は ADR-015 §1 `FeatureOp` と同 pattern で一貫 | Phase 12 (Refactor Pass 1) で variant 数が 30 を超えた場合、または trait object のみで表現可能な拘束ソルバ統合が必要になった場合 |
| Conic 表現 | 一般二次形式 5 自由度 (`coeffs: [f64; 5]`) | discriminator 別 enum (`Conic::Parabola{...} \| Conic::Hyperbola{...}`) | NURBS で全曲線統一表現 | **Option A (一般 5 自由度)** | NURBS は Phase 15 で導入予定で先取り過剰、判別 enum は退化判定で分岐 1 階層増える | Phase 15 NURBS 導入時に conic を NURBS で再表現する選択肢 |
| スケッチ編集モデル | 各編集 op を独立 `Feature` variant として履歴に残す (純関数) | 既存 sketch を in-place mutation で書き換える (1 sketch = 1 Feature) | edit DAG (各 sketch ごとに局所履歴) | **Option A (履歴 Feature)** | in-place は ADR-001 「Feature history = source of truth」と矛盾、DAG は実装複雑 | 1 sketch 当たり編集 op > 100 (Phase 11 拘束ソルバ自動編集が大量発生する場合) |
| ID 安定性 | 編集後も既存 ID 保持 + 派生 ID (`a_split_1` 等) | 毎編集で全 ID 再採番 | UUID で完全分離 | **Option A (保持 + 派生)** | 再採番は ADR-005 Topological Naming と衝突、UUID は決定性要件 (CLAUDE.md) と衝突 | ADR-005 改訂 (= TN の編集後再計算規約を変える場合) |
| 既存 SketchSegment 互換 | `SketchElement::Line` に enum 内包 + `schema_version` v1→v2 バンプ + Document loader に migration hook (v1 YAML を inline 変換) | v1 据え置き + serde default のみ (= 型差し替えで wire format は変わるが version は変えない) | dual-write (legacy + new を両方持つ移行期間) | **Option A (v2 バンプ + migration hook)** | v1 据え置きは「型を変えたのに version 不変」で `CURRENT_SCHEMA_VERSION` ガードが意味を失う (3 ペルソナ refute 受容)。dual-write は遷移期間のロジック複雑 | YAML 1 字も書き換えずに既存 example が読めなくなる問題が判明した場合 (migration hook 自体が不十分なら別 ADR で書き戻し戦略を再検討) |
| ε 値域 | ADR-004 既存値を流用 + Phase 10 新規 ε を追加 (`ε_axis_ratio` 等) | Phase 10 専用 ε モジュールを別 crate に切り出す | tolerance を全部 runtime config 化 | **Option A (ADR-004 流用 + 追加)** | 別 crate は Phase 10 単独では over-engineering、runtime config は決定性要件と相性悪い | Phase 11 拘束ソルバで tolerance を user 指定にする必要が出た場合 |

---

## Consequences

### Positive

- **Phase 10 子 Issue #274-#278 の前提が確定** する。各 Issue は本 ADR を参照しつつ独立に実装でき、相互の方針齟齬が起きない
- 既存 example YAML は **書き換えなしで読める** (= migration hook が透過的に v1 → v2 変換)。`SketchElement::Line` 内包 + serde fallback で wire format の連続性は保たれる
- `engawa-kernel::geometry::tolerances` 1 module に ε 値が集約され、tolerance 規約の保守性が上がる
- スケッチ編集 op が独立 Feature variant として並ぶことで、`Feature::apply()` ディスパッチ (engawa-build) に自然に乗る

### Negative

- enum variant 増加で `match` の網羅性チェックが厳しくなる (= Phase 12 Refactor Pass 1 で variant 数 30 超なら trait object 移行を再評価する trigger)
- Conic の 5 自由度表現は人間が読み書きしにくい (= Phase 15 NURBS 導入で別表現に upgrade する余地を残す)
- 編集 op を独立 Feature variant にすると `Feature` enum が膨らむ (Phase 12 で `Feature::Sketch(SketchOp)` のネスト化を再評価する可能性)

### Neutral

- `ε_axis_ratio = 1e-6` / `ε_discriminant = 1e-9` の具体値は本 ADR で固定するが、子 Issue で property test を回した結果次第で再調整する余地はある (= 次の小 ADR で値域更新の可能性)

---

## Migration Plan

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
