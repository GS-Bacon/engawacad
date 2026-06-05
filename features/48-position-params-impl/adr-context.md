# ADR 抜粋（#48 レビュー用コンテキスト）

本 Issue は CreateCylinder/CreateSphere に位置パラメータ (origin/center) を追加する impl Issue。
ADR-005 §7 の position 段落（#47 で追記済み）と ADR-004 の数値方針が前提。

## ADR-005 §7 — 位置パラメータの扱い（#47 で追記済み）

**位置パラメータの扱い**: canonical local frame を導く feature パラメータには、形状パラメータ
(radius/height 等) に加え位置パラメータ (`CreateCylinder` の origin、`CreateSphere` の center 等) を含む。
position は frame の**原点を決めるだけ**で、role 名 (`lateral`/`cap_top`/`seam`/`surface` 等) や
内部 canonical name grammar `<feature_id>;<kind>:<role>` には影響しない。position を省略した場合は
canonical 原点 (0,0,0) にデフォルトし、既存の example YAML は不変のまま有効である。
回転 (rotation) の扱いは別 Issue で決定する。

face role: CreateCylinder = `lateral`/`cap_top`/`cap_bottom`、CreateSphere = `surface`。
専用 role: sphere 極 edge `north_pole`/`south_pole`、seam edge `seam`、cylinder rim `cap_top_rim`/`cap_bottom_rim`、cylinder seam `seam`。

## ADR-004 — 数値方針（要点）

- tolerant 方式を採用。点の同一判定・座標比較に許容誤差を用いる。
- 既存 `assert_solids_equal` の座標比較許容は `< 1e-12`。
- 本 Issue は新規 tolerance を導入せず、位置は座標へ exact に反映する（平行移動のみ）。
