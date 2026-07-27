# test-spec.md — Issue #299 (Sketch Pattern)

STEP 6 コア実装 (`git diff d7d5cfa..working-tree`) を read し、plan.md のテスト計画 ID 表と突き合わせた結果。
補足: 本リポジトリのトランク branch 名は `main` ではなく `claude/add-claude-guidelines-BKKtD` (GitHub default branch)。
`cad/299-sketch-pattern` の分岐点は commit `d7d5cfa` (= 現トランク tip)。`check-spec-divergence.ts` は `main` 固定参照のため
このリポジトリでは空振りする既知の制約 — 本ファイルでは diff を手動で読んで代替した。

## 期待値乖離チェック結果

乖離なし。plan.md の T02/T03/T04/T05 の期待値 (STEP 3.5 Codex R02 採択後の修正版) と実装 (`crates/engawa-kernel/src/geometry/sketch_pattern.rs` 内 `#[cfg(test)]` および
`crates/engawa-build/tests/sketch_pattern_acceptance.rs`) の assertion 値を目視で突き合わせ、完全一致を確認した
(T04: `step=π/2` → `center=(0,2)`, `start_angle=π/2`, `end_angle=π` で一致)。

## 不足テスト（plan 計画分）

以下は plan.md テスト計画表に ID があるが、STEP 6 コア実装では `#[ignore]` のまま (`crates/engawa-build/tests/sketch_pattern_acceptance.rs`)。
STEP 6.6 (GLM テスト実装) で `#[ignore]` を解除し、テスト本体を実装すること。

| plan.md ID | acceptance.rs 関数名 | 内容 |
|---|---|---|
| T_DEG_pattern_n0 | `t_deg_pattern_n0_linear` | `SketchPatternLinear` count=0 → `sketch_pattern_linear_count_zero` |
| T_DEG_pattern_n0_circular | `t_deg_pattern_n0_circular` | `SketchPatternCircular` count=0 → `sketch_pattern_circular_count_zero` |
| T_DEG_pattern_n1 | `t_deg_pattern_n1_linear` | count=1 + 未知 selection id → `Ok`、profile 不変（kernel 直呼び版。CRUD 版は `t_crud_pattern_n1_no_validation` で実装済み） |
| T_DEG_pattern_n1_circular | `t_deg_pattern_n1_circular` | 同上 Circular 版 |
| T_DEG_zero_direction | `t_deg_zero_direction` | `direction=(0,0)`, count=2 → `sketch_pattern_linear_direction_degenerate` |
| T_DEG_zero_distance | `t_deg_zero_distance` | `distance=0.0`, count=2 → `sketch_pattern_linear_distance_zero` |
| T_DEG_zero_angle | `t_deg_zero_angle` | `total_angle=0.0`, count=2 → `sketch_pattern_circular_angle_degenerate` |
| T_DEG_unknown_element_linear | `t_deg_unknown_element_linear` | 未知 selection id (count>=2) → `sketch_pattern_linear_unknown_element_id` |
| T_DEG_unknown_element_circular | `t_deg_unknown_element_circular` | 同上 Circular 版 → `sketch_pattern_circular_unknown_element_id` |
| T_DEG_unsupported_type_linear | `t_deg_unsupported_type_linear` | selection に Ellipse id → `sketch_pattern_linear_of_ellipse_or_conic` |
| T_DEG_unsupported_type_circular | `t_deg_unsupported_type_circular` | selection に Conic id → `sketch_pattern_circular_of_ellipse_or_conic` |
| T_DEG_duplicate_id | `t_deg_duplicate_id` | 派生 id `"{elem_id}_pattern_linear_1"` が既存 → `DegenerateSketchElement { reason: "pattern_linear_duplicate_id" }` |

## 実装差分から追加すべきテスト（plan に無かったが実装から生じた分岐）

追加なし。実装 (`sketch_pattern.rs` の 8 エラーkind + 2 変換関数) は plan.md の疑似コード・数値モデル節と
1 対 1 対応しており、plan にない新規分岐は確認できなかった。

念のため確認した項目:
- `resolve_selection` の unknown-id 判定は linear/circular で kind 文字列のみ異なる共通関数 (`mode` 引数) — plan の想定どおり
- `translate_element`/`rotate_element` の Ellipse/Conic 拒否は plan の「lazily inside transform」方針と一致（selection 解決時ではなく変換適用時に判定）
- `count=1` の即時 `Ok(source.to_vec())` は selection を一切参照しない — plan の自律判断ログ §6 のとおり

## エッジケース・退化入力

上記「不足テスト」がエッジケースの全量（plan.md 記載分）。追加で気づいた潜在エッジケースは無し
（`total_angle` に 2π 超を渡した場合の多重回転オーバーラップは plan.md Non-Goals で明示的に対象外としており、
テスト追加は不要）。

## 数値境界

- `LENGTH_TOLERANCE` (`1e-9`) 境界: `distance` がちょうど `1e-9` のとき `distance_zero` にならない（`<=` 判定なので `1e-9` は degenerate 側）ことを
  `t_deg_zero_distance` で `distance=0.0` に加えて `distance=1e-10`（許容誤差内）のケースも入れておくと境界值の理解が深まるが、
  plan.md には明示的な境界値テスト ID が無いため、GLM テスト実装時の裁量に委ねる（必須ではない、追記は歓迎）。
- `ANGLE_TOLERANCE` (`1e-9`) も同様。`t_deg_zero_angle` は `total_angle=0.0` の直接ゼロケースで十分（plan.md の要求どおり）。

## 決定性

`sketch_pattern.rs` 内に `t_determinism_100_runs`（100 回呼び出しの決定性検証）が既に実装済み。T01 (plan.md) の要求を上回る
カバレッジがあるため追加不要。
