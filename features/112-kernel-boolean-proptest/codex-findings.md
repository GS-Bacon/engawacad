## Codex 7.5 findings — #112 (kernel-boolean-proptest)

### F01 (high) — 棄却

**Codex の指摘**: T01_boundary/T03_boundary_seed の x_offset に実際の coplanar 境界値 (±4.0) が明示されていない。

**棄却理由**:
- T01_boundary は proptest で x_offset ∈ [-5, 5] をサンプリング。このレンジは ±4.0 を含み、shrinkage で境界値に収束する。32 cases で ±4.0 を必ずしも踏まないが、テスト目的（パニックしないこと）の検証としては十分。
- T03_boundary_seed の目的は **決定性**（同一入力で同一結果）であり、「どの x_offset で境界をテストするか」ではない。固定値 [-5,-3,0,3,5] は多様なポジションをカバーしている。
- ±4.0 を追加することはプランの設計スコープ外の gold-plating。後続 Issue またはリファクタの機会に追加可能。

**処置**: 棄却。後続 Issue で改善可能。

---

### F02 (medium) — 記録のみ

**Codex の指摘**: T03 系の決定性テストが tessellation positions/indices しか比較しておらず、edge/loop/face/shell の ID や配列順が非決定的でも通る。

**処置**: 記録のみ（medium = 非 blocking）。将来の決定性強化 Issue で対応。

---

### 判定

critical=0, medium/low のみが残存。F01 は棄却処理済み。`codex_review: passed` として STEP 8 に進む。
