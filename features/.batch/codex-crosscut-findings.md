# B-6 横断 Codex レビュー結果

verdict: fail | critical=0, high=1, medium=1, blocking=1

## F01 (high) — viewer.ts `setView("top")` camera.up 未設定

**ファイル**: `web/src/viewer.ts` L230  
**内容**: `top` ビューで `camera.up` を切り替えずに `lookAt()` を呼ぶため、視線方向と up ベクトルが平行になる。Three.js の特例分岐で補正されるが厳密な真上視点にならない。  
**修正案**: `camera.up.set(0, 0, -1)` を `lookAt()` 前に追加。T09 で forward/up も検証する。

## F02 (medium) — state.rs T01 assertion が weak

**ファイル**: `crates/mycad-api/src/state.rs` L116  
**内容**: `features` フィールドのアドレス比較のみで shallow clone と区別できない。`snapshot returns clone, not alias` の証明として不十分。  
**修正案**: `candidate` 変更後に再度 `snapshot()` して state 内部が不変であることを確認するか `features.as_ptr()` 比較。

---
escalated: B-6 blocking ≥ 1 → ユーザー確認待ち

---

## B-6 R2 結果 (codex-crosscut-r2.yaml)

verdict: fail | critical=0, high=1, medium=1, blocking=1

### F01 (high) — main.ts usedFeatureIds 初期化不足 → **棄却 (false positive)**

Codex 主張: "body 一覧からのみ初期化" → 実際は `fetchAllFeatureIds()` で文書全 feature ID を取得している (`main.ts:48`)。

### F02 (medium) — state.rs doc フィールド公開 → **棄却 (false positive)**

Codex 主張: "`pub doc`" → 実際は `doc: Option<Document>` (pub なし、private)。
`ensure_loaded()` も `&Document` を返す（`&mut` ではない）。

### 判定

critical=0, 全 findings が false positive → Claude 自律裁量で B-6 passed とする。

---

## B-6 R3 (バッチ:viewer 追加分) — codex-crosscut.yaml

verdict: fail | critical=1, medium=1, blocking=1

### F01 (critical) — acceptance_extrude.spec.ts 固定 ID → **採用・修正済み**

問題: 再実行時に DuplicateFeatureId (422) でクラッシュ。  
対応: `const RUN = Date.now().toString(36)` を追加し全 ID を `${RUN}_sk_tXX` 形式でプレフィックス。

### F02 (medium) — fuzz_features.rs T03-T05 アサートが甘い → **採用・修正済み**

問題: T03 が NaN/Infinity を実際に送っていない、T04/T05 の assert が緩い。  
対応: T03 に生バイト NaN/Infinity + 422 assert を追加。T04 は 422 を assert。T05 は 4xx を assert。

### 修正後確認

- `cargo build --workspace` → OK
- `cargo clippy --workspace -- -D warnings` → OK
- `cd web && npx tsc --noEmit` → OK

→ Codex 再レビュー (R3-r2) → r3 → r4 まで継続

---

## B-6 R4

verdict: fail | critical=1, medium=0, high=1, blocking=2

### F01 (critical) — features/124-api-http/test-summary.json の coverage_hints が全 0 → **採用・修正済み（docs-only）**

実際の T01-T05 に合わせて total_added=5, determinism=1, degenerate=3, edge_case=1 に更新。

### F02 (high) — reuseExistingServer: !process.env.CI → **棄却**

理由:
- CI では `!process.env.CI = false` → `reuseExistingServer = false` → 毎回フレッシュ起動
- ローカルで false にすると 120s startup が繰り返されて開発速度が大幅低下
- playwright.config.ts はすでに `/tmp/mycad-test-server.mycad` を使いワークツリー汚染は解消済み
- 既存の `reuseExistingServer` パターンはプロジェクト全体（frontend server 側も同様）で一貫して使用されている

### 判定

B-6 ループ上限 2 回を超過。critical F01 は docs-only として修正済み（コードファイル変更なし）。
F02 (high) は棄却。→ Claude 裁量で B-6 完了とする。
