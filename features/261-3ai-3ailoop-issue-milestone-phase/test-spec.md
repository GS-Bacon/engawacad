# Test spec — #261 子 Issue 起票時に親 milestone を継承する

## 対象スコープ

- `.claude/skills/3ailoop/scripts/loop-split-detector.ts` の `fetchParentMilestone`
- `createChild` の `--milestone` 引数組み立て (統合テストは gh 副作用ありで省略、ロジック単体で代表)

## 不足テスト（plan 計画分）

plan のテスト計画 ID 表 (T01-T07) は **すべて `loop-split-detector.test.ts` に実装済み**。

| ID | 状態 | テスト関数 |
|----|------|------------|
| T01 | (純粋関数性で内包) | 各 case が pure ghFn を呼ぶことで決定性は自然に成立 |
| T02 | ✅ implemented | `親に milestone が紐付いている → title を返す` |
| T03_boundary_null | ✅ implemented | `親に milestone なし (null) → null` |
| T04_degen_gh_fail | ✅ implemented | `親 Issue の取得失敗 (gh exit != 0) → null` |
| T05_degen_invalid_json | ✅ implemented | `gh 出力が JSON でない → null` |
| T06_degen_missing_field | ✅ implemented | `milestone フィールドが欠落 → null` |
| T07_degen_missing_title | ✅ implemented | `milestone.title が undefined → null` |

## 実装差分から追加すべきテスト

- なし — `fetchParentMilestone` の全分岐 (success / parse-fail / exit-nonzero / null-milestone / missing-title) は T02-T07 で網羅
- `createChild` の `--milestone` 引数追加は gh 副作用に依存 (実 issue 作成)。production 副作用テストは loop 本番 cycle で間接観測する (今後の split で生成される子 Issue の milestone を batch-select の phase-feature tier で検証)

## エッジケース・退化入力

- gh 標準出力が空文字列 → `JSON.parse("")` で SyntaxError → catch 節で `null` を返す (T05 の "not json at all" と同一経路でカバー)
- gh が ProcessError を投げる → `Bun.spawn` の `exitCode` が null になり `?? 0` で 0 扱い → JSON.parse は空文字列で fail → null (T04/T05 で代表)

## 数値境界

- N/A (文字列 / null のみ)

## 決定性

- 全 6 ケースで fake ghFn を DI、external state ゼロ → 同一入力で同一出力が保証される
- 本番 `runGh` も `gh issue view` を読み取り専用で呼ぶだけ (副作用なし) → 同 cycle 内では決定的
