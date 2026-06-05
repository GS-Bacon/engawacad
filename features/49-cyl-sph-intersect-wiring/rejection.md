
## Round 3
- IN01 (invariant, critical): 棄却 — 事実誤認。plan「設計方針 > 決定性」(line 66) は「全 EntityID は IdGenerator で決定的に生成」と明記。Uuid::new_v4() は plan のどこにも存在しない（grep で 0 件）。指摘は実在しない記述に対するもの。
- IN02 (invariant, high): 棄却 — 事実誤認。plan「テスト計画」(line 84) に「T01 | 決定性 | 同一 Intersect を 2 回 build し全 EntityID・座標一致 | assert_eq!」が存在。T01 は計画済み。

## Round 4
- NU01 (numeric, low): 棄却 — スコープ外。r_sq の次元（length²）比較は幾何コア `intersect_cylinder_sphere`（surface_intersect.rs:288,291、実装済み・テスト済み）の挙動であり、#49 は当該コアを変更しない（Out-of-Scope: 幾何コアは実装済み）。plan はコアの既存 tolerant 挙動を正しく記述しているのみ。コアの ε 次元見直しは別 Issue 相当。
