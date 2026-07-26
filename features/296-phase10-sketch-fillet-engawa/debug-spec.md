# debug-spec: #296 STEP 6.7/7.5 で検出された問題の修正

STEP 6.7 (Claude self-review) と STEP 7.5 (Codex final gate, non-blocking medium) で
以下の問題が見つかった。実装アルゴリズム自体 (角の二等分線法・tangent point・sweep 符号・
wraparound splice) は手計算で正しさを確認済み。修正対象は限定的。

## 仮説

1. `compute_fillet` のゼロ長判定 (`len_a || len_b` の1本化) が `elem_b` 側の退化を `a_id` として
   誤報告する (Codex A01)。ロジックのコピペミス。
2. アクセプタンステストのうち複数件が「主張どおりの不変条件を検証していない」骨抜き状態になっている
   (Claude self-review high 2件 + medium 2件)。実装ではなくテストコードのバグ。
3. golden fixture (`golden_examples.rs`) への `sketch_fillet` 追加漏れ (plan In-Scope 記載済みだが未実装)。

## 関連ファイル

- `crates/engawa-kernel/src/geometry/sketch_fillet.rs` (production code 修正 #1 + inline test 修正 #2 の一部)
- `crates/engawa-build/tests/sketch_fillet_acceptance.rs` (#2 の大部分)
- `crates/engawa-format/tests/golden_examples.rs` (#3)

## 修正方針

### A. production code 修正 (`sketch_fillet.rs::compute_fillet`, Codex A01, medium)

現状:
```rust
let len_a = dist(&a_from, &a_to);
let len_b = dist(&b_from, &b_to);
if len_a <= LENGTH_TOLERANCE || len_b <= LENGTH_TOLERANCE {
    return Err(KernelError::DegenerateSketchElement {
        element_id: a_id.to_string(),
        reason: "fillet_zero_length_input_line",
    });
}
```
`elem_b` だけがゼロ長でも `element_id` に常に `a_id` を返してしまう。`len_a`/`len_b` を個別判定し、
どちらが退化したかに応じて `a_id`/`b_id` を返すよう分岐すること。あわせて「`elem_b` のみゼロ長」の
回帰テストをどこかに1件追加する (kernel inline test でよい)。

### B. `normalize()` の tolerance 結合について (architect medium, 任意対応)

`bisector = normalize(add(&d_a, &d_b))` は `ANGLE_TOLERANCE == LENGTH_TOLERANCE == 1e-9` という
偶然の一致に暗黙で依存している (詳細は `claude-self-review.md` の architect 節参照)。
本 Issue のスコープでは **最小対応 (コメント追記) のみで良い**。`normalize` 関数の直上に
「呼び出し元の corner 角度 guard は `ANGLE_TOLERANCE` を要求するため、本関数の near-zero 判定
`LENGTH_TOLERANCE` との大小関係が正しさに影響する。`ANGLE_TOLERANCE >= LENGTH_TOLERANCE` を前提とする」
という趣旨の1行コメントを追加すること。アルゴリズムの変更 (近似式の置き換え) は不要 (Out-of-Scope、
別 Refactor Issue で検討)。

### C. `t08_cw_profile_negative_sweep` の build 部分が fillet を適用していない (high)

現状は `apply_sketch_fillet_build` の戻り値 `out` を sweep 符号 assert にのみ使い、その後の
`Feature::CreateSketch { profile: cw, .. }` には **fillet 前の CW 矩形** (`cw` そのもの) を渡して
いる。feature 列に `Feature::SketchFillet` が含まれないため、build 経路での負 sweep Arc の統合検証
になっていない。

修正: feature 列を `[CreateSketch(cw), SketchFillet{sketch:"sk", elem1_id:"l1", elem2_id:"l2",
radius:1.0}, Extrude]` に直す。手計算では修正後も build 成功するはず
(`center=(1,4)`, `start=π`, `end=π/2`, arc は `(0.293, 4.707)` 側に膨らみ profile は convex かつ
simple のまま)。`euler_poincare() == 0` の assert も追加する。

### D. kernel `t03_normal_60deg` の tangent length assert が捨てられている (high)

`crates/engawa-kernel/src/geometry/sketch_fillet.rs` の `t03_normal_60deg` 付近
(現状 `let _ = expected_t;` という行がある箇所) で、計算した `expected_t` を実際に使わず捨てている。
plan T03 の期待結果「tangent length が解析解 (`t=r/tan(30°)`) と一致」の t 側 assert が存在しない。
90° コーナーは `tan(45°)=1` で `t == radius` になるため、`t=radius` という誤実装と区別できない
唯一のテストがここで骨抜きになっている。

修正: `expected_t` を実際に使い、`trimmed_a.to` (または `tangent_a`) が
`corner + d_a * expected_t` に一致することを `LENGTH_TOLERANCE` 以内で assert する。
`trimmed_b.from` 側も同様。あわせて `dist(center, tangent_a) == radius` の接線性 assert も
1件追加する (60° コーナーで初めて非自明な検証になる)。

### E. acceptance の空テスト3件 (`t02/t03/t05_..._covered_at_..._level`) が無条件 pass (medium)

`compute_fillet` は `pub` なので `engawa-build` の統合テストから直接呼べる (実際
`apply_sketch_fillet_build` は同ファイルで import 済み)。「kernel level に委譲したので空」という
根拠が成立しない。

修正: `t02_normal_90deg_covered_at_kernel_level` と `t03_normal_60deg_covered_at_kernel_level` は
`compute_fillet` を直接呼ぶ実テストに置き換えるか、削除して kernel 側テストへの doc コメント参照
のみ残す (関数自体は削除してよい)。`t05_roundtrip_covered_at_format_level` は roundtrip が完全に
format crate 側の責務なので削除して構わない (feature.rs 側に既に inline test あり)。

### F. `t04_build_rectangle_corner_fillet` の頂点座標 assert 欠落 (medium)

plan T04 は「旧コーナー頂点が消え tangent point 近傍の頂点が存在する」ことを assert する設計
だったが、現状は `built.all().len() == 1` と `euler_poincare() == 0` のみ (plan 自身が「検出力は
低い」と書いた回帰ネットだけが残っている)。

修正: 矩形 corner `(10,0,z)` (fillet 前の頂点) が Solid の vertex 集合に存在しないこと、かつ
`(9,0,z)` と `(10,1,z)` 付近 (tangent point) に頂点が存在することを assert する
(z は Extrude の押し出し方向、上下2面分をチェックすること)。

### G. golden fixture 追加漏れ (migration medium)

`crates/engawa-format/tests/golden_examples.rs` に `golden_sketch_fillet()` が無い
(#295 の `golden_sketch_offset()` が前例)。`examples/sketch_fillet.engawa` を byte-identical
roundtrip で検証する golden test を1件追加すること。

### H. (任意) CRUD gate テストの経路追加

`claude-self-review.md` の contrarian 節にある通り、`edit(SketchFillet 自身の elem2_id を破壊)`
と `reorder(SketchFillet を CreateSketch より前に移動)` の2経路は配線上カバーされているが
テストが無い。時間が許せば各1件追加するが、A-G に比べ優先度は低い (blocking ではない)。

## 次にやること

A, C, D, E, F, G を実装し (B はコメント追記のみ)、`cargo xtask ci` green を確認すること。
H は余裕があれば追加。修正後は `cargo test -p engawa-build --test sketch_fillet_acceptance --
--include-ignored` と `cargo test -p engawa-kernel --lib` の両方が通ることを確認すること。
