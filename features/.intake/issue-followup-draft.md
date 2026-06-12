# fix(tessellation): trimmed sphere face の入力 validation 強化 + T04 共有境界の strict 比較 (#137 follow-up)

## 位置付け

**`type: foundation`** (ADR-002「ラベル運用: 2 軸ラベル制」)。Phase 7 milestone への差し込み作業として起票。Phase 7 完了判定 (xy/xz/yz スケッチ → Extrude/ExtrudeCut → CreateSketch Feature) には Phase 7 type: feature Issue 群が直接寄与するが、本 Issue は Phase 7 で **「描いたスケッチから Extrude/ExtrudeCut を実行する」完了条件の支え** として、後段で Boolean 結果を表示する trimmed sphere face のロバスト性を向上させる。

## 背景

#137 (circ_normal Z 決め打ちの一般化) で `tessellate_sphere_face_trimmed` を circ_normal ベース化したのち、STEP 7.5 Codex independent review が以下の改善余地を指摘した:

### 1. sphere face 入力 validation の不足

現状 (#137 完了後の状態) は `signed_offset.abs() > radius + LENGTH_TOLERANCE` のみを validate するため、以下のケースが素通りする:

- **`circ_center` の軸直交ずれ**: `circ_center - sphere_center` が axis 方向だけでなく直交方向にも有意な成分を持つ円。`perp = (circ_center - center) - signed_offset * axis` のノルムが `LENGTH_TOLERANCE` を超える場合、円は B-rep edge とは別の緯線として球面上に再合成され、隣接面との共有境界が破綻する。
- **`circ_radius` の整合性不全**: `circ_radius^2 + signed_offset^2 != radius^2` (期待半径との不一致)。やはり隣接面との境界がずれる。

これらは現状 production code path では発生しないが (B-rep 整合性は upstream で保証されるはず)、defensive layer が不在のため不正な入力が静かに誤メッシュを生成する。

### 2. T04_shared_boundary の strict 化

#137 で実装された `t04_shared_boundary_with_cyl_lateral` は `count_naked_edges(&mesh, LENGTH_TOLERANCE) == 0` を共有境界整合の代理 assertion として採用している (test-spec.md がシンプルバリエーションとして明示許可)。watertight 制約は満たすが、以下の問題を捕捉できない:

- 境界 ring の **位相ずれ** (両側面が同じ点列だが index がローテーションしている)
- 境界 ring の **向きずれ** (両側面が逆順で巡回)
- 個別頂点の microscopic ずれ (naked_edge 判定の quantize tolerance より小さい)

#129-F02 で codex-review が提案した「twin ベースの直接頂点比較」が本来の意図。

## 作業内容

### 1. 入力 validation 追加 (`tessellate_sphere_face_trimmed`)

`mod.rs:1040` 付近の `signed_offset` validation の直後に追加:

```rust
// circ_center の軸直交成分が有意でないこと
let perp = (circ_center - center).coords - signed_offset * axis;
if perp.norm() > LENGTH_TOLERANCE {
    return Err(TessellationError::InvalidTrimCircle {
        signed_offset,
        sphere_radius: radius,
    });
}

// circ_radius が期待半径 sqrt(R^2 - signed_offset^2) に一致すること
let expected_radius_sq = radius * radius - signed_offset * signed_offset;
let expected_radius = expected_radius_sq.max(0.0).sqrt();
if (circ_radius - expected_radius).abs() > LENGTH_TOLERANCE {
    return Err(TessellationError::InvalidTrimCircle {
        signed_offset,
        sphere_radius: radius,
    });
}
```

### 2. T04_shared_boundary を twin ベースに置換

`tests/trim_sphere_circ_normal_acceptance.rs` の `t04_shared_boundary_with_cyl_lateral` を以下のように書き換え:

1. `boolean_cut_sphere_dimple` の Solid 内で `Surface::Sphere` 型 trimmed face を特定
2. inner_loop の `half_edges[k]` の twin index で対面 face (cyl lateral) と対応 HalfEdge を辿る
3. `TriangleMesh::face_ids` (もしくは別の face → vertex range map) を使って両 face の境界 ring 頂点列を抽出
4. index 順に position を `LENGTH_TOLERANCE` 以内で比較し、不一致時にどの index で何 mm ずれたかをエラー出力

`TriangleMesh::face_ids` の現状を調査し、ない場合は (a) 追加するか (b) 代替の vertex range tracking を実装する。

### 3. 回帰テスト

- `t_degen_offset_axis_circ_center_rejected`: `circ_center` を axis から外したケース → `InvalidTrimCircle`
- `t_degen_mismatched_circ_radius_rejected`: `circ_radius` を期待値からずらしたケース → `InvalidTrimCircle`

## 完了条件

- `cargo xtask ci` green (Playwright #145 既知 infrastructure 失敗を除く)
- 新 validation テスト 2 件 + T04 strict 版が green
- 既存 #137 acceptance テスト 10 件が引き続き green

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|---|---|
| `tessellate_sphere_face_trimmed` の入力 validation 強化 (`perp.norm` と `circ_radius` 整合性) | 上流 surface_intersect の MVP 制約解除 |
| T04_shared_boundary の twin ベース strict 比較への置換 | sphere 以外の trimmed face (cylinder lateral 等) への同種強化 |
| `TriangleMesh::face_ids` 等の調査・必要に応じた追加 | Boolean partition 側の交線エッジ生成変更 (ADR-009 マター) |
| validation 違反の回帰テスト追加 | 完全な #129-F02 提案実装 (#144 が別 Issue で処理) |

## Non-Goals

- ADR-009 案 A 実装本体 (`partition.rs` 周期エッジ化) — Phase 4 再訪時に別 Issue
- per-entity tolerance 移行 (ADR-004 #34 以降の Phase 5 候補)
- sphere face 以外の入力 validation 強化

## 数値モデル

- 比較公差: `LENGTH_TOLERANCE` (1e-9 mm) 流用
- 退化判定: `perp.norm() > LENGTH_TOLERANCE` または `(circ_radius - expected_radius).abs() > LENGTH_TOLERANCE` で `InvalidTrimCircle`

## 関連

- #137 (本 Issue 起票元、原 scope は `circ_normal` Z 一般化に集中、本件は scope 拡張分の follow-up)
- ADR-004 (数値モデル、`LENGTH_TOLERANCE` 流用)
- ADR-009 (Boolean 交線エッジ周期化、本件と独立に進行)
- codex-review #137 round 2 F02 / round 3 F01 + F02
- #144 (共有境界比較テスト追加、ADR-009 派生 — 本件と方向性は近いが別 scope)
