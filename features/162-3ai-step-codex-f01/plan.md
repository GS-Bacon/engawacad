## 自律判断ログ (B-3 front-load)

本 Issue は #158 STEP 7.5 で Codex r2 (Option B 推奨) と r4 (Option A 推奨) が逆の指摘を出し、Phase 7 では確定できなかった「child Component が空 `ref_planes` の場合の解決ルール」を Phase 8 で正式化するために自動起票された。3 択は (A) parent 継承 / (B) canonical fallback / (C) parent+canonical マージ。

**自律モードの確定方針**: **Option B (canonical fallback) を正式採用**し ADR-014 として記録する。
理由:
1. 既存実装 (`crates/engawa-build/src/lib.rs:497-504`) が既に Option B を実装済み。
2. 既存テスト T15 (`child_component_uses_own_ref_planes`) と T16 (`empty_child_falls_back_to_canonical_not_parent`) が Option B を明示的に regression する。Option A を採ると T16 が壊れ、F01 r2 の指摘 (custom-only parent + child empty + `plane_ref: Front` の UnknownRefPlane) を再発させる。
3. `Document::from_yaml` で root_component の空 `ref_planes` は canonical three に自動補完される (`crates/engawa-format/src/document.rs:68-70`)。各 Component が独立に canonical three を持つので、子 Component の sketch は常に Front/Top/Right を解決できる。
4. Phase 8 で導入する「モデル面上のスケッチ」(model face selection) は ref_planes 継承とは別経路。parent datum を child から参照したい場合は Phase 9+ で `face_ref` のようなトポロジカル参照 (ADR-005) を導入する道があり、ref_planes 継承を急ぐ必要はない。
5. Component の coordinate frame は局所であるべき (assembly モデルの自然な期待)。custom datum は局所宣言する設計とする。

**ADR 起こし**: Phase 8 の正式化として `docs/decisions/014-component-refplane-isolation.md` を新規作成する。ADR-013 (auto-accept) フローにより、次サイクル L-5.6 で `gate:adr-review` が起票され Multi-LLM Review で検証される。

## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| ADR-014 を新規作成 (Option B 正式化 + 設計判断・トレードオフ・代替案・将来拡張点) | parent 継承 (Option A) や merge (Option C) への切り替え |
| `crates/engawa-build/src/lib.rs` の dead パラメータ `_ref_planes` を削除し caller 3 箇所を更新 | `Document::from_yaml` の canonical 自動補完ロジックの変更 |
| `effective_ref_planes` の inline コメントを「F01 r2 fix」表記から ADR-014 参照に置換 | RefPlane 検証 (NaN/Inf offset チェック等、F02 系) — 別 Issue 範囲 |
| T17 retain regression: 親 custom-only + 2 段ネスト孫 component の plane_ref: Front 解決テスト追加 | Phase 9+ の parent datum トポロジカル参照 (face_ref 等) |
| `docs/decisions/013-adr-auto-accept-flow.md` の relation 行に ADR-014 を追加 (cross-link) | examples/*.engawa の親 custom datum パターン追加 |

## Non-Goals
- parent 継承 (Option A) / merge (Option C) は採用しない
- ref_planes 検証 (offset の `is_finite()` 等) は別 Issue
- Phase 8 のモデル面選択 (model face selection) 機能の追加
- examples/ の新規 assembly fixture 追加
- 既存 T15/T16 のリネーム/書き換え (regression として保持)

## 実装対象
- Issue: #162
- 影響ファイル:
  - 新規: `docs/decisions/014-component-refplane-isolation.md` (Claude が直接 Write)
  - `crates/engawa-build/src/lib.rs` (GLM dispatch)
  - `crates/engawa-build/tests/refplane_acceptance.rs` (T17 追加、GLM dispatch)

### 既存関数の修正 (before/after)

#### `crates/engawa-build/src/lib.rs:463-471` build_component_tree シグネチャ

**Before**:
```rust
#[allow(clippy::too_many_arguments)]
fn build_component_tree(
    component: &Component,
    base_dir: &Path,
    visiting: &mut Vec<PathBuf>,
    depth: usize,
    accumulated_offset: Vec3,
    accumulated_rotation: [[f64; 3]; 3],
    gen: &mut IdGenerator,
    out: &mut Vec<Body>,
    _ref_planes: &[RefPlane], // Kept for signature compatibility; unused per F01 r2 fix
) -> Result<(), KernelError> {
```

**After**:
```rust
fn build_component_tree(
    component: &Component,
    base_dir: &Path,
    visiting: &mut Vec<PathBuf>,
    depth: usize,
    accumulated_offset: Vec3,
    accumulated_rotation: [[f64; 3]; 3],
    gen: &mut IdGenerator,
    out: &mut Vec<Body>,
) -> Result<(), KernelError> {
```

(`#[allow(clippy::too_many_arguments)]` も引数 8 個なので削除可能。clippy 既定上限は 7 なので残す方向で GLM に判断委ねる。clippy::too_many_arguments が出たら維持。)

#### `crates/engawa-build/src/lib.rs:497-504` effective_ref_planes ブロック

**Before**:
```rust
    // 1. Build this component's own features and apply total transform.
    //
    // child Component が自前の ref_planes を持つ場合はそれを優先 (= child ローカル plane_ref id が
    // 解決される)。空の Component は親を継承せず、ローカルでデフォルト 3 件を独立に解決する。
    // 親が custom-only ref_planes を持つ場合に child の plane_ref: "Front" が解決失敗するのを防ぐため。
    let canonical = RefPlane::default_canonical_three();
    let effective_ref_planes: &[RefPlane] = if !component.ref_planes.is_empty() {
        component.ref_planes.as_slice()
    } else {
        canonical.as_slice()
    };
```

**After**:
```rust
    // 1. Build this component's own features and apply total transform.
    //
    // ADR-014: 各 Component は独立した coordinate frame を持ち、空の `ref_planes` は親を継承せず
    // canonical three (Front/Top/Right) にフォールバックする。child の `plane_ref: "Front"` は親が
    // custom-only ref_planes であっても常に解決可能。custom datum を child で使うには child 自身に
    // 宣言する。
    let canonical = RefPlane::default_canonical_three();
    let effective_ref_planes: &[RefPlane] = if !component.ref_planes.is_empty() {
        component.ref_planes.as_slice()
    } else {
        canonical.as_slice()
    };
```

#### `crates/engawa-build/src/lib.rs:441-453` build_assembly 呼び出し

**Before**:
```rust
    let ref_planes = doc.root_component.ref_planes.as_slice();
    build_component_tree(
        &doc.root_component,
        base_dir,
        &mut visiting,
        0,
        Vec3::zeros(),
        IDENTITY3,
        gen,
        &mut bodies,
        ref_planes,
    )?;
```

**After**:
```rust
    build_component_tree(
        &doc.root_component,
        base_dir,
        &mut visiting,
        0,
        Vec3::zeros(),
        IDENTITY3,
        gen,
        &mut bodies,
    )?;
```

#### `crates/engawa-build/src/lib.rs:543-554` reference subtree 再帰呼び出し

**Before**:
```rust
        build_component_tree(
            &ref_doc.root_component,
            &child_base_dir,
            visiting,
            depth + 1,
            total_offset,
            total_rotation,
            gen,
            out,
            &ref_doc.root_component.ref_planes,
        )?;
```

**After**:
```rust
        build_component_tree(
            &ref_doc.root_component,
            &child_base_dir,
            visiting,
            depth + 1,
            total_offset,
            total_rotation,
            gen,
            out,
        )?;
```

#### `crates/engawa-build/src/lib.rs:560-571` inline child 再帰呼び出し

**Before**:
```rust
    // 再帰側で各 child.ref_planes が空なら親 (= 今 effective_ref_planes) を継承する。
    for child in &component.children {
        build_component_tree(
            child,
            base_dir,
            visiting,
            depth,
            total_offset,
            total_rotation,
            gen,
            out,
            effective_ref_planes,
        )?;
    }
```

**After**:
```rust
    // 各 child は独立した coordinate frame (ADR-014) を持ち、自身の effective_ref_planes を導出する。
    for child in &component.children {
        build_component_tree(
            child,
            base_dir,
            visiting,
            depth,
            total_offset,
            total_rotation,
            gen,
            out,
        )?;
    }
```

注: clippy::too_many_arguments の `#[allow]` は引数が 9→8 になるので維持/削除どちらでも可。GLM に「clippy --workspace -- -D warnings green の範囲で判断」と指示する。

## 設計方針
- **決定性要件**: パラメータ削除は呼び出しシグネチャの単純化のみで、生成される EntityID / Body の順序・値は変わらない。T01 (determinism) で 2 回実行同一性を確認。
- **B-rep トポロジー妥当性**: シグネチャ変更のみで topology 構築ロジックは無変更。Euler 不変は維持。
- **退化幾何の扱い**: 該当なし (param 削除のみ)
- **derive 規約**: 該当なし
- **エラーハンドリング**: 該当なし
- **workspace.dependencies**: 変更なし

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | 既存 examples/two_bodies.engawa を 2 回 build し bodies の Vec が deep-equal | assert_eq! pass |
| T15 | regression (既存) | child explicit ref_planes が parent canonical に優先される | 変更なし pass |
| T16 | regression (既存) | parent custom-only + child empty → child は canonical three fallback | 変更なし pass |
| T17_degen_grandchild_canonical_fallback | 退化/深ネスト | 親 custom-only + 中間 child custom-only + 孫 empty で plane_ref: "Front" を解決 | 全 Component で canonical fallback、build_assembly success |

## 幾何的不変条件チェックリスト
- N/A (param signature cleanup のみで topology 操作は無変更)
