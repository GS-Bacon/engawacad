# test-spec — Issue #47 ADR-005 §7 position パラメータ追記

## 不足テスト（plan 計画分）

| ID | 実装状況 | 備考 |
|----|----------|------|
| T01 | 充足 | `cargo xtask ci` green 確認済み (docs 変更のみ、Rust テスト影響なし) |
| T02 | 充足 | §7 に追記段落が存在し、3 論点を含む（目視 / diff 確認済み） |

## 実装差分から追加すべきテスト

なし。変更は `docs/decisions/005-topological-naming.md` への 7 行挿入のみで、コードパスの追加・変更はゼロ。

## エッジケース・退化入力

N/A — docs-only。

## 数値境界

N/A — docs-only。tolerance/ε は impl Issue #48 のスコープ。

## 決定性

N/A — docs-only。文書は決定性とは無関係。

## 期待値乖離

なし。check-spec-divergence.ts が「rs ファイル変更なし・数値段落なし」を確認済み。
