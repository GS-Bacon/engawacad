# test-spec.md — #255 Feature CRUD Insert

STEP 6.5: 実装差分と plan T01–T_DEG_* を突合した結果。

## 期待値乖離

**なし** — plan の T ID と実装の assertion はすべて一致。

| Plan T ID | 実装テスト関数 | ファイル | 期待値の整合 |
|-----------|----------------|----------|--------------|
| T01 | `t01_determinism` | `engawa-build/tests/feature_crud_insert_acceptance.rs` | byte-identical YAML assert — OK |
| T02 | `t02_normal_build_tail_insert` | 同上 | `features[1].id() == "box_2"` (fixture と整合) — OK |
| T03 | `t03_entry_add_dry_run_matches_golden` | `engawa-cli/tests/entry_add.rs` | golden `expected.engawa` 完全一致 — OK |
| T_BOUNDARY_at_zero | `t_boundary_at_zero` | build 側 | `[0]=sphere_1, [1]=box_1` — OK |
| T_BOUNDARY_at_end | `t_boundary_at_end` + `t_boundary_at_end_empty_document` | build 側 | `at=len` と空 Document の `len=0,at=0` を分離 — OK |
| T_DEG_at_out_of_range | `t_deg_at_out_of_range` | build 側 | `OutOfRange { index: len+1, len }` match — OK |
| T_DEG_duplicate_id | `t_deg_duplicate_id` | build 側 | `DuplicateFeatureId { id: "box_1" }` match — OK |
| T04 | `t04_entry_add_output_flag_writes_separate_file` | cli 側 | output flag → 別ファイル書込 — OK |
| T05 | `t05_entry_add_default_overwrites_input` | cli 側 | flag 省略時 input 上書き — OK |

## 不足テスト（plan 計画分）

なし — plan の 9 件すべて実装済み。

## 実装差分から追加すべきテスト

`feature_crud.rs` 内の **inline unit tests** (`#[cfg(test)] mod tests`) に 4 件 (`test_insert_at_end / test_insert_out_of_range / test_insert_duplicate_id / test_insert_at_zero`) があり、integration acceptance と重複している。冗長だが「インライン unit テストはモジュール境界内のロジックを検査」「integration は public API + fixture を経由する」の 2 層が別意義を持つため両方とも残す (Rust 一般的なパターン)。

不足は **以下 2 件、STEP 6.6 で追加すべき**:

1. **T06_insert_then_to_yaml_roundtrip** — Insert 後の Document を `to_yaml()` → `from_yaml()` ラウンドトリップで完全復元できることを検証する。Determinism (T01) は byte-identical を見るが、ラウンドトリップ可能性は別の不変量 (YAML 表現と Rust struct の双方向整合) として確認する価値がある。
2. **T07_input_validation_propagates** — `feature.yaml` に invalid な Feature (例: 重複した id を含む新規 Feature ではなく、Feature 構造自体が invalid な YAML、たとえば `id: ""` 空文字列) を渡したとき、CLI が `error:` prefix で exit 1 する。現在は YAML parse error → `failed to parse feature YAML` まで到達するが、`FormatError::InvalidName` のような validate side のエラーパスをカバーしていない。

## エッジケース・退化入力

- 空 Document への at=0 insert: `t_boundary_at_end_empty_document` でカバー済み
- 既存 Feature が複数 (>1) のケース: T_BOUNDARY_at_zero (Box の後に Sphere を先頭挿入) でカバー済み
- 範囲外 index の上限超過 (at=len+1): T_DEG_at_out_of_range でカバー済み
- 重複 id: T_DEG_duplicate_id でカバー済み
- 不変条件 (RefPlane / Variable scope) の二重検証: `next.validate()?` で内部呼び出ししているため `t02_normal_build_tail_insert` などで間接的にカバーされる

## 数値境界

N/A — 本 Issue は Document YAML 変換のみで数値計算/tolerance を扱わない。

## 決定性

T01 が `to_yaml()` byte-identical で検証。`Vec::insert` は要素位置を決定的に挿入し、`Document::clone()` は決定的、`to_yaml()` の serde 順序も決定的 (engawa-format `Conventions` に「シリアライズは決定的でなければならない」と明記)。
