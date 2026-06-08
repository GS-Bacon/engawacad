# #90 plan: fix(viewer): render loop を rAF に統一し damping と DPR を設定する

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|---|---|
| `controls.dampingFactor = 0.1` の明示設定 | フィット表示・標準ビューボタン (#91) |
| `window.__viewer = { renderer, camera, controls }` をテスト検査フックとして露出 | 面ピッキング・Feature 書き込み |
| Playwright T05: `renderer.getPixelRatio() === window.devicePixelRatio` | キーボードショートカット |
| Playwright T06: `controls.enableDamping === true` | 本番環境でのフック除去 |

## Non-Goals

- フィット表示・標準ビューボタン（#91 で対応）
- 面ピッキング・Feature 操作
- キーボードショートカット

## 実装対象

`web/src/viewer.ts` および `web/tests/viewer.spec.ts` のみ。`crates/**` は変更なし。

**調査済み**: `enableDamping = true`(L50) / rAF ループ(L182-188) / `setPixelRatio(devicePixelRatio)`(L46) はすべて実装済み。

**修正箇所 1** — viewer.ts: `dampingFactor` 明示追加:
```typescript
// before (L50)
controls.enableDamping = true;

// after
controls.enableDamping = true;
controls.dampingFactor = 0.1;
```

**修正箇所 2** — viewer.ts: `animate()` 呼び出し直前に `window.__viewer` 露出:
```typescript
// before
  animate();
  return { ... };

// after
  (window as any).__viewer = { renderer, camera, controls };
  animate();
  return { ... };
```

**追加** — viewer.spec.ts: T05/T06 を `T04` の後に追加（既存 `setupPageWithFixture` を再利用）

## 設計方針

- `window.__viewer` はローカル開発ツールとして露出（prod 環境も同じサーバのため除去不要）
- `(window as any)` でキャスト — TypeScript の型定義ファイル追加は不要（テスト側でも `any` キャスト）
- `dampingFactor = 0.1` は Three.js デフォルト(0.05)より大きく、適度な制動感を提供

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|---|---|---|---|
| T05 | Playwright 正常系 | `__viewer.renderer.getPixelRatio() === window.devicePixelRatio` | pass |
| T06 | Playwright 正常系 | `__viewer.controls.enableDamping === true` | pass |
| T06_degen_no_hook | 境界 | `__viewer` 未設定時: optional chaining `?.` で `undefined` を返し例外なし | テスト側で `?.` を使用することを確認 |

## 幾何的不変条件チェックリスト

N/A（レンダリング設定変更のみ、幾何演算なし）
