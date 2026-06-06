## ADR-007 §4 — 簡略 M5x20 ボルト（Issue #75 関連抜粋）

`stdlib/fasteners/jis_b1176/M5x20.mycad` を既存プリミティブで構成:
- 軸: `CreateCylinder` (radius=2.5mm, height=20mm)
- 頭: `CreateCylinder` (radius=4.5mm, height=3mm, origin=[0,20,0])
※ネジ山・六角形状は作らない。Phase 5 の完了条件は「ボルトが正しい位置に見える」まで。

stdlib のフォルダ規約: `stdlib/<category>/<standard>/<part_number>.mycad`

## ADR-007 ASSEMBLY ペルソナ レビュー観点（5点）
1. transform 合成の順序（親→子の適用順）が正しいか
2. 参照解決の循環・深さガードが全パスで有効か
3. 平行移動・回転で EntityID が変わっていないか（決定性）
4. pcurve と曲面パラメータの整合が transform 後も保たれるか
5. stdlib_root が未設定の場合の fallback が明記されているか
