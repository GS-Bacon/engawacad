## 仮説

STEP 6.7 (Claude self-review) と STEP 7.5 (Codex final gate) が独立に同一の high 指摘を発見した。加えて self-review が数値精度・overflow・golden test 欠落の medium 指摘を複数発見した。実装は機能的には動作するが、以下の修正が必要。

## 関連ファイル

- `crates/engawa-build/src/feature_crud.rs` (`refs_resolve_in_state` 関数、`check_refs_resolve_before` 関数)
- `crates/engawa-kernel/src/geometry/sketch_mirror.rs` (Arc 角度計算、`dist`/`normalize`、多重回転 sweep 判定)
- `crates/engawa-format/tests/golden_examples.rs`
- `crates/xtask/src/main.rs` (`t02_feature_tagged_union` のタグ一覧)

## 修正方針

### 1. [必須・high] `refs_resolve_in_state` に `Feature::SketchMirror` の arm を追加

**問題**: `crates/engawa-build/src/feature_crud.rs` の `refs_resolve_in_state` 関数 (`SketchOffset`/`SketchFillet`/`SketchChamfer` は全て arm を持つ) に `SketchMirror` の arm がなく、`_ => true` に落ちている。この結果、参照している sketch が削除・suppress された `SketchMirror` を含む Document が `FeatureCrud::delete`/`suppress` で `EditBreaksConsumer` 拒否されず通ってしまい、後で `build_bodies_from_features` が `SketchNotFound` で失敗する壊れた Document を生成できてしまう。

再現: `SketchOffset` を参照する sketch を `FeatureCrud::delete` すると `EditBreaksConsumer` で正しく拒否されるが、同じ状況で `SketchMirror` を使うと `Ok` が返ってしまう。

**修正**:
```rust
// feature_crud.rs の refs_resolve_in_state 内、既存の SketchChamfer arm の直後に追加
Feature::SketchMirror { sketch, .. } => sketches_at.contains_key(sketch.as_str()),
```

**再現テスト先行**: `FeatureCrud::delete`/`suppress` が `SketchMirror` の壊れた consumer を検出して `EditBreaksConsumer` を返すことを検証するテストを `crates/engawa-build/tests/sketch_mirror_acceptance.rs` に追加すること（`sketch_chamfer_acceptance.rs::t07_crud_gate_rename_breaks_consumer` 相当）。まずテストが失敗することを確認してから修正すること。

### 2. [medium] `check_refs_resolve_before` に SketchMirror の element-level gate を追加

`SketchOffset`/`SketchFillet`/`SketchChamfer` には insert 経路の element-level gate があるが `SketchMirror` にはない。`selection` が非空の場合のみ、selection の各 id が現在の profile 内に存在するかを検査する arm を追加すること（`selection` 空は「全要素対象」なので検査不要）。

### 3. [medium] Arc 反射角の計算式を `2φ - θ` の直接算出に変更

**問題**: 現在の実装 (`sketch_mirror.rs` の `mirror_element` 内 Arc 分岐) は `p_start` を反射してから `atan2` で角度を読み直しているが、これは `|center| >> radius` のとき大きな数同士の差で桁落ちする（実測: center=(1e6,1e6) radius=1e-3 で誤差 15 倍 ANGLE_TOLERANCE、center=(1e10,1e10) で 707 倍）。この誤差により `is_axis_coincident` の Arc 判定が真に軸対称な Arc を見逃す（false negative）。

**修正**: plan.md の導出根拠どおり、軸方向角 `phi = d[1].atan2(d[0])` を使って直接 `new_start = 2.0 * phi - start_angle` を計算する。`new_end = new_start - (end_angle - start_angle)` は変更不要。`p_start`/`p_start_m`/中間 atan2 の計算は不要になる。

`new_start` が `(-π, π]` に正規化されなくなるが、`is_axis_coincident` は `angles_near_mod_2pi` で mod 2π 比較しており、tessellation も `sweep.abs()` しか見ないため問題ない。T04 の期待値 (`start=0, end=-π/2`) は `phi=0` なので変わらない。

**回帰テスト追加**: center=(1e6,1e6) radius=1e-3、軸を 45 度 (`axis_p1=(0,0)`, `axis_p2=(1,1)`) とした軸対称 Arc (`start`/`end` が軸に対して対称になるよう選ぶ) が `mirror_axis_coincident` で正しく検出されることを追加。

### 4. [medium] multi-turn Arc (`|sweep| >= 2π`) の on-axis 自己一致検出を追加

**問題**: `is_axis_coincident` の Arc 判定は `|sweep| < 2π` の Arc に対しては正しいが、`|sweep| >= 2π` (1周以上) の Arc は角度の点集合が全円になるため、現在の角度比較だけでは自己一致を検出できない。center が軸上にある場合、`|sweep| >= 2π` の Arc は必ず軸対称（というより、そもそも全円なので mirror すると常に自己重複）。

**修正**: `is_axis_coincident` の Arc 分岐に、`(end_angle - start_angle).abs() >= TAU - ANGLE_TOLERANCE` かつ `center` が軸上（`points_near` で判定）の場合は無条件で `true` を返す分岐を追加する。

**回帰テスト追加**: center=(0,0) radius=1 start=0 end=2.5π を x 軸で mirror すると `mirror_axis_coincident` になることを確認。

### 5. [medium] `dist`/`normalize` の overflow を `hypot` で解消

**問題**: `dist`/`normalize` が `dx*dx + dy*dy` で長さを計算しており、成分が 1.3e154 を超えると平方が overflow して `inf` になる。この結果 `axis_len = inf` が退化チェックを素通りし、`normalize` が `[0.0, 0.0]`（零ベクトル）を返し、その状態で `reflect_point` を通すと軸に対する線対称ではなく **点 `axis_p1` を中心とする点対称** という誤った結果が silent に (エラーなく) 返る。

**修正**: `dist`/`normalize` 内の長さ計算を `dx.hypot(dy)` に置き換える（`f64::hypot` は中間 overflow しない）。

**回帰テスト追加**: axis=(0,0)-(1e300,0)（数学的には x 軸そのもの）で Line (2,0)-(2,3) を mirror すると、正しく (2,0)-(2,-3) になる（点対称の (-2,0)-(-2,-3) にならない）ことを確認。

### 6. [medium] `golden_examples.rs` に `golden_sketch_mirror` を追加

`crates/engawa-format/tests/golden_examples.rs` に `golden_sketch_offset`/`golden_sketch_fillet`/`golden_sketch_chamfer` と同型で `golden_sketch_mirror` を追加すること。`examples/sketch_mirror.engawa` の現在の内容（GLM が Circle ベースに変更済み）に対応する YAML 文字列をハードコードして `assert_golden` に渡す。

### 7. [low、ついでに] `xtask` の tag assert 一覧に `sketch_mirror` を追加

`crates/xtask/src/main.rs` の `t02_feature_tagged_union` テストの `assert!(actual.contains(...))` 一覧に `"sketch_mirror"` を追加する（`sketch_chamfer` の隣に並べる）。

## 試した修正と結果

### Round 2 結果 (2026-07-26)

項目 1-5 (refs_resolve_in_state arm, check_refs_resolve_before gate, Arc 角度式 2φ-θ, multi-turn 自己一致検出, hypot) は全て実装され `cargo xtask ci` green を確認済み。

**未実装のまま残っている項目**: 6 (`golden_examples.rs` への `golden_sketch_mirror` 追加) と 7 (`xtask` の tag assert 一覧への `sketch_mirror` 追加)。この 2 項目のみ **今回追加で実装してください**（他はやり直し不要、既に完了済み）。

### 今回やってほしいこと（項目 6・7 のみ）

1. `crates/engawa-format/tests/golden_examples.rs` に `golden_sketch_offset`/`golden_sketch_fillet`/`golden_sketch_chamfer` (同ファイル内) と同型で `golden_sketch_mirror` テストを追加する。`examples/sketch_mirror.engawa` の**現在の内容**（`cat examples/sketch_mirror.engawa` で確認すること。GLM が Circle ベースに変更済みのはず）を YAML 文字列としてハードコードし `assert_golden("sketch_mirror.engawa", ...)` に渡す。既存 3 本の書き方（`concat!` マクロで行ごとの文字列を連結）を踏襲すること。
2. `crates/xtask/src/main.rs` の `t02_feature_tagged_union` テスト内、`sketch_chamfer` の tag assert の隣に `"sketch_mirror"` の assert を追加する。

### Round 3 結果

項目 6・7 完了。`cargo xtask ci` green。STEP 6.6.5 (6.7 self-review + STEP 7 GLM final review + STEP 7.5 Codex final gate) round 2 を実行し、6.7/7 は pass (medium 以下のみ) だったが STEP 7.5 Codex が高 severity 1 件 (A01) を検出、STEP 6.7 self-review も同一種の指摘 (R2-2) を独立発見した。

### 今回 (round 4) やってほしいこと — Codex A01 / self-review R2-2 の解消（コメント + 回帰テストのみ、アーキテクチャ変更は不要）

**背景**: `crates/engawa-build/src/feature_crud.rs` の `SketchMirror` element-level gate（`check_refs_resolve_before` 内、STEP 6-C round2 で追加した箇所）は `selection` を**元の `CreateSketch.profile`** に対してのみ解決する。これは `SketchFillet`/`SketchChamfer` (#296/#297) が持つ既知の "false-reject" 制約と全く同型（先行 Fillet/Chamfer が挿入した派生要素を対象にする 2 段目の編集は、build では成功するのに CRUD insert では拒否される）。この制約はアーキテクチャ全体の問題として Issue #331 (Fillet/Chamfer/Offset 横断) に **Mirror も統合済み**（本 Issue でアーキテクチャ修正はしない、#331 に委譲）。

**今回やってほしいこと（軽量、コメント + テストのみ）**:

1. `crates/engawa-build/src/feature_crud.rs` の `SketchMirror` element-level gate 部分（`check_refs_resolve_before` 内）のコメントに、この既知制約 (false-reject) と Issue #331 への参照を追記する。既存の `SketchFillet`/`SketchChamfer` の同種コメント（例: `t10_crud_gate_known_limitation_false_reject` 近くの説明）を参考にすること。
2. `crates/engawa-build/tests/sketch_mirror_acceptance.rs` に、この既知制約を固定する回帰テストを 1 本追加する。`sketch_chamfer_acceptance.rs::t10_crud_gate_known_limitation_false_reject` と同型のパターンで:
   - (a) `SketchFillet` (or `SketchChamfer`) で派生要素 (例: fillet の `{a}_{b}_fillet_arc`) を作る
   - (b) その派生要素 id を `selection` に含む `SketchMirror` を features に push した場合、build (`build_bodies_from_features`) は成功することを確認
   - (c) 同じ `SketchMirror` を `FeatureCrud::insert` しようとすると `FeatureCrudError::SketchElementNotResolved { reason: "sketch_mirror_elem_not_found", .. }` で拒否される（＝現状の既知制約どおり）ことを確認
   - テスト名は `t_known_limitation_mirror_derived_elem_false_reject` 等、既知制約であることが分かる名前にすること
3. plan.md の Non-Goals には既に「CRUD gate の element-level 検証は元の CreateSketch.profile 基準」の節を追記済み（Claude 側で対応済み、変更不要）

Codex round1 M01 / round2 M01 (`sketch_chamfer_acceptance.rs` T10 の非対称カバレッジ) は #297 の既存コードで #298 の diff に含まれないため **対応不要**（棄却済み）。

## 次にやること

上記 7 項目を実装し、`cargo xtask ci` が green になることを確認する。

## 追加で書いてほしいテスト

- 項目 1 の再現テスト（`FeatureCrud::delete`/`suppress` が `EditBreaksConsumer` を返す）
- 項目 3 の回帰テスト（大座標での軸対称 Arc 検出）
- 項目 4 の回帰テスト（multi-turn Arc の自己一致検出）
- 項目 5 の回帰テスト（巨大座標軸での正しい線対称）
- 既存の T01-T05・T_DEG_* 系のテストは全て pass を維持すること（後退させない）
