# debug-spec: #297 STEP 6-B (`cargo xtask ci` 赤) の修正

結論: **実装 (kernel / format / build) は正しい。落ちているのはテスト側のハードコード golden 文字列 1 箇所だけ**。
`crates/xtask/src/main.rs` の `FEATURE_GOLDEN` に `sketch_chamfer` variant が未反映。

ci.log: `/home/bacon/worktrees/w1/features/297-phase10-sketch-chamfer-engawa/ci.log`

| 項目 | 内容 |
|---|---|
| 失敗ステージ | `=== Running tests ===` (ci.log:72 で開始 → ci.log:2339 `FAILED: Running tests`) |
| 失敗テスト | `xtask` bin の `tests::t02_feature_tagged_union` の **1 件のみ** (ci.log:2310, 2333-2336: `27 passed; 1 failed`) |
| panic 位置 | `crates/xtask/src/main.rs:1076` の `assert_eq!(actual, FEATURE_GOLDEN)` |
| fmt / clippy | ともに pass (ci.log:63, 65) |
| chamfer 本体テスト | 全 pass (ci.log:901-919 acceptance, 1342-1345 format roundtrip, 1679-1700 kernel inline) |

## 仮説

1. **(確定) `FEATURE_GOLDEN` の更新漏れ**
   `GLM` は `Feature::SketchChamfer` を `crates/engawa-format/src/feature.rs` に正しく追加し、
   ts-rs 生成物 `web/src/generated/Feature.ts` も更新・コミット済み (commit `78ad1b4`)。
   しかし `crates/xtask/src/main.rs` の `static FEATURE_GOLDEN`(1032-1070 行) だけが `sketch_fillet` までの古い内容。
   バイト単位で独立検証した結果:

   | 対象 | 長さ | 関係 |
   |---|---|---|
   | `FEATURE_GOLDEN` (main.rs:1032-1070) | 2067 bytes | `Feature.ts` の **strict prefix** (先頭 2065 バイトまで完全一致) |
   | `web/src/generated/Feature.ts` | 2461 bytes | 差分 394 bytes = `sketch_chamfer` variant のみ |

   つまり **差分は末尾に chamfer variant を足すだけ**。他フィールドの並び替え・改名は一切発生していない。

2. **(確定) plan.md の実装対象リストに `crates/xtask/src/main.rs` が入っていない**
   `plan.md` を grep しても `xtask` / `FEATURE_GOLDEN` / `Feature.ts` / `gen-ts` の記載が 0 件。
   GLM が触らなかったのは指示漏れが原因で、GLM 側の判断ミスではない。
   同種の漏れは #296 でも発生している (`features/296-.../debug-spec.md` の仮説 3 = `golden_examples.rs` への `sketch_fillet` 追加漏れ)。
   → `Feature` enum に variant を足す Issue では **`crates/xtask/src/main.rs::FEATURE_GOLDEN` が常に追随対象**。

3. **(確定) 他に隠れた golden / snapshot 不整合は無い**
   独立に以下を確認した。

   | 確認対象 | 結果 |
   |---|---|
   | ci.log 内の `FAILED` 行 | `t02_feature_tagged_union` の 1 件のみ (他は全 `test result: ok`) |
   | `crates/xtask/src/main.rs` の他の golden (`TRIANGLE_MESH_GOLDEN` / `DOCUMENT_GOLDEN` / `COMPONENT_GOLDEN` / `COMPONENT_REF_GOLDEN` / `ERROR_RESPONSE_GOLDEN` / `BODY_MESH`) | 全 pass。`Feature` variant 追加の影響を受けない (`Component.ts` は `Feature` を import 型として参照するだけで内容を展開しない) |
   | `crates/engawa-format/tests/golden_examples.rs` | 全 pass。既存 golden は既存 example ファイルの YAML であり、新 variant の追加で変化しない |
   | `crates/engawa-format/tests/sketch_element_golden_acceptance.rs` | 全 pass。`SketchElement` は本 Issue で未変更 |
   | JSON Schema スナップショット (`*.schema.json` / `schema/`) | リポジトリに存在しない → 追随不要 |
   | web/ 側で feature type を列挙する switch | `grep -rn "sketch_fillet" web/src` は `web/src/generated/Feature.ts` のみヒット。手書きの網羅 switch は無いので TS 型チェックは壊れない |

   ただし **カバレッジの欠落**が 2 件ある (CI は赤にならないが Phase 10 の既存慣行から外れる) → 「追加で書いてほしいテスト」節を参照。

## 関連ファイル

| ファイル | 役割 | 本修正での扱い |
|---|---|---|
| `crates/xtask/src/main.rs` (1032-1070 行) | `FEATURE_GOLDEN` 静的文字列 | **唯一の必須修正対象** |
| `crates/xtask/src/main.rs` (1072-1112 行) | `t02_feature_tagged_union` 本体 | assert 追加 (推奨、下記) |
| `web/src/generated/Feature.ts` | ts-rs 生成物 (期待値の正解) | **編集禁止**。既に `sketch_chamfer` を含む正しい状態で commit 済み (`78ad1b4`)。ここからコピーする |
| `crates/engawa-format/src/feature.rs` | `Feature::SketchChamfer` 定義 + doc comment | **変更不要**。doc comment を変えると golden も同時に変える必要が出るので触らない |
| `crates/engawa-build/tests/examples_smoke.rs` | example ファイルの smoke | chamfer 分の追加 (推奨) |
| `crates/engawa-format/tests/golden_examples.rs` | example ファイルの byte-identical golden | chamfer 分の追加 (推奨) |

## 修正方針

### A. `crates/xtask/src/main.rs::FEATURE_GOLDEN` に `sketch_chamfer` variant を追加 (必須)

`FEATURE_GOLDEN` は `r#"..."#` raw string。**末尾 1 行 (1069 行) だけを差し替える**。

#### before (main.rs:1066-1070、現状)

```
/**
 * Fillet radius (positive)
 */
radius: number, suppressed?: boolean, };
"#;
```

#### after (1069 行を 17 行に展開)

```
/**
 * Fillet radius (positive)
 */
radius: number, suppressed?: boolean, } | { "type": "sketch_chamfer", id: string, 
/**
 * Reference to CreateSketch.id
 */
sketch: string, 
/**
 * First element ID (any of the two adjacent elements)
 */
elem1_id: string, 
/**
 * Second element ID (any of the two adjacent elements)
 */
elem2_id: string, 
/**
 * Chamfer length (positive), measured from the shared corner along each element
 */
length: number, suppressed?: boolean, };
"#;
```

#### ⚠️ 行末スペースが有意 (最重要注意点)

ts-rs の出力は 4 行の**末尾に半角スペース 1 個**を持つ。`·` を半角スペースとして表記すると:

```
|·|·{·"type":·"sketch_chamfer",·id:·string,·|   ← 末尾スペースあり
|/**|
|·*·Reference·to·CreateSketch.id|
|·*/|
|sketch:·string,·|                             ← 末尾スペースあり
|/**|
|·*·First·element·ID·(any·of·the·two·adjacent·elements)|
|·*/|
|elem1_id:·string,·|                           ← 末尾スペースあり
|/**|
|·*·Second·element·ID·(any·of·the·two·adjacent·elements)|
|·*/|
|elem2_id:·string,·|                           ← 末尾スペースあり
|/**|
|·*·Chamfer·length·(positive),·measured·from·the·shared·corner·along·each·element|
|·*/|
|length:·number,·suppressed?:·boolean,·};|     ← 末尾スペースなし
```

- 行末スペースを trim すると `assert_eq!` は再度落ちる。**trailing whitespace を削る整形を絶対にかけない**こと
- `cargo fmt` は raw string の中身を書き換えないので安全 (既存の `sketch_offset` / `sketch_fillet` 行も同じ末尾スペースを持ったまま `cargo fmt --check` を通っている)

#### 最も安全な手順 (手打ち転記より確実)

`FEATURE_GOLDEN` の中身は `web/src/generated/Feature.ts` の**全内容とバイト単位で一致する**必要がある。
そこで手で escape を書き写すのではなく、コミット済みの生成物からそのままコピーする:

1. `web/src/generated/Feature.ts` を読む (すでに `sketch_chamfer` を含む正解)
2. `static FEATURE_GOLDEN: &str = r#"` と `"#;` の間を、その内容で**丸ごと置換**する
   - ファイル先頭の `// This file was generated by [ts-rs]...` 行から、末尾の `length: number, suppressed?: boolean, };` + 改行 まで全部
   - `Feature.ts` は `#` を含まないので `r#"..."#` の閉じ記号と衝突しない (`r##"` に変える必要はない)
3. 期待バイト長は 2461 (現状 2067)

#### 参照: panic メッセージからの復元も可

`ci.log:2328` の `left:` 側が `Feature.ts` の全内容 (escape 済み)。`\n` を改行に戻せば同じものが得られる。
`right:` 側が現状の古い `FEATURE_GOLDEN`。差分は `radius: number, suppressed?: boolean, }` の直後に
` | { "type": "sketch_chamfer", ...` が挿入されるだけ。

### B. 触ってはいけないもの

- `web/src/generated/Feature.ts`: 生成物。手編集すると `=== Checking TS drift ===` (main.rs:828-854) で落ちる
- `crates/engawa-format/src/feature.rs` の `SketchChamfer` doc comment: 文言を変えると ts-rs 出力が変わり `FEATURE_GOLDEN` も再更新が必要になる。今回は変更しない
- 実装コード全般 (`sketch_chamfer.rs` / `lib.rs` / `feature_crud.rs`): 該当テストは全て緑。触らない

## 試した修正と結果

| # | 実施内容 | 結果 |
|---|---|---|
| 1 | `ci.log` の全 `FAILED` / `test result:` 行を走査 | 失敗は `t02_feature_tagged_union` の 1 件のみと確定。他の全テストバイナリは `ok` |
| 2 | `crates/xtask/src/main.rs:1032-1070` を精読 | `FEATURE_GOLDEN` の末尾が `sketch_fillet` の `radius: number, suppressed?: boolean, };` で終わっていることを確認 |
| 3 | `FEATURE_GOLDEN` と `web/src/generated/Feature.ts` をバイト比較 | 2067 vs 2461 bytes、先頭 2065 バイト一致 = golden は生成物の strict prefix。差分は chamfer variant 394 bytes のみ |
| 4 | `sketch_fillet` / `sketch_offset` を含むファイルを repo 全体 grep | 追随が必要な他の golden / snapshot / 手書き列挙は無しと確認 (詳細は仮説 3 の表) |
| 5 | `examples_smoke.rs` / `golden_examples.rs` の chamfer カバレッジ確認 | いずれも **未追加**。#295 / #296 は両方追加していたので慣行から外れている |

**コード修正はまだ 1 行も入れていない** (原因調査と検証のみ)。`git status` は STEP 6-A 時点のまま。

## 次にやること

1. 上記 **A** を適用する (`crates/xtask/src/main.rs` の `FEATURE_GOLDEN` を `Feature.ts` の内容で丸ごと差し替え)
2. 単体で確認: `cargo test -p xtask --bin xtask t02_feature_tagged_union`
3. xtask 全体で確認: `cargo test -p xtask --bin xtask` (28 tests 全緑を確認)
4. 「追加で書いてほしいテスト」を実装する
5. `cargo fmt --all` → `cargo clippy --workspace -- -D warnings`
   - fmt 後に `FEATURE_GOLDEN` の行末スペースが消えていないか (= 2 の test が通るか) を必ず再確認
6. **`cargo xtask ci` をフルで再実行する**
   - 今回の run は `Running tests` で abort したため、その後段の
     `=== Checking TS drift ===` (main.rs:828) / Release build (859) / Release smoke test (869) /
     Playwright (881) / bats 3ai (898) / bun 3ailoop (916) は **一度も実行されていない**
   - `Checking TS drift` は `git status --porcelain --untracked-files=all -- web/src/generated/` が空であることを要求する。
     `Feature.ts` は commit 済みなので通る想定だが、未検証なので必ず通す

## 追加で書いてほしいテスト

### T-A. `t02_feature_tagged_union` に tag 存在 assert を追加 (必須)

`crates/xtask/src/main.rs:1078-1111` の `assert!(actual.contains(...))` 群は `sketch_offset` で止まっており、
`sketch_fillet` / `sketch_chamfer` の tag assert が無い。golden 差し替えのついでに 2 件追加する。

```rust
assert!(
    actual.contains(r#""type": "sketch_fillet""#),
    "missing sketch_fillet tag"
);
assert!(
    actual.contains(r#""type": "sketch_chamfer""#),
    "missing sketch_chamfer tag"
);
```

理由: `assert_eq!` の全文比較が落ちたとき、どの variant が欠けたのかを panic 出力の目視 diff に頼らず特定できる。

### T-B. `crates/engawa-build/tests/examples_smoke.rs` に chamfer smoke を追加 (必須)

`examples/sketch_chamfer.engawa` は plan In-Scope で追加済みだが smoke テストが無い。
#295 / #296 の直後に同形で足す (ファイル末尾に追記):

```rust
/// Issue #297: Phase 10 Sketch Chamfer smoke テスト
#[test]
fn sketch_chamfer() {
    smoke(include_str!("../../../examples/sketch_chamfer.engawa"));
}
```

### T-C. `crates/engawa-format/tests/golden_examples.rs` に `golden_sketch_chamfer` を追加 (必須)

`golden_sketch_offset` (281 行) / `golden_sketch_fillet` (311 行) の直後に追加する。
`examples/sketch_chamfer.engawa` は `examples/sketch_fillet.engawa` と構造が同一 (同じ 10x5 矩形 4 Line、
同じ `offset: 0.0`、同じ `depth: 3.0`) なので、期待値は fillet golden の
`name` / `type` / `id` / パラメータ行だけが違う形になる:

```rust
#[test]
fn golden_sketch_chamfer() {
    assert_golden(
        "sketch_chamfer.engawa",
        concat!(
            "schema_version: 2\nversion: 0.1.0\nroot_component:\n",
            "  name: 'Sketch Chamfer Example (Phase 10 #297)'\n",
            "  features:\n",
            "  - type: create_sketch\n",
            "    id: sketch_0\n",
            "    plane: xy\n",
            "    profile:\n",
            "    - kind: line\n",
            "      id: l1\n",
            "      from:\n",
            "      - 0.0\n",
            "      - 0.0\n",
            "      to:\n",
            "      - 10.0\n",
            "      - 0.0\n",
            "    - kind: line\n",
            "      id: l2\n",
            "      from:\n",
            "      - 10.0\n",
            "      - 0.0\n",
            "      to:\n",
            "      - 10.0\n",
            "      - 5.0\n",
            "    - kind: line\n",
            "      id: l3\n",
            "      from:\n",
            "      - 10.0\n",
            "      - 5.0\n",
            "      to:\n",
            "      - 0.0\n",
            "      - 5.0\n",
            "    - kind: line\n",
            "      id: l4\n",
            "      from:\n",
            "      - 0.0\n",
            "      - 5.0\n",
            "      to:\n",
            "      - 0.0\n",
            "      - 0.0\n",
            "  - type: sketch_chamfer\n",
            "    id: chamfer_1\n",
            "    sketch: sketch_0\n",
            "    elem1_id: l1\n",
            "    elem2_id: l2\n",
            "    length: 1.0\n",
            "  - type: extrude\n",
            "    id: extrude_1\n",
            "    sketch: sketch_0\n",
            "    depth: 3.0\n",
        ),
    );
}
```

注意: 上記は `golden_sketch_fillet` (既に緑) からの機械的な差分導出であり、シリアライザを実行して確認したものではない。
`cargo test -p engawa-format --test golden_examples golden_sketch_chamfer` を実行し、
もし mismatch したら **panic 出力の `left:` (= 実際のシリアライズ結果) をそのまま期待値に反映**すること
(`assert_golden` は `Document::from_path` → `to_yaml()` の結果を比較するので `left` が正解)。
`offset: 0.0` が golden に現れない点は fillet / offset の既存 golden と同じ挙動 (default 値は skip される)。

### T-D. (任意、再発防止) `Feature` variant 追加時の golden 追随を機械的に検出する

`t02_feature_tagged_union` は「golden を手で更新しないと落ちる」設計なので、**現状でも検出自体は機能している**
(今回まさにこれが検出した)。よって追加の仕組みは不要。
代わりに plan.md / ADR-017 側の作業チェックリストへ
「`Feature` enum に variant を追加したら `crates/xtask/src/main.rs::FEATURE_GOLDEN` と
`examples_smoke.rs` / `golden_examples.rs` を同時に更新する」を残すのが本筋
(STEP 8 以降で `docs/decisions/017-phase10-sketch-curves-and-edits.md` に 1 行追記、または follow-up Issue)。
本 debug-spec のスコープでは実装不要。
