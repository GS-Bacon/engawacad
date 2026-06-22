# debug-spec round 5 — xtask inline test の import 順 mismatch を修正

## 経緯

- Round 1-4: SketchElement enum 導入 + tests/examples migration + 2 件 inline golden の `kind: line` 同期完了
- Round 5 で xtask inline test 1 件のみ残った

## 現在のエラーパターン

```
---- tests::t02_feature_tagged_union stdout ----
thread 'tests::t02_feature_tagged_union' panicked at crates/xtask/src/main.rs:1037:9:
assertion `left == right` failed
```

`left` (実際の auto-generated TS file 内容):
```
import type { PlaneRef } from "./PlaneRef";
import type { SketchElement } from "./SketchElement";  // ← alphabetical で SketchElement が先
import type { SketchPlane } from "./SketchPlane";
import type { Variable } from "./Variable";
```

`right` (test expected 文字列リテラル):
```
import type { PlaneRef } from "./PlaneRef";
import type { SketchPlane } from "./SketchPlane";  // ← 期待では SketchPlane が先
import type { SketchElement } from "./SketchElement";  // ← SketchElement を末尾に置いている
import type { Variable } from "./Variable";
```

## 仮説

`ts-rs` は dependent type を import 文として **alphabetical 順** で出力する。`SketchElement` < `SketchPlane` (E < P) なので `SketchElement` が先に来る。

xtask test の expected 文字列は SketchElement が無かった時代に書かれて、SketchSegment 削除のついでに SketchElement を末尾 (= SketchPlane の後) に挿入したが、ts-rs の sort 規約と矛盾している。

## 修正方針

`crates/xtask/src/main.rs` 内、`fn t02_feature_tagged_union` テスト本体 (line 1037 周辺) の expected 文字列リテラルの import 順を以下のとおり alphabetical に並べ替える:

```
import type { PlaneRef } from "./PlaneRef";
import type { SketchElement } from "./SketchElement";
import type { SketchPlane } from "./SketchPlane";
import type { Variable } from "./Variable";
```

= **`SketchElement` を `SketchPlane` より前に置く** (それ以外の行は keep)。これだけで test green になる。

その他の本体 (= export type Feature = ... の長い 1 行) は **変更しない**。stdout の left / right を見比べて差分は import 順だけと確認済み。

## 試した修正と結果

- [x] Round 1: SketchSegment `#[deprecated]` 残置 → clippy fail
- [x] Round 2: SketchSegment 完全削除 → tests/ 81 件 unresolved (修正済)
- [x] Round 3: tests/sketch_element_acceptance.rs の assertion / examples/circle_arc.engawa の simplify (Claude が修正)
- [x] Round 4: inline golden 2 件 (document.rs / feature.rs) を `kind: line` 同期 (GLM が修正)
- [ ] Round 5 (これから): xtask inline test の import 順を alphabetical に揃える

## 次にやること

GLM-implementer に上記 1 ヶ所の修正のみを依頼。他の inline test や golden は壊さないこと (= 既に round 4 で fix 済み)。修正後 `cargo xtask ci` で全 green を確認。

## 追加で書いてほしいテスト

不要。
