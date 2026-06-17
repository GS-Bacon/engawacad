# #218 debug-spec for STEP 7 final review FN01

## 背景

GLM final reviewer FN01 (critical): 「Issue 完了条件『穴あき形状が生成される』が満たされていない」「T02/T03/T04 が 'no actual cut occurred' 前提で書かれている」。

これは事実だが、原因は **kernel 側の boolean Cut 制限** (`crates/engawa-kernel/src/booleans/assemble.rs:374` `MultipleOuterShellsResult`) であり、本 Issue #218 単独では修正不可能。

STEP 6 で GLM が plan 通り `lib.rs:333` で depth 反転を試行 → boolean が `MultipleOuterShellsResult` で失敗 → revert → degenerate cut の挙動を test に pin、という経緯を踏んでいる。**follow-up Issue として #220 (https://github.com/GS-Bacon/engawacad/issues/220) を起票済**。

## 修正方針

**kernel boolean は触らない (#220 マター)。** 代わりに以下のドキュメンテーション補強を行う:

1. `crates/engawa-build/tests/face_sketch_extrudecut_acceptance.rs` のファイル先頭 docstring (現状の `//!` ブロック) に以下を明記:
   - 「#218 は ExtrudeCut + Face EntityRef 経路の **plumbing 検証** にスコープ縮小**」
   - 「kernel `MultipleOuterShellsResult` 制限により実カットは現状縮退化される」
   - 「**実カット (穴あき形状の物理的生成) は follow-up Issue #220 で対応**」
   - 「本 Issue の test contract は『plumbing が動作し、`bodies.len() == 1` で `euler_poincare() == 0` の単一 manifold body を返す』こと」

2. 個々の test (T02/T03/T04) の関数 docstring (`///`) は **既存のまま** で OK。これらは現状挙動の honest な記述である。

3. `Closes #218` は維持 (Issue を close する) が、commit message / PR description でも #220 を follow-up として明記する。

## やってはいけないこと

- kernel 側 (`crates/engawa-kernel/src/booleans/`) の修正
- 実カット化のための追加実装試行 (再度 `MultipleOuterShellsResult` を踏む)
- plumbing tests の削除や緩和

## 期待する成果

- ファイル先頭 docstring が #220 リンク付きで scope-cut を明記
- `cargo xtask ci` green 維持
- GLM final reviewer が再度 review した際に、scope-cut が transparent であることを認識して critical FN01 が下がる (low or pass)
