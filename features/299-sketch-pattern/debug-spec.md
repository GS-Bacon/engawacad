# debug-spec — Issue #299 STEP 7.5 / 6.7 合流後の修正指示

STEP 6.7 (Claude self-review) と STEP 7.5 (Codex final gate) の指摘を Claude が採否判定した結果、
**`crates/**/src/` に 2 件の修正**が必要になった。**受け入れテストは Claude が既に追加済み**
(現時点で 5 テストが赤)。この spec の実装だけを行い、テストを green にすること。

**重要**: プランの再実装は不要。既存実装は CI green で完成している。以下の 2 点**のみ**を変更する。
テストファイル (`crates/engawa-build/tests/*.rs`) は Claude が確定させたので**変更しないこと**
(コンパイルエラーの修正が必要な場合を除く)。

## 現在赤いテスト (この spec の実装で green になるべきもの)

| テスト | ファイル | 対応する修正 |
|---|---|---|
| `t_crud_pattern_edit_breaks_selection_linear` | `crates/engawa-build/tests/sketch_pattern_acceptance.rs` | 修正 1 |
| `t_crud_pattern_edit_breaks_selection_circular` | 同上 | 修正 1 |
| `t11_crud_gate_edit_breaks_mirror_selection` | `crates/engawa-build/tests/sketch_mirror_acceptance.rs` | 修正 1 |
| `t_deg_count_too_large` | `crates/engawa-build/tests/sketch_pattern_acceptance.rs` | 修正 2 |
| `t_boundary_count_at_max` | 同上 | 修正 2 |

加えて、既に green のもので **回帰させてはいけない**もの:
`t_crud_pattern_edit_n1_selection_ignored` / `t_crud_pattern_n1_no_validation` /
`t_known_limitation_mirror_derived_elem_false_reject` / `t10_crud_gate_rejects_rename_breaking_fillet`。

---

## 修正 1: `refs_resolve_in_state` の Mirror / Pattern arm を element-level 検証にする

**ファイル**: `crates/engawa-build/src/feature_crud.rs`

**根本原因** (Codex A01, high): `refs_resolve_in_state` の `SketchMirror` / `SketchPatternLinear` /
`SketchPatternCircular` の各 arm が「sketch id が `sketches_at` にあるか」だけを見ている。
そのため `FeatureCrud::edit` で `CreateSketch` の要素を rename / 削除しても
`simulate_history` 上は consumer が実行中のままになり、`check_edit_preserves_consumers` が
`EditBreaksConsumer` を出さない。edit は成功し、後で `build_bodies_from_features` を呼んだ時に
初めて `sketch_{mirror,pattern_*}_unknown_element_id` で落ちる (false-accept)。

`SketchFillet` / `SketchChamfer` の arm は既に element-level 検証をしており
(`sketch_fillet_acceptance.rs::t10_crud_gate_rejects_rename_breaking_fillet` がその契約を固定)、
Mirror / Pattern だけが同じ file 内で契約から外れている。insert 経路
(`check_refs_resolve_before` の element-level gate, 851-959 行付近) は既に selection を検証済みなので、
**insert 経路と edit 経路の判定基準を揃える**のがこの修正の趣旨。

### 実装

1. `refs_resolve_in_state` の直前 (または `collect_named_feature_ids` の近く) に helper を追加する:

```rust
/// Resolve a `selection` of sketch element ids against the *original* `CreateSketch.profile`
/// that `sketch` refers to.
///
/// Empty selection means "all elements" and always resolves (matches the SketchOffset
/// precedent and the insert-path element-level gate in `check_refs_resolve_before`).
///
/// This keeps `refs_resolve_in_state` (edit/suppress/delete/reorder path) in agreement with
/// the insert-path gate: without it, `FeatureCrud::edit` could rename or remove an element a
/// downstream SketchMirror/SketchPattern* `selection` depends on without tripping
/// `EditBreaksConsumer`, and the breakage would only surface later in
/// `build_bodies_from_features` (Codex #299 STEP 7.5 A01, false-accept-on-edit).
///
/// Known limitation (unchanged, tracked in #331): the check is against the ORIGINAL
/// `CreateSketch.profile`, not the current profile that preceding sketch-edit features
/// (Fillet/Chamfer/Offset/Mirror/Pattern) would have produced. A selection naming such a
/// derived id is therefore treated as unresolved here — the same false-reject already
/// documented for the insert path.
fn sketch_selection_resolves(
    features: &[Feature],
    sketches_at: &HashMap<String, usize>,
    sketch: &str,
    selection: &[String],
) -> bool {
    let Some(&idx) = sketches_at.get(sketch) else {
        return false;
    };
    if selection.is_empty() {
        return true;
    }
    match &features[idx] {
        Feature::CreateSketch { profile, .. } => {
            let ids: std::collections::HashSet<&str> = profile.iter().map(element_id_of).collect();
            selection.iter().all(|s| ids.contains(s.as_str()))
        }
        _ => false,
    }
}
```

2. `refs_resolve_in_state` の以下の 2 arm (現在 335-339 行付近) を置き換える:

```rust
        Feature::SketchMirror { sketch, .. } => sketches_at.contains_key(sketch.as_str()),
        Feature::SketchPatternLinear { sketch, .. }
        | Feature::SketchPatternCircular { sketch, .. } => {
            sketches_at.contains_key(sketch.as_str())
        }
```

を、次に差し替える:

```rust
        Feature::SketchMirror {
            sketch, selection, ..
        } => sketch_selection_resolves(features, sketches_at, sketch, selection),
        Feature::SketchPatternLinear {
            sketch,
            selection,
            count,
            ..
        }
        | Feature::SketchPatternCircular {
            sketch,
            selection,
            count,
            ..
        } => {
            // count<=1 は kernel 側が `selection` を一切参照しない no-op (count=0 はエラー、
            // count=1 は source をそのまま返す)。insert 経路の element-level gate と同じく
            // `count >= 2` のときだけ selection を検証する (Codex #299 R01 と整合)。
            if *count >= 2 {
                sketch_selection_resolves(features, sketches_at, sketch, selection)
            } else {
                sketches_at.contains_key(sketch.as_str())
            }
        }
```

3. `refs_resolve_in_state` の doc comment (221-231 行付近) に 1 行追記する:
   「SketchMirror / SketchPattern* は sketch の存在に加えて `selection` の各 id が
   元 `CreateSketch.profile` で解決することも要求する (Pattern は `count >= 2` のときのみ)」。

### やらないこと (Claude が意図的に棄却した範囲)

- Codex suggestion の後半「`selection=[]` の場合に profile が unsupported element kind
  (Ellipse/Conic) を含んでいたら reject する」は **実装しない**。これは edit 固有ではなく
  insert 経路にも同じだけ存在する別軸の false-accept で、insert gate も同時に変えないと
  逆向きの非対称を生む。#331 の横断対応に委譲する。
- `refs_resolve_in_state` を current profile 基準 (先行 sketch-edit を逐次適用した profile) に
  する根本修正も **行わない** (#331 の範囲)。

---

## 修正 2: `count` の上限ガード `MAX_PATTERN_COUNT`

**ファイル**: `crates/engawa-kernel/src/geometry/sketch_pattern.rs`

**根本原因** (self-review A1, medium): `count: u32` に上限が無く、`build_bodies_from_features` は
`.engawa` の値をそのまま渡す。`count: 30000000` のような桁ミス 1 つで数千万要素を生成して
OOM kill される (self-review が `count = 200_000` で実測済み)。Pattern は「繰り返し回数」
パラメータを持つ最初の Feature で、リポジトリに前例となる上限定数が無い。

### 実装

1. モジュール先頭 (`use` 群の直後) に定数を追加する:

```rust
/// Upper bound on the TOTAL instance count of a single pattern feature.
///
/// `count` comes straight from the `.engawa` document, so a single digit slip
/// (`count: 30000000`) would otherwise allocate tens of millions of `SketchElement`s and
/// OOM-kill the process. 10_000 instances of one element is already far beyond any
/// plausible 2D sketch, so the cap only ever fires on input errors. Note the total output
/// size is `selected_elements * count`, i.e. the cap bounds the multiplier, not the profile.
pub const MAX_PATTERN_COUNT: u32 = 10_000;
```

2. `apply_sketch_pattern_linear`: 既存の `count == 0` チェックの**直後**、`count == 1` の
   early return の**前**に追加する:

```rust
    if count > MAX_PATTERN_COUNT {
        return Err(KernelError::InvalidParameter {
            kind: "sketch_pattern_linear_count_too_large",
        });
    }
```

3. `apply_sketch_pattern_circular`: 同じ位置に、kind を
   `"sketch_pattern_circular_count_too_large"` にして追加する。

4. モジュール doc の `# Errors` 節に 2 行追記する:

```
//! - `InvalidParameter { kind: "sketch_pattern_linear_count_too_large" }` — count exceeds
//!   `MAX_PATTERN_COUNT` (resource guard against a mistyped count)
```

Circular 側は既存の「Mirror-image errors exist for Circular ...」行でカバーされるので、
その行に `count_too_large` も含まれる旨を足すだけでよい。

5. 同ファイルの doc の `count=1` に関する記述を 1 箇所だけ精緻化する (self-review C2)。現状:

```
//! `count` is the TOTAL instance count
//! INCLUDING the original (so `count=1` is a true no-op and does not even consult
//! `selection`; `count=0` is an error).
```

「`selection` を参照しない」のは正しいが、`direction` / `center` / `total_angle` / `distance` の
finite・退化チェックは `count` 判定より前に走るため「完全な no-op」ではない。
次のように直す:

```
//! `count` is the TOTAL instance count
//! INCLUDING the original (`count=0` is an error). At `count=1` no copies are produced and
//! `selection` is not consulted at all, but the parameter checks that precede the count
//! branch (finite/degenerate `direction`, `center`, `total_angle`) still apply.
```

`crates/engawa-format/src/feature.rs` の `SketchPatternLinear` / `SketchPatternCircular` の
doc comment にも「count=1 → no-op」の記述があるが、そちらは**変更不要**
(variant doc は概要で十分)。

---

## 完了条件

1. 上記 2 修正を実装する。**それ以外の機能追加・リファクタは行わない**
2. `cargo xtask ci` が green (build / test / clippy -D warnings / fmt --check)
3. 上表の 5 テストが pass し、既存テストが 1 件も落ちていないこと
4. 結果を `features/299-sketch-pattern/glm-fix-result.json` に JSON で書き出す
