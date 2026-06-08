## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `e2e_api_scenarios.rs` に S08 多段操作シナリオを追加 | `S06`/`S07` の変更（#115 で実装済み） |
| box → sketch → extrude → extrude_cut（対象: extrude_0）の E2E | 新規 example ファイルの追加 |
| face_id フォーマット検証（cut 後に `cut` を含む face_id が存在する） | GUI/Playwright テスト |
| 200 OK + body 数の不変条件検証 | fuse_target を使った挙動 |

## Non-Goals

- 既存 S01〜S07 の変更・削除
- GUI/viewer の変更
- proptest / property-based test（#112 で対応）
- パフォーマンステスト

## 実装対象

Issue: #116
影響ファイル:
- `crates/mycad-api/tests/e2e_api_scenarios.rs` — S08/S08_boundary/S08_degen 関数を追加

## 設計方針

### S08 テストシナリオ

```
simple_box.mycad (box_1: 10×20×30)
  ↓ POST create_sketch (xy plane, [-3,3]×[-3,3])
  ↓ POST extrude (id=extrude_0, depth=5, no fuse_target) → 2 bodies {box_1, extrude_0}
  ↓ POST create_sketch (xy plane, [-1,1]×[-1,1]) id=sketch_1
  ↓ POST extrude_cut (id=cut_0, sketch=sketch_1, depth=2, target=extrude_0)
  → 200 OK, body 数 >= 2, cut face_id 含む
```

### face_id フォーマット検証

`extrude_cut` 後の face_id に `cut` キーワードが含まれることを確認（`D(F;cut;...)` 形式）:
```rust
let has_cut_face_id = bodies.iter().any(|b| {
    b.mesh.face_ids.iter().any(|id| id.contains("cut"))
});
assert!(has_cut_face_id, "extrude_cut must produce at least one cut face_id");
```

### body 数の不変条件

- extrude 後: 2 bodies (box_1, extrude_0)
- extrude_cut on extrude_0 後: extrude_0 が void solid になる
  - API は outer + inner を separate BodyMesh として返す可能性あり → `bodies.len() >= 2`
  - 実測値を GLM が確定した後に assertion を絞る

### S08_boundary

extrude_0 の depth(=5) より小さい depth=4 で extrude_cut → 200 OK (ギリギリ埋没しない)

### S08_degen

存在しない target_id ("nonexistent_0") を extrude_cut に指定 → 422

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| S08 | E2E 正常系 | box → extrude → extrude_cut(extrude_0) 多段 | 200 OK, bodies >=2, cut face_id 含む |
| S08_boundary | 境界 | depth を extrude_0 辺長ギリギリ未満に設定 | 200 OK |
| S08_degen | 退化 | extrude_cut で存在しない target_id を指定 | 422 |

## 幾何的不変条件チェックリスト

- N/A: API 層 E2E テストのため、カーネル不変条件は対象外
