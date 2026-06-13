## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `raise-issue-on-failure.ts` に「`ci.log` の失敗周辺を Issue 本文に転載するセクション」を追加 | ci.log 全文を Issue 本文に貼り付け (長すぎて読みづらい) |
| `SKILL.md` の STEP 1 (Issue 選択・作業ディレクトリ作成) に「自動起票 Issue 検出 → 前回 feature-dir の ci.log 必須調査」レールを追加 | 自動起票判定ロジックを Claude 内蔵化 (テキスト指示で十分) |
| `extract-ci-failure-context.ts` ロジックを `raise-issue-on-failure.ts` から純関数として export しユニットテスト追加 | 既存 issue 本文への遡及修正 (今後の起票分のみ対象) |

## Non-Goals

- raise-issue-on-failure.ts の起票判定ロジック改修 (現状の呼び出し条件はそのまま)
- 自動起票 Issue の自動修正・自動マージ (STEP 1 で人間 (Claude) が読んで判断する位置を維持)
- ci.log 解析の高度化 (機械的にエラー周辺行を抜くだけで足りる)

## 実装対象

<!-- Issue: #150 -->
<!-- 影響ファイル -->
- `.claude/skills/3ai/scripts/raise-issue-on-failure.ts` (ci.log 抜粋ロジック + 本文への追記)
- `.claude/skills/3ai/scripts/__tests__/raise-issue-on-failure.test.ts` (新規ユニットテスト)
- `.claude/skills/3ai/SKILL.md` (STEP 1 にレール追記)

### 1. `raise-issue-on-failure.ts` への ci.log 抜粋追加

純関数 `extractCiFailureContext(ciLog: string, maxLines: number = 40): string` を追加し export する:
- `error:`, `FAILED`, `panicked at`, `error[E` のいずれかが現れる最後の出現位置を探す
- その行の前後 `maxLines / 2` 行ずつを抜き出して返す
- どれもヒットしなければ ci.log の **末尾 maxLines 行** を返す
- ci.log が空・存在しない場合は空文字列を返す

呼び出し側で `${featureDir}/ci.log` を読んで `extractCiFailureContext` を通し、Issue body の `## エラー概要` の直後に追加:

```markdown
## ci.log 失敗周辺 (抜粋)
```
(extractCiFailureContext の出力)
```
```

### 2. `raise-issue-on-failure.test.ts` の単体テスト

`extractCiFailureContext` をターゲットに以下をテスト:

- T01: `error:` を含む ci.log の前後行を含む抜粋を返す
- T02: `FAILED` を含む ci.log の前後行を含む抜粋を返す
- T03: `panicked at` を含む ci.log の前後行を含む抜粋を返す
- T04: 何もヒットしないログ → 末尾 maxLines 行を返す
- T05: 空文字列入力 → 空文字列を返す
- T_boundary_short_log: 5 行のログから maxLines=40 を要求 → 全行返す (truncate なし)

### 3. `SKILL.md` の STEP 1 に追記

`## STEP 1: Issue 選択・作業ディレクトリ作成` の末尾に以下のサブセクションを追加:

```markdown
### STEP 1-A: 自動起票 Issue 検出時の前回 ci.log 必須調査

Issue タイトルに `[自動起票]` が含まれる、または body に `*このIssueは raise-issue-on-failure.ts により自動起票されました。*` が含まれる場合:

1. Issue body の `feature-dir: features/N-slug/` から元の作業ディレクトリを抽出する
2. **必須**: その feature-dir の `ci.log` を読み、`FAILED` / `error:` / `panicked` などのキーワードでエラー周辺を確認する
3. plan.md の「根本原因」セクションは **その grep 結果 + 関連コードを読んで自分の言葉で書く** (Issue 本文の error_summary を鵜呑みにしない)
4. 起票元 Issue の本文 `## ci.log 失敗周辺 (抜粋)` セクション (本 Issue #150 で導入) も併読すると効率的
```

## 設計方針

- **決定性**: ログ抜粋は純関数化 (`extractCiFailureContext`) → unit test 可能 → 同じ入力で同じ出力
- **エラーハンドリング**: ci.log が無くても起票本文を出す。抜粋セクションのみ省略
- **derive 規約**: 該当なし (TypeScript)
- **workspace.dependencies**: 該当なし

## テスト計画 (ID 付き)

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 単体 | `error:` を含む ci.log の前後行抜粋 | 該当行と前後を返す |
| T02 | 単体 | `FAILED` を含む抜粋 | 該当行と前後を返す |
| T03 | 単体 | `panicked at` を含む抜粋 | 該当行と前後を返す |
| T04 | 単体 | 何もヒットしないログ | 末尾 maxLines 行を返す |
| T05 | 単体 | 空文字列入力 | 空文字列を返す |
| T_boundary_short_log | 境界 | 5 行のログ・maxLines=40 | 全 5 行を返す (truncate なし) |

## 幾何的不変条件チェックリスト

- [ ] N/A (3ai スキル auto-raise ロジック改善。Boolean/Partition/Assemble 非関連)
