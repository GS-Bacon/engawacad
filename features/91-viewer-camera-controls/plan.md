# #91 plan: fix(viewer): 初期フィット表示と Front/Top/Iso 標準ビューボタンを追加する

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|---|---|
| `index.html` に `#view-buttons` div + Front/Top/Iso の 3 ボタンを追加（`data-testid`付き） | damping/render loop（#90 で完了） |
| `viewer.ts` の `ViewerHandle` に `setView(view)` を追加 | 面ピッキング・Feature 書き込み |
| `main.ts` でボタンクリックを `handle.setView(...)` に接続 | キーボードショートカット |
| Playwright T07: 3 ボタンが DOM に存在 | アニメーション付きトランジション |
| Playwright T08: front ボタン後 camera が +Z 方向 | |
| Playwright T09: top ボタン後 camera が +Y 方向 | |
| T02 スクリーンショット更新（ボタン写り込みのため） | |

## Non-Goals

- damping/render loop（#90 で対応済み）
- 面ピッキング・Feature 操作
- キーボードショートカット
- アニメーション付きカメラトランジション

## 実装対象

### web/index.html

`#app` 内の最後（`<script>` タグ前）に追加:

```html
<style>
  #view-buttons {
    position: absolute;
    bottom: 1rem;
    right: 1rem;
    display: flex;
    gap: 0.4rem;
    z-index: 10;
  }
  #view-buttons button {
    background: rgba(0,0,0,0.5);
    color: white;
    border: none;
    padding: 0.4rem 0.6rem;
    border-radius: 4px;
    font-family: monospace;
    font-size: 12px;
    cursor: pointer;
  }
</style>

<!-- 既存 #extrude-panel の後に追加 -->
<div id="view-buttons">
  <button data-testid="btn-view-front">Front</button>
  <button data-testid="btn-view-top">Top</button>
  <button data-testid="btn-view-iso">Iso</button>
</div>
```

### web/src/viewer.ts

`ViewerHandle` インターフェース に追加:
```typescript
setView(view: "front" | "top" | "iso"): void;
```

実装（return オブジェクト内に追加）:
```typescript
setView(view: "front" | "top" | "iso") {
  const d = camera.position.distanceTo(controls.target);
  const t = controls.target;
  if (view === "front") {
    camera.position.set(t.x, t.y, t.z + d);
  } else if (view === "top") {
    camera.position.set(t.x, t.y + d, t.z);
  } else {
    // iso: 既存 fitCamera と同方向 (0.5, 0.5, 1) を正規化
    const len = Math.sqrt(1.5);
    camera.position.set(t.x + d * 0.5 / len, t.y + d * 0.5 / len, t.z + d / len);
  }
  camera.lookAt(controls.target);
  controls.update();
},
```

### web/src/main.ts

`initViewer` の後に追加:
```typescript
const viewFront = document.querySelector<HTMLButtonElement>('[data-testid="btn-view-front"]')!;
const viewTop   = document.querySelector<HTMLButtonElement>('[data-testid="btn-view-top"]')!;
const viewIso   = document.querySelector<HTMLButtonElement>('[data-testid="btn-view-iso"]')!;
viewFront.addEventListener("click", () => handle.setView("front"));
viewTop.addEventListener("click",   () => handle.setView("top"));
viewIso.addEventListener("click",   () => handle.setView("iso"));
```

### web/tests/viewer.spec.ts

T07/T08/T09 を追加。`window.__viewer` は #90 で露出済み。
camera position を `controls.target` からの相対値で検証する（model center に依存しないため堅牢）:

```typescript
// T07: view buttons exist
test("T07 view buttons exist in DOM", ...)

// T08: front view
test("T08 btn-view-front positions camera on +Z axis of target", ...)
// delta.dz > 0, |delta.dx| < 0.01, |delta.dy| < 0.01

// T09: top view  
test("T09 btn-view-top positions camera on +Y axis of target", ...)
// delta.dy > 0, |delta.dx| < 0.01, |delta.dz| < 0.01
```

## 設計方針

- `setView` は `camera.position.set()` → `camera.lookAt()` → `controls.update()` の順。damping 有効だが直接 set するため rAF ループでの位置ドリフトなし
- ボタンは `bottom: 1rem; right: 1rem` 配置（extrude-panel は top-right で衝突なし）
- T02 スナップショットはボタン写り込みのため `--update-snapshots` で再生成

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|---|---|---|---|
| T07 | Playwright 正常系 | 3 ボタンが DOM に存在し visible | pass |
| T08 | Playwright 正常系 | front ボタン後: camera.position 相対 dz > 0, |dx| < 0.01, |dy| < 0.01 | pass |
| T09 | Playwright 正常系 | top ボタン後: camera.position 相対 dy > 0, |dx| < 0.01, |dz| < 0.01 | pass |
| T08_degen_zero_distance | 境界 | camera が target と同位置 (d=0) でもクラッシュしない | 例外なし・位置変化なし |

## 幾何的不変条件チェックリスト

N/A（カメラ位置変更のみ、B-rep 演算なし）
