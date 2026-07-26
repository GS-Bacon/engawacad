# Claude Self-Review for #297 (STEP 6.7 / shift-left)

レビュー対象: working tree の全差分 (origin/HEAD = `claude/add-claude-guidelines-BKKtD` = 9cd87e1 起点)。
実装は未コミット (working tree + untracked) の状態で `cargo xtask ci` green。
`glm-self-review.md` が既に挙げた弱点 (FEATURE_GOLDEN の `include_str!` 化、helper 複製、
follow-up Issue 未起票) は重複記述せず、**GLM が見落とした点**と**GLM の根拠が誤っている点**に絞る。

検証は読解のみに頼らず、一時テスト (`crates/engawa-build/tests/zz_tmp_review_297.rs`, 実行後削除) で
5 本の仮説を実測した。実測ログは各項に `PROBE_*` として引用する。

---

## architect 観点 (既存 invariant / API 契約 / トポロジー保証)

### A-1. `T_DEG_corner_angle_flat` が θ≈π 分岐を一切通っていない (false-green テスト) — **high**

`crates/engawa-build/tests/sketch_chamfer_acceptance.rs:419-433` の構成:

```rust
line("l1", [0.0, 0.0], [5.0, 0.0]),
line("l2", [5.0, 0.0], [5.0 + 1e-12, 0.0]),
```

`l2` の長さは 1e-12。`LENGTH_TOLERANCE = 1e-9` (`crates/engawa-kernel/src/geometry/math.rs:10`) なので
`compute_chamfer` の零長判定 (`sketch_chamfer.rs:67-72`) が先に発火し、角度退化判定
(`sketch_chamfer.rs:82`) には到達しない。assert が
`reason == "chamfer_corner_angle_degenerate" || reason == "chamfer_zero_length_input_line"`
という OR になっているため、この取り違えがテスト上は緑で隠れている。

実測:

```
PROBE_E as_written_err=DegenerateSketchElement { element_id: "l2", reason: "chamfer_zero_length_input_line" }
PROBE_E true_flat_err=DegenerateSketchElement { element_id: "l1_l2", reason: "chamfer_corner_angle_degenerate" }
```

結果として、**plan.md の自律判断ログが 1 段落を費やして「維持する理由」を論証したコーナー角度退化
チェック (θ≈π 側) はテストカバレッジ 0** になっている。`t_deg_corner_angle_zero` (θ≈0) は正しく
角度分岐を通っているので、退化 2 方向のうち片方だけが未検証。

- production code 自体は正しい (`PROBE_E true_flat_err` が期待どおり `chamfer_corner_angle_degenerate`)。
  つまり機能バグではなく **テスト完全性の欠陥**であり、修正は test-only で risk ゼロ。
- plan.md のテスト表 (T_DEG_corner_angle_flat) は期待結果を
  `DegenerateSketchElement { reason: "chamfer_corner_angle_degenerate" }` と単一指定している。
  実装テストが OR を足したのは plan からの逸脱。
- なお #296 の `crates/engawa-build/tests/sketch_fillet_acceptance.rs:392-406` が同一構成・同一 OR で
  同じ欠陥を持つ (継承元)。本 Issue の scope 外なので別 Issue 推奨。

### A-2. `length` 比較規約の受理境界にテストが無い — **medium**

Issue 完了条件が「`length` の比較規約を本文で固定する」を明示要求し、plan.md 数値モデルは
`t > len_a - LENGTH_TOLERANCE || t > len_b - LENGTH_TOLERANCE` で発火・
`length == len - LENGTH_TOLERANCE` は許容と固定した。実装 (`sketch_chamfer.rs:90`) は一致している。

実測:

```
PROBE_D just_under_ok=true exact_ok=false exact_err=Some(ChamferLengthTooLarge { elem1_id: "l1", elem2_id: "l2", length: 1.0 })
```

規約どおり。ただし **境界を固定するテストが存在しない**。既存の退化テストは
`length=10.0` vs 線長 1.0 (10 倍) と `f64::MAX` / `f64::MIN_POSITIVE` / `NaN` / `Inf` という
極端値のみで、`>` を `>=` に変えても、`- LENGTH_TOLERANCE` を落としても全テストが緑のまま通る。
Issue 完了条件で明示要求された規約が回帰検出できない状態。

追加すべき最小テスト: 線長 1.0 のペアに対し `length = 1.0 - 1e-9` は Ok、`length = 1.0` は
`ChamferLengthTooLarge` を期待する 2 assert。

### A-3. solid レベルのテストが #296 と非対称 (B-rep 到達性が未検証) — **medium**

#296 Fillet の acceptance は 2D プロファイル検証に加えて 3 本の solid レベルテストを持つ:

| #296 test | 内容 |
|---|---|
| `t04_build_rectangle_corner_fillet` (`sketch_fillet_acceptance.rs:78-121`) | Extrude 後 `euler_poincare()==0` + 旧コーナー消滅 / tangent 点存在を z=0,3 両層で assert |
| `t06_extrude_uses_filleted_profile` (`:123-143`) | baseline 矩形 vs fillet 後で V/E 増加 + 両者 euler=0 |
| `t07_closed_loop_wraparound_corner` (`:145-`) | wraparound (l4/l1) を build 経路で検証 |

Chamfer 側 (`sketch_chamfer_acceptance.rs`) にはこの 3 本の対応物が無い。`euler_poincare` の assert は
T12 (Fillet+Chamfer 連鎖) の 1 箇所だけで、**chamfer 単体の solid が妥当かを固定するテストが無い**。
特に wraparound は `apply_sketch_chamfer_build` が派生 Line を配列**末尾**に挿入する
(`sketch_chamfer.rs:156`, `insert_at = n`) ため、出力配列順が `[l1', l2, l3, l4', chamfer_line]` という
非自明な並びになる。この並びを `build_bodies_from_features` の profile→Loop 変換が受け付けるかは
2D レベル (T01e/T06 の連続性チェック) だけでは保証できない — #296 が T07 をわざわざ置いたのは
まさにこの経路のためである。

実測 (欠けているテストを書いて回すと全て通る = 実装は正しい、カバレッジだけが無い):

```
PROBE_A euler=0 V=10 E=15 F=7
PROBE_A z=0 old_corner_present=false cut_a=true cut_b=true
PROBE_A z=3 old_corner_present=false cut_a=true cut_b=true
PROBE_B OK euler=0 V=10 E=15 F=7        (wraparound l4/l1)
PROBE_C baseline V=8 E=12 F=6 | chamfered V=10 E=15 F=7
```

機能バグではないが、Phase 10 の直前 Issue が確立した検証水準からの後退。

### A-4. ADR-017 §3 の API 抽象表が shipped Feature と矛盾したまま — **medium**

`docs/decisions/017-phase10-sketch-curves-and-edits.md:89`:

```
| Sketch Chamfer | `SketchChamfer` | `sketch_ref`, `vertex_ref`, `distance_a: f64`, `distance_b: f64` |
```

実装は対称単一 `length: f64`。plan.md は自律判断ログでこの乖離を宣言しているが、ADR 側には何も
書かれていない。Fillet (line 88 `vertex_ref: (e1_id, e2_id)`, `radius: f64`) は命名 narrowing のみで
意味は一致していたので #296 では問題化しなかったが、Chamfer は **パラメータ個数が 2→1 という意味論の
相違**であり、ADR が shipped API を誤記している状態になる。

影響: 「非対称 Chamfer は別 Issue」という Non-Goal を後続で拾う人が ADR-017 を読むと、
「既に distance_a/distance_b で実装済み」と誤読する。ADR-017 は Phase 10 全体の設計根拠なので
放置コストが高い。ADR-002 / ADR 運用に従い、ADR-017 に「#297 実装時 narrowing: 対称 `length` 単一
パラメータを採用、非対称は別 variant で追加予定」の注記を入れる (または改訂 ADR を起こす) のが筋。

### A-5. 満たしていることの確認 (invariant 破壊なし) — **none**

- **CRUD gate 6 箇所は Fillet の完全な鏡写し**。`feature_crud.rs` の Fillet 実装
  (`:261-286` / `:707-745`) と Chamfer 実装 (`:287-313` / `:748-790`) を行単位で突合し、
  `find_adjacent_pair` 呼び出し・Line 判定・`Err(InvalidParameter{kind}) => Some(kind)` の
  reason 伝播・`sketches_at` miss 時の pass-through まで同一構造であることを確認した。
  `feature_sketch_refs` / `feature_variant_name` / `set_feature_suppressed` も追加漏れなし。
- **登録漏れの網羅チェック**: `Feature::SketchOffset` の出現箇所 (最古の sketch 編集 op = 必要登録先の
  上位集合) を workspace 全体で grep し、`feature_crud.rs` 6 箇所 + `lib.rs` dispatch +
  `feature.rs` の `id()`/`is_suppressed()` が全てであることを確認。`engawa-api` / `engawa-cli` /
  `engawa-viewer` に Feature variant 別分岐は存在せず、追加不要。
- `is_body_producer` (`lib.rs:586-598`) は allowlist 型 `matches!` なので SketchChamfer は自動的に
  false。plan.md の「変更不要」は正しい。
- build 側の `suppressed: _` は問題なし — ループ先頭 (`lib.rs:206-208`) で
  `if feature.is_suppressed() { continue; }` が効く。
- 数値モデルは plan.md と一致: 非正/NaN/Inf → `InvalidParameter{kind:"length"}`、
  零長入力 → `chamfer_zero_length_input_line` (a/b 個別報告)、θ 退化 → `chamfer_corner_angle_degenerate`、
  長さ超過 → `ChamferLengthTooLarge`、判定順序も Fillet と同一。
- B-rep トポロジー保証: PROBE_A/B/C の実測で euler=0、F が 6→7 に増加 (面取り面 1 枚が正しく生成)。

---

## contrarian 観点 (採用方針の反論可能性)

### C-1. GLM の「TS drift check が実質同じ検証をしている」という mitigation は事実誤り — **medium**

`glm-self-review.md` は `include_str!` 化のリスク緩和根拠として
「`=== Checking TS drift ===` (main.rs:828-854) が実質同じ検証を行っている」と書いている。
これは誤り。当該ステップは `git status --porcelain -- web/src/generated/` を見るだけで、
**`cargo xtask gen-ts` を実行しない** (`main.rs:802-813` の `cargo_steps` は fmt/clippy/test/build のみ)。
したがって drift check が検出できるのは「gen-ts を実行して commit を忘れた」ケースのみで、
「Rust 型を変えたが gen-ts を実行していない」ケースは検出しない。

結論としては GLM の判断 (CI は依然 drift を捕まえる) は正しいが、根拠は別:
`t02_feature_tagged_union` は `actual` をテスト実行時に ts-rs で生成し直して
`assert_eq!(actual, FEATURE_GOLDEN)` する。`include_str!` は **compile 時**の commit 済みファイル内容を
取り込むので、この assert は「commit 済み Feature.ts == ts-rs 実行時出力」を今も厳密に検証している。
Rust→TS の drift 検出力は失われていない。

実際に失われたのは狭い 1 層のみ:
- ts-rs の出力フォーマット自体が変化 (dependency upgrade) した場合、Feature.ts については検出できない。
  ただし `TRIANGLE_MESH_GOLDEN` / `DOCUMENT_GOLDEN` / `COMPONENT_GOLDEN` / `ENTITY_REF_GOLDEN` 等が
  raw literal で残っているので workspace レベルでは検出できる。
- Feature.ts を手編集し、かつ Rust 型も整合的に手編集した場合の 3-way 検出。

また GLM が挙げた「path `../../../web/...` の脆弱性」は過大評価。同じ相対 `include_str!` は
`crates/engawa-build/tests/examples_smoke.rs` が `../../../examples/*.engawa` で既に多用している
確立パターンであり、かつ `#[cfg(test)] mod tests` 内なので `cargo build` は壊れない (test 対象のみ)。

残る本質的な問題は技術的リスクではなく **scope**: plan.md 「実装対象 / 既存ファイル修正」の 6 項目に
`crates/xtask/src/main.rs` は含まれておらず、debug-spec は raw string への 1 行追記を指示していた。
Feature variant 全体で共有される CI ガードの機構を、Issue の設計文書に載っていない形に置換した。
判断自体は妥当 (メンテコスト削減) だが、**plan.md への追記か ADR-017 作業チェックリストへの反映が
無いと、次に Feature variant を足す人が debug-spec 世代の手順書と矛盾した現実に当たる**。

### C-2. Non-Goals「CRUD gate の current-profile 再構築を見送る」は実装に正しく反映されている — **none (検証済み)**

これは task 指定の重点確認項目なので、T10/T11/T12 が実際に既知制約を固定しているかを個別に追跡した。

- **T11 (false-accept)**: `rect_profile` に c1(l1,l2) 適用後の current profile は
  `[l1', chamfer_line, l2', l3, l4]` (idx 0/1/2/3/4)。2 段目の c2(l1,l2) では
  `find_adjacent_pair` が idx1=0, idx2=2 を得て 4 分岐すべて外れ
  `sketch_fillet_elems_not_adjacent`。一方 gate は元 profile `[l1..l4]` を見るので通す。
  → テストは意図した乖離を正しく固定している。
- **T10 (false-reject)**: (a) build 経路では current profile に `l1` と `l1_l2_chamfer_line` が
  隣接 Line として存在 (corner=(9,0)、len_b=√2、θ=135°、length=0.5 で全ガード通過)。
  (b) gate は元 profile に `l1_l2_chamfer_line` が無く `sketch_fillet_elem_not_found`。
  → 乖離の向きは正しい。
- **T12**: Fillet が触らない l3/l4 を対象にすれば build も gate も通ることを確認しており、
  乖離が「派生要素 / 変形済み要素」に限局することを固定できている。

Non-Goals の宣言と実装・テストの対応は取れている。反論可能性として残るのは次の C-3/C-4 のみ。

### C-3. T10 の (a)/(b) が同一 history でない — **medium**

T10 (a) は `chamfer_rect_features` (= `[CreateSketch, c1, Extrude]`) の**末尾**に c2 を push する
(`sketch_chamfer_acceptance.rs:269-277`) 一方、(b) は `FeatureCrud::insert(&doc, bad_chamfer, 2)` で
**Extrude の前**に挿入する (`:297`)。「同じ feature 列に対して build は通るが gate は弾く」という
主張の対照実験になっていない (build 側では c2 が Extrude より後なので、どの body にも消費されない)。

実装挙動としては c2 を index 2 に置いても build は成功するので結論は変わらないが、
テストとしては主張より弱い。(a) を `features_build.insert(2, ...)` に揃えるべき。

### C-4. `ChamferLengthTooLarge` の `elem1_id`/`elem2_id` はユーザー入力順ではなく配列順 — **low**

`compute_chamfer` は `a_id`/`b_id` (= `find_adjacent_pair` が正規化した配列順) をエラーに載せる
(`sketch_chamfer.rs:92-93`)。ユーザーが `("l2","l1")` で呼んでも `elem1_id: "l1", elem2_id: "l2"` が
返る。Feature の docstring は「入力順は幾何と派生 Line ID に影響しない」とだけ書いており、
**エラーフィールドの順序も正規化される**ことは明記されていない。Fillet と同挙動なので整合性はあるが、
ADR-018 が「reason/kind 文字列は contract 外」と言っている一方 `elem1_id`/`elem2_id` は
構造化フィールド (= contract 内) なので、docstring に 1 行足す価値はある。逆順入力での
エラーフィールドを固定するテストも無い。

### C-5. T11 の false-accept は「document 全体が build 不能になる」ところまで踏み込んでいない — **low**

`build_bodies_from_features` は `?` で即 return するため、gate を通ってしまった c2 を含む Document は
**その feature だけが失敗するのではなく build 全体が失敗**する。Non-Goals はこの class を受容すると
宣言済みだが、テストのコメントは「build で初めて失敗する」までしか書いておらず、blast radius
(= ユーザーは該当 feature を delete するまで一切 build できない) が記録されていない。
Non-Goal の受容判断の重さを後から読み返す人には情報が足りない。

---

## migration 観点 (既存テスト互換 / 後方互換性 / public API 破壊)

### M-1. 既存 Fillet / Offset は無傷 — **none (実測確認)**

```
cargo test -p engawa-kernel --lib sketch_fillet   → 20 passed
cargo test -p engawa-kernel --lib sketch_offset   → 20 passed
cargo test -p engawa-build --test sketch_fillet_acceptance → 24 passed
```

`sketch_fillet.rs` / `sketch_offset.rs` は 1 行も変更されていない (diff で確認)。
Chamfer は `find_adjacent_pair` を read-only で import するのみ。

### M-2. `Feature` enum への追加は後方互換 — **none**

- variant は enum 末尾に追加、internally tagged (`#[serde(rename = "sketch_chamfer")]`) なので
  既存 `.engawa` の deserialize に影響しない。variant 順序依存は無い。
- `JsonSchema` / `TS` derive は enum 全体に付いているため自動追随。ts-rs 出力は union 末尾に
  member が 1 つ増えるだけ (`web/src/generated/Feature.ts` の diff がまさにそれ)。
- committed JSON schema golden は存在しない (`find -name "*.schema.json"` → 0 件) ので追随漏れなし。
- `id()` / `is_suppressed()` の or-pattern に追加済み。両方とも網羅 match (`_` 無し) なので
  追加漏れは compile error になる = 構造的に安全。

### M-3. `KernelError` に `#[non_exhaustive]` が無い — **low**

`crates/engawa-kernel/src/error.rs` の `KernelError` は `#[non_exhaustive]` を持たない
(`engawa-format::FormatError` と `engawa-build::FeatureCrudError` は持つ)。variant 追加は
crate 外の網羅 match を壊す semver-breaking 変更になる。#296 の `FilletRadiusTooLarge` も
同じ形で追加されており、pre-1.0 のため実害は無い。本 Issue で直すべきではないが、
Phase 12 Refactor Pass 候補として記録する価値がある。

### M-4. unit test と acceptance test の重複 + plan とのテスト ID 衝突 — **low**

`sketch_chamfer.rs` の `#[cfg(test)] mod tests` にある `t01_determinism_and_derived_line_id` /
`t01b_input_order_invariance` / `t02_normal_90deg` / `t_deg_chamfer_too_long` /
`t_determinism_100_runs_build` は、`sketch_chamfer_acceptance.rs` の T01 / T01e / T02 /
T_DEG_chamfer_too_long とほぼ同一内容の二重実装。`sketch_fillet.rs` も同じ構成なので慣行としては
一貫しているが、**unit test 側の `t01b` / `t01c` は plan.md のテスト表では
`suppressed` シリアライズ検証 (engawa-format 側) に割り当てられた ID** であり、
ID が別物を指している。テスト ID をトレーサビリティに使う運用 (plan.md のテスト表) と衝突する。

### M-5. follow-up Issue 起票が未了 (plan.md の明示的完了条件) — **medium**

plan.md Non-Goals 末尾が「Chamfer merge 後に `type: refactor, batch:kernel` で
『`feature_crud` の element-level gate を current profile ベースに再構築する』を起票する」を
**本 Issue の完了条件**と宣言している (#296 が同種の先送りを宣言して起票漏れした反省として明記)。
GLM も残課題 4 として挙げているが「(低)」評価。plan.md が完了条件に格上げしている以上、
STEP 8 で確実に実行する必要がある (medium)。同様に、ADR-017 作業チェックリストへの
「Feature variant 追加時に更新すべきファイル一覧」追記 (GLM 残課題 3) も未了。

---

## 結論

**critical 0 / high 1 / medium 6 / low 4**

high が 1 件あるため STEP 7 前に修正が必要。修正は test-only で production code 変更は不要
(実装ロジック自体は PROBE_A〜E で正しさを実測確認済み)。

### 必須修正 (high)

**A-1: `crates/engawa-build/tests/sketch_chamfer_acceptance.rs:419-433` `t_deg_corner_angle_flat`**

1. profile の `l2` を真に平坦なコーナーに直す:
   ```rust
   line("l1", [0.0, 0.0], [5.0, 0.0]),
   line("l2", [5.0, 0.0], [10.0, 0.0]),   // 旧: [5.0 + 1e-12, 0.0]
   ```
2. assert の OR 逃げ道を外し、`reason: "chamfer_corner_angle_degenerate"` のみを期待する:
   ```rust
   assert!(matches!(
       err,
       KernelError::DegenerateSketchElement {
           reason: "chamfer_corner_angle_degenerate",
           ..
       }
   ));
   ```
   実測で `element_id: "l1_l2"`, `reason: "chamfer_corner_angle_degenerate"` が返ることを確認済み。

### 同時に入れることを強く推奨 (medium、いずれも追加のみ・既存テスト非破壊)

- **A-2**: 長さ境界テストを `sketch_chamfer_acceptance.rs` に追加。線長 1.0 のペアで
  `length = 1.0 - 1e-9` → Ok、`length = 1.0` → `ChamferLengthTooLarge` の 2 assert。
  Issue 完了条件「`length` の比較規約を固定する」の回帰ガードになる。
- **A-3**: #296 の T04/T06/T07 に対応する solid レベルテスト 3 本を追加
  (chamfer 単体で `euler_poincare()==0` + 旧コーナー (10,0,z) 消滅 / cut 点 (9,0,z)(10,1,z) 存在、
  baseline vs chamfered の V/E 増加、wraparound l4/l1 の build 成功)。
  実測で全て通ることは確認済みなので、追加は緑のまま完了する。
- **C-3**: T10 (a) の `features_build.push(...)` を `features_build.insert(2, ...)` に変更し、
  (a)/(b) を同一 feature 位置に揃える。

### STEP 8 で必ず実行 (medium)

- **M-5**: follow-up Issue 起票 (`type: refactor, batch:kernel`、milestone 継承必須) —
  plan.md の明示的完了条件。
- **A-4**: ADR-017 §3 line 89 に #297 実装時 narrowing (対称 `length` 単一パラメータ採用、
  非対称は別 variant) の注記を追加。
- **C-1**: plan.md または ADR-017 作業チェックリストに、`FEATURE_GOLDEN` が `include_str!` 化されて
  手動追随不要になったこと / 逆に `examples_smoke.rs` と `golden_examples.rs` は依然手動追随が必要な
  ことを明記。debug-spec 世代の手順書と現実の矛盾を残さない。

### 別 Issue 推奨 (scope 外)

- #296 `crates/engawa-build/tests/sketch_fillet_acceptance.rs:392-406` の
  `t_deg_corner_angle_flat` は A-1 と同一の false-green 欠陥を持つ。Fillet 側の θ≈π 分岐も
  カバレッジ 0。`bug, batch:kernel` で起票。
- **M-3**: `KernelError` への `#[non_exhaustive]` 付与 (Phase 12 Refactor Pass 候補)。

## 追記: A-1 (high) 対応済み

上記 A-1 の修正案 (l2 座標 + assert OR 除去) をそのまま `sketch_chamfer_acceptance.rs` に適用し、
`cargo test -p engawa-build --test sketch_chamfer_acceptance t_deg_corner_angle_flat -- --exact` で
pass することを確認した。production code の変更は無い (test-only fix)。medium/low (A-2〜A-4, C-1,
C-3, M-3, M-5 等) はブロッキングでないため plan.md の Non-Goals/follow-up 起票にて後続対応とし、
STEP 7 の GLM final review・STEP 7.5 の Codex final gate を diff 更新後に再実行する
(barrier ルール「6.7 critical/high 検出 → 7/7.5 の結果は破棄」に従う)。
