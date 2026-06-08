## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|---|---|
| T02 の `waitForTimeout(1000)` をフレーム描画完了待ちに変更 | Rust API サーバーの起動（別 Issue #117） |
| T04 の `waitForTimeout(2000)` を要素可視化待ちに変更 | Playwright 設定の matrix 化 |
| プラットフォーム依存スナップショット名についてコメント追記 | スナップショット画像の再生成 |

## Non-Goals

- `retries` 設定の変更
- 他テスト（T01/T03/T05〜T09）のタイムアウト変更
- Rust API サーバーの起動設定（#117）

## 実装対象

`web/tests/viewer.spec.ts` の T02 と T04 のみ変更。

**T02 修正**:
```ts
// Before
await page.waitForTimeout(1000);

// After
await page.waitForFunction(
  () => (window as any).__viewer?.renderer?.info?.render?.frame > 0,
  { timeout: 10_000 },
);
```

**T04 修正**:
```ts
// Before
await page.waitForTimeout(2000);

// After
await page.waitForSelector("#error", { state: "visible", timeout: 10_000 });
```

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|---|---|---|---|
| T01 | 正常系 | 修正後の T02 が canvas 描画を確実に検出して pass | PASS |
| T01_boundary_degen | 境界 | `__viewer` 未設定時は waitForFunction がタイムアウト | timeout error |

## 幾何的不変条件チェックリスト

N/A（Playwright テスト設定のみ）
