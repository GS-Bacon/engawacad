# 既存実装コンテキスト（偽陽性防止）

以下の機能は **今回の diff に含まれない pre-existing 実装** です。reviewer はこれらが既に存在していることを前提にレビューしてください。

## renderer.setPixelRatio

`web/src/viewer.ts` L46（変更なし・既存コード）:
```typescript
renderer.setPixelRatio(window.devicePixelRatio);
```
→ T05 テストはこの既存実装を検証します。

## controls.update() in animate()

`web/src/viewer.ts` L185-186（変更なし・既存コード）:
```typescript
function animate() {
  animFrameId = requestAnimationFrame(animate);
  controls.update();  // ← 毎フレーム呼び出し済み
  renderer.render(scene, camera);
}
```

## 今回の diff で追加した内容のみ

1. `controls.dampingFactor = 0.1;`（L51 付近）
2. `(window as any).__viewer = { renderer, camera, controls };`（animate() 直前）
3. Playwright T05/T06 テスト（viewer.spec.ts）
