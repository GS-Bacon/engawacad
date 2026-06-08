<!-- Round ごとに以下の形式で追記すること -->
<!-- ## Round N -->
<!-- - R0X: 「<指摘の要点>」を棄却 — <理由（scope 外、Non-Goals に記載済み、次 issue で対応 等）> -->

## Round final-1

- FN01 (medium): 「commit() が validate 済みであることを検証しない」を棄却 —
  設計方針に「呼び出し側が validate を通す責務を持つ（IN01 順序不変条件）」と明記済み。
  ValidatedDocument 型ラッパーは Phase 7 以降の別 Issue で検討する事項（今回の Non-Goals）。

- FN02 (low): 「commit() 内で事後条件 assert を追加する提案」を棄却 —
  commit() 内で validate() を呼ぶと IN01 の validate→assemble→write→commit 順序が崩れ、
  パフォーマンス二重コストになる。handler.rs のコードパスで validate は常に先行する。
