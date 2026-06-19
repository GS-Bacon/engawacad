# debug-spec.md — Codex 7.5 r1 修正反映

STEP 7.5-C: Codex 3-persona review r1 で blocking=1 (M-F01 high)。M-F01 は scope-defend (#263 で別 Issue 化)、medium/low の以下 4 件を採用して GLM に修正を委譲する。

## 仮説 / 関連ファイル

実装は概ね正しいが、以下の局所的不備がレビューで検出された:

1. **A-F01** — `crates/engawa-build/src/feature_crud.rs:72` `#[cfg(test)] mod tests` で `use engawa_format::{Feature, SketchPlane};` の `SketchPlane` が unused。`cargo clippy --all-targets -- -D warnings` 通過のため削除する。
2. **C-F03** — `crates/engawa-build/src/lib.rs:1` `mod feature_crud;` (private) のままだと `engawa_build::feature_crud::FeatureCrud` で downstream が触れない。plan は `pub mod feature_crud;` を約束していた。`pub mod feature_crud;` に変更する。
3. **A-F02 + C-F01 + M-F02 (重複)** — `crates/engawa-cli/tests/entry_add.rs` の T03 (`t03_entry_add_dry_run_matches_golden`) と T04 (`t04_entry_add_output_flag_writes_separate_file`) は input ファイルが不変であることを assert していない。`--dry-run` と `--output` の非破壊契約を回帰検出するため、両テストとも input を一時ファイルにコピーして実行し、実行前後で input bytes が変わっていないことを assert する。
4. **C-F02 + M-F03 (重複)** — `crates/engawa-build/tests/fixtures/insert/invalid_empty_id.yaml` の `type: CreateBox` (PascalCase) は `serde_yaml::from_str::<Feature>` で先に parse 失敗する。`type: create_box` (snake_case) に修正し、`t07_input_validation_propagates` の assertion を「stderr に `empty` または `invalid` のような validate 由来文言が含まれる」まで強化する。

## 修正方針

### 1. crates/engawa-build/src/feature_crud.rs (A-F01)

```rust
// before:
#[cfg(test)]
mod tests {
    use super::*;
    use engawa_format::{Feature, SketchPlane};
    ...

// after:
#[cfg(test)]
mod tests {
    use super::*;
    use engawa_format::Feature;
    ...
```

### 2. crates/engawa-build/src/lib.rs (C-F03)

```rust
// before:
mod feature_crud;
pub use feature_crud::{FeatureCrud, FeatureCrudError};

// after:
pub mod feature_crud;
pub use feature_crud::{FeatureCrud, FeatureCrudError};
```

### 3. crates/engawa-cli/tests/entry_add.rs (A-F02 / C-F01 / M-F02)

`t03_entry_add_dry_run_matches_golden`:
- 現状: 直接 fixture の `input.engawa` を引数に渡し、stdout の YAML を expected と比較するのみ
- 修正: 一時ファイル `tmp/input.engawa` に fixture をコピーして `--dry-run` を実行し、stdout 検証に加えて `tmp/input.engawa` の bytes が **コピー直後の original と等しい** (= 上書きされていない) ことを assert する

`t04_entry_add_output_flag_writes_separate_file`:
- 現状: input は fixture 直接で、output のみ tmp。output 内容のみ検証
- 修正: input を tmp にコピーした上で実行し、`-o tmp/out.engawa` の結果検証に加えて、`tmp/input.engawa` が変更されていないことを assert する

### 4. fixture (C-F02 / M-F03)

`crates/engawa-build/tests/fixtures/insert/invalid_empty_id.yaml`:

```yaml
# before:
type: CreateBox
id: ""
width: 10.0
height: 20.0
depth: 30.0

# after:
type: create_box
id: ""
width: 10.0
height: 20.0
depth: 30.0
```

`t07_input_validation_propagates`:
- 現状: stderr に `error:` または `Error` を含むかのみ assert
- 修正: stderr に `empty` または `invalid` のような validate 起因文言 (現実装の `FormatError::InvalidName { reason: "feature_id must not be empty", .. }` を想定) を含むことを assert
  - 注: 現状の `engawa-format` validate がどの文言を出すか先に grep で確認すること。文言が見当たらなければ「FormatError 系の文言が `error:` の後に続く」程度の緩い assert で OK。

## M-F01 (high) について

scope-defend して `#263` を起票済み。plan.md Non-Goals に追記済み。本 round では実装変更しない。

## 次にやること

1. 上記 4 件を修正
2. `cargo xtask ci` を green に保つ (workspace test + clippy + fmt + build)
3. 完了したら glm-result.json を success で書き出す

## 追加で書いてほしいテスト

- 上記 #3 の T03/T04 強化のみ。新規テスト関数の追加は不要 (既存関数に assert を追加する形)
