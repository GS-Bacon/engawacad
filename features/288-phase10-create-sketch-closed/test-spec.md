# #288 test-spec — multi-closed-primitive profile reject guard

## 不足テスト（plan 計画分）

plan の T01-T05 (`tests/create_sketch_closed_acceptance.rs`) は STEP 6 で全件実装済み・通過確認済み:

| ID | 関数 | 状態 |
|----|------|------|
| T01 | `t01_single_circle_ok` | passed |
| T02 | `t02_lines_rectangle_ok` | passed |
| T03_degen | `t03_degen_two_circles_rejected` | passed |
| T04_boundary_reject | `t04_boundary_reject_circle_plus_line` | passed |
| T05 | `t05_extrudecut_rejects_multi_closed` | passed |

plan 計画分での追加実装は不要。

## 実装差分から追加すべきテスト

実装は単一関数 `validate_sketch_profile_contours` のため新たな分岐は plan に含まれている。
GLM の実装で plan のシグネチャ（`&[SketchElement]`）はそのままだが、呼び出し側は
`SketchEntry.profile: &[SketchElement]` が `as_slice()` で構築済みのため
`validate_sketch_profile_contours(entry.profile)` (`&` なし) になっている — 動作影響なし。

guard の condition は `profile.len() > 1 && profile.iter().any(is_closed_primitive)` のため、
以下の境界に独立テストがあると分岐網羅が完全になる:

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T06_mid_closed | 境界 | `[Line, Circle, Line]` (中央に closed、3 要素) → reject | `Err(InvalidParameter { kind: "profile" })` |
| T07_two_open_ok | 正常系 | `[Line, Arc]` (2 要素・全 open) → guard 通過 (downstream の挙動は問わない) | guard では reject されない (`InvalidParameter` 以外の error or Ok) |
| T08_single_ellipse_ok | 正常系 | `[Ellipse]` (単一 closed primitive) → guard 通過 | guard では reject されない |

T07/T08 の主目的は「len > 1 かつ any(closed)」の **両方が必要** であることを reject 反対側
(= 非該当ケースで通過する) からも担保する regression guard。downstream の `make_extrusion`
が n<3 で error を出す可能性がある (Line+Arc / Ellipse の tessellation 結果次第) ため、
「`KernelError::InvalidParameter { kind: "profile" }` **ではない** こと」を assert する形
(not_eq) で表現する。

## 類似ケース（未カバー）

修正した関数 `validate_sketch_profile_contours` を呼び出すコードパスは
`build_bodies_from_features` 内の **Feature::Extrude** (L261) と **Feature::ExtrudeCut** (L367)
の 2 箇所のみ。他の `entry.profile` を flat_map で潰すコードパスは現状リポジトリ内に存在しない
(`grep -rnE 'entry\.profile|sketch\.profile' crates/engawa-build/src/` で確認済)。
Revolve / Loft / Sweep 等は Phase 11+ で未実装のため、同じ defensive guard を将来追加する
場合の参照点として `validate_sketch_profile_contours` を再利用する想定 (Phase 11+ で
multi-contour 正規実装に置き換わる)。

`#[ignore]` 状態の関連テストは無し (`cargo test --list -p engawa-build` で grep 確認)。

## エッジケース・退化入力

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T09_empty_profile | 退化 | `[]` (空 profile) → guard 通過 (len == 0 < 1) → downstream `make_extrusion` の n<3 check が拾う | guard では reject されない (downstream の既存 error に委譲) |

`len > 1` の境界 (= len == 0, len == 1) はガードを抜けるべきで、これは plan の `## 設計方針`
「profile.len() == 0 はここでは扱わない (make_extrusion 側の `n < 3` チェックで補足される既存
挙動を維持)」に対応する regression test。

## 数値境界

N/A — guard 関数は純粋な discriminant match のみで数値演算なし。

## 決定性

T01 (`t01_single_circle_ok`) と T03_degen が validate の純粋性を間接的に担保している
(同一入力 → 同一 Ok / 同一 Err)。新規追加テスト不要。
