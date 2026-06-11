<!-- STEP 7.5 Codex review の各ラウンドにおける採用・棄却を記録 -->

## STEP 7.5 Round 1 (codex-final.yaml)

- **F01 (high) 採用**: `isAllTodoIgnoreStub` のロジック弱点 (全 fn 数チェック、属性挿入、メッセージ付き macro) → ロジック全面修正 + 回帰テスト 6 件追加。コミット `f4b9a12`。

## STEP 7.5 Round 2 (codex-final-r2.yaml)

- **F01 (high) 部分採用**: 「STEP 5.5 acceptance skeleton を hard-fail させる」懸念について、本 guard は /3ai STEP 8 文脈での厳格判定が設計意図 (その時点では skeleton 実装完了が正常) と明記 + `--allow-skeleton` オプトアウトフラグ追加。コミット `04d46c9`。
- **F02 (medium) 採用**: `#[tokio::test]` / `async fn` 対応。属性 regex を `#[(?:\\w+::)*test]` に、fn を `(async )? fn` に拡張。回帰テスト 4 件追加。同コミット。

## STEP 7.5 Round 3 (codex-final-r3.yaml)

- **F01 (high) 棄却**: Codex は `extrude_negative_direction_acceptance.rs` の削除で「T01〜T04 が diff 内で 1 件も置き換えられていない」と指摘するが、これは事実誤認。削除した 4 関数 (`t01_kernel_neg_depth_determinism` / `t02_kernel_neg_depth_manifold` / `t_boundary_zero_depth_rejected` / `t_degen_nonfinite_depth_rejected`) は `crates/mycad-kernel/src/primitives/extrusion.rs:783-866` のインラインに**同名同ロジックで全 4 件完全実装済み** (#110 で実装、本コミット時点で稼働中)。Codex が見た diff は `tests/` 配下のみで `src/` 配下のインライン実装を参照できなかったため発生した誤判定。テストカバレッジは構造的に保全されている。
- **F02 (medium) 採用**: マクロ delimiter 形式 `todo!{}` / `todo![]` 対応。正規表現を `(?:\\(.*\\)|\\{.*\\}|\\[.*\\])` に拡張。string-literal 内のコメント記号は known limitation として docstring に明記。回帰テスト 3 件追加。

## STEP 7.5 Round 4 (codex-final-r4.yaml)

- **F01 (high) 採用**: `#[tokio::test(flavor = "multi_thread")]` のような引数付き属性が未対応。regex を `#\[(?:\w+::)*test(?:\([^\]]*\))?\]` に拡張。回帰テスト 3 件追加 (multi_thread / 複数引数 / smol::test 引数)。コミット `cb252e6`。
- **F02 (medium) 採用**: `git status` / `git ls-files` の exit code 未検証で fail-open。両 subprocess の非 0 を明示的に error 化し fail-closed return 1 に修正。同コミット。

## STEP 7.5 Round 5 (codex-final-r5.yaml)

- **F01 (high) 採用**: `pub fn` / `pub(crate) fn` 等の可視性修飾子が未対応、brace matcher が文字列内 `}` で depth カウントを誤る。fnHead に `(?:pub(?:\(...\))?\s+)?` 追加、findMatchingClose を string/comment-aware 実装に置き換え、isBodyEffectivelyTodo の comment strip も string-aware 化。回帰テスト 6 件追加 (pub fn, pub(crate) fn, todo!("}"), todo!("// ..."), todo!("/* ... */"), nested brace in real impl)。コミット `9395788`。

## STEP 7.5 Round 6 (codex-final-r6.yaml) — 棄却 (打ち切り判断)

ユーザー判断: 6 ラウンド経て収束見込みなし、現状を ship する。Round 6 の指摘は両方とも理論的 edge case で原 Issue #139 の対象パターンには影響しない:

- **F01 (high) 棄却**: 「`git ls-files` で `crates/` を repo root 解決なしで実行している」指摘。本スクリプトは /3ai STEP 8 文脈での実行を前提とし (B-1 直後の `git status` も同様の前提)、cwd は常に repo root。非 root から起動する運用は想定外。修正は容易だが本 Issue の scope 外として別 Issue に委ねる。
- **F02 (medium) 棄却**: 「属性と fn の間にコメントが挟まると認識しない」指摘。Rust 文法上は valid だがプロジェクトで遭遇していないパターン。実害ある事例が出たら回帰テスト + 修正を起こす。

**収束判断の根拠**:

5 ラウンドの fix iteration を経て、本ガードは以下の全パターンに対応する状態:
- `#[test]` / `async #[*::test(args)]` / 可視性修飾子 (`pub`, `pub(crate)` 等)
- 全マクロ delimiter `()`, `{}`, `[]`
- メッセージ付き `todo!`/`unimplemented!`、属性挿入、属性順序入れ替え
- 文字列内 brace / コメント記号
- `git` subprocess fail-closed (exit code 検証)

原 Issue #139 の対象 (`extrude_negative_direction_acceptance.rs`、`test_bool_probe.rs`) を完全に検出する。これ以上の iteration は scope creep のため打ち切り。残課題は known limitation として `pre-step8-check.ts` docstring に追記。

## Known limitations (round 6 棄却分の記録)

- 非 repo root からの起動: 想定外。`git -C <root>` パターンへの移行は別 Issue。
- 属性ブロックと fn の間のコメント挿入: 実害事例なし。要発生時に対応。
- 文字列内のエッジケース (raw string ネスト hash 数の極端なケース等): heuristic ガードなので 100% lexer 厳密性は目標外。

## 関連 Issue

- #146 (auto-raised at codex_loops=3 threshold): 本 judgment summary をもって解消扱い → ship 後に close。

