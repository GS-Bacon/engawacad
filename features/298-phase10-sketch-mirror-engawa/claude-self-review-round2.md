# Claude Self-Review round 2 (STEP 6.7) — Issue #298 Sketch Mirror

対象: round 1 (`claude-self-review.md`) の指摘に対する修正 (`debug-spec.md` 項目 1-7) の再検証。
実施方法: 差分精読 + 一時 probe (`crates/engawa-kernel/tests/zz_probe_r2.rs` / `crates/engawa-build/tests/zz_probe_r2b.rs`、実行後削除) による実測。
加えて **修正箇所を一時的に剥がして再現テストが赤になることを実測**し、テストが本物の regression guard かを検証した。

**判定サマリ**: critical 0 / high 0 / **medium 2** / low 5
round 1 の high (A1) は解消済み。medium 2 件はいずれも「コード変更不要・記録/コメント追記で足りる」レベル。

## round 1 指摘の解消状況

| # | 重要度 | 解消 | 実測根拠 |
|---|---|---|---|
| A1 | high | ✅ 完全 | arm を剥がすと T07/T08 が赤、戻すと緑。reorder 経路も保護を実測 |
| A2 | medium | ✅ (副作用 R2-2) | gate を剥がすと T10 が赤、戻すと緑 |
| A3 | medium | ⚠️ **部分** | 角度誤差は 0 に改善。ただし退化検出の false negative は \|center\| ≳ 1e7 で残存 |
| C2 | medium | ✅ 完全 | 2π / 2.5π / 4π / -2.5π 全て検出。off-axis は正しく通過 |
| C3 | medium | ✅ (狭い残存 R2-3) | 1e300 / 1e308 軸で正しい線対称。減算段 overflow のみ残る |
| M1 | medium | ✅ | `golden_sketch_mirror` 追加、green |
| M2 | low | ✅ | xtask tag assert 追加 |

| 新規 # | 観点 | 重要度 | 要旨 |
|---|---|---|---|
| R2-1 | architect | **medium** | A3 修正は角度式のみ。退化検出 false negative は残存し、回帰テストが境界値ちょうどで安心を与えすぎる |
| R2-2 | architect | **medium** | A2 gate が build で通る組合せを insert で false-reject する (chamfer と同種だが未文書) |
| R2-3 | contrarian | low | hypot 化は減算段の overflow を防げない (コメントが過大な保証を示唆) |
| R2-4 | contrarian | low | multi-turn 閾値が半径非依存で、大半径 near-full arc を false-reject |
| R2-5 | contrarian | low | `2φ - θ` により出力 start_angle が軸のパラメータ化 (p1→p2 の向き) に依存 |
| R2-6 | architect | low | `element_id_of` のコメントが存在しない API (`SketchElement::id`) を参照 |
| R2-7 | architect | low | `SketchElementNotResolved.elem2_id` に空文字を詰めている |

---

## architect

### A1 [解消・high → 0] `refs_resolve_in_state` の SketchMirror arm

`crates/engawa-build/src/feature_crud.rs:331` に

```rust
Feature::SketchMirror { sketch, .. } => sketches_at.contains_key(sketch.as_str()),
```

が入り、`simulate_history` (`:517-529`) にも兄弟 3 feature と同型の arm がある。

**再現テストが本物であることを実測で確認した**。上記 1 行だけを一時削除して
`cargo test -p engawa-build --test sketch_mirror_acceptance` を実行:

```
---- t07_crud_gate_delete_sketch_breaks_mirror ----
called `Result::unwrap_err()` on an `Ok` value: Document { ... }   ← 赤
---- t08_crud_gate_suppress_sketch_breaks_mirror ----
called `Result::unwrap_err()` on an `Ok` value: Document { ... }   ← 赤
test result: FAILED. 18 passed; 2 failed
```

1 行を戻すと 20/20 green。**T07/T08 は正しく `EditBreaksConsumer` を検証する再現テストであり、
CLAUDE.md の「再現テスト先行」要件を満たしている**。
テストは `broken_consumer_id == "m1"` まで確認しており、単なる「何かエラーが出た」ではない。

reorder 経路も追加で probe した (round 1 未検証):

```
reorder mirror before sketch => Err(EditBreaksConsumer { edit_feature_id: "m1",
  broken_consumer_id: "m1", broken_ref: "reorder broke this consumer (...)" })
```

`FeatureCrud` の 4 op (insert / suppress / reorder / delete) すべてで保護されている。
`update` op はそもそも存在しないので、「CreateSketch.profile を書き換えて selection を壊す」経路は無い。

### A2 [解消] `check_refs_resolve_before` の element-level gate

`feature_crud.rs:821-847`。同様に gate ブロックを一時削除して実測:

```
test t10_crud_gate_insert_mirror_unknown_element ... FAILED
test result: FAILED. 19 passed; 1 failed
```

こちらも本物の再現テスト。`reason` / `elem1_id` / `sketch_ref` / `feature_id` を全て検証している点も良い。

`selection` 空をスキップする判断も正しい (空 = 全要素なので検査対象が無い)。
Fillet/Chamfer が `find_adjacent_pair` で *original* `CreateSketch.profile` を見るのと同じ解決範囲。

### R2-1 [MEDIUM・新規] A3 修正は角度式のみで、退化検出の false negative は残存している

**角度式の修正自体は完全に正しい**。`sketch_mirror.rs:137-139`:

```rust
let phi = d[1].atan2(d[0]);
let new_start = 2.0 * phi - start_angle;
let new_end = new_start - (end_angle - start_angle);
```

数学的検証: 方向角 φ の直線 (原点通過) による反射は θ ↦ 2φ - θ。軸が点 p だけ平行移動しても
反射は affine なので `R(center + v) = R_center + R_lin(v)` が成り立ち、center を `reflect_point`、
角度を `2φ - θ` で別々に扱うのは厳密に正しい。`new_end = new_start - sweep` も
`2φ - end = (2φ - start) - (end - start)` で恒等。**式は正しい**。

精度も実測で改善を確認 (45° 軸、θ=0.3):

| center | radius | start_angle 誤差 |
|---|---|---|
| (1e0, 1e0) | 1e0 | 0 |
| (1e6, 1e6) | 1e-3 | 0 |
| (1e8, 1e8) | 1e-3 | 0 |
| (1e10, 1e10) | 1e0 | 0 |
| (1e14, 1e14) | 1e0 | 0 |

round 1 の 7.07e-7 rad (1e10) が **0 になった**。ここは完全な改善。

**しかし round 1 が A3「影響 1」として挙げた退化検出の false negative は解消していない**:

```
A3-coincidence center=(1e6,1e6)  r=1e-3 => Err mirror_axis_coincident   ← 検出 (改善)
A3-coincidence center=(1e8,1e8)  r=1e-3 => Ok (NOT detected)            ← 依然すり抜け
A3-coincidence center=(1e10,1e10) r=1e0 => Ok (NOT detected)            ← 依然すり抜け
```

**真因は角度側ではなく center 側**だった。`is_axis_coincident` (`:212`) は
`points_near(o_c, m_c)` を **絶対** `LENGTH_TOLERANCE = 1e-9` で比較する。
45° 軸 (`d = [0.7071.., 0.7071..]`) で center=(1e8,1e8) を反射すると丸め誤差が
`1e8 × 2^-52 ≈ 2e-8` 出るため、1e-9 の閾値を原理的に満たせない
(そもそも 1e8 の f64 ulp が ~1.5e-8 で 1e-9 を下回れない)。検出可能な実効上限は **|center| ≲ 1e7**。

問題は 2 点:

1. **コード コメントが誤誘導**。`sketch_mirror.rs:132-135` は
   「The earlier implementation derived new_start from atan2 ... suffers catastrophic cancellation」
   と書き、round 1 の false negative の原因を角度式に帰している。事実の半分でしかない。
2. **回帰テストが境界値ちょうどを選んでいる**。`t_item3_large_coord_axis_coincident_arc`
   (`sketch_mirror.rs:675`) は center=(1e6,1e6) を使う。これは検出可能な実効上限のすぐ内側で、
   1e8 に変えると即座に赤になる。テスト名「large_coord」が示唆する一般的な大座標カバーは無い。

補足として、center を軸方向ベクトルに **厳密に平行**な位置 (`c = d * k`) に置いた場合は
1e14 でも検出される (丸めが相殺するため)。つまり「大座標なら必ず落ちる」わけでもなく、
軸の向きと center の関係次第という不安定な性質を持つ。

**対処**: コード変更は不要。(a) `mirror_element` のコメントから「これで退化検出が直る」という
含意を外し、(b) `t_item3_...` テストに実効上限 (~1e7) と 1e8 以上は未検出である旨をコメントで残す。
根本解決は ADR-004 の相対 tolerance 化であり、本 Issue のスコープ外
(round 1 も「絶対 tolerance モデル全体の性質」と整理済み)。

### R2-2 [MEDIUM・新規] A2 gate が build で成功する組合せを insert で false-reject する

新 gate は `selection` の各 id を *original* `CreateSketch.profile` に対してのみ照合するため、
先行 feature が生成した派生要素を選択できない。実測:

```
build with chamfer-derived selection => Ok("Ok")
insert mirror(selection=chamfer-derived) at 2
  => Err(SketchElementNotResolved { feature_id: "m1", sketch_ref: "sk1",
       elem1_id: "l1_l2_chamfer_line", elem2_id: "", reason: "sketch_mirror_elem_not_found" })
```

構成: `CreateSketch` (4 Line の矩形) → `SketchChamfer(l1, l2)` → `SketchMirror(selection = ["l1_l2_chamfer_line"])`。
`build_bodies_from_features` は `built_sketch_profiles` を引き継ぐので **成功する**。
一方 CRUD の insert は拒否する。「面取りした辺をミラーする」は自然なワークフローで、
kernel は対応しているのに CRUD 層だけが弾く。

これは SketchChamfer が既に持っている T10 false-reject と**完全に同種**であり、
`refs_resolve_in_state` (`:301-306`) と `check_refs_resolve_before` (`:775-781`) の
chamfer 側コメントには明示的に文書化されている。しかし **SketchMirror の新 gate のコメント
(`:821-825`) は「Empty selection means all elements ... matches SketchOffset precedent」しか書いておらず、
この false-reject に一切触れていない**。契約として明記されていない制限は、後で「バグ」として再発見される。

**対処**: コメント 2 行の追記か plan.md Non-Goals への追記。コード変更は不要
(gate を後続 feature 込みで解決するのは #295-#297 全体の設計変更になり本 Issue のスコープ外)。

### R2-6 [LOW・新規] `element_id_of` のコメントが存在しない API を参照

`feature_crud.rs:122-124`:

```rust
/// Return the id field of a SketchElement by reference (matches SketchElement::id helper
/// on the format crate but returns `&str` to fit HashSet<&str> without allocation).
```

`engawa-format` に `impl SketchElement` は存在しない (`grep "impl SketchElement"` → 0 件。
`pub fn id()` は `Feature` にしかない)。存在しない API を根拠にした説明になっている。

併せて、同等実装は kernel 4 ファイル (`sketch_offset.rs:85` / `sketch_fillet.rs:220` /
`sketch_chamfer.rs:174` / `sketch_mirror.rs:252`) に加えて今回 build にも 1 つ増え、**計 5 重**になった。
round 1 の A5 (2D helper 重複) は rejection 済みだが、これはクレートを跨いだ 5 個目という新しい事実。
本質的な対処は `SketchElement::id()` を format 側に生やして全員が使うこと (1 箇所・数行)。
本 Issue では不要、Phase 10 締めの refactor pass 候補として記録。

### R2-7 [LOW・新規] `SketchElementNotResolved.elem2_id` に空文字を詰めている

`feature_crud.rs:840` で `elem2_id: String::new()`。
エラー Display は `#[error("feature {feature_id} references elements ({elem1_id:?}, {elem2_id:?}) ...")]`
なので `references elements ("nope", "")` と出る。この型は Fillet/Chamfer の 2 要素前提で作られており、
1 要素しか持たない Mirror には形が合っていない。実害なし (メッセージがやや読みづらいだけ)。
新しい variant を足すほどではない。記録のみ。

---

## contrarian

### C2 [解消] multi-turn Arc の自己一致検出

`sketch_mirror.rs:207-215`:

```rust
let sweep = (*o_e - *o_s).abs();
if sweep >= TAU - ANGLE_TOLERANCE {
    points_near(o_c, m_c)
} else { /* center 一致 + swapped 角度一致 */ }
```

ロジックは正しい。実測:

| 入力 (center=(0,0) r=1、x 軸 mirror) | 結果 |
|---|---|
| sweep = 2.5π | Err `mirror_axis_coincident` ✅ |
| sweep = 2π (ちょうど) | Err ✅ |
| sweep = 2π - 1e-10 (閾値内) | Err ✅ |
| sweep = 2π - 1e-8 (閾値外) | Ok (非 multi-turn 扱い) ✅ 正しい |
| sweep = 2π - 0.1 | Ok ✅ |
| sweep = 4π | Err ✅ |
| sweep = **-2.5π** (負 sweep) | Err ✅ `.abs()` が効いている |
| sweep = 2.5π, center=(0,5) (軸外) | Ok ✅ 正しく通る |

round 1 の probe5 で挙がった 2 ケース (`start=0 end=2.5π`、`start=1 end=1+2π`) は両方とも塞がれた。
負 sweep も `.abs()` でカバーされているのは良い (mirror 結果を再度 mirror する経路で効く)。

`sweep < 2π` 側の判定 (`m_s ≡ o_e ∧ m_e ≡ o_s`) も改めて検証したが正しい。
円周上の 2 弧が点集合として一致する条件は「反転した端点対の一致」で、
反射は必ず向きを反転するのでこの形が必要十分になる。
「端点は一致するが弧が逆側」(半円 × 軸上端点、`t_boundary_arc_endpoints_on_axis`) を
誤って coincident にしないことも実測済み。

### R2-4 [LOW・新規] multi-turn 閾値が半径非依存で、大半径 near-full arc を false-reject する

閾値は角度絶対値 `ANGLE_TOLERANCE = 1e-9` なので、半径が大きいと欠損弧長が
`LENGTH_TOLERANCE` を大きく超えても coincident 扱いになる。実測:

```
r=1e6, sweep = 2π - 1e-10  (欠損弧長 = 1e-4 mm = 1e5 × LENGTH_TOLERANCE)
  => Err DegenerateSketchElement { reason: "mirror_axis_coincident" }
```

修正前は Ok だったので**挙動変化**。ただし対象は「ほぼ全円」の縮退入力のみで、
mirror しても元とほぼ重なるので fail-fast する方が親切とも言える。
角度閾値を `angles_near_mod_2pi` と揃えている点は ADR-004 の絶対 tolerance 方針と一貫している。
**弱点として認識しつつ、修正不要**と判断。

### C3 [解消] `dist` / `normalize` の hypot 化

`sketch_mirror.rs:268` / `:295` に `hypot` が正しく入っている。実測:

```
1e300 x-axis  => Ok Line { from: [2.0, 0.0], to: [2.0, -3.0] }    ← 正しい線対称
1e308 x-axis  => Ok Line { from: [2.0, 0.0], to: [2.0, -3.0] }    ← 正しい
1e300 45deg   => Ok Line { from: [-4.4e-16, 2.0], to: [3.0, 2.0] } ← 誤差 ~1e-15、正しい
1e-300 x-axis => Err sketch_mirror_axis_degenerate                 ← 退化検出も維持
```

round 1 の「原点対称になる silent bug」は消えた。回帰テスト
`t_item5_large_axis_no_overflow` も、点対称結果 (-2,0)-(-2,-3) と区別できる assert になっており妥当。

副次的に良い点: `apply_sketch_mirror` の `axis_len = dist(p1,p2)` と `normalize(sub(p2,p1))` が
**同一の hypot 計算**になったため、退化チェックを通った軸に対して `normalize` の
零ベクトル分岐 (`:296-297`) が到達不能になった。両者の閾値がズレる隙間が閉じている。

### R2-3 [LOW・新規] hypot 化は減算段の overflow を防げない

`dist(a, b)` は `(b[0]-a[0]).hypot(...)` で、**hypot の前に減算がある**。
成分が逆符号で各々 ~9e307 を超えると減算で `inf` になり、hypot は救えない。実測:

```
axis_p1 = (-1e308, 0), axis_p2 = (1e308, 0)   ← 数学的には x 軸そのもの
  => Ok Line { id: "l1_mirror", from: [NaN, NaN], to: [NaN, NaN] }
```

`sub` → `[inf, 0]` → `normalize` → `inf.hypot(0) = inf` → `inf/inf = NaN` → 全 NaN が `Ok` で返る。

修正前も同じ壊れ方だったので**退行ではない**。ただし
`sketch_mirror.rs:267` / `:292-294` のコメント「hypot avoids intermediate overflow for very large
coordinates」は「あらゆる大座標で安全」と読めてしまい、実際には「差が f64 の範囲に収まる場合に限る」。
根本対処は出力の有限性検査 (round 1 の C4) だが、これは rejection.md で棄却済み。
**コメント文言を限定するだけでよい**。

### R2-5 [LOW・新規] `2φ - θ` により出力 start_angle が軸のパラメータ化に依存するようになった

旧実装は反射点の `atan2` を取るので出力が常に (-π, π] に正規化されていた。
新実装は正規化されないため、**同じ直線でも p1→p2 の向きで数値が変わる**。実測 (arc center=(0,3) r=1 start=0.5 end=1.2):

| 軸 | 出力 |
|---|---|
| (0,0) → (1,0) | `start_angle: -0.5, end_angle: -1.2` |
| (0,0) → **(-1,0)** | `start_angle: 5.783185307179586, end_angle: 5.083185307179586` (差 2π) |
| (1000,0) → (1001,0) | `start_angle: -0.5, end_angle: -1.2` (軸上の点の位置には非依存) |

幾何は同一であることを実測で確認した:

```
tess-equiv len_a=4 len_b=4 maxdiff=4.44e-16   (2π ずらした等価 Arc と tessellation 一致)
```

決定性 (同一入力 → 同一出力) は保持されているので ADR の決定性要件は破っていない。
`is_axis_coincident` も `angles_near_mod_2pi` で periodic、tessellation も
`sweep = end - start` と cos/sin しか使わないので機能影響は無い。
`built_sketch_profiles` はメモリ内のみで永続化経路が無いため、YAML の diff にも出ない。

ただし **#278 の Pattern 系が同じ角度導出を踏襲し、かつ結果を永続化する設計にすると
「同じ形なのに YAML の数値が違う」問題になる**。今のうちに記録しておく価値がある。
なお `-0.0` の混入は無いことも確認済み (`start=0.0, is_sign_negative=false`)。

### C1 (round 1 medium、rejection 済み) の現状確認

```
C1 chained => Err(DegenerateSketchElement { element_id: "c1_mirror", reason: "mirror_duplicate_id" })
```

mirror の連鎖 (4 回対称) は依然ブロックされる。rejection.md の判断どおり本 Issue では扱わないが、
状態は round 1 から変わっていないことを記録。

---

## migration

### M1 [解消] `golden_sketch_mirror`

`crates/engawa-format/tests/golden_examples.rs:427-455` に追加済み。green。

`assert_golden` は `Document::from_path` → `to_yaml()` の**全文一致**なので、狙いどおりの pin ができている:

- `examples/sketch_mirror.engawa` には `selection: []` と `offset: 0.0` が書かれているが、
  golden 文字列には**両方とも無い**。`skip_serializing_if = "Vec::is_empty"` で roundtrip 時に
  消える挙動が golden で固定された。round 1 の M1 が指摘した通りの効果が出ている。
- `name: 'Sketch Mirror Example (Phase 10 #298)'` のシングルクォートも pin されており、
  `#` を含む名前の serde_yaml 出力形が固定されている。

兄弟 3 本 (`golden_sketch_offset` / `_fillet` / `_chamfer`) と同じ `concat!` スタイルで揃っている。

### M2 [解消] xtask tag assert

`crates/xtask/src/main.rs:1082-1085` に `"type": "sketch_mirror"` の assert が
`sketch_chamfer` の直後に追加済み。#297 の precedent に揃った。

### CI gate 実測

| gate | 結果 |
|---|---|
| `cargo test --workspace` | **全 green (0 failed)**。集計: `test result: ok` のみ、`FAILED` 0 件 |
| `cargo fmt --all -- --check` | OK |
| `cargo clippy --workspace -- -D warnings` | OK (exit 0) — `xtask ci` が呼ぶのと同一コマンド (`main.rs:808-809`) |
| ts-rs drift (`git status --porcelain web/src/generated/`) | クリーン (workspace test 実行後も差分なし) |

補足: `cargo clippy --workspace --all-targets -- -D warnings` は赤になるが、
全て `tessellation/sketch.rs:991` (`len_zero`) / `tessellation/mod.rs:2277` (`cloned_ref_to_slice_refs`) 等の
**既存テストコードの lint** で、#298 が触ったファイルは 0 件。`xtask ci` は `--all-targets` を
付けないので gate 外。**#298 の退行ではない** (別 Issue 候補としてのみ記録)。

### 後方互換 / スキーマ (round 1 から変化なし・問題なし)

- `Feature::SketchMirror` は enum 末尾追加 + internally tagged。既存 variant のワイヤ形式は不変。
- serde roundtrip テスト 3 本 (`feature.rs:1974 / 2019 / 2043`) が default 省略・非空 selection・
  suppressed=true をカバー。
- `examples_smoke.rs::sketch_mirror` 追加済み (parse + build 疎通)。
- `FeatureCrud` の網羅 match (`feature_variant_name` / `set_feature_suppressed` / `simulate_history`) は
  全て arm 追加済み。コンパイラ強制なので漏れなし。

---

## 結論

round 1 の **high (A1) は完全に解消**し、修正を剥がすと赤になる本物の再現テスト (T07/T08) が残った。
A2 も同様 (T10)。C2 / C3 / M1 / M2 も実測で解消を確認。

**新規に発見した medium 2 件はいずれもコード変更不要**:

- **R2-1**: A3 は角度精度を完全に直したが、round 1 が「A3 影響 1」として挙げた退化検出の
  false negative は真因が別 (center 側の絶対 tolerance) で残っている。
  コメントとテストが実際より広いカバー範囲を示唆しているのが問題。
  → `mirror_element` のコメント修正 + `t_item3_...` に実効上限 (~1e7) を明記。
- **R2-2**: 新 gate が「chamfer 派生要素を mirror する」ような build では通る組合せを弾く。
  SketchChamfer の T10 と同種で precedent に沿っているが、mirror 側だけ文書化されていない。
  → gate コメントに 2 行追記 (または plan.md Non-Goals へ)。

low 5 件 (R2-3 〜 R2-7) はいずれも記録のみで足りる。
とくに **R2-5 (角度の非正規化)** は #278 Pattern 系が同じ導出を持ち込む前に
判断しておく価値があるので、Phase 10 締めの申し送りに残すことを推奨。

**マージ可否: 可**。CI gate は `xtask ci` 相当で全 green。
必須の修正は無く、上記のコメント 3 箇所を直せば round 2 の指摘は全てクローズできる。
