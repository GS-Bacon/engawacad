# tessellation/mod.rs: cylinder lateral の adj_is_sphere 死に分岐を削除する (ADR-009)

## 位置付け

**`type: foundation`** (ADR-002「ラベル運用: 2 軸ラベル制」)。Phase 7 milestone への差し込み作業として起票する。Phase 7 完了判定 (xy/xz/yz スケッチ → Extrude/ExtrudeCut → CreateSketch Feature) には Phase 7 type: feature Issue 群が直接寄与するが、本 Issue は Phase 7 で **「描いたスケッチから Extrude/ExtrudeCut を実行する」完了条件の支え**として、後段で Boolean 結果を表示するキャップ・側面の `n_u` 決定ロジックをクリーン化する。死に分岐の存在は新 Phase で Extrude 結果のテッセレーションを拡張する際の読解コストとなり、`/3ai` の GLM ワーカーがこの分岐を意味のあるロジックと誤認する誤動作経路を残す。ADR-002 §「差し込み作業は奉仕する Phase の Milestone に入れる」運用に沿う。

## 背景

`crates/mycad-kernel/src/tessellation/mod.rs:649-680` (cylinder lateral 面の `n_u` 決定ロジック) に、両腕とも `arcs_per_rev` を返す**死に分岐**が残存している。

```rust
let adj_is_sphere = outer_loop.half_edges.iter().any(|&he_idx| { ... });
if adj_is_sphere {
    arcs_per_rev   // Sphere 隣接
} else {
    arcs_per_rev   // Plane/other 隣接 — 値が同じ
}
```

これは #131 (cylinder の n_u arcs_per_rev 化) 試行錯誤跡。`adj_is_sphere` 計算自体は走るが、結果は使われない。

ADR-009 で交線円を周期エッジで保持する方針 (案 A) が決まり、`n_u` ヒューリスティクスは将来撤去予定。それまでの間、本分岐は読者 (人間・AI) に「将来 Sphere/Plane を区別するロジックがある」と誤認させる。

## 作業内容

1. `adj_is_sphere` の計算と if/else 構造を削除
2. `let n_u = if arcs_per_rev > 1 { arcs_per_rev } else { opts.angular_segments.max(3) };` に簡略化
3. 周辺コメントを 1〜2 行に簡潔化し、「ADR-009 実装で本ロジック全体がエッジ駆動に置換される予定」と明記

## 完了条件

- `cargo xtask ci` green
- 既存 cylinder tessellation テスト全て pass (回帰なし)
- 削除後の diff が 30 行以内 (純粋なクリーンアップ)

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|---|---|
| `tessellation/mod.rs:649-680` 死に分岐削除と周辺コメント簡潔化 | 共有境界の直接比較テスト追加 (別 Issue で対応) |
| | `n_u` ヒューリスティクスの撤去 (ADR-009 実装本体) |
| | `ANGULAR_SEGMENTS_DEFAULT` の意味変更 (同上) |

## Non-Goals

- 共有境界一致の構造的保証 (ADR-009 実装本体の責務)
- partition.rs 側の変更

## 関連

- ADR-009 §Implementation Outline Phase 3 即時実施分の片方
- #131 (試行錯誤跡を残した親 Issue)
- ADR-002 §差し込み作業ルール
