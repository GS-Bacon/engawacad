## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|---|---|
| `extrude.ts` 全公開関数の Vitest ユニットテスト追加 | Playwright E2E（別 Issue #116） |
| 正常系・退化入力・境界値の網羅 | GLM / API 呼び出しのモック |
| `EPSILON_GUARD` の有効性チェック（depth クランプ確認） | EPSILON_GUARD 値の変更（#111 で対応） |

## Non-Goals

- `viewer.ts` / `main.ts` のテスト（別 Issue）
- API サーバーへの実リクエスト
- `EPSILON_GUARD` の修正（#111 で対応）

## 実装対象

新規ファイル: `web/src/extrude.test.ts`

対象関数: `planeForFaceId`, `planeForFaceNormal`, `footprintProfile`, `faceOffsetFromPlane`, `faceToCanonicalPlaneDistance`, `insetRect`, `buildExtrudeFeatures`, `buildExtrudeCutFeatures`

## 設計方針

- テストフレームワーク: vitest（既に devDependencies に存在、`vite.config.ts` で `src/**/*.test.ts` を include）
- ヘルパー: box mesh の flat positions/indices/faceIds を作成するユーティリティ関数でテストデータを共有
- 決定性: pure functions なので同一入力→同一出力は自明
- 幾何: N/A（純粋な座標計算関数のみ）

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|---|---|---|---|
| T01 | 正常系 | `planeForFaceId` が x/y/z 面で yz/xz/xy を返す | 各 plane 正しい |
| T01_boundary_null | 境界 | `planeForFaceId` に空文字・未知 face_id → null | null |
| T02 | 正常系 | `footprintProfile` yz 平面で Y:[-10,10] Z:[-15,15] の bounding box | 4 セグメント正しい |
| T02_boundary_degen | 退化 | `footprintProfile` 縮退面（全頂点同一）→ null | null |
| T03 | 正常系 | `faceToCanonicalPlaneDistance` yz/X=5 面 → 5.0 | 5.0 |
| T03_boundary_missing | 境界 | `faceToCanonicalPlaneDistance` 存在しない face_id → Infinity | Infinity |
| T04 | 正常系 | `buildExtrudeFeatures` が sketch offset と depth を正しく設定する | offset=5, depth=3 |
| T04_boundary_invalid | 境界 | `buildExtrudeFeatures` depth<=0 → null | null |
| T05 | 正常系 | `buildExtrudeCutFeatures` depth > face_distance → depth が face_distance-ε にクランプされる | depth < face_dist |
| T05_boundary_zero_dist | 境界 | `buildExtrudeCutFeatures` face が原点上（distance=0）→ null | null |
| T06 | 正常系 | `insetRect` ratio=0.25 で各辺が 25% 縮む | 正しい座標 |
| T06_boundary_degen | 退化 | `insetRect` ratio=0.5 → null（extent が 0 になる） | null |

## 幾何的不変条件チェックリスト

N/A（TS pure functions のため）
