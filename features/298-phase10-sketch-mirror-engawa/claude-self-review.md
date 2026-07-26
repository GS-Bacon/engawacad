# Claude Self-Review (STEP 6.7) — Issue #298 Sketch Mirror

対象 diff: `origin/claude/add-claude-guidelines-BKKtD` → `cad/298-phase10-sketch-mirror-engawa` (working tree 含む)
実施方法: 差分 + plan.md の精読に加え、一時 probe テスト (`crates/engawa-build/tests/zz_selfreview_probe.rs`、実行後削除) で全ての主張を実測検証した。以下「実測」と書いた項目は再現ログを取得済み。

**判定サマリ**: critical 0 / **high 1** / medium 5 / low 5

| # | 観点 | 重要度 | 要旨 |
|---|---|---|---|
| A1 | architect | **high** | `refs_resolve_in_state` に SketchMirror arm がなく CRUD/build 整合性が破れる |
| A2 | architect | medium | `check_refs_resolve_before` の element-level gate 欠落 (兄弟 3 feature との非対称) |
| A3 | architect | medium | Arc 反射角が plan.md の導出式と別実装 (数値的に劣る)。退化検出が false negative |
| A4 | architect | low | Arc sweep 符号反転 × SketchOffset chain は Non-Goals 記載で妥当 |
| A5 | architect | low | 2D ベクトル helper が 3 ファイル目の重複 (kernel CLAUDE.md 規約と逆行) |
| C1 | contrarian | medium | 派生 id `{elem}_mirror` 固定で mirror の連鎖 (4 回対称) が構造的に不可能 |
| C2 | contrarian | medium | on-axis swapped 比較は `|sweep| >= 2π` で false negative |
| C3 | contrarian | medium | axis 座標が巨大 (>1.3e154) だと **線対称でなく原点対称**になる silent bug |
| C4 | contrarian | low | mirror 結果の非有限値 (inf/NaN) を検証していない |
| C5 | contrarian | low | `mirror_duplicate_id` が同一 call 内で生成される id の衝突を見ない |
| M1 | migration | medium | `golden_examples.rs` に `golden_sketch_mirror` がない (#295-297 は全て追加済み) |
| M2 | migration | low | `xtask` `t02_feature_tagged_union` の tag assert 一覧に `sketch_mirror` がない |

---

## architect

### A1 [HIGH] `refs_resolve_in_state` に `Feature::SketchMirror` の arm がなく、CRUD 層が保証した Document がビルド不能になる

`crates/engawa-build/src/feature_crud.rs:216-322`。`SketchOffset` (:249) / `SketchFillet` (:263) / `SketchChamfer` (:289) は全て `sketches_at` に対する sketch 存在チェックを持つが、`SketchMirror` は arm が無く `_ => true` (:321) に落ちる。`simulate_history` (:504-515) が「実行された」と判定し続けるため、`delete` / `suppress` の pre/post `executed_at` 比較 (:1340-1367 / :1242) が壊れた consumer を検出できない。

plan.md L56 は「sketch 自体の存在確認は `feature_sketch_refs` 経由で別途行われる」としているが、**これは誤り**。`feature_sketch_refs` は `check_refs_resolve_before` (insert 経路) と `feature_transitive_implicit_body_refs` でしか使われず、`simulate_history` の gate ではない。

**実測** (probe1):

```
SketchOffset delete(sk1) => Err: edit of feature "sk1" would break consumer "off1" at index 1
SketchMirror delete(sk1) => Ok (ALLOWED)          ← 通ってしまう
  → post-delete build   => Err: sketch not found: sk1   ← ビルド不能な Document が生成される
SketchMirror suppress(sk1) => Ok (ALLOWED)        ← suppress も同様
```

`FeatureCrud` の公開契約は「壊れた consumer を残す編集は `EditBreaksConsumer` で拒否する」であり、それが破れている。#297 (SketchChamfer) は同じ arm を明示的に追加している (`e89357d`)。

**修正案** (再現テスト先行):

```rust
// feature_crud.rs refs_resolve_in_state 内、SketchChamfer arm の後
Feature::SketchMirror { sketch, .. } => sketches_at.contains_key(sketch.as_str()),
```

再現テストは `FeatureCrud::delete` / `suppress` が `EditBreaksConsumer` を返すことを検証する形で `feature_crud.rs` の `mod tests` か `crates/engawa-build/tests/sketch_mirror_acceptance.rs` に追加する。

### A2 [MEDIUM] `check_refs_resolve_before` に SketchMirror の element-level gate がない

`feature_crud.rs:703` (SketchOffset) / `:722` (SketchFillet) / `:768` (SketchChamfer) には insert 経路の element-level gate があるが、`SketchMirror` 相当がない。結果、`selection` に profile 内に存在しない element id を指定した Feature を `FeatureCrud::insert` できてしまい、エラーは build 時 (`sketch_mirror_unknown_element_id`) まで遅延する。

Mirror は `selection` 空を許容する仕様なので Fillet/Chamfer ほど必須ではない (gate は ref-resolution チェックであってジオメトリ検証ではない、という :764-767 の明記された方針とも整合する) が、`selection` が非空のときだけでも検査すれば兄弟 feature と対称になる。A1 と同じ関数群なので同時に直すのが安い。

### A3 [MEDIUM] Arc 反射角の実装が plan.md の導出式と異なり、数値的に劣る方を採っている

plan.md L161 は導出根拠として `new_start = 2φ - start_angle` (φ = 軸方向角) を明示している。しかし `sketch_mirror.rs:129-136` の実装は point 往復 (`p_start` を反射 → `atan2` で角度を読み直す) を採っている。数学的には等価だが、`|center| >> radius` のとき `p_start_m - new_center` が大きな数どうしの差になり桁落ちする。

**実測** (probe10、45 度軸):

| center | radius | 実装の誤差 | ANGLE_TOLERANCE 比 |
|---|---|---|---|
| (1e6, 1e6) | 1e-3 | 1.50e-8 rad | 15 倍 |
| (1e10, 1e10) | 1.0 | 7.07e-7 rad | 707 倍 |

`2φ - θ` を使えば誤差は ~1e-16 rad で center の大きさに依存しない。

**影響 1 (退化検出の穴)**: `is_axis_coincident` の Arc 判定 (`sketch_mirror.rs:199-203`) が `ANGLE_TOLERANCE` (1e-9) を使っているため、真に軸対称な Arc が検出されず重複複製が黙って追加される。**実測** (probe4b): center=(1e6,1e6) r=1e-3、center=(1e8,1e8) r=1e-3、center=(1e10,1e10) r=1.0 の 45 度軸対称 Arc は全て `Ok (coincidence NOT detected)`。plan.md L169 が根拠にしている「Opus 4.7 fuzz 20000 ケース mismatch 0」は原点近傍のサンプルしか踏んでいないと推定される。

**影響 2 (出力精度)**: mirrored Arc の `start_angle` 自体がこの誤差を持つため、端点位置が `radius × err` ずれる (1e10 ケースで 7e-7 mm)。

**修正案** (1 行):

```rust
let phi = d[1].atan2(d[0]);
let new_start = 2.0 * phi - start_angle;
// p_start / p_start_m / atan2 の 3 行は不要になる
```

`new_start` が (-π, π] に正規化されなくなるが、`is_axis_coincident` は `angles_near_mod_2pi` を使っており、`tessellation/sketch.rs` も `sweep.abs()` しか見ないので問題ない (現行実装も `new_end` は既に非正規化)。T04 の期待値 (`start=0, end=-π/2`) も 2φ=0 なので変わらない。

### A4 [LOW / 妥当] Arc sweep 符号反転 × SketchOffset の chain

`Mirror` は必ず sweep 符号を反転させるため、Arc を含む mirror 結果に `SketchOffset` を掛けると常に `arc_negative_sweep` になる。plan.md Non-Goals L33 に明記済みで、STEP 3.5 の R01 修正 (Line-Arc-Line 連結の端点対応を保つ) とのトレードオフとして妥当。負 sweep 自体は `tessellation/sketch.rs` が `sweep.abs()` でしか判定せず既存テスト `t_edge_negative_sweep_arc_succeeds` (`sketch.rs:663`) が許容を明示しているので、Arc 型の不変条件違反ではない。**弱点ではない**と判断。

B-rep トポロジー保証についても、本 Feature は body を生成せず `built_sketch_profiles` のみを更新する (`lib.rs:568-592`) ため Euler-Poincaré 等は非該当という plan.md の整理で正しい。

補足 (情報): `mirror → extrude` は `validate_sketch_profile_contours` (`lib.rs:746-753`) で `InvalidParameter { kind: "profile" }` になる (実測 probe7)。Non-Goals L32 の「golden は extrude を含めない」方針と整合し fail-fast されているが、`refs_resolve_in_state` はこれを予測しないので CRUD は通る。これは SketchChamfer で既に明記された gate の限界 (:764-767) と同種なので許容範囲。`suppressed: true` の Mirror は build で正しく no-op する (実測 probe8)。

### A5 [LOW] 2D ベクトル helper が 3 ファイル目の重複

`dist` / `sub` / `add` / `scale` / `dot` / `normalize` が `sketch_fillet.rs:230-264`、`sketch_chamfer.rs:184-212`、`sketch_mirror.rs:254-287` に同一実装で 3 重に存在する。`crates/engawa-kernel/CLAUDE.md` は「共有数学関数は `geometry/math.rs` に配置」と規定しており、逆行している。#297 が既に 2 重化しているので #298 固有の退行ではないが、3 個目で `math.rs` への 2D helper 集約 (`vec2` サブモジュール等) を follow-up Issue にする閾値。本 Issue では修正不要。

---

## contrarian

### C1 [MEDIUM] 派生 id が `{elem_id}_mirror` 固定のため、mirror の連鎖が構造的に不可能

`sketch_mirror.rs:87` は派生 id を `format!("{eid}_mirror")` に固定している。同じ sketch に 2 つ目の `SketchMirror` を適用すると、1 回目が作った `c1_mirror` が profile に残っているため、2 回目の `c1` → `c1_mirror` が必ず `mirror_duplicate_id` で fail する。

**実測** (probe3b): circle center=(3,4) を y 軸 → x 軸で 2 段 mirror。

```
chained mirror (off-axis circle) => Err: degenerate sketch element: c1_mirror, reason: mirror_duplicate_id
```

「Y 軸で mirror → X 軸で mirror して 4 回対称プロファイルを作る」は CAD の最頻ワークフローの 1 つだが、これが**回避不能**にブロックされる。`selection` で元要素だけを選んでも派生 id は同じなので逃げ道がない。根本原因は kernel 関数が feature id を受け取らない API 設計 (`apply_sketch_mirror(source, axis_p1, axis_p2, selection)`) で、`{elem_id}_{feature_id}` のような衝突しない導出ができない。

さらにエラーが分かりにくい: ユーザーが `m2` を追加したのに、`m1` が作った要素の id が「重複」だと言われる。

plan.md の Non-Goals には未記載。**最低限 Non-Goals への追記 + follow-up Issue 起票**が必要。API 変更 (feature id を渡す or 接尾辞をパラメータ化) は本 Issue のスコープ外でよい。

### C2 [MEDIUM] on-axis swapped 比較は `|sweep| >= 2π` で false negative

`is_axis_coincident` の Arc 分岐が課している条件は、展開すると「center が軸上」かつ「`2φ ≡ start + end (mod 2π)`」である。これは `|sweep| < 2π` の Arc に対しては幾何的自己一致と厳密に同値 (swapped 比較の 2 条件は mod 2π で互いに同値なので冗長だが無害) で、plan.md の主張どおり正しい。

しかし plan.md L161 は multi-turn Arc (`|sweep| > 2π`) を明示的にサポート範囲に含めており、そこでは点集合が全円になるため `2φ ≡ start+end` は自己一致の必要十分条件でなくなる。

**実測** (probe5):

```
center=(0,0) r=1 start=0 end=2.5π 、x 軸 mirror => Ok (NOT detected)
center=(0,0) r=1 start=1 end=1+2π 、x 軸 mirror => Ok (NOT detected)
```

いずれも点集合としては軸上の全円で完全に自己一致しているのに、退化検出をすり抜けて重なった複製が profile に追加される。`T_DEG_mirror_on_axis` の完了条件 (「軸上要素の重複検出」) を部分的にしか満たしていない。

対処は 2 択。(a) `|sweep| >= 2π - ANGLE_TOLERANCE` かつ center が軸上なら無条件で coincident とする 1 行追加、(b) plan.md Non-Goals に「multi-turn Arc の自己一致検出は非対象」と明記。どちらでもよいが現状は仕様にも記載がなく穴になっている。

### C3 [MEDIUM] axis 座標が巨大 (>1.3e154) だと線対称でなく**原点対称**になる silent bug

`dist` (`:254-258`) と `normalize` (`:280-287`) が両方 `dx*dx + dy*dy` で長さを計算するため、成分が ~1.3e154 を超えると平方が f64 overflow して `inf` になる。`axis_len = inf` は degenerate チェック (`<= LENGTH_TOLERANCE`) を素通りし、`normalize` は `a[0]/inf = 0` を返して `d = [0.0, 0.0]` になる。この「零単位ベクトル」で `reflect_point` を通すと `v_dot_d = 0` → `r = -v` → 結果は `2p - q`、すなわち **点 p を中心とする点対称**になる。

**実測** (probe2): 軸 (0,0)-(1e300, 0) は数学的には x 軸そのもの。線 (2,0)-(2,3) の正解は (2,0)-(2,-3)。

```
huge axis  => Ok: Line { id: "l1_mirror", from: [-2.0, 0.0], to: [-2.0, -3.0] }   ← 原点対称になっている
1e10 axis  => Ok: Line { id: "l1_mirror", from: [ 2.0, 0.0], to: [ 2.0, -3.0] }   ← 正しい
```

エラーも NaN も出ず、幾何的に誤った結果が Ok で返る。Codex R02 が axis の NaN/Inf 拒否を要求した意図 (「軸の妥当性を fail-fast する」) から見て、有限だが表現不能な軸を silent に誤解釈するのは同じ穴。

**修正案** (2 行): `dist` と `normalize` の長さ計算を `dx.hypot(dy)` に置き換える。`hypot` は中間 overflow しないので `1e300` の軸長は `1e300` のまま得られ、`normalize` も正しい単位ベクトルを返す。同じ overflow は `sketch_fillet.rs:257` / `sketch_chamfer.rs:206` の `normalize` にもあるが既存穴。

### C4 [LOW] mirror 結果の非有限値を検証していない

**実測** (probe9):

```
from=[1e308, 1] を x 軸 mirror  => Ok: Line { from: [inf, NaN], to: [2.0, -3.0] }
center=[NaN, 0] を x 軸 mirror  => Ok: Circle { center: [NaN, NaN], radius: 1.0 }
```

`2.0 * v_dot_d` が `inf` になり `inf * 0.0 = NaN` が混入する。axis 側は Codex R02 で `is_finite_point` チェックが入ったのに、element 座標側と出力側には対称なチェックがない。

ただし `SketchElement` 座標の有限性は `Document::validate` でも検証されていない (`validate_component` の `check_finite_position` は `CreateCylinder.origin` / `CreateSphere.center` のみ、`document.rs:320-331`) ので、既存の穴を踏襲したにすぎない。本 Issue で直すなら `mirror_element` の出力に `is_finite_point` を掛けて `InvalidParameter { kind: "sketch_mirror_non_finite_result" }` を返すのが素直だが、必須ではない。

### C5 [LOW] `mirror_duplicate_id` が同一 call 内で生成される id の衝突を見ない

`existing_ids` (`:72`) は `source` からのみ構築され、ループ中に `appended` へ積んだ派生 id は照合対象に入っていない。source に同じ element id が 2 つあると衝突した派生 id が黙って通る。

**実測** (probe6): profile が `["l1", "l1"]` のとき出力は `["l1", "l1", "l1_mirror", "l1_mirror"]`。

ただし profile 内 element id の一意性はそもそもどこでも強制されていない (`validate_component` は feature id しか検査しない) ので、前提が既に壊れている入力に対する挙動。優先度低。

### tolerance の使い方について (弱点なし)

`LENGTH_TOLERANCE` / `ANGLE_TOLERANCE` を `<=` で使う比較スタイルは `math.rs` の `length_near` / `angle_near` (`:35-42`) および `sketch_offset.rs` / `sketch_fillet.rs` と一致しており、ADR-004 の暫定 (絶対 tolerance) 方針とも整合する。`angles_near_mod_2pi` の mod 2π 正規化 (`:290-294`) も実装として正しい。ここは弱点なし。

`points_near` が絶対 1e-9 であるため、軸から 5e-10 より遠い要素は「軸上」と判定されない (反射で距離が 2 倍になるため実効閾値は 5e-10) が、これは絶対 tolerance モデル全体の性質で本 Feature 固有の問題ではない。

---

## migration

### M1 [MEDIUM] `golden_examples.rs` に `golden_sketch_mirror` がない

`crates/engawa-format/tests/golden_examples.rs` には `golden_sketch_offset` (:281) / `golden_sketch_fillet` (:311) / `golden_sketch_chamfer` (:369) があり、#295-#297 は全て自分の example に対応する YAML golden roundtrip テストを追加している。#298 はこのファイルを一切触っていない。

結果、`examples/sketch_mirror.engawa` の serialize 後の形が pin されていない。特に `selection: []` は `skip_serializing_if = "Vec::is_empty"` により roundtrip で消えるので、その挙動が golden で固定されていない。`examples_smoke.rs::sketch_mirror()` は parse + build が通るかしか見ておらず (`smoke()` は body 数も検証しない、`examples_smoke.rs:6-19`)、serialize 形の退行は検出できない。

**対処**: `golden_sketch_mirror` を追加する。兄弟 3 本と同型なのでコストは低い。

### M2 [LOW] `xtask` の tag assert 一覧に `sketch_mirror` がない

`crates/xtask/src/main.rs:1078-1081` の `t02_feature_tagged_union` は、golden 全文比較 (`:1038`) の後に個別 tag の `assert!(actual.contains(...))` を並べており、`sketch_chamfer` で止まっている。#297 は chamfer を追加していたので precedent 逸脱。全文比較があるため実害はないが、1 行追加で揃う。

### スキーマ生成系 (問題なし)

- **ts-rs**: `web/src/generated/Feature.ts` に `"type": "sketch_mirror"` が入っており commit `1d5d9ab` で反映済み。`cargo xtask ci` の drift gate (`main.rs:828-854`、`git status --porcelain web/src/generated/`) を通る。
- **JSON Schema**: ディスクへ export された `*.schema.json` はリポジトリに存在しない。`schemars::JsonSchema` は derive のみで、唯一の golden schema テスト (`feature.rs:1070` `t11_json_schema_golden`) は `EntityRef` 限定なので `Feature` の variant 追加に影響しない。

### 後方互換 (問題なし)

- variant は enum 末尾追加、internally tagged (`#[serde(tag = "type")]`) なので既存 variant のワイヤ形式は不変。旧 `.engawa` の読み込みに影響なし。
- `selection` / `suppressed` の `#[serde(default, skip_serializing_if = ...)]` 属性は `SketchOffset` / `SketchFillet` / `SketchChamfer` と同形。roundtrip テスト 3 本 (`feature.rs:1975 / 2020 / 2043`) で default 省略・非空 selection・suppressed=true をカバーできている。
- `Feature::id()` / `is_suppressed()` / `feature_variant_name` / `set_feature_suppressed` / `simulate_history` / build dispatcher はいずれも網羅的 match で、コンパイラ強制により arm 追加漏れなし。

### ADR-017 との field 名乖離 (判断済み・問題なし)

ADR-017 §3 は `sketch_ref: EntityRef` + `mirror_line: MirrorLine (EntityRef | LineEq)` を規定しているが、実装は `sketch: String` + `axis_p1` / `axis_p2: [f64; 2]`。plan.md 自律判断ログ §1 で明示的に判断済みで、#295-#297 も同じ乖離パターン (chamfer は ADR の `distance_a`/`distance_b` に対し対称 `length` を採用)。ADR-017 側の後追い更新は Phase 10 締めの課題として別途扱う。

### `docs/file-format.md` (既存の穴、#298 の退行ではない)

`docs/file-format.md:21-61` の Feature Types 一覧は `create_box` / `create_cylinder` / `create_sphere` / `extrude` / `cut` / `fuse` / `intersect` しか載っておらず、`create_sketch` / `extrude_cut` / `sketch_offset` / `sketch_fillet` / `sketch_chamfer` も既に欠落している。#297 も docs を触っていないため #298 固有の退行ではない。Phase 10 締めで一括更新するのが妥当。

---

## 結論と推奨アクション

**必須 (high)**:

1. **A1** — `refs_resolve_in_state` に `Feature::SketchMirror { sketch, .. } => sketches_at.contains_key(sketch.as_str())` を追加。`FeatureCrud::delete` / `suppress` が `EditBreaksConsumer` を返すことを検証する再現テストを先に書いて赤を確認してから直す。

**強く推奨 (medium、いずれも 1-2 行で silent-wrong-result のクラスを潰せる)**:

2. **A3** — Arc 角度を `2φ - start_angle` で直接算出する (plan.md の導出式に揃える)。退化検出の false negative (C2 の一部を含む) と出力精度が同時に改善する。回帰テストとして center=(1e6,1e6) r=1e-3 の 45 度軸対称 Arc が `mirror_axis_coincident` になることを追加。
3. **C3** — `dist` / `normalize` の長さ計算を `hypot` に変更。回帰テストは axis=(0,0)-(1e300,0) で正しい線対称結果が出ること。
4. **M1** — `golden_examples.rs` に `golden_sketch_mirror` を追加。

**記録のみ / follow-up Issue 起票 (medium-low)**:

5. **C1** — mirror 連鎖不可を plan.md Non-Goals に追記し、派生 id 命名の見直し (feature id を含める) を follow-up Issue へ。#278 の Pattern 系が同じ id 導出パターンを踏襲する前に決着させたい。
6. **C2** — `|sweep| >= 2π` の自己一致検出を Non-Goals に明記する (または 1 行で無条件 coincident 判定を足す)。
7. **A2 / M2** — 兄弟 feature との対称性のため揃える (低コスト)。
8. **A5 / C4 / C5 / docs** — 既存穴の踏襲。本 Issue では対応不要、Phase 10 締め or refactor pass で。
