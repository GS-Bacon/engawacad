## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `.claude/settings.json` に PostToolUse hook (`Edit\|Write` matcher) を追加 | 案 B (スキル定義の手順追加) — 案 A の hook で代替済み |
| `.claude/skills/3ai/scripts/post-rust-fmt.ts` を新規追加: tool 入力の file_path を読み `*.rs` なら `cargo fmt --all` を実行 | NotebookEdit 経由の rust 編集 (現実装でフロー上発生しない) |
| 同スクリプトの bun:test ユニットテスト (`__tests__/post-rust-fmt.test.ts`) を追加 | GLM 自体が tests/ を書いたケースの自動 fmt (GLM dispatch 内で `cargo xtask ci` を回しており別経路でカバー) |

## Non-Goals

- ファイル単位での fmt-check (案 A 後段)。`cargo fmt --all` は workspace 全体に走るため、対象ファイルだけを fmt する細粒度は不要
- guard-crates の挙動変更
- スキル定義 (`SKILL.md`) の手順テキスト変更 — hook 化により obligatory が自動担保される

## 実装対象

<!-- Issue: #149 -->
<!-- 影響ファイル -->
- `.claude/settings.json` (PostToolUse hook 追加)
- `.claude/skills/3ai/scripts/post-rust-fmt.ts` (新規)
- `.claude/skills/3ai/scripts/__tests__/post-rust-fmt.test.ts` (新規)

### 1. `.claude/settings.json` への hook 追加

既存の `hooks.PreToolUse` セクションはそのまま維持。`hooks` キー直下に `PostToolUse` セクションを追加し、`Edit|Write` matcher で `post-rust-fmt.ts` を呼ぶ。

### 2. `post-rust-fmt.ts` の実装方針

- stdin から JSON を読む (Claude Code が PostToolUse hook に流すペイロード)
- `tool_input.file_path` を取り出し、`*.rs` で終わるなら `cargo fmt --all` を CWD=`/home/bacon/mycad` で実行
- file_path がない / rust 以外 / malformed JSON はすべて no-op で exit 0
- `cargo fmt --all` が失敗してもプロセスは block しない (`stderr` に [post-rust-fmt] prefix でログ → exit 0)
- 純関数 `extractRustFilePath(stdin: string): string | null` を export して unit test 可能にする

### 3. `post-rust-fmt.test.ts` の実装方針

`extractRustFilePath` に対する以下のケースをカバー:
- Edit tool で .rs file_path を渡す → path を返す
- Write tool で .toml file_path を渡す → null
- tool_input が無い → null
- file_path が無い → null
- malformed JSON → null
- .rs.bak (rust ではない) → null (T_boundary_rs_bak)

## 設計方針

- **決定性**: hook は stdin の JSON のみで判定 (purely functional)。`cargo fmt --all` 自体も deterministic。
- **エラーハンドリング**: `cargo fmt --all` 失敗時も exit 0 で flow を block しない。stderr に [post-rust-fmt] prefix で記録。
- **derive 規約**: 該当なし (TypeScript)。
- **workspace.dependencies**: 該当なし。

## テスト計画 (ID 付き)

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 単体 | `extractRustFilePath` に Edit tool の .rs file_path を渡す | path を返す |
| T02 | 単体 | Write tool の .toml file_path を渡す | null を返す |
| T03 | 単体 | tool_input が無い | null を返す |
| T04 | 単体 | file_path が無い | null を返す |
| T05 | 単体 | malformed JSON | null を返す |
| T_boundary_rs_bak | 境界 | .rs.bak (rust ではない) | null を返す |

## 幾何的不変条件チェックリスト

- [ ] N/A (3ai スキル hook 改修。Boolean/Partition/Assemble 非関連)
