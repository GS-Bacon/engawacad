## 自律判断ログ（自律バッチモード）

- **曖昧点**: 修正方針として選択肢 A（offset 追加）/ B（Extrude 再設計）/ C（先送り）が Issue に記載
- **採用**: 選択肢 A の簡略版
  - `CreateSketch` に `offset: f64` (後方互換のため `#[serde(default)]`) を追加
  - `Extrude` に `fuse_target: Option<String>` を追加してカーネル側で boolean fuse 実行
  - **理由**: Phase 8 で再設計予定の ADR なし。現状の「2ボディが重なる」挙動は Phase 6 完了条件「形状が更新される」に対して UX 上明らかに問題。最小変更で修正できる。
  - **C（先送り）を採用しない理由**: #106 で E2E テストを書く際にこのバグが残ると正しいシナリオが書けない。バッチ同一グループで修正すべき。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| mycad-format: CreateSketch に offset: f64 追加（#[serde(default)] で後方互換） | offset の符号対応（負方向押出）— 常に正方向と仮定 |
| mycad-format: Extrude に fuse_target: Option<String> 追加 | 曲面・回転体の offset 計算 |
| mycad-build: Extrude でスケッチ offset を Plane::translate() に適用 | ExtrudeCut の offset 修正（別途 #105 で対応済み） |
| mycad-build: fuse_target がある場合 boolean fuse を実行 | ExtrudeCut の fuse_target 追加 |
| web/src/extrude.ts: face 頂点から法線方向 offset を計算し sketch/extrude に設定 | Phase 8 の完全な面上スケッチ機能 |

## Non-Goals
- 負方向（法線反対方向）への押出
- ExtrudeCut の位置修正（offset なしで coplanar 問題は #105 でクランプ対応済み）
- Phase 8 の任意面スケッチ設計
- 既存 .mycad ファイルの移行（offset 未記載 = 0.0 として後方互換）

## 実装対象
- Issue: #104
- 影響ファイル:
  - `crates/mycad-format/src/feature.rs` — CreateSketch.offset と Extrude.fuse_target 追加
  - `crates/mycad-build/src/lib.rs` — Extrude 処理で offset / fuse_target 対応
  - `web/src/extrude.ts` — faceOffset() 追加、buildExtrudeFeatures 修正
  - `web/src/main.ts` — fuse Feature の POST 追加
  - `web/src/generated/Feature.ts` — スキーマ変更に追従（手動更新）

**修正箇所 (before/after)**

before: `mycad-format` の `Feature::CreateSketch`
```rust
CreateSketch {
    id: String,
    plane: SketchPlane,
    profile: Vec<SketchSegment>,
}
```
after:
```rust
CreateSketch {
    id: String,
    plane: SketchPlane,
    #[serde(default)]
    offset: f64,
    profile: Vec<SketchSegment>,
}
```

before: `mycad-format` の `Feature::Extrude`
```rust
Extrude {
    id: String,
    sketch: String,
    depth: f64,
}
```
after:
```rust
Extrude {
    id: String,
    sketch: String,
    depth: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fuse_target: Option<String>,
}
```

before: `mycad-build` の Extrude 処理
```rust
let plane = match sketch_plane {
    mycad_format::SketchPlane::Xy => Plane::xy(),
    mycad_format::SketchPlane::Xz => Plane::xz(),
    mycad_format::SketchPlane::Yz => Plane::yz(),
};
let solid = make_extrusion(&plane, &profile_uv, *depth, gen)?;
built.register(id.to_string(), solid);
```
after:
```rust
let base_plane = match sketch_plane {
    mycad_format::SketchPlane::Xy => Plane::xy(),
    mycad_format::SketchPlane::Xz => Plane::xz(),
    mycad_format::SketchPlane::Yz => Plane::yz(),
};
let plane = if *offset != 0.0 {
    base_plane.translate(base_plane.normal * *offset)
} else {
    base_plane
};
let extruded = make_extrusion(&plane, &profile_uv, *depth, gen)?;
if let Some(target_id) = fuse_target {
    let t_solid = built.get(target_id)
        .ok_or_else(|| KernelError::BodyNotFound { id: target_id.clone() })?;
    let result = boolean(&t_solid.solid, &extruded, BooleanOp::Fuse, gen)?;
    built.consume(target_id);
    built.register(id.to_string(), result);
} else {
    built.register(id.to_string(), extruded);
}
```

before: `web/src/extrude.ts` の buildExtrudeFeatures の sketch 生成
```typescript
const sketch: Feature = {
    type: "create_sketch",
    id: sketchId,
    plane,
    profile,
};
const extrude: Feature = {
    type: "extrude",
    id: extrudeId,
    sketch: sketchId,
    depth,
};
return { sketch, extrude };
```
after: offset 計算 + fuse_target 追加
```typescript
const offset = faceOffsetFromPlane(positions, indices, faceIds, faceId, plane);
const sketch: Feature = {
    type: "create_sketch",
    id: sketchId,
    plane,
    offset,
    profile,
};
// 既存ボディの中で選択面を持つボディを探す
const targetBodyId = targetBody ?? null;
const extrude: Feature = {
    type: "extrude",
    id: extrudeId,
    sketch: sketchId,
    depth,
    ...(targetBodyId ? { fuse_target: targetBodyId } : {}),
};
return { sketch, extrude };
```

## 設計方針
- `offset` は face の法線方向（スケッチ平面の法線）の符号付き座標。例: yz 平面の face で x=5 なら offset=5
- `fuse_target` がある場合、カーネルは extrude solid と target solid を boolean fuse してから target を consume する
- 後方互換: offset 未指定 = 0.0 (serde default)、fuse_target 未指定 = None (skip_serializing_if)
- 決定性: offset は面頂点の座標から決定的に計算 (最初の三角形の代表頂点の法線方向座標)
- エラー: fuse_target が見つからない場合は KernelError::BodyNotFound

### 数値モデル
- ε 値は `mycad_kernel::LENGTH_TOLERANCE` を使用（新規 epsilon 追加なし）

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01_unit_offset_yz | 正常系 | faceOffsetFromPlane が yz 面の頂点(x=5) から 5.0 を返す | 5.0 |
| T02_unit_offset_xy | 正常系 | faceOffsetFromPlane が xy 面の頂点(z=10) から 10.0 を返す | 10.0 |
| T03_degen_no_match | 縮退 | faceId が存在しない場合 0.0 を返す | 0.0 |
| T04_boundary_fuse | 境界 | fuse_target ありの Extrude Feature を POST すると 1 ボディで返る | bodies.length === 1 |

## 幾何的不変条件チェックリスト
- N/A（Boolean fuse は既存 Feature::Fuse と同実装を再使用）
- N/A
- N/A
- N/A
