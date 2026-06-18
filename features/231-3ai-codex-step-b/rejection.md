<!-- Round ごとに以下の形式で追記すること -->
<!-- ## Round N -->
<!-- - R0X: 「<指摘の要点>」を棄却 — <理由（scope 外、Non-Goals に記載済み、次 issue で対応 等）> -->

## B-6 Round 2

- R01: 「`--persona` が design 経路にも適用される」(F-cont-03, low) を棄却 — plan.md Non-Goals「dispatch-codex の `--mode design` 経路への persona 適用」と明記済。`buildPrefix` を mode 共通実装にしたのは設計選択で、未使用 design+persona の組合せは behavior は出すが副作用なし (PERSONA SCOPE prefix が追加されるだけ)。mode 分岐を入れると複雑化する一方で得るものがない。
