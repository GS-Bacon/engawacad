# Codex final gate (STEP 7.5) — findings と判定

verdict (Codex 原文): **fail** (blocking=1, high 1件)
判定 (Claude STEP 7.5-C): **A01 を部分採用 (id 解決は修正、kind 検証は #331 に委譲) → codex_review = passed**

## A01 (high) — 部分採用 (partially adopted)

- file: `crates/engawa-build/src/feature_crud.rs:335`
- finding (原文要旨): `refs_resolve_in_state` が `SketchMirror` / `SketchPattern*` を
  「sketch ID が存在するか」だけで判定している。これらが history に入った後で
  `FeatureCrud::edit` が selection 対象の要素を rename / 削除 / 非対応型へ変更しても
  `EditBreaksConsumer` が発火せず edit が成功し、後で build したときに初めて
  `sketch_*_unknown_element_id` / unsupported-type エラーになる。
- suggestion (原文要旨): history simulation で Mirror/Pattern consumer の
  current sketch profile を検証する。(1) 非空 selection の id を編集後 profile に対して再チェック、
  (2) `selection=[]` の場合は非対応 element kind を含む profile を reject、
  (3) `EditBreaksConsumer` の回帰テストを追加。

### 採用部分: selection id の解決を edit 経路でも検証する (suggestion 1 + 3)

**指摘は正しく、しかも同じファイル内に反例がある**。`SketchFillet` / `SketchChamfer` の
`refs_resolve_in_state` arm は既に element-level 検証をしており、その契約は
`sketch_fillet_acceptance.rs::t10_crud_gate_rejects_rename_breaking_fillet`
(CreateSketch の要素 rename → `EditBreaksConsumer`) で固定されている。
Mirror / Pattern だけが同じ file 内で契約から外れていた。

再現テストを先に書いて 3 件とも赤を実測してから修正した (CLAUDE.md「バグ修正は再現テスト先行」):

| 再現テスト | ファイル | 修正前 |
|---|---|---|
| `t_crud_pattern_edit_breaks_selection_linear` | `tests/sketch_pattern_acceptance.rs` | edit が `Ok` (bug) |
| `t_crud_pattern_edit_breaks_selection_circular` | 同上 | edit が `Ok` (bug) |
| `t11_crud_gate_edit_breaks_mirror_selection` | `tests/sketch_mirror_acceptance.rs` | edit が `Ok` (bug) |

修正: `sketch_selection_resolves()` helper を追加し、`refs_resolve_in_state` の
Mirror / PatternLinear / PatternCircular arm から呼ぶ。判定基準は insert 経路の
element-level gate (`check_refs_resolve_before`) と同一 = 元 `CreateSketch.profile` に対する
id 解決。Pattern は `count >= 2` のときのみ検証する (Codex R01 の no-op 契約を維持。
`t_crud_pattern_edit_n1_selection_ignored` で固定)。

### Mirror (#298) 分も本 Issue で直した理由 (スコープ判断)

Mirror は既にマージ・クローズ済みで本 Issue の diff 範囲外だが、以下より
「スコープ違反」ではなく「同一バグの完全な修正」と判定した:

1. Codex が `SketchMirror` **and** `SketchPattern*` と両方を名指ししている。Pattern だけ直すと
   high 指摘が半分残る
2. 修正箇所は同一 `match` の隣接 arm (335-339 行)。Pattern だけ直すと
   「Fillet/Chamfer/Pattern は検証あり、Mirror だけ無し」という**新たな**不整合を作る
3. 差分は helper 1 個 + arm 2 個の置換で、Mirror 側の回帰テストも同時に追加した。
   CLAUDE.md が避けよと言う「複雑さが減らないリファクタ」ではなく再現テスト付きのバグ修正

### 棄却部分: `selection=[]` 時の element kind 検証 (suggestion 2)

**棄却理由**: これは edit 固有の穴ではなく、**insert 経路にも同じだけ存在する別軸の false-accept**。

- 現状 `FeatureCrud::insert` も、`selection=[]` の Mirror/Pattern を Ellipse 入り profile に対して
  受け入れる (build で `sketch_pattern_*_of_ellipse_or_conic`)。
- `refs_resolve_in_state` だけを kind 検証するよう変えると、insert は通るのに
  edit 経路の simulation では inert という**逆向きの非対称**を新たに作る。両経路を同時に変えるなら
  それは Mirror/Pattern に閉じない gate 設計変更であり、本 Issue の粒度を超える。
- Non-Goals に既知制約として明記し、#331 (CRUD gate = 元 profile 基準 vs build = current profile 基準の
  横断統合) に含めて追跡する。

### 併せて記録した既知の副作用

修正により、先行 Fillet/Chamfer/Offset が挿入した**派生 id** を selection に持つ Mirror/Pattern は
`simulate_history` 上 inert 扱いになる (元 profile に該当 id が無いため)。これは
`t_known_limitation_mirror_derived_elem_false_reject` が固定している insert 側 false-reject と同根で、
そもそもそうした Document は `FeatureCrud::insert` で作れない (手書き YAML のみ到達可能)。
根本解決は #331 の current-profile 基準化。

### 修正内容 (実装は `dispatch-glm.ts --mode core --debug-spec` 経由、spec は `debug-spec.md`)

`crates/engawa-build/src/feature_crud.rs`:

- `sketch_selection_resolves(features, sketches_at, sketch, selection) -> bool` を新設。
  空 selection = 全要素で常に解決 (SketchOffset 前例 / insert gate と同一基準)。
- `refs_resolve_in_state` の 3 arm を差し替え:
  - `SketchMirror` → `sketch_selection_resolves(..)`
  - `SketchPatternLinear` / `SketchPatternCircular` → `*count >= 2` のときだけ
    `sketch_selection_resolves(..)`、それ以外は従来どおり sketch 存在確認のみ

これにより `simulate_history` → `check_edit_preserves_consumers` が
edit / suppress / delete / reorder の全経路で selection 破壊を検知する。

### 検証

`cargo xtask ci` → `=== All CI checks passed ===` (exit 0)。

- 再現テスト 3 件すべて green (修正前は 3 件とも赤を実測)
- `t_crud_pattern_edit_n1_selection_ignored` green (R01 の count=1 no-op 契約を維持)
- `t10_crud_gate_rejects_rename_breaking_fillet` / `t_known_limitation_mirror_derived_elem_false_reject`
  green (既存契約に回帰なし)
- `sketch_mirror_acceptance` 22 passed / `sketch_pattern_acceptance` 29 passed / workspace `0 failed`

**最終判定: A01 部分採用 + 修正・回帰テスト追加済み、blocking=0 相当。STEP 8 へ進める。**
