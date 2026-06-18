<!-- Round ごとに以下の形式で追記すること -->
<!-- ## Round N -->
<!-- - R0X: 「<指摘の要点>」を棄却 — <理由（scope 外、Non-Goals に記載済み、次 issue で対応 等）> -->

## Round 1 / Round 2 全採用警告について (自律モード判断)

`check-full-adoption-warning` が RC=1 を返したが、両 round とも **issues 自体が 0 件** で「採用も棄却も発生しなかった」ケース。これは「scope 防衛不足で全採用」のシグナルではなく、本 Issue の実コードスコープが極小なため GLM 3 ペルソナとも指摘不要と判断した結果。

具体スコープ:
- `migration.rs` 新規 (trait 定義のみ、~30 LOC)
- `error.rs` に `UnknownSchemaVersion` variant 1 行追加
- `document.rs` で 2 箇所 (`from_yaml` / `impl Deserialize`) に reject ロジック 5 行ずつ追加
- 単体テスト 7 本

合計 ~80 LOC 程度の追加で、副作用範囲も `crates/engawa-format` 内に閉じる。plan.md 時点で In-Scope/Out-of-Scope/Non-Goals が明文化されており、GLM が指摘する余地が物理的にない。

scope 防衛は確認済み — Non-Goals に列挙した「実 migration ロジック / downgrade migration / 他レイヤへの伝搬」を本 Issue に含めない判断は固定で、これらを誘惑する設計部分は plan.md に存在しない。

## STEP 7.5 Codex r2 棄却

- **M-F01** (high, file: `crates/engawa-format/src/error.rs`): 「`#[non_exhaustive]` 付与で既存 exhaustive match が壊れる」を棄却 — 理由:
  - 本 Issue は phase 9 の format 拡張で ADR-014 (draft 中) が breaking 変更を許容する前提 (新 trait + 新 variant)
  - `#[non_exhaustive]` を外すと次回 variant 追加でまた exhaustive match を壊す。今回の breaking を呑む代わりに forward-looking compat を獲得するのが r1 M-F02 採用の趣旨
  - `engawa-api/src/error.rs` の `From<FormatError>` は本 issue 内で追従済み (CI green)
  - M-F01 は r1 M-F02 と直接矛盾する指摘で、両方同時に採用することは構造的に不可能
