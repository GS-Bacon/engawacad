# plan: #296 Sketch Fillet を engawa-format / engawa-build に実装

## 自律判断ログ (autonomous mode, --issue 296 --autonomous)

- Issue 本文の `SketchEdit::SketchFillet { elem1_id, elem2_id, radius }` は ADR-017 §3 表 (`SketchFillet { sketch_ref, vertex_ref: (e1_id, e2_id), radius }`) と概念的に一致するが命名が異なる。ADR-017 は accepted 済で authoritative だが、`vertex_ref` タプル表現より、#295 (SketchOffset) が確立した規約および既存 `Feature::Fuse{target,tool}` / `Feature::Cut{target,tool}` 式の**フラットな2フィールド命名**の方が既存コードと整合するため、`elem1_id: String, elem2_id: String` (Issue 本文の命名をそのまま採用) を選ぶ。
- `sketch_ref: EntityRef` は #295 で Codex architect が「EntityRef は B-rep Face/Edge/Vertex 専用」として refute 済み (ADR-005 契約)。#295 と同じ理由・同じ解決 (`sketch: String`、既存 `Feature::Extrude.sketch` 慣習) を継承する。
- 「適用に伴う元 element の Trim」は ADR-017 §3 の generic `SketchTrim` Feature (#276 → 子 #307、まだ未実装) には依存しない。Fillet 自身の純関数内で指定された2 element (`elem1_id`/`elem2_id`) をその場で短縮する局所操作であり、`SketchTrim` (任意点でのユーザー指定 Trim) とは別物。#307/#308 が未実装でも本 Issue のブロッカーにならない。
- #295 は build-level 契約を「Circle 単一要素のみ」に絞った (profile 全体の Line/Arc corner join が Trim/Extend #276 待ちのため)。本 Issue の Fillet は **profile 全体の corner join** ではなく **指定された2 Line 間のローカルな corner 処理**なので、#276 を待たずに Line-Line ペアの build 経路を実装できる (Fillet 自身が trim ロジックを内包するため、Offset とは事情が異なる)。
- Arc-Arc / Line-Arc / Circle を含む fillet は正接円構築問題で数式が別になるため Out-of-Scope とする (Line-Line のみ In-Scope。親 #277 のテスト仕様 T04 "L字 line ペアに半径 1.0 fillet" に一致)。

## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| `Feature::SketchFillet { id, sketch, elem1_id, elem2_id, radius, suppressed }` の YAML 型定義 + serde tagged (`type: sketch_fillet`) | Sketch Chamfer (#297) — 別 Issue |
| serialize → deserialize roundtrip | Sketch Trim / Extend (#307/#308) — 別 Issue、本 Issue はそれらに依存しない |
| **Line-Line のみ**: profile 内で隣接する (配列上で連続、または閉ループの末尾↔先頭) 2 つの `SketchElement::Line` への fillet 適用 (kernel + build 経路の両方) | Arc-Arc / Line-Arc / Circle を含む fillet (正接円構築問題、別 ADR/Issue) |
| corner (共有端点) の検出、tangent point 計算、fillet 用 Arc の挿入、両 Line の trim (endpoint 差し替え) | Ellipse/Conic に対する fillet |
| `radius` 過大時の `KernelError::FilletRadiusTooLarge { elem1_id, elem2_id, radius }` (新規 error variant、issue 完了条件で名指し) | 3 要素以上をまたぐ fillet chain (1 回の Feature 呼び出しは常に2 element ペアのみ) |
| corner 角度退化 (ほぼ0° または ほぼ180°) 時の `DegenerateSketchElement` | Fillet 後の Arc が profile 内の他の遠い要素と self-intersect する一般判定 (2 element 自身の長さチェックのみ、下記 SCOPE DEFENSE 参照) |
| `engawa-build` dispatch: `built_sketch_profiles` (#295 で導入済み) を読み書きする配線 (Offset と同じ HashMap を共有する dispatch まで。Offset が fillet 済み profile を実際に処理できるようになることは含まない — `apply_sketch_offset_build` の Circle-only guard が先に効くため) | 履歴ロールバック / 削除時の SketchRef 解決失敗経路 (別 Issue) |
| `examples/sketch_fillet.engawa` 追加 (矩形の1角を fillet) + `examples_smoke.rs` に entry | 3D fillet (本 Issue は sketch = 2D 平面内) |
| Golden YAML fixture 1件 (CreateSketch → SketchFillet → Extrude で fillet が反映されることを assert) | `CURRENT_SCHEMA_VERSION` の bump (加算的 variant 追加のみ、#295 Non-Goals と同理由) |

## Non-Goals
- Sketch Chamfer (#297)、Sketch Trim / Extend (#307/#308) — 別 Issue で個別実装 (本 Issue はそれらに依存しない、並行実装可)
- Arc-Arc / Line-Arc / Circle / Ellipse / Conic を含む fillet (正接円構築の数式が別、独立 Issue で提案)
- 3要素以上の fillet chain、複数コーナーの一括 fillet
- 一般的な `SelfIntersection` 判定 (本 Issue は `elem1`/`elem2` 自身の長さ制約のみで代替、SCOPE DEFENSE 節参照)
- Fillet 適用後の profile に対する再 fillet (elem1_id/elem2_id は常に Line である前提。既に fillet 済みの Arc を挟んだ状態からの再 fillet は対象外)
- `CURRENT_SCHEMA_VERSION` の bump (#295 Non-Goals と同理由: 加算的 variant 追加は breaking ではない)
- Property test 網羅 (本 Issue は fixed input のテストのみ、Refactor Pass で追加)
- 負 sweep Arc に対する `sketch_offset` 対応 (`offset_arc` の `arc_negative_sweep` 拒否の緩和)。現状 Circle-only guard により到達不能な潜在衝突であり、#295 の guard が外れる Phase 11+ で扱う

### SCOPE DEFENSE: 一般 SelfIntersection → 2-element 長さ制約で代替

親 #277 の完了条件は fillet 適用時の一般的な自己交差を明示していないが、念のため明記する:

- `FilletRadiusTooLarge` は `elem1`/`elem2` **自身の長さ**に対する tangent length (`t = radius / tan(theta/2)`) の超過のみを検出する。fillet の Arc が profile 内の **他の遠い要素**と交差するケース (非常に大きい radius で profile 全体を覆うような病的入力) は検出しない。
- 一般的な self-intersection 判定は Sketch 全体の 2D 交差計算が必要で、#295 が Trim/Extend 待ちとして先送りした scope と同種の実装量。本 Issue でも同じ理由で先送りする。
- `elem1`/`elem2` 自身の長さ制約は fillet の必要条件であり、CAD 実務上最も頻出するケース (角に対して大きすぎる半径を指定するミス) を捕捉する。

## 実装対象
- 影響クレート/ファイル:
  - `crates/engawa-format/src/feature.rs` — `Feature::SketchFillet` variant 追加、`id()` / `is_suppressed()` match arm 拡張、roundtrip test 追加
  - `crates/engawa-kernel/src/error.rs` — 新規 `KernelError::FilletRadiusTooLarge { elem1_id, elem2_id, radius }` を追加 (issue 完了条件で明示的に要求されている名称)
  - `crates/engawa-kernel/src/geometry/sketch_fillet.rs` (新規、`sketch_offset.rs` の構成を踏襲) — `compute_fillet` (element-level 純関数) + `apply_sketch_fillet_build` (profile-level)
  - `crates/engawa-kernel/src/geometry/mod.rs` (or 該当の re-export 箇所) — 新規モジュール `pub mod sketch_fillet;` の追加
  - `crates/engawa-build/src/lib.rs` — `Feature::SketchFillet` dispatch 追加 (`built_sketch_profiles` 読み書き、#295 と同じ合成パターン)
  - `crates/engawa-build/src/feature_crud.rs` — `feature_sketch_refs` / `feature_variant_name` / `set_feature_suppressed` / `refs_resolve_in_state` / `check_refs_resolve_before` / `simulate_history` の match arm 拡張 (`Feature::SketchOffset` の対応箇所 L104, L133, L233, L395, L595, L880 相当)。`set_feature_suppressed` (L869-884) は `_` arm を持たない exhaustive match なので arm 追加が**必須** (漏れるとコンパイル不可、Codex R01 で発見)。加えて `FeatureCrudError` に新 variant `SketchElementNotResolved` を追加 (`#[non_exhaustive]` なので加算的変更)
  - `examples/sketch_fillet.engawa` — 新規 fixture (矩形4 Line の1角を fillet)。**STEP 6 で SketchFillet 実装と同時に追加すること** (STEP 5.5 で先行作成を試みたが `crates/engawa-format/src/document.rs` の全 example スキャンテスト (`test_all_example_files_have_schema_version` / `test_ts_derive_backward_compat`) が未実装 variant を含む YAML で即失敗するため、#295 Offset 同様に実装完了後に追加する運用とする)
  - `crates/engawa-build/tests/examples_smoke.rs` — 上記 fixture の smoke entry 追加 (同じく STEP 6 で追加)
  - `crates/engawa-build/tests/sketch_fillet_acceptance.rs` (新規) — acceptance test skeleton (STEP 5.5 で作成済み、26 関数中 3 つは kernel/format 委譲の空 pass、23 つは `#[ignore]` 付き todo!()) → STEP 6 で本実装

- 変更する型・関数のシグネチャ:

```rust
// engawa-format/src/feature.rs
pub enum Feature {
    // ... 既存 variant ...
    #[serde(rename = "sketch_fillet")]
    SketchFillet {
        id: String,
        /// 対象 CreateSketch.id (ADR-017 §3 の sketch_ref を #295 踏襲で String に変更)
        sketch: String,
        /// corner を共有する2 element の ID (配列内の位置で前後を正規化、入力順序は結果に影響しない)
        elem1_id: String,
        elem2_id: String,
        /// fillet 半径 (正値)
        radius: f64,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        suppressed: bool,
    },
}
```

- `Feature::id()` / `Feature::is_suppressed()` の match arm に `Feature::SketchFillet { id, .. }` / `Feature::SketchFillet { suppressed, .. }` を追加。

- 新規 kernel module (`crates/engawa-kernel/src/geometry/sketch_fillet.rs`、`sketch_offset.rs` の構成を踏襲):

```rust
use crate::error::KernelError;
use crate::geometry::math::{LENGTH_TOLERANCE, ANGLE_TOLERANCE};
use engawa_format::SketchElement;

/// Element-level pure function: corner を trim + Arc 挿入で fillet 化する。
/// `elem_a`/`elem_b` は profile 配列上で連続する順序 (elem_a.to == elem_b.from == corner) を前提とする。
/// 呼び出し側 (`apply_sketch_fillet_build`) が elem1_id/elem2_id の入力順序を配列順に正規化してから渡す。
///
/// Returns: (trimmed_a, new_arc, trimmed_b) — 元の配列順 (a, b) を保った3要素。
///
/// # Errors
/// - `UnsupportedFeature { kind: "sketch_fillet_only_line_line" }` — a/b どちらかが Line でない
/// - `InvalidParameter { kind: "sketch_fillet_no_shared_corner" }` — a.to != b.from (LENGTH_TOLERANCE 超)
/// - `InvalidParameter { kind: "radius" }` — radius が非正 or non-finite
/// - `DegenerateSketchElement { reason: "fillet_zero_length_input_line" | "fillet_corner_angle_degenerate" }`
/// - `FilletRadiusTooLarge { elem1_id, elem2_id, radius }` — tangent length が a または b の長さを超過
pub fn compute_fillet(
    elem_a: &SketchElement,
    elem_b: &SketchElement,
    radius: f64,
) -> Result<(SketchElement, SketchElement, SketchElement), KernelError> {
    let (a_id, a_from, a_to) = as_line(elem_a)?;
    let (b_id, b_from, b_to) = as_line(elem_b)?;

    if !radius.is_finite() || radius <= LENGTH_TOLERANCE {
        return Err(KernelError::InvalidParameter { kind: "radius" });
    }

    if dist(a_to, b_from) > LENGTH_TOLERANCE {
        return Err(KernelError::InvalidParameter { kind: "sketch_fillet_no_shared_corner" });
    }
    let corner = a_to; // == b_from

    let len_a = dist(a_from, a_to);
    let len_b = dist(b_from, b_to);
    if len_a <= LENGTH_TOLERANCE || len_b <= LENGTH_TOLERANCE {
        return Err(KernelError::DegenerateSketchElement {
            element_id: a_id.to_string(),
            reason: "fillet_zero_length_input_line",
        });
    }

    let d_a = normalize(sub(a_from, corner)); // corner→far_a 方向
    let d_b = normalize(sub(b_to, corner));   // corner→far_b 方向

    let theta = dot(d_a, d_b).clamp(-1.0, 1.0).acos();
    if theta <= ANGLE_TOLERANCE || theta >= std::f64::consts::PI - ANGLE_TOLERANCE {
        return Err(KernelError::DegenerateSketchElement {
            element_id: format!("{a_id}_{b_id}"),
            reason: "fillet_corner_angle_degenerate",
        });
    }

    let t = radius / (theta / 2.0).tan();
    if t > len_a - LENGTH_TOLERANCE || t > len_b - LENGTH_TOLERANCE {
        return Err(KernelError::FilletRadiusTooLarge {
            elem1_id: a_id.to_string(),
            elem2_id: b_id.to_string(),
            radius,
        });
    }

    let tangent_a = add(corner, scale(d_a, t));
    let tangent_b = add(corner, scale(d_b, t));

    let center_dist = radius / (theta / 2.0).sin();
    let bisector = normalize(add(d_a, d_b));
    let center = add(corner, scale(bisector, center_dist));

    let angle_a = angle_of(sub(tangent_a, center));
    let angle_b = angle_of(sub(tangent_b, center));
    // tangent_a → tangent_b への最小絶対値の符号付き回転角 (-π, π) に正規化。
    // profile の走査順 (a → arc → b) と接続する向きを保証する。
    let raw_sweep = normalize_angle(angle_b - angle_a);

    let new_a = SketchElement::Line { id: a_id.to_string(), from: a_from, to: tangent_a };
    let new_b = SketchElement::Line { id: b_id.to_string(), from: tangent_b, to: b_to };
    let new_arc = SketchElement::Arc {
        id: format!("{a_id}_{b_id}_fillet_arc"),
        center,
        radius,
        start_angle: angle_a,
        end_angle: angle_a + raw_sweep,
    };

    Ok((new_a, new_arc, new_b))
}

/// Build-level: profile 内で elem1_id/elem2_id を検索し、配列順に正規化してから `compute_fillet` を適用する。
/// 隣接性 (配列上で連続、または閉ループの末尾↔先頭) を要求する。
///
/// # Errors (上記に加えて)
/// - `InvalidParameter { kind: "sketch_fillet_elem_not_found" }` — elem1_id/elem2_id が profile 内に無い
/// - `InvalidParameter { kind: "sketch_fillet_same_element" }` — elem1_id == elem2_id
/// - `InvalidParameter { kind: "sketch_fillet_elems_not_adjacent" }` — 2 element が配列上で隣接していない
/// `elem1_id`/`elem2_id` を profile 内で探し、配列順 `(a_idx, b_idx)` (b が a の直後、
/// 閉ループ wraparound 込み) に正規化する。
///
/// `pub` にする理由 (Codex R01 対応): `engawa-build` の `FeatureCrud` が history gate 時に
/// **同じ隣接性契約**をインデックス演算を複製せずに評価するため。幾何計算は一切含まない。
///
/// # Errors
/// - `InvalidParameter { kind: "sketch_fillet_same_element" }` — elem1_id == elem2_id
/// - `InvalidParameter { kind: "sketch_fillet_elem_not_found" }` — elem1_id/elem2_id が profile 内に無い
/// - `InvalidParameter { kind: "sketch_fillet_elems_not_adjacent" }` — 2 element が配列上で隣接していない
pub fn find_adjacent_pair(
    source: &[SketchElement],
    elem1_id: &str,
    elem2_id: &str,
) -> Result<(usize, usize), KernelError> {
    if elem1_id == elem2_id {
        return Err(KernelError::InvalidParameter { kind: "sketch_fillet_same_element" });
    }
    let idx1 = source.iter().position(|e| element_id(e) == elem1_id)
        .ok_or(KernelError::InvalidParameter { kind: "sketch_fillet_elem_not_found" })?;
    let idx2 = source.iter().position(|e| element_id(e) == elem2_id)
        .ok_or(KernelError::InvalidParameter { kind: "sketch_fillet_elem_not_found" })?;
    let n = source.len();
    if idx2 == idx1 + 1 { Ok((idx1, idx2)) }
    else if idx1 == idx2 + 1 { Ok((idx2, idx1)) }
    else if idx1 == n - 1 && idx2 == 0 { Ok((idx1, idx2)) }
    else if idx2 == n - 1 && idx1 == 0 { Ok((idx2, idx1)) }
    else { Err(KernelError::InvalidParameter { kind: "sketch_fillet_elems_not_adjacent" }) }
}

pub fn apply_sketch_fillet_build(
    source: &[SketchElement],
    elem1_id: &str,
    elem2_id: &str,
    radius: f64,
) -> Result<Vec<SketchElement>, KernelError> {
    let n = source.len();
    let (a_idx, b_idx) = find_adjacent_pair(source, elem1_id, elem2_id)?;

    // ID 衝突ガード (GLM round 2 IN02 部分採用): 新規 Arc の派生 ID がユーザー既存 element と衝突しないことを確認
    let arc_id_a = element_id(&source[a_idx]);
    let arc_id_b = element_id(&source[b_idx]);
    let prospective_arc_id = format!("{arc_id_a}_{arc_id_b}_fillet_arc");
    if source.iter().any(|e| element_id(e) == prospective_arc_id) {
        return Err(KernelError::InvalidParameter { kind: "sketch_fillet_arc_id_collision" });
    }

    let (new_a, new_arc, new_b) = compute_fillet(&source[a_idx], &source[b_idx], radius)?;

    // splice: a_idx→new_a, b_idx→new_b、new_arc を a_idx と b_idx の間に挿入
    let mut out = source.to_vec();
    out[a_idx] = new_a;
    out[b_idx] = new_b;
    let insert_at = if b_idx == a_idx + 1 { a_idx + 1 } else { n }; // wraparound (b_idx=0,a_idx=n-1) は末尾に挿入
    out.insert(insert_at, new_arc);
    Ok(out)
}
```

上記の helper (`as_line` / `dist` / `normalize` / `sub` / `add` / `scale` / `dot` / `angle_of` / `normalize_angle` / `element_id`) は **すべて `sketch_fillet.rs` 内の新規 private fn として実装する** (GLM round 1 IN01/AM01 判定、Opus 4.7 subagent 確認済み)。既存 `geometry/math.rs` にはこのうち1つも存在しない (math.rs の公開関数は `length_near` / `angle_near` / `point_near` / `point_near_scaled` / `orthonormal_basis` / `arc_segment_count` / `unwrap_periodic_uv` のみ)。

- `element_id` は `crates/engawa-kernel/src/geometry/sketch_offset.rs:85` の同名 private fn と実装が重複するが、**共通化せず sketch_fillet.rs 内に複製する** (実装時に判断を委ねない、確定事項)。
  - 却下: `geometry/math.rs` への切り出し。math.rs は `nalgebra` のみに依存する純数学モジュール (engawa-kernel/CLAUDE.md の依存グラフ `geometry → (nalgebra)`)。`&SketchElement` を取る helper を置くと `engawa_format` 依存が持ち込まれ役割が壊れる。
  - 却下: `sketch_offset.rs::element_id` の `pub(crate)` 昇格。fillet → offset というモジュール依存は意味論的に逆で、Chamfer (#297) 追加時にさらに歪む。
  - 3つ目の消費者 (#297 Sketch Chamfer) が現れた時点で `geometry/sketch_common.rs` への抽出を検討する (rule of three)。本 Issue では抽出しない。
- `normalize_angle` の契約: **戻り値は `(-π, π]`**。既存実装は repo 内に無い (`grep -rn "normalize_angle"` 0 hits) ので新規 private fn として実装する:
  ```rust
  /// 角度を (-π, π] に正規化する (新規 private helper。math.rs には存在しない)。
  fn normalize_angle(a: f64) -> f64 {
      use std::f64::consts::{PI, TAU};
      let mut r = a % TAU; // (-2π, 2π)
      if r > PI {
          r -= TAU;
      } else if r <= -PI {
          r += TAU;
      }
      r
  }
  ```
  境界 `±π` の扱いが結果に影響しない根拠: corner 角度 guard により `theta ∈ (ANGLE_TOLERANCE, π - ANGLE_TOLERANCE)`、fillet arc の sweep 絶対値は常に `π - theta` なので `|raw_sweep| < π - 1e-9 < π` となり `±π` に到達しない。よって `(-π, π]` / `[-π, π)` のどちらを採っても出力は同一。

- `engawa-build/src/lib.rs` dispatch (`Feature::SketchOffset` の直後に追加、#295 と同じ `built_sketch_profiles` 合成パターン):

```rust
Feature::SketchFillet {
    id: _,
    sketch,
    elem1_id,
    elem2_id,
    radius,
    suppressed: _,
} => {
    let entry = sketches.get(sketch.as_str())
        .ok_or_else(|| KernelError::SketchNotFound { sketch: sketch.clone() })?;
    let source: Vec<engawa_format::SketchElement> = built_sketch_profiles
        .get(sketch.as_str())
        .cloned()
        .unwrap_or_else(|| entry.profile.to_vec());
    let out = engawa_kernel::geometry::sketch_fillet::apply_sketch_fillet_build(
        &source, elem1_id, elem2_id, *radius,
    )?;
    built_sketch_profiles.insert(sketch.clone(), out);
}
```

- `feature_crud.rs` の対応箇所 (Codex R01 判定、Opus 4.7 subagent 確認済み — SketchOffset の各 match arm に倣いつつ element-level 参照ゲートを追加する):
  - `feature_sketch_refs`: `Feature::SketchFillet { sketch, .. } => vec![sketch.as_str()],`
  - `feature_variant_name`: `Feature::SketchFillet { .. } => "SketchFillet",`
  - `set_feature_suppressed`: `| Feature::SketchFillet { suppressed, .. }` を既存の or-pattern に追加 (exhaustive match、漏れるとコンパイル不可)
  - `simulate_history`: SketchOffset と同様「body を生成しない」枝。`suppressed` 判定 → `refs_resolve_in_state` 判定 → `executed_at.insert(i)` のみ (`live_bodies_at` には登録しない)
  - `refs_resolve_in_state`: **element-level 参照ゲートを入れる (Codex R01、必須)**。理由: `SketchOffset` の `selection` は `apply_sketch_offset` が BTreeSet membership 判定のみで未知 ID を黙って無視するため build が失敗せず crud gate と build が乖離しない。一方 `SketchFillet` は `sketch_fillet_elem_not_found` で hard error するため、`sketches_at.contains_key(sketch)` だけでは `CreateSketch.profile` の rename/reorder edit が history gate をすり抜け、build で初めて壊れる (新規リスク)。SketchOffset の A01 (L233-246) と同型で、参照先 CreateSketch の**元** profile を引いて検証する:

    ```rust
    Feature::SketchFillet { sketch, elem1_id, elem2_id, .. } => {
        match sketches_at.get(sketch.as_str()) {
            Some(&idx) => match &features[idx] {
                Feature::CreateSketch { profile, .. } => {
                    match engawa_kernel::geometry::sketch_fillet::find_adjacent_pair(
                        profile, elem1_id, elem2_id,
                    ) {
                        Ok((a, b)) => matches!(profile[a], SketchElement::Line { .. })
                            && matches!(profile[b], SketchElement::Line { .. }),
                        Err(_) => false,
                    }
                }
                _ => false,
            },
            None => false,
        }
    }
    ```

    これにより `edit` / `suppress` / `reorder` は既存の `check_edit_preserves_consumers` (pre/post の `executed_at` 比較) 経由で自動的に `EditBreaksConsumer` を返す。専用配線の追加は不要。
  - `check_refs_resolve_before`: insert 経路用に、SketchOffset の A01 fix (round 4, L593-608) と同型のブロックを追加する:

    ```rust
    if let Feature::SketchFillet { sketch, elem1_id, elem2_id, .. } = f {
        if let Some(&idx) = sketches_at.get(sketch.as_str()) {
            if let Feature::CreateSketch { profile, .. } = &features[idx] {
                let reason = match find_adjacent_pair(profile, elem1_id, elem2_id) {
                    Err(KernelError::InvalidParameter { kind }) => Some(kind),
                    Err(_) => Some("sketch_fillet_elem_unresolved"),
                    Ok((a, b)) => (!matches!(profile[a], SketchElement::Line { .. })
                        || !matches!(profile[b], SketchElement::Line { .. }))
                        .then_some("sketch_fillet_only_line_line"),
                };
                if let Some(reason) = reason {
                    return Err(FeatureCrudError::SketchElementNotResolved {
                        feature_id: fid.to_string(),
                        sketch_ref: sketch.clone(),
                        elem1_id: elem1_id.clone(),
                        elem2_id: elem2_id.clone(),
                        reason,
                    });
                }
            }
        }
    }
    ```

  - `FeatureCrudError` に新 variant を追加する (SketchOffset A01 が既存 `SketchNotFound` を流用しているのは踏襲しない。sketch 自体は実在するため「sketch が存在しない」と報告するのは誤誘導になる):

    ```rust
    /// Feature references sketch elements that do not resolve in the referenced CreateSketch profile.
    #[error("feature {feature_id} references elements ({elem1_id:?}, {elem2_id:?}) of sketch {sketch_ref:?} which do not resolve ({reason})")]
    SketchElementNotResolved {
        feature_id: String,
        sketch_ref: String,
        elem1_id: String,
        elem2_id: String,
        reason: &'static str,
    },
    ```

  - **検証範囲の分離 (over-engineering 回避)**: `refs_resolve_in_state` / `check_refs_resolve_before` が見るのは (1) elem1_id ≠ elem2_id (2) 両 ID が参照先 CreateSketch の**元** profile に存在 (3) 両者が Line (4) 配列上で隣接 (wraparound 含む) の4点のみ。`FilletRadiusTooLarge` / 角度退化 / `sketch_fillet_no_shared_corner` / `sketch_fillet_arc_id_collision` の幾何判定は build 側に残す (SketchOffset も `distance` の NaN 検査は build のみで行う precedent と一致。history gate は「参照解決ゲート」であり幾何バリデータではない)。`simulate_history` 内で `apply_sketch_fillet_build` の幾何計算 (tangent/radius) を再実行すると CRUD 層と build 層の二重実装・発散を招くため行わない。
  - **元 profile 近似の健全性**: fillet は要素を削除・改名せず挿入のみを行うため「元 profile で不在/非 Line/非隣接 ⇒ effective profile でも同様」が成立し、build が受理する編集を誤って拒否することはない (sound but incomplete)。取りこぼすのは「元 profile では隣接だが先行 fillet の Arc 挿入で非隣接化した」ケース、および下記の mixed profile corner 破壊ケースの2種で、いずれも Non-Goals に含まれる (build 側の `sketch_fillet_elems_not_adjacent` / `sketch_fillet_arc_id_collision` / `sketch_fillet_no_shared_corner` で捕捉)。
  - **Codex round 2 A01 (再指摘) の判定 — 既存 scope 決定を維持**: 「CRUD gate が共有コーナー (`a.to == b.from`) を検証していないため、`CreateSketch` の endpoint 座標だけを編集すると gate を通過して build で落ちる」という指摘が STEP 7.5 round 2 で再度上がったが、gate 実装は変更しない (Opus 4.7 subagent 判定)。根拠:
    1. **全 Line profile では到達不能**: `build_bodies_from_features` は CreateSketch 処理時点で `validate_profile_closed` (`crates/engawa-build/src/lib.rs:229` → `:644-661`) を呼び、`elements[i].to == elements[(i+1)%n].from` を `LENGTH_TOLERANCE` で強制する。全 Line 閉ループでは「コーナー破壊」=「閉路破壊」なので、fillet dispatch より前に CreateSketch 自身が `InvalidParameter { kind: "profile" }` で落ちる。Codex の想定した `sketch_fillet_no_shared_corner` には到達しない。
    2. **残余の取りこぼしは mixed profile のみ (上記「元 profile 近似の健全性」の記述を訂正)**: `line_count != elements.len()` の場合 `validate_profile_closed` の閉路検査はスキップされる。したがって `[Line, Line, Arc]` のような mixed profile では隣接 Line ペアのコーナー破壊が build まで残り `sketch_fillet_no_shared_corner` に到達する。
    3. **fillet 固有の欠陥ではない**: `refs_resolve_in_state` の `Feature::Extrude` 分岐 (`crates/engawa-build/src/feature_crud.rs:229-236`) も profile 幾何を一切検証しないため、「CreateSketch endpoint 編集 → CRUD 通過 → build で `kind:"profile"` 落ち」は #296 以前から存在する。SketchFillet だけ塞ぐと同じ編集が Extrude 経由で素通りし続けるので一貫性のない部分硬化になる。正しい解は「全 sketch consumer に対する profile 閉路 gate」で、別 Issue (Phase 11+ の CRUD gate 強化) に切り出す。
    4. **data corruption なし**: `FeatureCrud::edit` は新 `Document` を返すだけで in-place 変更をせず、build 失敗は `Err` を返すのみ。実害は「CRUD 層で通ったのに build で落ちる」UX ギャップに限定される。
    5. **提案された error 種別は不適合**: Codex は `EditBreaksConsumer` での reject を提案したが、`check_edit_preserves_consumers` の `broken_ref` 探索 (`feature_crud.rs:925-951`) は sketch が live である限りヒットせず `lost ref "<unknown>"` という誤誘導メッセージになる。
    - **部分採用**: 実装は変えないが、1. の到達不能性を prose ではなく実行可能な assertion で固定するため回帰テスト T13 を追加する (テスト計画参照)。

## 設計方針
- **決定性**: `compute_fillet` / `apply_sketch_fillet_build` は共に純関数。elem1_id/elem2_id の入力順序を配列位置で正規化するため、`SketchFillet{elem1_id:"l1",elem2_id:"l2"}` と `{elem1_id:"l2",elem2_id:"l1"}` は同一結果を返す (T01b で検証)。
- **ID 体系の区別 (IdGenerator は不使用が正、GLM round 2 IN01 で誤検知として棄却・Opus 4.7 subagent 確認済み)**: `SketchElement::id` は `String` (`crates/engawa-format/src/feature.rs`) で、YAML にユーザーが書く profile 要素の可読 ID。一方 `IdGenerator` が発番する `EntityId = u64` (`crates/engawa-kernel/src/brep/topology.rs:11`) は B-rep トポロジー (Vertex/Edge/Face/Loop/Shell) 専用であり、型も層も別物。`Feature::SketchFillet` の dispatch は `Feature::SketchOffset` (`crates/engawa-build/src/lib.rs:489-515`) と同型で `gen: &mut IdGenerator` に一切触れない純2D profile 変換であり、IdGenerator 呼び出しレイヤー (`make_extrusion` / boolean / primitives) の手前にある。
  - `format!("{a_id}_{b_id}_fillet_arc")` は入力 `(a_id, b_id)` のみに依存する純関数で、乱数・時刻・イテレーション順序を含まないため「同一入力→同一出力」の決定性要件を満たす。
  - ADR-017「ID 安定性」(`docs/decisions/017-phase10-sketch-curves-and-edits.md:93-96`) が派生 element ID を `{a}_split_{n}` 形式の決定的文字列で割り当てると規定し、同 ADR Decision Matrix (`:153`) は「毎編集で全 ID 再採番」(= カウンタ発番) を ADR-005 Topological Naming と衝突するとして棄却済み。sketch element ID にカウンタを導入すると履歴前方への feature 挿入で全 ID がシフトし、YAML の `selection:` / `elem1_id:` 参照が壊れる。
  - 既存先例: `crates/engawa-kernel/src/geometry/sketch_offset.rs` は既存 ID を `id.to_string()` で維持するのみ (新規 ID 発行なし)。Fillet は新規 element ID を発行する初のケースだが、発行方式は ADR-017 の文字列派生規約に従う。
- **B-rep トポロジー妥当性**: 本 Issue は2D sketch 編集で B-rep には直接触れない。Extrude 経由で最終 Solid が Euler-Poincaré (V-E+F=2) を満たすかは既存 `make_extrusion` が担保。
  - **注意 (Codex R02、Opus 4.7 subagent 確認済み)**: `validate_sketch_profile_contours` (`crates/engawa-build/src/lib.rs:669-676`) が保証するのは「multi-element profile に closed primitive が混在していないこと」**だけ**であり、連結性・閉路性・Euler-Poincaré は検証しない。fillet 後 profile の integrity をこの関数に期待してはいけない (旧記述はこの関数の保証範囲を過大表現していた)。
  - 実際に連結性を検査しているのは `make_extrusion` の profile ゲート (`crates/engawa-kernel/src/primitives/extrusion.rs:150-185`): 連続重複点 (`dist <= length_eps`)、`turn_cross.abs() <= area_eps` (退化 turn)、`is_simple` (自己交差)、`is_convex` を全て拒否する。wraparound splice の順序を誤ると polyline が自己交差または逆折れするため `InvalidParameter { kind: "profile" }` で build が落ちる。よって T04/T06 の「build 成功」は実質的な連結性テストとして機能する。
  - ただし `tessellate_sketch_element` は `SketchElement::Line { from, .. } => vec![*from]` (`crates/engawa-kernel/src/tessellation/sketch.rs:25`) で **`Line.to` を使わない** (Arc も start 含む・end 除外で chain する規約)。したがって `compute_fillet` が返す `new_a.to` / `new_b.from` の正しさは Extrude 経由では検証されない。これを直接検査する T09 (profile chain continuity) を追加する。
  - fillet 後 profile が凸性を保つこと: 凸多角形の凸コーナーを丸めた結果は凸のままなので、矩形 fixture (T04/T06/T07/T08) は `is_convex` を通る。凹コーナー fillet は `make_extrusion` の凸制約により現状 build 不可 (kernel 単体テストでのみ扱う、Out-of-Scope)。
- **退化幾何の扱い**:
  - corner 角度 `theta` が `<= ANGLE_TOLERANCE` または `>= π - ANGLE_TOLERANCE` → `DegenerateSketchElement { reason: "fillet_corner_angle_degenerate" }`
  - 入力 Line 自体が退化長 (`<= LENGTH_TOLERANCE`) → `DegenerateSketchElement { reason: "fillet_zero_length_input_line" }`
  - tangent length が入力 Line の長さを超過 → `FilletRadiusTooLarge`
- **derive 規約**: `Feature::SketchFillet` は既存 `Feature` enum に variant を追加するので既存の Debug/Clone/Serialize/Deserialize/JsonSchema/TS derive を継承。
- **エラーハンドリング**: `thiserror` の `KernelError` を使用。新規 variant は `FilletRadiusTooLarge` の1種のみ追加 (issue 完了条件で明示的に名前が指定されているため)。それ以外 (element not found, same element, not adjacent, no shared corner, radius invalid) は既存 `InvalidParameter { kind: &'static str }` の kind 文字列追加で表現し、新規 variant を増やさない (#295 の「3種で足りる」哲学を継承しつつ1種だけ例外)。 加えて `sketch_fillet_arc_id_collision` (GLM round 2 IN02 部分採用、Opus 4.7 subagent 判定: 新規 Arc ID がユーザー既存 element と衝突するケースの fail-fast ガード。新規 variant は増やさず `InvalidParameter` の kind として扱う)。
- **workspace.dependencies**: 追加不要 (既存の serde / thiserror / ts-rs / schemars で足りる)。

### 数値モデル

- 使用 tolerance:
  - `LENGTH_TOLERANCE = 1e-9` (`crates/engawa-kernel/src/geometry/math.rs`, ADR-004): 入力 Line の退化長判定、corner 共有判定 (`dist(a.to, b.from) <= LENGTH_TOLERANCE`)、tangent length の長さ超過判定
  - `ANGLE_TOLERANCE = 1e-9`: corner 角度の退化判定 (ほぼ0 または ほぼπ)
- 幾何アルゴリズム (角の二等分線法):
  - `corner = a.to (= b.from)`、`d_a = normalize(a.from - corner)`、`d_b = normalize(b.to - corner)`
  - `theta = acos(clamp(dot(d_a, d_b), -1, 1))` — 2 direction 間の角度 (0, π)
  - tangent length: `t = radius / tan(theta/2)`
  - arc 中心までの距離: `center_dist = radius / sin(theta/2)`、中心 = `corner + normalize(d_a + d_b) * center_dist`
  - tangent points: `tangent_a = corner + d_a * t`、`tangent_b = corner + d_b * t`
  - arc の sweep は `tangent_a` → `tangent_b` の最小絶対値回転角 (`(-π, π)` に正規化) を採用し、profile の元の走査順 (a → arc → b) と接続する向きを保証する
- **Arc sweep の符号規約** (GLM round 1 AM01 判定で追加、Opus 4.7 subagent 発見): `raw_sweep` は符号付きで、profile の巻き方向と corner の凹凸で符号が決まる。
  - CCW profile の凸コーナー → 正 sweep (`end_angle > start_angle`)。例: 矩形の corner `(10,0)` に radius=1.0 → `center=(9,1)`, `angle_a=-π/2`, `raw_sweep=+π/2`
  - CW profile または凹コーナー → **負 sweep** (`end_angle < start_angle`)
  - `make_extrusion` は `signed_area().signum()` (`crates/engawa-kernel/src/primitives/extrusion.rs:176,195`) で両巻き方向を受け付けるため、CW profile は正当な In-Scope 入力。負 sweep は本 Issue の scope 内で到達可能であり、エラーにはしない。
  - 下流互換 (OK): `tessellate_sketch_element` は sweep 符号をそのまま使い `arc_segment_count` は `.abs()` を取るため負 sweep を正しく扱う (`crates/engawa-kernel/src/tessellation/sketch.rs:82,89`、回帰テスト `T_EDGE_negative_sweep_arc` あり)。`engawa-build` の `is_closed_primitive` も `.abs()` 判定で安全。
  - 下流の**既知の非互換 (本 Issue では受容)**: `crates/engawa-kernel/src/geometry/sketch_offset.rs:199` の `offset_arc` は `end_angle < start_angle` を `InvalidParameter { kind: "arc_negative_sweep" }` で拒否する。ただし build 経路では `apply_sketch_offset_build` の Circle-only guard (`sketch_offset.rs:31`) が先に効くため、Fillet → SketchOffset の合成は sweep 符号に関わらず現状通らない。潜在的衝突として記録し、#295 の Circle-only guard が外れる Phase 11+ で解消する。
- 数値判定:
  - `radius` が非正 (`<= LENGTH_TOLERANCE`) または非有限 (NaN/Inf) → `InvalidParameter { kind: "radius" }`
  - `FilletRadiusTooLarge` 判定: `t > len_a - LENGTH_TOLERANCE || t > len_b - LENGTH_TOLERANCE`
- ADR-004 準拠方針: **tolerant** (`LENGTH_TOLERANCE`/`ANGLE_TOLERANCE` 相対誤差以内は同一扱い)。exact 算術は使わない。

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 (kernel + 派生 ID 規約) | 同一の SketchFillet を2回適用し、生成 element (trimmed line×2 + arc) の ID・座標が完全一致。加えて生成 Arc の element id が `"l1_l2_fillet_arc"` に文字列完全一致することを assert し、ADR-017 の派生 ID 規則を回帰固定する | `assert_eq!(out1, out2)` + `assert_eq!(arc_id, "l1_l2_fillet_arc")` |
| T01c | 退化 (ID 衝突) | profile に既に id=`"l1_l2_fillet_arc"` の element が存在する状態で (l1, l2) に fillet を適用 → `InvalidParameter { kind: "sketch_fillet_arc_id_collision" }` で fail-fast | `assert_matches!` |
| T01d | 決定性 (build 全体) | 同一 Document を `IdGenerator::new(0)` で2回 `build_bodies_from_features` し、fillet 適用後の profile 全要素 ID/座標に加え、Extrude 後 Solid の全 Vertex/Edge/Face の `EntityId` と座標が完全一致 (Arc 1要素追加による発番シフトが決定的であることを担保) | `assert_eq!` (両 build 結果) |
| T01b | 決定性 (入力順序不変性) | `elem1_id`/`elem2_id` を入れ替えて適用し、出力 profile が完全一致 | `assert_eq!(out_forward, out_reverse)` |
| T02 | 正常系 (kernel) | `compute_fillet` を90°コーナー (L字) に radius=1.0 で適用 → tangent point / arc center / radius が解析解と一致 | `assert!((x - expected).abs() < LENGTH_TOLERANCE)` |
| T03 | 正常系 (kernel, 非90°) | 60°コーナーに radius=1.0 で適用 → tangent length / center distance が解析解 (`t=r/tan(30°)`, `d=r/sin(30°)`) と一致 | 同上 |
| T04 | 正常系 (build, L字 line ペア) | 矩形 (4 Line) の1角に radius=1.0 の SketchFillet → Extrude (`fuse_target: None` の純 Extrude)、Solid が生成され旧コーナー頂点が消え tangent point 近傍の頂点が存在する | build 成功 + 頂点座標 assert + `assert_eq!(solid.euler_poincare(), 0)` (回帰ネット。検出力は低い、T09 が本体) |
| T05 | Roundtrip | `Feature::SketchFillet` を YAML serialize → deserialize → 完全一致 | `assert_eq!` |
| T06 | 統合 (Extrude 経由) | `examples/sketch_fillet.engawa` (fuse_target なし純 Extrude) を build → baseline (fillet無し矩形) と比較し、V/E/F count が変化する (Arc 挿入で E,V が増える) | build 成功 + count 比較 + baseline/fillet 後の両 Solid で `assert_eq!(euler_poincare(), 0)` |
| T07 | 閉ループ wraparound | 4 Line 矩形の「最後→先頭」corner (l4/l1) に fillet を適用しても同様に成功する | build 成功 |
| T08 | 符号規約 (CW profile) | CW 巻きの矩形 (`(0,0)→(0,5)→(10,5)→(10,0)→(0,0)`) の1角に radius=1.0 の fillet → 生成 Arc が負 sweep (`end_angle < start_angle`) になり、かつ build が成功する (tessellation は負 sweep 対応済) | `assert!(end_angle < start_angle)` + build 成功 |
| T09 | トポロジー (profile 連結性、Codex R02) | 矩形4 Line に対する fillet 出力 (T04 中間ペア版・T07 wraparound 版の両方) について、`out[i]` の終点と `out[(i+1) % n]` の始点が `LENGTH_TOLERANCE` 以内で一致することを全 i で assert する。Line の終点は `to`、Arc の終点は `center + radius*(cos(end_angle), sin(end_angle))` で評価 (Extrude 経路は `Line.to` を無視するためこの不変条件はそちらでは検出できない) | 全隣接ペアで `assert!(dist <= LENGTH_TOLERANCE)` |
| T10 | CRUD gate (Codex R01 中核) | `[CreateSketch(sk,[l1,l2,l3,l4]), SketchFillet(f1,sk,l1,l2), Extrude]` に対し `FeatureCrud::edit` で CreateSketch の profile を `[l1b,l2,l3,l4]` (l1 を rename) に差し替える → 編集が拒否される (element-level ガード無しだと成功し build で初めて落ちる) | `assert_matches!(err, FeatureCrudError::EditBreaksConsumer { broken_consumer_id, .. } if broken_consumer_id == "f1")` |
| T11 | CRUD gate (insert 経路) | 存在しない element id を持つ SketchFillet を `FeatureCrud::insert` → `SketchElementNotResolved { reason: "sketch_fillet_elem_not_found" }` | `assert_matches!` |
| T12 | CRUD gate (隣接性) | CreateSketch の profile を `[l1,l3,l2,l4]` に並べ替える edit で SketchFillet(l1,l2) が非隣接化 → `EditBreaksConsumer` | `assert_matches!` |
| T13 | CRUD gate 境界 (Codex round2 A01 部分採用) | `[CreateSketch(sk,[l1..l4]), SketchFillet(f1,sk,l1,l2), Extrude]` に対し `FeatureCrud::edit` で `l1.to` を `[10,0]`→`[11,0]` に変更 (ID・並び順は保持)。(a) 編集は**成功する** (gate は ID/隣接性のみを見る設計であることを固定)、(b) 続く `build_bodies_from_features` は `sketch_fillet_no_shared_corner` ではなく `InvalidParameter { kind: "profile" }` で失敗する (`validate_profile_closed` が CreateSketch 時点で捕捉することを固定) | `FeatureCrud::edit(..).is_ok()` + `assert_matches!(build_err, KernelError::InvalidParameter { kind: "profile" })` |
| T_DEG_fillet_too_large | 退化 (issue 完了条件) | 短い Line ペア (長さ1.0) に radius=10.0 → `FilletRadiusTooLarge` | `assert_matches!(err, KernelError::FilletRadiusTooLarge { .. })` |
| T_DEG_corner_angle_flat | 退化 (境界) | ほぼ180° (直線に近い) コーナーに fillet → `DegenerateSketchElement { reason: "fillet_corner_angle_degenerate" }` | `assert_matches!` |
| T_DEG_corner_angle_zero | 退化 (境界) | ほぼ0° (折り返し) コーナーに fillet → 同上 | `assert_matches!` |
| T_DEG_no_shared_corner | 退化 (入力不整合) | corner を共有しない2 Line (endpoint が離れている) → `InvalidParameter { kind: "sketch_fillet_no_shared_corner" }` | `assert_matches!` |
| T_DEG_non_line_element | 退化 (仕様) | elem1_id が Circle を指す → `UnsupportedFeature { kind: "sketch_fillet_only_line_line" }` | `assert_matches!` |
| T_DEG_same_element | 退化 (入力不整合) | elem1_id == elem2_id → `InvalidParameter { kind: "sketch_fillet_same_element" }` | `assert_matches!` |
| T_DEG_not_adjacent | 退化 (入力不整合) | 配列上で隣接しない2 Line (間に他要素あり) → `InvalidParameter { kind: "sketch_fillet_elems_not_adjacent" }` | `assert_matches!` |
| T_DEG_elem_not_found | 退化 (参照) | 存在しない element id → `InvalidParameter { kind: "sketch_fillet_elem_not_found" }` | `assert_matches!` |
| T_DEG_negative_radius | 退化 (数値入力) | `radius = -1.0` → `InvalidParameter { kind: "radius" }` | `assert_matches!` |
| T_DEG_nan_radius | 退化 (数値入力) | `radius = NaN` → `InvalidParameter { kind: "radius" }` | `assert_matches!` |
| T_DEG_sketch_ref_not_found | 参照 | 存在しない sketch id を指す SketchFillet → `SketchNotFound` | `assert_matches!` |

## 幾何的不変条件チェックリスト
- [ ] N/A: partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか (Boolean 系のみ)
- [ ] N/A: 各プリミティブの face ごとの outer_loop 2D 向き（CW/CCW）が文書化されているか (Boolean 系のみ)
- [ ] N/A: flip_normals / same_sense の意味論が明確か (Boolean 系のみ)
- [ ] N/A: pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか (partition 系のみ)

本 Issue はスケッチ (2D) 編集のため上記4項目はすべて N/A。既存 `validate_sketch_profile_contours` と `make_extrusion` の hardening が下流で integrity を担保する。
