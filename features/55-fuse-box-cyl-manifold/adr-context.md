# ADR-004 抜粋 (#55 関連: 公差モデル)

## 公開公差定数 (geometry::math / crate root 再エクスポート)
| 定数 | 値 | 用途 |
|------|-----|------|
| `LENGTH_TOLERANCE` | `1e-9` (mm) | 距離・座標の絶対比較 |
| `ANGLE_TOLERANCE` | `1e-9` (rad) | 角度・パラメータの絶対比較 |
| `RELATIVE_TOLERANCE` | `1e-9` (無次元) | スケール比例の相対比較 |

## 比較 helper (geometry::math に集約済み — 新規に書かず再利用すること)
- `length_near(a, b) -> bool`
- `angle_near(a, b) -> bool`
- `point_near(a, b) -> bool`
- `point_near_scaled(a, b, scale) -> bool`

## 数値モデル: トレラント方式 (Phase 4 #31 で確定)
- Parasolid/ACIS 流 per-entity トレラント。比較は両端 Tolerance の max。
- グローバル定数は `Tolerance::DEFAULT` 値として残し、**新規コード経路から順次移行**。
  一括差し替えは決定性回帰リスクが高い。
- #34 (曲面 Boolean MVP, Plane×Cylinder 等) では per-entity tolerance 埋め込みは Non-Goal。

## #55 への含意
- 候補2(円弦頂点マージの許容緩和)を採る場合、独自の許容値を発明せず
  既存 `point_near` / `point_near_scaled` を局所利用すること。
- グローバル `LENGTH_TOLERANCE` の値自体は変更しない(決定性回帰リスク)。
  変更するなら「この円弦マージ経路のみ」に限定した局所的判定にとどめる。
