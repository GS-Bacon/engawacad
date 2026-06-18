## 自律判断ログ

- Issue body は `docs/decisions/015-*.md` を作る指示だが、015 は #237 で `015-phase9-design-foundations.md` に採用済み。**ADR 番号を 015→016 に繰り上げる**。Issue title は gh edit で `ADR-016` に変更済。
- light flow / intent-check 不要 / keep_codex_gate=false → STEP 7.5 skip
- STEP 6 (core) / STEP 6.6 (test) は ADR ファイル + tests/ 配下のため Claude が直接書く (guard-crates 対象外)

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `docs/decisions/016-engawa-cli-naming.md` を draft 状態 (Status: Proposed) で commit | CLI コマンドの実装 (#242 で実装) |
| 動詞/名詞順序 (`engawa <object> <verb>` 採用 + 理由) | サブコマンドのオプション具体形 (各 op で詰める) |
| サブコマンド階層深さ上限 (= 2 階層 `engawa <object> <verb>`) | 既存 `engawa run` `engawa convert` 等の改名 |
| 共通フラグ命名 (`--feature-id`, `--at`, `--before` など) | フラグの実コード追加 |
| Decision Matrix (Options A/B/C + Trade-off + 採用前提崩壊 trigger) | 命名以外の CLI UX (color / interactive prompt 等) |
| 既存 ADR (-006 / -013 / -015) との関係 | 既存 ADR の改訂 |

## Non-Goals

- CLI 実装は本 Issue で行わない
- 既存 `engawa run`/`engawa convert` 命令の改名なし
- 色付け、対話プロンプト等の UX (本 ADR は命名規約のみ)

## 実装対象

- Issue: #238
- 影響ファイル:
  - `docs/decisions/016-engawa-cli-naming.md` (新規)
  - `crates/engawa-cli/tests/adr_016_cli_naming_doc_acceptance.rs` (新規、ADR file 構造の smoke test)

## 設計方針

ADR スタイルは ADR-015 と同じ heading 構造 (Context / Decision / Decision Matrix / Trade-off / 採用前提崩壊 trigger / 既存 ADR との関係 / Open Questions)。Decision Matrix lint 準拠。

### 数値モデル

N/A

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | ADR file を 2 回 read → 同一 | assert_eq |
| T_DOC_required_sections | 正常系 | 必須 6 セクション存在 | grep pass |
| T_DOC_decision_matrix_has_options | 正常系 | Options A/B/C 記載 | grep pass |
| T_DEG_file_missing | 退化 | file 存在 check | exists() == true |
| T_BOUNDARY_status_accepted | 境界 | Status: Proposed/Accepted | grep pass |

## 幾何的不変条件チェックリスト

- [x] N/A — ADR draft Issue
