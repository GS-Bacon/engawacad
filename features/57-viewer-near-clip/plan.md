## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| camera.near を 0.01 → 1.0 に変更 | ビューアのレンダリング品質改善 |
| controls.minDistance を maxDim×0.1 で動的設定 | カーネル/フォーマット変更 |

## Non-Goals
- カメラ FOV の変更
- OrbitControls の詳細設定 (dampingFactor 等)
- モバイル対応

## 実装対象

**影響ファイル:** `web/src/viewer.ts`

**before/after:**
```typescript
// Before
const camera = new PerspectiveCamera(50, width / height, 0.01, 10000);
// (controls.minDistance 未設定)

// After
const camera = new PerspectiveCamera(50, width / height, 1.0, 10000);
// bbox.isEmpty() block 内:
controls.minDistance = maxDim * 0.1;
```

## 設計方針
- 決定性: UI 変更のみ、出力数値への影響なし。
- minDistance はモデルサイズに比例（10%）で汎用性あり。

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01_viewer_loads | 正常系 | viewer.ts が TypeScript コンパイルを通過 | エラーなし |
| T02_degen_boundary | 退化/境界 | bbox が empty (0 bodies) の場合 minDistance 未設定でも問題ない | panic なし |

## 幾何的不変条件チェックリスト
- N/A: カーネル層変更なし
