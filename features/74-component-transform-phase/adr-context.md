# ADR-007 抜粋（#74 関連）

## §1 transform を B-rep 本体に焼き込む（案A）
transform (位置・回転) を幾何型に直接適用する。#74 は平行移動のみ。

## §2 平行移動を先に、回転は独立 Issue で実装
- 回転リスク: ①Euler角→sin/cos の誤差 ②Cylinder/Cone パラメータ基準 ③Sphere シームと pcurve。
- `examples/assembly.mycad` の rotation は全て `[0,0,0]` → 平行移動だけで Phase 5 完了条件を満たせる。
- #74 は `transform.rotation` を**無視**（全ゼロ前提。非ゼロでも警告なしに無視）。

## §3 参照解決 → #73 完了済み
#73 で `build_assembly` が実装済み（循環検出・深さ上限・stdlib/file 解決）。
#74 はその `build_component_tree` に `accumulated_offset: Vec3` を追加して translation を合成する拡張。

## §ASSEMBLY ペルソナ観点（#74 向け）
① transform 合成順: 親→子の順に `accumulated_offset + local_position`、子の Body に `Solid::translate` 適用。
② 参照解決のガードは #73 実装を再利用。
③ EntityID 決定性: translate は EntityID を変更しない（#72 T04 保証済み）。
④ pcurve 整合: `Solid::translate` は UV 空間の pcurve を変えない（#72 設計方針）。
⑤ stdlib_root: #73 実装を再利用。
