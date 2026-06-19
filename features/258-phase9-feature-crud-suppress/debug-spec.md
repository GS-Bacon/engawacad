# Debug Spec — #258 phase9-feature-crud-suppress (round 7, post-Codex)

## 試した修正と結果 (round 1-6)

- r1-r6: schema 拡張 + simulate skip + suppress fn + cli 実装 + 全 491 既存 fixture 修正 → CI green
- Codex r1: critical 1 + high 2 = blocking 3 を指摘

## 仮説

### 1. C-F01 (high): build_component_tree が全 suppress component で EmptyFeatureList 落ちる

GLM r1-r6 で `build_assembly`/`build_component_tree` に `if feature.is_suppressed() { continue; }` を追加したが、suppressed feature を skip した後の `built.is_empty()` 判定が依然として `EmptyFeatureList` エラーになる。component の全 feature が suppress された場合、empty 成功 (= 空 component) を返すべき。

### 2. C-F02 (high): restore (suppress(false)) で復帰 feature の ref 再検証なし

`FeatureCrud::suppress` で `on=false` (restore) の場合、現実装は `check_edit_preserves_consumers` のみ呼び、復帰させる feature 自身の `check_self_reference` / `check_refs_resolve_before_for_edit` を再実行していない。API or 手編集で `suppressed: true` の壊れた Extrude (sketch ref が前に存在しない) を持つ document を `entry restore` に通すと、検証なしで成功扱いで書かれる。

### 3. M-F01 (critical): hallucination — source-of-truth 同梱なしという主張

Codex は staged diff を見て「`web/src/generated/Feature.ts` の差分だけで `engawa-format/src/feature.rs` の Feature 定義更新がない」と主張しているが、これは **誤認**。`git status` で確認: `crates/engawa-format/src/feature.rs` は staged 済みで suppressed フィールド追加されている (L285+)。Codex のレビュー対象 diff には Rust source も含まれている。

TEST SUMMARY `total_added=0` は `extract-test-summary.ts` が main branch 不在で `git diff base...HEAD` を空 diff 計算する工程問題。本 Issue 範囲外。

**M-F01 → 棄却** (hallucination + 工程問題)。

## 関連ファイル

- `crates/engawa-build/src/lib.rs` (build_assembly / build_component_tree の EmptyFeatureList 判定)
- `crates/engawa-build/src/feature_crud.rs` (FeatureCrud::suppress の restore path)
- `crates/engawa-format/src/feature.rs` (source-of-truth、すでに suppressed フィールド追加済み)

## 修正方針

### C-F01: build empty path

`crates/engawa-build/src/lib.rs` の `build_assembly` or `build_component_tree` 内で、`built` が `EmptyFeatureList` を返す前に「全 suppress による empty か否か」をチェックし、suppress 由来の空なら成功 (空 component / 空 bodies) を返す:

```rust
// before (推定):
let built = build_bodies_from_features(features)?;
if built.is_empty() {
    return Err(BuildError::EmptyFeatureList);
}
// after:
let non_suppressed_count = features.iter().filter(|f| !f.is_suppressed()).count();
let built = build_bodies_from_features(features)?;
if built.is_empty() && non_suppressed_count > 0 {
    return Err(BuildError::EmptyFeatureList);
}
// 全 suppress (non_suppressed_count == 0) なら built も空でよい (空 component)
```

正確な箇所は GLM が `EmptyFeatureList` を grep して特定し、上記 semantic に修正する。

回帰テスト: `crates/engawa-build/tests/258_phase9_feature_crud_suppress_acceptance.rs` に追加
```rust
#[test]
fn t09_all_suppressed_component_builds_empty() {
    // 全 feature suppress された component で build_assembly が成功 (空 bodies) を返すこと
    todo!("GLM 実装")
}
```

### C-F02: restore で ref 再検証

`FeatureCrud::suppress` 内で `on=false` 時:
```rust
pub fn suppress(doc: &Document, feature_id: &str, on: bool) -> Result<Document, FeatureCrudError> {
    let idx = ...;
    let mut new_feature = doc.root_component.features[idx].clone();
    set_feature_suppressed(&mut new_feature, on);
    if !on {
        // restore path: verify the feature's own refs resolve against the prefix (excluding old)
        // 注: suppressed=false の new_feature を idx 位置に挿入する semantics で edit と同じ
        let mut without_old: Vec<Feature> = doc.root_component.features.clone();
        without_old.remove(idx);
        check_self_reference(&new_feature)?;
        check_refs_resolve_before_for_edit(&new_feature, &doc.root_component.features, idx)?;
    }
    check_edit_preserves_consumers(&new_feature, &doc.root_component.features, idx)?;
    let mut next = doc.clone();
    next.root_component.features[idx] = new_feature;
    next.validate()?;
    Ok(next)
}
```

回帰テスト追加:
```rust
#[test]
fn t10_restore_invalid_feature_rejected() {
    // suppressed=true の壊れた Extrude (sketch ref 前に存在せず) を持つ doc を restore →
    // InsertBeforeProducer or SketchNotFound エラー
    todo!("GLM 実装")
}
```

### M-F01: 棄却 (再 codex に「Rust source も同梱、Feature.ts は gen-ts 出力」と明記)

debug-spec に「M-F01 は Rust source 同梱の確認漏れによる hallucination。`git status` で `crates/engawa-format/src/feature.rs` は staged 済み」を明記し、codex-input に追記して再 review で重複指摘を防ぐ。

## 次にやること

GLM で C-F01 + C-F02 + 関連 回帰テスト 2 件 を実装。M-F01 は棄却。`cargo xtask ci` green 確認。

## Round 8 (Codex r2 fix)

### 試した修正と結果 (round 7)
- C-F01: build_assembly empty 判定に `non_suppressed_count > 0` 条件追加 → r2 で M-F01 として再指摘 (条件が不十分)
- C-F02: restore 時に `check_self_reference` + `check_refs_resolve_before_for_edit` 追加 → r2 で A-F01 として PlaneRef Entity 完全解決の追加要求

### Codex r2 残指摘の対応方針

#### A-F01 (high): PlaneRef::Entity 完全解決検証 → **#270 に統合 deferred**
restore 時の `check_refs_resolve_before_for_edit` は `Named.feature_id` 生存性しか見ない。face role 不一致や非平面 face 変更を検出できない → 後段 build で `FaceEntityRefNotFound` / `FaceNotPlanar`。これは **#256 で deferred した #270 と完全同じ論点** (PlaneRef::Entity 完全解決検証)。本 Issue scope を超えるため #270 に統合して deferred。

GLM は A-F01 への追加対応は **不要**。

#### M-F01 (high): body producer 0 case → 採用、修正

`[CreateSketch, Extrude(suppressed)]` のように **body producer が全 suppress** されたが CreateSketch などの非 body-producer が残るケースで、`built.is_empty()` 判定が `EmptyFeatureList` で落ちる。

修正: empty 成功条件を「**active な body producer が 0**」に拡張:

```rust
// 推定箇所: build_assembly or build_component_tree
// before (round 7 修正後):
let non_suppressed_count = features.iter().filter(|f| !f.is_suppressed()).count();
let built = build_bodies_from_features(features)?;
if built.is_empty() && non_suppressed_count > 0 {
    return Err(BuildError::EmptyFeatureList);
}

// after (round 8 修正):
let active_body_producers = features.iter().filter(|f| !f.is_suppressed() && is_body_producer(f)).count();
let built = build_bodies_from_features(features)?;
if built.is_empty() && active_body_producers > 0 {
    return Err(BuildError::EmptyFeatureList);
}
// active body producer が 0 (sketch だけ or 全 suppress) なら built も空で OK
```

helper:
```rust
fn is_body_producer(f: &Feature) -> bool {
    matches!(f, 
        Feature::CreateBox { .. } | Feature::CreateCylinder { .. } | Feature::CreateSphere { .. }
        | Feature::Extrude { .. } | Feature::ExtrudeCut { .. }
        | Feature::Cut { .. } | Feature::Fuse { .. } | Feature::Intersect { .. }
    )
}
```

回帰テスト追加:
```rust
#[test]
fn t11_body_producer_suppressed_with_sketch_builds_empty() {
    // [CreateSketch(sk_1), Extrude(suppressed=true)] → build_assembly が空 bodies で成功
    todo!("GLM 実装")
}
```

## 次にやること (round 8)

GLM で M-F01 のみ修正 (A-F01 は #270 統合 deferred)。`cargo xtask ci` green 確認。
