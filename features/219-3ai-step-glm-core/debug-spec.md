# debug-spec round 2

## 仮説
- 前回 (round 1) のコンパイル失敗: `crates/engawa-build/src/lib.rs:348` で `id.as_str()` を呼んだが、`id` の実際の型は `&str` (`Feature::id(&self) -> &str` の戻り値、line 189 `let id = feature.id();`)。
- `&str` に対する `.as_str()` 呼び出しは unstable `str_as_str` feature (issue #130366) が必要なため、stable rustc では `error[E0658]` でリジェクトされる。

## 関連ファイル
- `crates/engawa-build/src/lib.rs` line 188-189: `let id = feature.id();` (type: `&str`)
- `crates/engawa-format/src/feature.rs` line 370: `pub fn id(&self) -> &str`
- `crates/engawa-build/src/lib.rs` line 342-350: `Feature::CreateBox` dispatcher

## 修正方針
- `id.as_str()` を **単に `id`** に変える (既に `&str`)。
- 念のため `crates/engawa-build/src/lib.rs` の他の make_* 呼び出し (cylinder/sphere) の周辺で `.as_str()` を **新規追加していない** ことを確認 (本 Issue scope は cuboid のみ、cylinder/sphere は不変)。
- それ以外の round 1 で完了した修正 (cuboid.rs シグネチャ変更、kernel tests の `"cuboid"` 追加、build tests の `"cuboid"` 追加、example YAML 修正、benches 修正、t02 `#[ignore]` 解除) はそのまま保持する。

## 試した修正と結果
- [ ] round 1: `id.as_str()` で `error[E0658]: use of unstable library feature str_as_str` (FAILED)

## 次にやること
1. `crates/engawa-build/src/lib.rs:348` の `id.as_str()` を `id` に置き換える。
2. `cargo xtask ci` を実行し、green を確認する。
3. もし他にも同種の `.as_str()` を `&str` に対して呼んでいる箇所があれば一括修正する。

## 追加で書いてほしいテスト
- なし (T02 既存テストが回帰検出する。コンパイル通過自体が今回の修正で担保される)。
