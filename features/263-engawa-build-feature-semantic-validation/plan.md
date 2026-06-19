## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `FeatureCrud::insert` に semantic validation を追加 (history 上で sketch/target/tool/fuse_target ref が解決可能か検証) | `FeatureCrud::delete` / `update` (本 Issue は insert のみ。別 Issue で対応) |
| 4 つの error variant: `SketchNotFound` / `BodyNotFound` / `InsertBeforeProducer` / `InsertBeforeConsumer` を `FeatureCrudError` に追加 | Document 全体の semantic validation 関数化 (apply_op レイヤーは ADR-015 (#246) の射程) |
| degen ケース: 自己参照 (feature の id がそれ自身の ref と一致) を `SelfReference` で弾く | 環参照のグラフ検出 (history が線形なため発生不可) |
| `engawa entry add` CLI が strict 化される (insert 経由のため自動的に semantic check が走る) | CLI のエラーメッセージ整形拡張 (本 Issue では `format!("insert failed: {e}")` の包装をそのまま使用) |
| 既存 4 unit test の互換性維持 (ref 無し feature しか使っていないため影響なし) | `FeatureOp` enum 採用 / `Document::apply_op` レイヤー再設計 (ADR-015 の領域、本 Issue は既存 FeatureCrud に閉じる) |

## Non-Goals

- `FeatureCrud::delete` / `move` / `update` 系の semantic validation (別 Issue)
- ADR-015 (#246) で議論されている `FeatureOp` enum / `Document::apply_op` の一元化レイヤー導入
- semantic validation を `Document::validate()` 本体に組み込むか (build_assembly が真の最終 gate という現行責務分割を維持する)
- CLI のエラーメッセージ多言語化や提案文 ("--at N 以降を試してください") の付与
- 履歴 N-1→N step の差分 lint や ID stability 検査 (#255 で既に validate 済み)
- `CreateSketch.plane_ref = PlaneRef::Entity(...)` の implicit body lifetime traversal (#263 C-F02 scope-defer → #264)
- pre-existing history の broken ref に対する defensive validation (`simulate_history` での未解決 ref 検知。#263 A-F01 round 2 scope-defer → #265)

## 実装対象

<!-- Issue: #263 -->
<!-- 影響クレート/ファイル -->
- `crates/engawa-build/src/feature_crud.rs` — `FeatureCrudError` に 5 variant 追加 + `FeatureCrud::insert` に semantic check 段を挿入 + 既存 inline tests を保持
- `crates/engawa-build/tests/feature_crud_acceptance.rs` (新規) — 受け入れテスト (T01〜T09)

<!-- 変更する型・関数のシグネチャ -->
**`FeatureCrudError`** (新 variant 追加、`#[non_exhaustive]` のため後方互換):
```rust
SketchNotFound  { feature_id: String, sketch_ref: String }
BodyNotFound    { feature_id: String, body_ref: String }
InsertBeforeProducer { feature_id: String, ref_id: String, producer_at: usize, requested_at: usize }
InsertBeforeConsumer { displaced_feature_id: String, consumed_ref: String, consumer_at: usize, requested_at: usize }
SelfReference { feature_id: String, ref_kind: &'static str }
```

**`FeatureCrud::insert`** シグネチャ不変、内部に semantic 検証段を追加。

### 既存関数の修正 (before / after)

**before** (`feature_crud.rs:47-66`、現状の `insert` 本体):
```rust
pub fn insert(doc: &Document, feature: Feature, at: usize) -> Result<Document, FeatureCrudError> {
    let len = doc.root_component.features.len();
    if at > len {
        return Err(FeatureCrudError::OutOfRange { index: at, len });
    }
    let new_id = feature.id().to_string();
    if doc.root_component.features.iter().any(|f| f.id() == new_id) {
        return Err(FeatureCrudError::DuplicateFeatureId { id: new_id });
    }
    let mut next = doc.clone();
    next.root_component.features.insert(at, feature);
    next.validate()?;
    Ok(next)
}
```

**after** (semantic check 段を追加。既存の format-level 検証は前後で保持):
```rust
pub fn insert(doc: &Document, feature: Feature, at: usize) -> Result<Document, FeatureCrudError> {
    let len = doc.root_component.features.len();
    if at > len {
        return Err(FeatureCrudError::OutOfRange { index: at, len });
    }
    let new_id = feature.id().to_string();
    if doc.root_component.features.iter().any(|f| f.id() == new_id) {
        return Err(FeatureCrudError::DuplicateFeatureId { id: new_id });
    }
    // --- semantic validation (本 Issue で追加) ---
    check_self_reference(&feature)?;                                  // degen ガード
    check_refs_resolve_before(&feature, &doc.root_component.features, at)?;
    check_no_downstream_break(&feature, &doc.root_component.features, at)?;
    // -----------------------------------------
    let mut next = doc.clone();
    next.root_component.features.insert(at, feature);
    next.validate()?;
    Ok(next)
}
```

新規 private helper (同ファイル内):

- `fn feature_sketch_refs(f: &Feature) -> Vec<&str>` — sketch を ref する各 variant の id 抽出 (Extrude.sketch / ExtrudeCut.sketch)
- `fn feature_body_refs(f: &Feature) -> Vec<&str>` — body を ref する id 抽出 (Extrude.fuse_target / ExtrudeCut.target / Cut.{target,tool} / Fuse.{target,tool} / Intersect.{target,tool})
- `fn feature_consumes(f: &Feature) -> Vec<&str>` — body を CONSUME する id 抽出 (Extrude.fuse_target / ExtrudeCut.target / Cut.{target,tool} / Fuse.{target,tool} / Intersect.{target,tool}) ※本仕様では body_refs と一致するが、将来 ref-without-consume が追加されたとき分離可能にしておく
- `fn simulate_history(features: &[Feature], up_to: usize) -> (HashMap<&str, usize> sketches_at, HashMap<&str, usize> live_bodies_at)` — features[0..up_to] を線形に走査し、sketches を「初出 index」マップに、bodies を「最後に register された index で live なもの」マップに記録
- `fn check_self_reference(f: &Feature)` — id が自身の ref と一致するか確認
- `fn check_refs_resolve_before(f, features, at)` — `simulate_history(features, at)` の結果に対し、sketch/body refs がすべて解決可能か確認。
  - 解決不可で、その id がどこにも存在しない → `SketchNotFound` / `BodyNotFound`
  - 解決不可で、id が features[at..] に存在 → `InsertBeforeProducer`
  - body ref が features[0..at] で既に consume 済み → `BodyNotFound` (live でない)
- `fn check_no_downstream_break(f, features, at)` — 新 feature が consume する各 body id について、`features[at..]` 中に同 id を ref する feature がいないか確認。
  - 同 id を ref する後続 feature があり、その feature 以前で再 register されない (= 後続が live と期待した body が新 feature 挿入で消える) → `InsertBeforeConsumer`

`simulate_history` のアルゴリズム:

| feature 種別 | sketches 更新 | live_bodies 更新 |
|--------------|---------------|------------------|
| `CreateSketch` | id → 出現 index | (無し) |
| `CreateBox` / `CreateCylinder` / `CreateSphere` | (無し) | id を live に追加 |
| `Extrude { fuse_target=None }` | (無し) | id を live に追加 |
| `Extrude { fuse_target=Some(t) }` | (無し) | t を live から除去 → id を live に追加 |
| `ExtrudeCut { target=t }` | (無し) | t を live から除去 → id を live に追加 |
| `Cut`/`Fuse`/`Intersect { target=t, tool=u }` | (無し) | t,u を live から除去 → id を live に追加 |

## 設計方針

- **決定性要件**: `FeatureCrud::insert` 自体は決定的なまま (HashMap の iter 順に依存しない設計、エラー判定は探索順固定)。同一入力 → 同一エラー / 同一成功 Document。T01 で検証。
- **B-rep トポロジー妥当性**: N/A (format-level、幾何処理なし)。
- **退化幾何の扱い**: N/A。ただし *論理的退化* として「自己参照 (feature.id == feature.ref)」を `SelfReference` で拒否する (T07_degen)。
- **derive 規約**: 既存 `FeatureCrudError` は `#[derive(Debug, Error)] #[non_exhaustive]` を維持。新 variant 追加は `#[non_exhaustive]` のため SemVer 上 minor 互換。
- **エラーハンドリング**: `thiserror::Error` の派生のみ使用。message は `#[error("...")]` 属性で人間可読 1 行に保つ。各 variant に十分な context (feature_id / ref_id / position) を持たせ、CLI が `format!("insert failed: {e}")` で包装するだけで原因特定可能にする。
- **workspace.dependencies**: 新規依存追加なし (`thiserror` / `engawa-format` は既存)。
- **責務分割**:
  - format-level validation = `Document::validate()` (重複 ID / variable name / ref_plane id) — 不変
  - history semantic validation = `FeatureCrud::insert` 内の新検証段 — 本 Issue 追加
  - geometry-level validation = `build_assembly` の `SketchNotFound` / `BodyNotFound` — 既存維持 (CLI が `entry add` を通さず直接 .engawa を編集した場合の最後の砦)
- **後方互換性**: `#[non_exhaustive]` enum に variant 追加なので semver 互換。既存 inline tests 4 件は `Feature::CreateBox` / `Feature::CreateSphere` (ref 無し) しか使っておらず影響なし。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | 同一 Document + 同一 Feature + 同一 at で `insert` を 2 回呼び、結果 Document の `to_yaml()` がバイト一致 | `assert_eq!(yaml1, yaml2)` |
| T02 | 正常系 | `[CreateSketch(s1), CreateBox(b1)]` に `Extrude(id=e1, sketch=s1)` を at=2 で挿入 → `Ok` | `features.len() == 3` |
| T03 | 異常系 | 空 Document に `Extrude(sketch=unknown)` を at=0 → `SketchNotFound { feature_id, sketch_ref: "unknown" }` | `matches!(err, SketchNotFound { .. })` |
| T04 | 異常系 | 空 Document に `Cut(target=unknown, tool=x)` を at=0 → `BodyNotFound { feature_id, body_ref: "unknown" }` | `matches!(err, BodyNotFound { .. })` |
| T05 | 異常系 (順序) | `[CreateBox(b1), CreateBox(b2)]` に `Cut(target=b1, tool=b2)` を at=0 で挿入 → `InsertBeforeProducer { ref_id: "b1", producer_at: 0, requested_at: 0 }` | `matches!(err, InsertBeforeProducer { .. })` |
| T06 | 異常系 (consumer) | `[CreateBox(b1), CreateBox(b2), Cut(c1, target=b1, tool=b2)]` に `Cut(c2, target=b1, tool=b2)` を at=2 で挿入 → c1 が b1/b2 を live と期待していた → `InsertBeforeConsumer { displaced_feature_id: "c1", consumed_ref: "b1" or "b2", consumer_at: 2, requested_at: 2 }` | `matches!(err, InsertBeforeConsumer { .. })` |
| T07_degen_self_reference | degen | `Cut(id=x, target=x, tool=anything)` を任意の位置に挿入 → `SelfReference { feature_id: "x", ref_kind: "target" }` | `matches!(err, SelfReference { .. })` |
| T08_boundary_at_zero_no_ref | 境界 | 空 Document に `CreateBox(b1)` を at=0 → `Ok` (ref を持たない feature は any 位置で OK) | `Ok` |
| T09 | 互換維持 | 既存の `test_insert_at_end` / `test_insert_out_of_range` / `test_insert_duplicate_id` / `test_insert_at_zero` (inline tests) がそのまま pass | (CI green) |

T09 は inline test の修正なしという negative spec で acceptance test には書かないが、`cargo test -p engawa-build` の既存 4 テストが pass し続けることで担保。

## 幾何的不変条件チェックリスト

- [N/A] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか — geometry を扱わないため N/A
- [N/A] 各プリミティブの face ごとの outer_loop 2D 向き (CW/CCW) が文書化されているか — N/A
- [N/A] flip_normals / same_sense の意味論が明確か — N/A
- [N/A] pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか — N/A
