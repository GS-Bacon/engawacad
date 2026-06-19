## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `loop-split-detector.ts createChild` で親 milestone を gh API で取得し `--milestone` を子に渡す | 既存 actionable 化していない子 Issue (#255-#260) の遡及修正 (cycle 38 で手動付与済み) |
| `fetchParentMilestone(parent, ghFn)` を pure 関数として export し bun:test で検証 | gh API 失敗時のリトライ・複雑な error handling (silent skip で十分) |
| `CLAUDE.md` の Issue 起票チェックリストに「親 milestone 継承」を追加 + ⚠️ 抜けの代償を明記 | `/3ai` 自律分割側のコード (今回は doc で誘導のみ — Claude 直接起票は規約 reading で防止) |
| TypeScript 内テストは bun:test (実 gh は呼ばず fake ghFn でロジック検証) | Live gh API テスト |

## Non-Goals

- 既存 Phase 9 子 #255-#260 の本来 milestone 付与: 既に手動付与済みで対応不要
- `/3ai` skill の自律分割パスにコード追加: doc 規約強化で十分 (将来の split は detector 経由が主経路)
- `gh issue create --milestone` の数値 ID 指定への対応: title 指定で gh は milestone を解決できるため不要

## 実装対象

<!-- Issue: #261 -->
<!-- 影響ファイル: .claude/skills/3ailoop/scripts/loop-split-detector.ts, .test.ts; CLAUDE.md -->

### 変更 1: `loop-split-detector.ts`

- 新規 export 関数 `fetchParentMilestone(parent: number, ghFn?): Promise<string | null>`
  - `gh issue view <parent> --json milestone` を呼び `milestone.title` を返す
  - exit≠0 / JSON parse 失敗 / milestone なし / title undefined → すべて `null`
  - `ghFn` 引数で gh wrapper を DI (bun:test で fake 化)
- `createChild(entry, parent, dryRun)` で `fetchParentMilestone` を呼び、`parentMilestone` を取得。non-null なら `--milestone <title>` を `gh issue create` の引数に追加
- dry-run のログにも milestone 値を表示 (`milestone=Phase 9: ...` or `milestone=(none)`)

### 変更 2: `loop-split-detector.test.ts` (新規)

- bun:test で 6 ケース:
  1. milestone あり → title を返す
  2. milestone null → null
  3. gh exit≠0 → null
  4. 非 JSON → null
  5. milestone フィールド欠落 → null
  6. milestone.title 欠落 → null

### 変更 3: `CLAUDE.md`

- 「新 Phase 着手 / 新 Issue 起票時のチェックリスト」の step 5 として親 milestone 継承を追加
- step 6 (旧 5) を後ろにずらす
- ⚠️ 警告ブロックに `milestone 抜けの代償` を追加

## 設計方針

- **決定性**: 親 Issue title は gh API 経由で都度取得 — 同一親 Issue・同一 cycle 内は決定的
- **エラーハンドリング**: gh 失敗時は `null` を返し継承を silent skip (子は milestone なしで作成される)。これは「親に milestone 無し」と区別がつかないが、どちらも子の手動付与で recover 可能なため許容
- **derive 規約**: TS なので該当なし
- **workspace.dependencies**: TS スクリプトは standalone (no deps 追加)

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | 同一 fake ghFn を 2 回呼んで同一 title が返る | `expect(...).toBe("Phase 9: ...")` を 2 回連続で実行 → 一致 |
| T02 | 正常系 | milestone title が "Phase 9: 履歴編集" → そのまま返す | `toBe("Phase 9: 履歴編集")` |
| T03_boundary_null | 境界 | milestone が `null` → `null` | `toBeNull()` |
| T04_degen_gh_fail | 退化 | gh exit≠0 → `null` | `toBeNull()` |
| T05_degen_invalid_json | 退化 | gh stdout が "not json at all" → `null` | `toBeNull()` |
| T06_degen_missing_field | 退化 | `{}` で milestone フィールド欠落 → `null` | `toBeNull()` |
| T07_degen_missing_title | 退化 | `{ milestone: {} }` で title 欠落 → `null` | `toBeNull()` |

決定性 (T01) は別テストとして並べず、各ケースで純粋関数性が担保されることで自然に成立する。

## 幾何的不変条件チェックリスト

- N/A (TypeScript skill tooling Issue — B-rep 幾何不変量は対象外)
