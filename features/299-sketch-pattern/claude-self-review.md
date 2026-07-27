# Claude self-review (shift-left) — Issue #299 Sketch Pattern

STEP 7.5 (Codex 独立レビュー) の前に、Claude が architect / contrarian / migration の 3 観点で
GLM 実装 (`git diff d7d5cfa -- crates/ examples/`) の弱点を能動的に探索した結果。

**総合判定: critical 0 / high 0 / medium 4 / low 6 / なし 9**

指摘は全て実測 (一時 probe テストを `crates/engawa-build/tests/` に置いて `cargo test` で実行、
測定後に削除) で裏取りした。plan.md の設計意図・Non-Goals・judgment-summary.md の R01/R02 採択内容と
突き合わせ済み。既に STEP 3 / 3.5 で決着した論点の蒸し返しはしていない。

対象コミット範囲: `d7d5cfa..working-tree` (branch `cad/299-sketch-pattern`)。

---

## architect

既存 invariant / API 契約 / B-rep トポロジー保証を破る変更がないかを検査した。

### A1. `count: u32` が無制限 — 単一 YAML 行で OOM 可能 (**medium**)

`crates/engawa-kernel/src/geometry/sketch_pattern.rs:108` / `:181`

```rust
for k in 1..count {          // count: u32、上限チェックなし
    appended.push(rename_to(&translated, &derived_id));
}
```

`count` は `u32` で上限ガードが無く、`build_bodies_from_features` は `.engawa` の値をそのまま渡す。

実測:
- `size_of::<SketchElement>() = 72` バイト + id `String` のヒープ (約 30 バイト)
- `count = 200_000` で 200,000 要素を実際に生成することを確認 (probe `probe_a1_unbounded_count` pass)
- 線形外挿で `count = 1e8` → 約 10 GB、`count = u32::MAX` → 約 430 GB

`count: 30000000` のような桁ミス 1 つで `engawa` CLI が OOM kill される。Pattern は
「繰り返し回数」パラメータを持つ **最初の Feature** であり、リポジトリ内に前例となる上限定数は存在しない
(`grep` で確認済み。Offset/Fillet/Chamfer/Mirror はいずれも出力要素数が入力要素数で bounded)。

- plan.md / Non-Goals には資源上限の記述が無い → 設計時に見落とされた軸
- 推奨: `MAX_PATTERN_COUNT`(例 10_000) を `geometry/math.rs` か `tolerance.rs` に追加し
  `InvalidParameter { kind: "sketch_pattern_{linear,circular}_count_too_large" }` で fail-fast。
  または follow-up Issue 化して Non-Goals に明記する。

### A2. CRUD gate の既存パターン整合性 — 問題なし (**なし**)

`crates/engawa-build/src/feature_crud.rs` の 6 箇所すべてが Mirror (#298) と同型で更新済みなのを確認した。

| 関数 | 行 | 形 | Mirror との整合 |
|---|---|---|---|
| `feature_sketch_refs` | 118-119 | `vec![sketch.as_str()]` | 一致 (`_ => vec![]` の前に配置) |
| `feature_variant_name` | 164-165 | 文字列返却 | 一致 (wildcard 無し = コンパイラ強制) |
| `refs_resolve_in_state` | 336-339 | `sketches_at.contains_key(sketch)` | 一致 (隣接ペア制約なし、`_ => true` の前) |
| `simulate_history` | 537-558 | suppressed → refs → `executed_at.insert(i)` | 一致 (body 非登録) |
| `check_refs_resolve_before` | 893-959 (gate 本体 913 / 940) | element-level gate | 一致 + `count >= 2` 追加条件 |
| `set_feature_suppressed` | 1234-1236 | `*suppressed = on` | 一致 |

gate の到達経路も確認した: `check_refs_resolve_before` は `insert` (1272) / `edit` (1324、
`check_refs_resolve_before_for_edit` 経由) / `suppress` (1393) の 3 経路から呼ばれる。
したがって「`count=1` で insert → `edit` で `count=3` に変更して未知 id を潜り込ませる」抜け道は無い
(edit 時に gate が再評価される)。

`feature_body_refs` に Pattern を足していないのも正しい (body を一切参照しない)。

### A3. `Feature::id()` / `is_suppressed()` の match 漏れ — 無し (**なし**)

`crates/engawa-format/src/feature.rs:713-714` / `:734-735` の両方に追加済み。
`Feature` enum の match は wildcard を持たないためコンパイラ強制。

コンパイラが守ってくれない **wildcard 付き match** をリポジトリ全域で列挙して個別確認した:

| 箇所 | wildcard | Pattern の扱い | 判定 |
|---|---|---|---|
| `feature_crud.rs:111` `feature_sketch_refs` | `_ => vec![]` | 明示追加済み | OK |
| `feature_crud.rs:138` `feature_body_refs` | `_ => vec![]` | 意図的に非追加 (body 参照なし) | OK |
| `feature_crud.rs:246` `refs_resolve_in_state` | `_ => true` | 明示追加済み | OK |
| `feature_crud.rs:707` implicit-ref producer 判定 | `_ => None` | Pattern は body producer ではないので非追加が正 | OK |
| `document.rs:320` `validate_component` | `_ => {}` | 検証対象の座標フィールドを持たないので非追加が正 | OK |
| `build/src/lib.rs:218` dispatch | wildcard 無し | 両 variant 追加済み | OK |
| `build/src/lib.rs:200` plane 抽出 | `_ => None` | 非該当 | OK |

`is_body_producer` 相当のロジックも変更不要 (plan §3 の想定どおり)。

### A4. Arc の絶対角度が正規化されない (**low**)

`sketch_pattern.rs:300-301`

```rust
start_angle: start_angle + angle,
end_angle: end_angle + angle,
```

`angle = step * k` は最大 `total_angle * (count-1)/count` まで大きくなるため、Pattern は
**`[0, 2π)` / `[-π, π]` の外側の Arc 角度を生成する最初の Feature** になる
(Mirror は `atan2` 経由で常に `[-π, π]`、Fillet も `atan2` 由来)。

下流の consumer を全て確認した結果、invariant 違反は無い:

- `crates/engawa-kernel/src/tessellation/sketch.rs:82` — `sweep = end_angle - start_angle` を使い
  絶対値に依存しない。`is_finite()` チェックのみ (line 70) で、大きな有限値は通る
- `crates/engawa-kernel/src/geometry/sketch_offset.rs:199` — `end_angle < start_angle` を拒否。
  剛体回転は両角に同一値を加えるため sweep 符号は不変 → invariant 保持
- `validate_profile_closed` は cos/sin 経由の端点比較で周期的 → 影響なし

残る実害は `total_angle` が非常に大きい場合の `cos`/`sin` の引数レンジ縮約による精度劣化のみ。
Non-Goals の「多重回転」に含まれる範囲であり、現状では受容可。

### A5. B-rep トポロジー保証 — 非該当 (**なし**)

`build_bodies_from_features` の Pattern 節は `built_sketch_profiles` のみを更新し、
`built` / `live_bodies` / `IdGenerator` に一切触れない。Solid を生成しないため
Euler-Poincaré・watertight・HalfEdge ペアリングの各 invariant は非該当 (plan §幾何的不変条件チェックリストと一致)。

### A6. 派生 id の衝突検出が source のみ対象 — 健全 (**なし**)

`sketch_pattern.rs:100` の `existing_ids` は `source` の id だけを集め、同一呼び出し内で
`appended` に積んだ id とは突き合わせていない。これが穴かを検証した結果、**穴ではない**:

`e1 + S + k1 == e2 + S + k2` (`S = "_pattern_linear_"`, `k` は数字のみ) が成り立つには、
`S` の最終出現位置が一致する必要がある。`k` は数字のみなので付加した `S` が必ず最終出現になり、
`|e1| == |e2|` → `e1 == e2` → `k1 == k2`。つまり異なる source 要素どうしの派生 id 衝突は原理的に起こらない。

実測で裏取り (`probe_c8_no_intracall_collision`):
- source に `"c"` と `"c_pattern_linear_1"` を並べて `count=2` → `DegenerateSketchElement { element_id: "c_pattern_linear_1", reason: "pattern_linear_duplicate_id" }` で正しく検出
- `"c_pattern_linear_1"` のみ選択して `count=3` → `["c", "c_pattern_linear_1", "c_pattern_linear_1_pattern_linear_1", "c_pattern_linear_1_pattern_linear_2"]` で全 id ユニーク

唯一の理論上の抜けは「source 自体に重複 id がある場合」だが、`CreateSketch.profile` は
`validate_sketch_element_ids` (`build/src/lib.rs:694`) で重複拒否され、各 sketch-edit も個別に
duplicate gate を持つため到達不能。

### A7. 文字列ディスパッチ `mode: &'static str` (**low**)

`sketch_pattern.rs:203-225` の `resolve_selection` は `mode` を `"circular"` / それ以外で分岐する。
呼び出し側が `"Circular"` などと書いても型エラーにならず、silently linear 側の error kind を返す。
同様に `translate_element` は linear の kind、`rotate_element` は circular の kind をハードコードしており、
ヘルパが片方の caller に暗黙結合している。

現状は両モードともテスト済みなので実害なし。enum (`enum PatternMode { Linear, Circular }`) にすれば
コンパイラ強制になる。style レベル。

### A8. `rename_to` の `unreachable!()` (**low**)

`sketch_pattern.rs:337-339`。`translate_element` / `rotate_element` が Ellipse/Conic を先に
`Err` で弾くため到達しないが、kernel に panic 経路が 1 本増えている。
`SketchElement` に新 variant が追加されると (Rectangle/Polygon/Slot は ADR-017 のロードマップ上に存在する)
transform 側の match はコンパイラが検出するが、`rename_to` は wildcard 相当の腕に吸収されて
panic に落ちる可能性がある。`translate_element` の戻り値を直接使い `rename_to` を廃す構造にすれば消える。

---

## contrarian

採用方針への反論可能性と、plan.md の数値モデル・自律判断ログとの一致を実測で検証した。

### C1. plan.md「パラメータ検証で複製の座標一致を完全に防げる」は偽 (**medium**)

plan.md §設計方針:

> Pattern の複製がオリジナルと幾何的に一致するのは「オフセット/回転角が実質ゼロのとき」のみであり、
> これは入力パラメータレベルの検証（`distance=0` かつ `count>=2`、`total_angle≈0` かつ `count>=2`）で
> **完全に防げる**。よって Mirror の `is_axis_coincident` のような複製後の事後幾何比較は不要

この主張は **反例が存在する**。`step = 2π·m` (整数 m) のとき全複製がオリジナルと厳密に重なるが、
`|step| = 2π > ANGLE_TOLERANCE` なので `sketch_pattern_circular_angle_degenerate` は発火しない。

実測 (`probe_c1_full_turn_step_coincident`、`total_angle = 4π, count = 2`):

```
orig=[3.0, 0.0]  copy id=c1_pattern_circular_1  center=[3.0, -7.34e-16]  r=1
dx=0  dy=7.3e-16
```

複製は浮動小数の丸め誤差 (7e-16) の範囲でオリジナルと完全一致する。同様に `total_angle=4π, count=4`
なら k=2 の複製だけがオリジナルに重なる。

重要な区別:
- **仕様違反ではない** — Non-Goals に「`total_angle` が 2π を超える多重回転による座標重複は許容する」と
  明記済みなので、挙動そのものは決着済み
- **根拠の記述が誤っている** — 「事後幾何比較が不要」の論拠が「パラメータ検証で完全に防げるから」に
  なっており、それが偽。同じ論拠を将来の Pattern 系 Issue が引用すると誤った判断につながる

推奨 (どちらか):
1. 記述修正のみ: plan.md §設計方針 の「完全に防げる」を「`step` が 2π の整数倍のケースは防げないが
   Non-Goals で許容する」に改める (コード変更なし)
2. 検査強化: 退化判定を `(step.rem_euclid(TAU)).min(TAU - step.rem_euclid(TAU)) <= ANGLE_TOLERANCE`
   に置き換える。ただし Non-Goals の「多重回転許容」と衝突するため要判断

### C2. 「`count=1` は真の no-op」は不正確 — direction 退化は count=1 でも発火 (**low**)

`sketch_pattern.rs` のチェック順は plan.md 疑似コード (step 1→2→3→4→5) どおりだが、その結果:

| 入力 | kernel | `FeatureCrud::insert` |
|---|---|---|
| `count=1, direction=[0,0]` | **Err** `sketch_pattern_linear_direction_degenerate` | **Ok** |
| `count=1, distance=0.0` | Ok (no-op) | Ok |
| `count=1, selection=[未知 id]` | Ok (no-op) | Ok (R01 の gate 条件) |

実測 (`probe_c2_*`) で 3 行とも確認済み。

つまり `direction` (と Circular の `center` / `total_angle` の非 finite) だけが count=1 の
早期 return より **前** にあり、`distance` は **後** にある。「count=1 は真の no-op」という
doc comment (`sketch_pattern.rs:10-11`、`feature.rs:638` / `:652`) の記述と実挙動が一致していない。

さらに `count=1, direction=[0,0]` は「build は Err だが insert は Ok」という、
Codex R01 が解消しようとしたのと同じ形の build/CRUD 非対称を残している。

反論の余地: 既存の CRUD gate は `SketchOffset.distance` / `SketchFillet.radius` の
ジオメトリ値を一切検証しない (「gate は参照解決のみ」既存方針) ため、この非対称は前例と整合しており、
R01 が対象とした「参照解決」レイヤの話ではない。よって **修正不要、記述の精緻化のみ推奨** と判定する。
doc comment を「count=1 は selection を参照しない no-op。ただしパラメータの finite / direction 退化
チェックは先に走る」に直せば十分。

### C3. R01 (CRUD gate の `count>=2` 限定) — 実装は正しいが Circular 側が完全に未テスト (**medium**)

R01 の反映は正確:

```rust
// feature_crud.rs:913 (Linear), :940 (Circular)
if *count >= 2 && !selection.is_empty() { ... }
```

Linear / Circular 両方に同一条件が入っており、`count == 0` を gate 対象外にする判断も
コメント (`:895-901`) に明記されている。

Linear は gate の両側がテストされている:
- `t_crud_pattern_n1_no_validation` (count=1 + 未知 id → Ok)
- `t_crud_pattern_unknown_element` (count=3 + 未知 id → `sketch_pattern_linear_elem_not_found`)

**しかし `sketch_pattern_circular_elem_not_found` を検証するテストが 1 つも無い。**
`grep -rn "sketch_pattern_circular_elem_not_found" --include=*.rs crates/` の結果は
`feature_crud.rs` の実装 1 箇所のみ。`t_crud_pattern_n1_no_validation` は Circular の
count=1 側 (gate がスキップされる側) だけを踏んでおり、count>=2 で gate が発火する側は未踏破。

Circular gate は Linear gate の 34 行コピペブロックであり、reason 文字列を間違えていても
現在のテストスイートでは検出できない。実測で手動確認した結果、現状は正しく動く:

```
probe_c3: SketchElementNotResolved { feature_id: "pc1", sketch_ref: "sk1",
          elem1_id: "nope", elem2_id: "", reason: "sketch_pattern_circular_elem_not_found" }
```

推奨: `t_crud_pattern_unknown_element_circular` を `sketch_pattern_acceptance.rs` に追加
(既存 Linear 版のコピーで `SketchPatternCircular` / `count: 3` / reason を差し替えるだけ)。

### C4. R02 (T04 期待値修正) — 完全に反映済み (**なし**)

judgment-summary.md STEP 3.5 R02 で採択された修正値と実装 assertion を実測で突き合わせた。

`t04_circular_arc_numeric_model` (acceptance 版 `:149-177` / kernel inline 版 `:499-528` の両方):

| 項目 | R02 採択値 | 実装 assertion | 一致 |
|---|---|---|---|
| step | `total_angle/count = π/2` | `apply_..._circular(.., 2, PI)` (= π/2) | ✓ |
| 複製個数 | k=1 の 1 個 | `assert_eq!(out.len(), 2)` | ✓ |
| center | `(0, 2)` | `center[0]≈0.0`, `center[1]≈2.0` | ✓ |
| start_angle | `π/2` | `(start_angle - FRAC_PI_2).abs() < ε` | ✓ |
| end_angle | `π` | `(end_angle - PI).abs() < ε` | ✓ |

棄却された Codex 提案 (b) (`total_angle/(count-1)` への変更) が実装に混入していないことも確認
(`sketch_pattern.rs:164` は `total_angle / count as f64`)。

`cargo test -p engawa-build --test sketch_pattern_acceptance` を実行し 19 tests 全 pass、
`#[ignore]` 0 件を実測確認した (test-spec.md が STEP 6 時点で列挙した 12 件の `#[ignore]` は
STEP 6.6 で全て解除・実装済み)。

### C5. 数値モデル・自律判断ログとの一致 — 全項目一致 (**なし**)

| plan.md の決定 | 実装箇所 | 一致 |
|---|---|---|
| `count` = 元を含む総数、複製は `k=1..count` | `:108` `for k in 1..count` | ✓ |
| `count=0` → error / `count=1` → no-op | `:82-90` | ✓ |
| `step = total_angle / count` | `:164` | ✓ |
| `unit_dir = direction/|direction|`、`hypot` で overflow 回避 | `:71`, `:97` | ✓ |
| `offset_k = unit_dir * distance * k` | `:109` | ✓ |
| 回転式 `v' = (vx·cos - vy·sin, vx·sin + vy·cos)` | `:366-373` | ✓ |
| Arc は両角に `angle_k` 加算 (sweep 不変) | `:300-301` | ✓ |
| 平行移動は Arc 角度不変 | `:249-255` | ✓ |
| `selection` 空 = 全要素、BTreeSet で重複除去 | `:203-225` | ✓ |
| 走査順は source 配列順 (selection 入力順非依存) | `:103` `for elem in source` | ✓ |
| `LENGTH_TOLERANCE`/`ANGLE_TOLERANCE` 再利用、新規 ε なし | `:44` import のみ | ✓ |
| 派生 id `{eid}_pattern_{mode}_{k}` | `:111`, `:184` | ✓ |
| 新規 `KernelError` variant を追加しない | 既存 3 種のみ使用 | ✓ |
| ADR-017 の `SketchPattern{kind}` 不採用、2 variant 分離 | `feature.rs:645` / `:671` | ✓ |
| `selection: Vec<String>` / `[f64;2]` 命名慣習 | 同上 | ✓ |

### C6. 宣言されている error kind のうち 4 種が未テスト (**medium**)

`sketch_pattern.rs` の module doc は 8 種 (+ circular ミラー) の error kind を契約として宣言しているが、
うち 4 種はテストスイートに 1 度も現れない。`grep -rn "<kind>" --include=*.rs crates/` の実測結果:

| error kind | 出現箇所 | テスト |
|---|---|---|
| `sketch_pattern_linear_distance_invalid` | 実装 + doc のみ | **なし** |
| `sketch_pattern_circular_angle_invalid` | 実装のみ | **なし** |
| `sketch_pattern_circular_center_degenerate` | 実装のみ | **なし** |
| `pattern_circular_duplicate_id` | 実装のみ | **なし** |
| `sketch_pattern_circular_elem_not_found` | 実装のみ (C3 と同件) | **なし** |
| 他 6 種 | 実装 + acceptance | あり |

Mirror (#298) は非 finite 入力を明示的にテストしている
(`sketch_mirror.rs:564` NaN axis_p1 / `:581` Inf axis_p2) ため、Pattern は **前例の水準を下回っている**。
NaN/Inf 入力は `.engawa` の手書き・外部ツール生成いずれからも到達し得る。

plan.md のテスト計画表に対応 ID が無いのが原因 (§数値モデルの退化判定リストには 6 条件すべて挙がっているが、
テスト表には NaN 系 3 件と circular duplicate id が落ちている)。plan 側の穴を GLM がそのまま継承した形。

推奨: 4 テストを `sketch_pattern_acceptance.rs` に追加 (各 5 行程度)。

### C7. `t01_determinism` の Document レベル assertion が空振り (**low**)

`sketch_pattern_acceptance.rs:78-80`

```rust
let a = build_bodies_from_features(&features, &ref_planes, &mut gen_a).unwrap();
let b = build_bodies_from_features(&features, &ref_planes, &mut gen_b).unwrap();
assert_eq!(a.len(), b.len());
```

fixture には body 生成 Feature が無い (CreateSketch + Pattern × 2) ため `a` も `b` も
空 Vec であり、`assert_eq!(0, 0)` になっている。build 経路を通した profile の決定性は何も検証していない。

kernel 直呼びの `t01` 前半と `t_determinism_100_runs` (`sketch_pattern.rs:560`) で
決定性そのものは十分に押さえられているので実害は小さいが、この 3 行は「通っているように見えて
何も守っていない」テストになっている。`build_bodies_from_features` は
`built_sketch_profiles` を返さないため、Document レベルで profile 決定性を検証するには
Pattern の後に `extrude` を足して body を比較する必要がある (ただし plan の Non-Goals
「golden example に extrude を含めない」とは別物なのでテスト内なら可)。

### C8. Ellipse/Conic 拒否のタイミング (**なし**)

`translate_element` / `rotate_element` の中で遅延判定しており、`resolve_selection` 時点では
種別を見ない。したがって `selection` に Ellipse を含めても `count=1` なら Ok になる。
これは plan.md 自律判断ログ §6 の「count=1 は selection の中身を検証しない」と整合しており、
test-spec.md でも意図どおりと確認済み。問題なし。

---

## migration

既存テスト互換性・後方互換性・public API 破壊を検査した。

### M1. public API 破壊なし (**なし**)

- `Feature` enum への **追加のみ**。既存 variant のフィールド・serde tag は一切変更なし
- `crates/engawa-kernel/src/geometry/sketch_pattern.rs` は新規モジュール。
  既存の `sketch_mirror` / `sketch_offset` / `sketch_fillet` / `sketch_chamfer` に変更なし
- `build_bodies_from_features` / `FeatureCrud::*` のシグネチャ不変
- 新規 `KernelError` / `FeatureCrudError` variant なし (既存 variant の再利用のみ)
  → downstream の error match が壊れない

### M2. `schema_version` は 2 のまま — 正しい (**なし**)

variant 追加は既存ドキュメントの読み込み互換を壊さない (additive)。
Mirror (#298) / Chamfer (#297) / Fillet (#296) も同様にバンプしていない前例と一致。
`document.rs:185` の v1→v2 migration hook は feature tag を列挙していないため影響なし。

「新しい tag を古い reader が読めない」forward-incompat は variant 追加に本質的に伴うもので、
ADR-017 §4 の v2 バンプ判断 (`SketchElement` の wire format 変更 = breaking) とは性質が異なる。

### M3. 既存テスト・golden への影響なし (**なし**)

- `examples/*.engawa` の既存ファイルは無変更。`golden_examples.rs` / `examples_smoke.rs` は
  新規テスト関数の追加のみで既存関数に手を入れていない
- 委譲されている JSON schema golden ファイルはリポジトリに存在しない
  (`find . -name "*.schema.json"` → 0 件) ため再生成漏れの余地なし
- `crates/xtask/src/main.rs:1086-1093` に `sketch_pattern_linear` / `sketch_pattern_circular` の
  タグ assertion を追加済み

### M4. TS 生成物の同期 — 済み (**なし**)

`web/src/generated/Feature.ts` に両 variant が生成済み (`:73-` 以降)。
フィールド名・optional マーカー (`selection?`, `suppressed?`) が Rust 側の
`skip_serializing_if` と一致していることを確認した。

web 側に feature type で分岐する switch/case は存在しない
(`grep -rn "case \"sketch" web/src` → 生成物のみヒット) ため、手書きコードの追随は不要。

### M5. `Feature` enum の網羅性チェックはコンパイラ強制以外にも穴がない (**なし**)

A3 で列挙したとおり、wildcard 付き match 7 箇所を全て個別に検査し、Pattern を追加すべき箇所
(2 箇所) は追加済み、追加すべきでない箇所 (5 箇所) は正しく非追加であることを確認した。

加えて、実行時に variant 名文字列を扱う箇所 (`feature_variant_name`) は wildcard 無しの
全網羅 match であり、追加漏れがあればコンパイルエラーになる。

### M6. serde 契約 (**なし**)

- `count` / `direction` / `distance` / `center` / `total_angle` に `#[serde(default)]` を
  付けていない = 必須フィールド。欠落した YAML は deserialize エラーになる (正しい)
- `selection` は `#[serde(default, skip_serializing_if = "Vec::is_empty")]`、
  `suppressed` は `#[serde(default, skip_serializing_if = "std::ops::Not::not")]` で Mirror と同型
- roundtrip テスト 3 本 (`feature.rs:2119` / `:2170` / `:2192`) が省略/出力の両方を assert しており、
  `examples/sketch_pattern.engawa` が `selection: []` を書いても golden 出力から落ちることを
  `golden_sketch_pattern` が固定している

### M7. ADR-017 §3 Decision Matrix が実装と乖離したまま (**low**)

`docs/decisions/017-phase10-sketch-curves-and-edits.md:91`

```
| Pattern | `SketchPattern` | `sketch_ref`, `kind: PatternKind` (`Rect` / `Polar`), `count_u`, `count_v`, `spacing` |
```

(ADR-017 の該当行は `:91`。) 実装は `SketchPatternLinear` / `SketchPatternCircular` の 2 variant で、
フィールド構成も全く異なる。
乖離は plan.md 自律判断ログ §1 に記録されているが、**ADR 本体には反映されていない**。

同じ行の 1 つ上 (`:90`) の Mirror も `mirror_line: MirrorLine (EntityRef | LineEq)` のままで、
実装の `axis_p1` / `axis_p2` と乖離している (#298 が同じ判断をして ADR を更新しなかった)。
つまり ADR-017 §3 は 7 行中 2 行が実装と食い違う状態で、今後 Trim/Extend が同じ経路を通ると
さらに増える。

Phase 10 の残 Issue が ADR-017 §3 を設計の起点として参照すると誤誘導される。
ADR-002 の Phase 完了手続きの一環として「§3 に実装差分の追記 (または Superseded 注記)」を
入れるのが望ましい。本 Issue 単独の責務ではないので **low**、横断 Issue (#331 と同様の
ドキュメント整合 Issue) 化を推奨。

---

## 推奨アクション優先度

| # | 指摘 | 重要度 | 推奨対応 | 本 Issue 内で直すか |
|---|---|---|---|---|
| A1 | `count` 無制限 → OOM | medium | `MAX_PATTERN_COUNT` guard 追加 or Non-Goals 明記 + follow-up Issue | どちらか必須 |
| C3 | Circular CRUD gate 未テスト | medium | `t_crud_pattern_unknown_element_circular` 追加 (10 行) | 直すべき |
| C6 | error kind 4 種が未テスト | medium | acceptance に 4 テスト追加 (各 5 行) | 直すべき |
| C1 | plan の「完全に防げる」が偽 | medium | plan.md §設計方針 の記述修正 (コード変更不要) | 直すべき |
| C2 | 「真の no-op」の doc 不正確 | low | doc comment の精緻化 | 任意 |
| C7 | 空振り determinism assertion | low | fixture に extrude 追加 or 3 行削除 | 任意 |
| A4 | Arc 絶対角度の非正規化 | low | 現状維持 (下流は全て sweep ベースで安全) | 不要 |
| A7 | `mode: &'static str` 分岐 | low | enum 化 | 不要 |
| A8 | `rename_to` の `unreachable!()` | low | 構造変更で除去 | 不要 |
| M7 | ADR-017 §3 の乖離 | low | 横断ドキュメント Issue 化 | 不要 |
