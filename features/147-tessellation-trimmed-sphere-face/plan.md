## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `tessellate_sphere_face_trimmed` の入力 validation 強化 (`perp.norm` 軸直交ずれ + `circ_radius` 整合性) | 上流 `surface_intersect` の MVP 制約解除 |
| T04_shared_boundary を twin ベース strict 比較 (位相ずれ・向きずれ・microscopic ずれを検出) に置換 | sphere 以外の trimmed face (cylinder lateral など) への同種強化 |
| `TriangleMesh::face_ids` 等の調査・必要に応じた face → vertex range 追跡の追加 | Boolean partition 側の交線エッジ生成変更 (ADR-009 案 A 本体) |
| 退化入力に対する回帰テスト 2 件 + T04 strict 版 | #129-F02 提案の完全実装 (#144 が別 Issue として処理) |

## Non-Goals

- ADR-009 案 A 実装本体 (`partition.rs` の周期エッジ化) — Phase 4 再訪時に別 Issue
- per-entity tolerance 移行 (ADR-004 #34 以降の Phase 5 候補)
- sphere face 以外の入力 validation 強化

## 実装対象

<!-- Issue: #147 -->
<!-- 影響クレート/ファイル -->
- `crates/mycad-kernel/src/tessellation/mod.rs` (`tessellate_sphere_face_trimmed`)
- `crates/mycad-kernel/tests/trim_sphere_circ_normal_acceptance.rs` (T04 置換 + 退化テスト追加)
- `crates/mycad-kernel/src/tessellation/mesh.rs` 周辺 (`face_ids` 等の face → vertex range 追跡確認・必要なら拡張)

### 1. `tessellate_sphere_face_trimmed` への入力 validation 追加

**位置**: `crates/mycad-kernel/src/tessellation/mod.rs:1024` 直下 (既存の `signed_offset.abs() > radius + LENGTH_TOLERANCE` ガードの直後)。

**before**:
```rust
    if signed_offset.abs() > radius + LENGTH_TOLERANCE {
        return Err(TessellationError::InvalidTrimCircle {
            signed_offset,
            sphere_radius: radius,
        });
    }

    // 上の validation で |signed_offset| <= radius + ε まで絞ったため、
    let rel_axis = (signed_offset / radius).clamp(-1.0, 1.0);
```

**after**:
```rust
    if signed_offset.abs() > radius + LENGTH_TOLERANCE {
        return Err(TessellationError::InvalidTrimCircle {
            signed_offset,
            sphere_radius: radius,
        });
    }

    // circ_center の axis 直交ずれを拒否: |perp| が許容外なら円は B-rep edge と別の
    // 緯線として球面上に再合成され、隣接面との共有境界が破綻する。
    let perp = (circ_center - center).coords - signed_offset * axis;
    if perp.norm() > LENGTH_TOLERANCE {
        return Err(TessellationError::InvalidTrimCircle {
            signed_offset,
            sphere_radius: radius,
        });
    }

    // circ_radius が期待半径 sqrt(R^2 - signed_offset^2) と一致することを確認:
    // 不一致だと隣接面側の境界半径と接続できない。
    let expected_radius_sq = radius * radius - signed_offset * signed_offset;
    let expected_radius = expected_radius_sq.max(0.0).sqrt();
    if (*circ_radius - expected_radius).abs() > LENGTH_TOLERANCE {
        return Err(TessellationError::InvalidTrimCircle {
            signed_offset,
            sphere_radius: radius,
        });
    }

    // 上の validation で |signed_offset| <= radius + ε まで絞ったため、
    let rel_axis = (signed_offset / radius).clamp(-1.0, 1.0);
```

### 2. T04_shared_boundary を twin ベース strict 比較に置換

`crates/mycad-kernel/tests/trim_sphere_circ_normal_acceptance.rs::t04_shared_boundary_with_cyl_lateral` を以下の流れに置換する:

1. `boolean_cut_sphere_dimple` で Solid を生成し tessellate する。
2. Solid 内で `Surface::Sphere` 型 trimmed face の `face_idx` と、その `inner_loops[0].half_edges[0]` の twin から対面 face (cyl lateral) の `face_idx` を特定する。
3. `TriangleMesh` から face → vertex range の対応を取り、両 face の boundary ring 頂点列を抽出する。
4. index 順に position を `LENGTH_TOLERANCE` 以内で比較し、不一致時は「どの index で何 mm ずれたか」を panic メッセージに含める。
5. ring の向き (CW/CCW) もチェックし、逆順なら明示的に失敗させる。

### 3. `TriangleMesh::face_ids` 等の調査

face → vertex range の対応を既存 API で取れるか確認する。`mesh.rs` を読み、

- 既に取れるなら → そのまま使う (#147 では追加実装なし)
- 取れない場合 → face_id ごとの boundary vertex range を tracking する補助 (`HashMap<String, Range<usize>>` 等) を追加し、T04 で使用する

調査結果は実装中に確定するが、いずれの分岐でも T04 で「両 face の boundary ring 頂点列を index で比較できる」状態にする。

## 設計方針

- **決定性**: 既存の決定性は崩さない。追加 validation は早期 return のため決定的、テストも `make_box` / `make_cylinder` / `boolean_*` 既存 API のみ使用。
- **B-rep トポロジー妥当性**: 既存 Boolean 出力の Euler-Poincaré 性に影響なし。
- **退化幾何の扱い**: `LENGTH_TOLERANCE` (1e-9 mm) を退化判定の閾値として `perp.norm()` と `|circ_radius - expected_radius|` の両方に適用する。
- **derive 規約**: 新規型なし (`TessellationError::InvalidTrimCircle` は既存)。
- **エラーハンドリング**: 既存 `InvalidTrimCircle { signed_offset, sphere_radius }` を流用 (新規 variant は追加しない、Issue 本文の方針通り)。理由は両ケースとも「球面と整合しない trim 円」という同種の不正で、呼び出し側が処理を分岐させる必要がないため。
- **workspace.dependencies**: 追加なし。

### 数値モデル

- 比較公差: `LENGTH_TOLERANCE` (1e-9 mm) を流用
- 退化判定:
  - `perp.norm() > LENGTH_TOLERANCE` → `InvalidTrimCircle`
  - `(*circ_radius - expected_radius).abs() > LENGTH_TOLERANCE` → `InvalidTrimCircle`
- ADR-004 準拠方針: tolerant (LENGTH_TOLERANCE による許容)

## テスト計画 (ID 付き)

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 (継承) | 既存 `t01_*` 系 (#137 acceptance) が変更なく green | 既存 assertion |
| T04_strict | 共有境界 strict | `boolean_cut_sphere_dimple` の trimmed sphere face と隣接 cyl lateral face の boundary ring 頂点列を twin で対応付け、index ごとに `LENGTH_TOLERANCE` 内一致を確認 | 全 index で position 差 < `LENGTH_TOLERANCE`、ring 向き一致 |
| T_degen_offset_axis_circ_center_rejected | 退化 (拒否) | `circ_center` を axis 直交方向にずらした入力で `tessellate_sphere_face_trimmed` を直接呼ぶ | `Err(InvalidTrimCircle { .. })` を返す |
| T_degen_mismatched_circ_radius_rejected | 退化 (拒否) | `circ_radius` を期待値 `sqrt(R^2 - signed_offset^2)` からずらした入力で直接呼ぶ | `Err(InvalidTrimCircle { .. })` を返す |

注: 既存 #137 acceptance テスト 10 件 (T01〜T03, T05〜) は変更せず green を維持する。

## 幾何的不変条件チェックリスト

- [ ] N/A (本 Issue は Boolean/Partition/Assemble の不変条件本体は変更せず、tessellation 入力 validation と境界 ring 比較テストのみ追加)
- [x] T04 strict 化により「隣接面 boundary ring の位相一致」という新規 invariant が test で固定される
