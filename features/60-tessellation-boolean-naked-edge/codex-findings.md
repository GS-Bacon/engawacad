# codex-findings.md — Issue #60 非 blocking 指摘記録

## F02 (medium) — 量子化グリッドの境界問題

**ファイル**: `crates/mycad-kernel/tests/bool_naked_edge_acceptance.rs:15`

**内容**: `round(p/tol)` 量子化では「tol 未満なら同一点」にならない場合がある。
0.49*tol と 0.51*tol は差が tol 未満でも別セルに落ちる可能性がある。
T09 も 1e-11 差のケースのみで境界ずれを検出できていない。

**判断**: medium 指摘。Boolean tessellate の実メッシュでは同一 B-rep 頂点は
同一 f64 値になるため実運用上の false positive リスクは低い。
pairwise weld への変更は O(n²) の計算コスト増を伴うため
現時点では受容して medium として記録する。
将来、より厳密な weld が必要になった場合は別 Issue で対応する。
