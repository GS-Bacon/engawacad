## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| Three.js `AxesHelper` を scene に追加（XYZ 常時表示） | 独立したコーナー viewport インジケータ（secondary renderer が必要） |
| `web/src/viewer.ts` の修正のみ | Rust カーネル・CLI への影響なし |
| AxesHelper のサイズを bbox から動的に設定 | アニメーション・フェードアウト等の UI 演出 |

## Non-Goals
- 軸ラベル（テキスト表示）の追加
- カメラに追従する固定コーナーインジケータ
- Playwright 視覚回帰テスト（Issue #61 で対応）

## 実装対象
- Issue: #58
- ファイル: `web/src/viewer.ts`
- 変更: `AxesHelper` を import して scene に追加する（bbox 計算後に配置）

### 修正箇所

**Before** (import section に AxesHelper を追加):
import { AmbientLight, Box3, ... } from "three";

**After**:
import { AmbientLight, AxesHelper, Box3, ... } from "three";

**Before** (initViewer 内 bbox 計算後):
```
scene.add(group);
const bbox = new Box3().setFromObject(group);
if (!bbox.isEmpty()) {
  const center = new Vector3();
  bbox.getCenter(center);
  controls.target.copy(center);
  const size = new Vector3();
  bbox.getSize(size);
  const maxDim = Math.max(size.x, size.y, size.z);
  ...
}
```

**After** (size を外に出して AxesHelper を追加):
```
scene.add(group);
const bbox = new Box3().setFromObject(group);
const size = new Vector3();
let axisSize = 10;
if (!bbox.isEmpty()) {
  const center = new Vector3();
  bbox.getCenter(center);
  controls.target.copy(center);
  bbox.getSize(size);
  const maxDim = Math.max(size.x, size.y, size.z);
  axisSize = Math.max(maxDim * 0.5, 1);
  ...
}
scene.add(new AxesHelper(axisSize));
```

## 設計方針
- 決定性: Three.js 設定追加のみ。状態・乱数なし
- 退化幾何: bodies=[] のとき bbox は空 → axisSize=10 の固定値を使用
- derive 規約: N/A
- エラーハンドリング: N/A
- workspace.dependencies: 変更なし

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01_type_check | 正常系 | TypeScript type-check + vite build が通る | CI green |
| T02_boundary_degen_empty | 境界 | bodies=[] でも AxesHelper が追加され crash しない | CI green |

視覚的確認は Issue #61 の Playwright テストで実施。

## 幾何的不変条件チェックリスト
- N/A（ビューア実装のみ）
