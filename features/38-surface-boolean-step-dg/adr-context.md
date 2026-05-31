# ADR Context for Issue #38

本ファイルは plan.md が参照する ADR の関連節を抜粋し、Codex レビューに必要な背景を渡す。

---

## ADR-004 (自由幾何コミット) — 関連節

### Decision 1 & 2

> 自由曲面・自由曲線を「いつか検討する」ものではなく **確定要件** として扱う。ただし NURBS 等の実装は今は行わず、Boolean が要求する範囲から漸進的に導入する。
>
> 1. **`Surface` / `Curve` enum を唯一の幾何拡張点**とする。`Nurbs` / スプライン variant の追加が加算的であり続けるよう、アルゴリズムは variant 集合を仮定しない。
> 2. **アルゴリズムは曲面・曲線の型に非依存**であること。平面・直線前提をアルゴリズムへ埋め込まない。

**#38 への含意**: arc 再構成は `Curve::Circle` / `Curve::Line` の 2 variant に依存するが、`assemble.rs::reconstruct_intersection_curve` のディスパッチは将来 `Curve::Spline` 等の追加で破綻しないよう、`ArcProvenance::parent_curve_3d` 経由で「親 curve の variant を見て同種にまとめる」汎用設計にする (variant 集合をハードコードしない)。

### 公開公差定数

| 定数 | 値 | 用途 |
|------|-----|------|
| `LENGTH_TOLERANCE` | `1e-9` (mm) | 距離・座標の絶対比較 |
| `ANGLE_TOLERANCE` | `1e-9` (rad) | 角度・パラメータの絶対比較 |
| `RELATIVE_TOLERANCE` | `1e-9` (無次元) | スケール比例の相対比較 |

**#38 への含意**: arc 再構成での「同一親 Circle 判定」は `LENGTH_TOLERANCE` (center/radius) と `ANGLE_TOLERANCE` (normal 方向) で行う。`BOOLEAN_ARC_SAMPLES = 32` の固定値はサンプル間距離が極端に小さくならない範囲 (Circle 周長 ≥ 32 · LENGTH_TOLERANCE) で安全。

### Decision 3 段階移行 — per-entity tolerance 配置

| Issue | 範囲 | 内容 |
|-------|------|------|
| #31 | 型導入 | `Tolerance` newtype + helper。既存 `LENGTH_TOLERANCE` call site は不変 |
| #34 | 曲面 Boolean MVP | Plane×Cylinder / Plane×Sphere / Cylinder×Sphere の軸整列 Boolean。per-entity tolerance 埋め込みは Non-Goal |
| #34 以降 (Phase 5 候補) | field 埋め込み + 完全移行 | `Vertex` / `Edge` / `Face` に `tolerance: Tolerance` field 追加 |

**#38 への含意**: per-entity `tolerance` 埋め込みは #34 と同じく Non-Goal。本 Issue は引き続きグローバル定数 `LENGTH_TOLERANCE` / `ANGLE_TOLERANCE` を使用。

---

## ADR-005 (Topological Naming) — 関連節

### §6 基底名 grammar

> **フォーマット (内部 canonical name)**: `<feature_id>;<kind>:<role>`
>
> | フィールド | 例 | 説明 |
> |-----------|-----|------|
> | `feature_id` | `box_1` | Feature の id |
> | `kind` | `F` / `E` / `V` | face / edge / vertex |
> | `role` | `top` / `seam` | 各 maker が割り当てる安定役割名 |
>
> **文字集合**: 各セグメントの許容文字は `[A-Za-z0-9_-]`。区切り文字 `;` と `:` は予約とし segment 内で禁止する。

**#38 への含意**: `make_sphere` の role 名はこの charset を守る必要がある。kind は別フィールドで表現されるため、role に kind prefix (`f_`/`e_`/`v_`) を付けるのは冗長。cuboid.rs が `f_z_pos` 系を使っているのは旧実装の慣習であり、新規 sphere は ADR-005 §7 の例に準拠する。

### §7 role 付与ルール (#38 が遵守すべき表)

> **face role の例**:
>
> | Feature | face role |
> |---------|-----------|
> | CreateBox | `top` / `bottom` / `front` / `back` / `left` / `right` |
> | CreateCylinder | `lateral` / `cap_top` / `cap_bottom` |
> | **CreateSphere** | **`surface`** |
> | Extrude | `cap_start` / `cap_end` / `side_<sketch要素安定名>` |
>
> **自己隣接・周期トポロジー**: 1 面が両側で接する seam や極 (sphere の北極・南極、cylinder の seam) は専用 role を必ず割り当てる。
>
> | エンティティ | 専用 role 例 |
> |------------|-------------|
> | **sphere 極 edge** | **`north_pole`, `south_pole`** |
> | **sphere seam edge** | **`seam`** |
> | cylinder seam edge | `seam` |
> | cylinder rim edges | `cap_top_rim`, `cap_bottom_rim` |

**#38 への含意**: `make_sphere` で付与する role 名は厳密にこの表に従う:
- face: `surface`
- seam edge: `seam`
- 極 vertex: `south_pole` / `north_pole`

(現状の `make_sphere` 実装は `v_south`, `v_north`, `e_seam` という変数名を使っているが、これらは Rust の変数名であり EntityRef.role としては付与されていない。本 Issue で初めて role 文字列を確定する)

### §9 topology materialization の決定性

> 各 maker は `HashMap`/`HashSet` 等の **順序非決定な反復を禁止** し、入力要素は stable id または明示ソート順で走査する。同一入力で `Solid` フラット配列順・`EntityId` 発番順・座標まで一致すること。

**#38 への含意**: arc 再構成の集約処理 (`reconstruct_intersection_curve`) は `HashMap` ではなく `BTreeMap` (キー: parent_curve_3d の lexicographic order) または `Vec` + `sort_by` で実装する。plan.md §決定性要件 で既に明記。

### §10 退化エンティティの命名方針

> 命名は topology validation 後の **非退化** エンティティにのみ付与する。退化入力は **`KernelError`** とし、名前集合に決して現れさせない。

**#38 への含意**: arc 再構成で sample 数が 1 にフォールバックして chord `Curve::Line` を作るケースは「退化」ではなく「正常な縮退」として扱い、edge name を付与する (clip で大部分が消えた arc も valid edge)。本当の退化 (全 sample が clip outside) は edge を作らず skip。
