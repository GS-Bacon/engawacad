# Codex final review findings (#218)

非 blocking medium/low の指摘記録。後続 Issue (#220) 完了後にまとめて拾うか、近接 cycle で別途対応する。

## Round 6 (blocking=0, verdict=pass)

- **F01 (medium)**: T10 `t10_very_small_depth` (depth=1e-6 境界) が `#[ignore = "blocked by #220"]` のままで、Face EntityRef + ExtrudeCut 経路の正の極小 depth が CI で全く検証されない
  - 推奨対応: 現在の plumbing contract (bodies.len()==1 + feature_id + validate_manifold) と同形の active test を残す
  - 本 Issue での扱い: scope内であれば追加可能だが、すでに codex_loops=6 上限到達 + verdict pass のため本 Issue では未対応。`#220` (kernel boolean) または近接 cycle の foundation Issue でまとめて拾う
  - 影響: medium のみ・blocking=0・STEP 7.5 verdict=pass

## Round 1〜5 で消化済 (累積)

| Round | Severity | ID | 対応 |
|-------|---------|-----|-----|
| r1 | critical | F01 | 新規ファイル untracked → cad ブランチ commit 済 |
| r1 | medium | F02 | T05 `matches!` で variant pin |
| r1 | medium | F03 | T11 unused `solid` 削除 |
| r2 | high | F01 | T12 plane_ref e2e 経路の z range 確認追加 (旧版) → r3 で構造変更 |
| r3 | high | F01 | T02 split (plumbing/topology) |
| r3 | medium | F02 | T11 を full snapshot に強化 |
| r4 | high | F01 | fixture を #215 t04 形状に揃え、T02 plumbing で validate_manifold active |
| r4 | medium | F02 | 例 file の `plane: xy` + `plane_ref: Entity` 重複コメントの整理 |
| r5 | high | F01 | T02 plumbing に XY 座標 (-2,0,5) など face basis sanity check 追加 |
| r5 | medium | F02 | T11 を serde_json snapshot に切替 (half_edges/loops/shells/names も比較) |

## 関連 follow-up Issue

- #220 — kernel boolean MultipleOuterShellsResult 制限解消 (real hole drilling 実現)
