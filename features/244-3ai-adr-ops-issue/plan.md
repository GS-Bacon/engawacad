## 自律判断ログ (#244 メタ Issue)

本 Issue は **コード変更を伴わない GitHub Issue 操作のみのメタ Issue**。実装すべき Rust コードが
ないため /3ai 標準フロー (STEP 5/5.5/6/6.5/6.6/7/7.5) を skip する。CLAUDE.md ルール
「branch・PR 不要 / commit に Closes #N を含めてメイン直 push」に従い main 上で完結。

memory `project_3ailoop_implementation_style` の「Skill メタ操作は Claude 直接実装」方針と整合。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| #240 と #242 で重複している 6 ops (Insert / Edit / Rollback / Suppress / Reorder / Delete) を **6 子 Issue** に再分割 | 子 Issue 各々の実装 (それは子 Issue が消化される時の話) |
| 各子で build (engawa-build の `FeatureCrud`) + cli (`engawa entry`) を同梱し ADR-006 §1「1 軸 × 2 op (build + cli)」に収める | 6 ops 以外の Phase 9 残タスク (履歴 CRUD 抽象設計 ADR-014, CLI 命名 ADR-015) — 別 Issue で管理 |
| 親 #240 / #242 は `blocked-by-split` 維持。両親に「6 子に再分割した」コメント追加 | 親 #240 / #242 の close (全 6 子 close 後に手動 close する想定) |
| `features/244-3ai-adr-ops-issue/{plan.md,state.json}` の commit | `loop-split-detector.ts` 経由の自動起票 (本 Issue は detector を介さず Claude が直接 `gh issue create`) |

## Non-Goals

- 親 #240, #242 自体を本 Issue で close すること: 6 子全 close まで `blocked-by-split` を維持
- 親 #194 (Phase 9 起点) の状態変更: 子の super-parent 階層は維持 (#194 → {#240,#242} → 6 子)
- 各子 Issue 内での実装着手: 起票するのみ。実装は loop の次サイクル以降で /3ai が消化
- `loop-split-detector.ts` の動作変更: 本 Issue は detector パスを使わない例外運用 (split_proposal yaml 手書きも省略)
- Phase 9 残 ADR (#246 ADR-015 / #245 ADR-016) の再 review: 別経路 (gate:adr-review L-1.5 rescan / needs-human) で処理済み

## 実装対象

<!-- Issue: #244 -->
<!-- コード変更なし。GitHub Issue 操作のみ。 -->

操作対象:

1. **6 子 Issue 起票** (`gh issue create`):
   - 各子 title: `feat(phase9): Feature CRUD <Op> (build + cli)`
   - 各子 body:
     - 親階層: 親 #194 (Phase 9 起点) / super-parent #240 (build), #242 (cli) (両方 `blocked-by-split`)
     - スコープ: build (engawa-build `FeatureCrud::<op>`) + cli (engawa-cli `engawa entry <op>`) を 1 Issue にまとめる
     - 前提 ADR: ADR-014 (Feature CRUD API 抽象) / ADR-015 (CLI 命名規約)
     - テスト計画 ID 表 (T01 決定性 + T_DEG 退化 + golden YAML)
   - labels: `type: feature,batch:kernel` (build + cli は Rust なので batch:kernel)
   - 起票後すぐ `lint-issue-labels.ts --issue <N>` で検証
2. **親 #240 / #242 に注記コメント** (`gh issue comment`):
   - 「#244 で 6 子に再分割した。子 Issue 番号一覧: #A〜#F。本 Issue は子全 close まで blocked-by-split で待機」
3. **`features/244-3ai-adr-ops-issue/{plan.md,state.json}` を main に直 commit + push**:
   - commit message: `refactor(3ai): #240/#242 を 6 子 Issue に再分割 (ADR-006 §1 違反修正) (Closes #244)`
   - `finalize-feature.ts --issue 244 --slug 3ai-adr-ops-issue` で state.json を merge=passed に進めてから commit

## 設計方針

- **branch 不要 / PR 不要**: CLAUDE.md ルール準拠。main 直 commit。
- **6 子 = 6 ops × (build + cli 同梱)**: #244 body の提案を踏襲。1 子で build + cli を扱うことで ADR-006 §1「1 軸 × 1〜2 op」範囲 (build + cli の 2 op) に収める。
- **依存順序の宣言** (Insert → Edit → Rollback → Suppress → Reorder → Delete):
  - Insert を先頭にする理由: Edit/Rollback/Suppress/Reorder/Delete は既存 entry を前提とするため、Insert で entry を増やせる状態が論理的に最初。
  - 各子 body に「依存: <前 op の子 Issue 番号>」を記述 (loop の batch-select が deps 順序で pick できるように)。
- **lint-issue-labels.ts 検証**: CLAUDE.md「起票直後に必ず exit 0 を確認」を全 6 子で実施。
- **エラーハンドリング**: `gh` コマンドが失敗したら `raise-issue-on-failure.ts` で起票して停止 (本 Issue 進行 = halt)。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 起票確認 | `gh issue list --search '#244 子'` で 6 件返る | 6 件 (Insert/Edit/Rollback/Suppress/Reorder/Delete) |
| T02 | label 検証 | 6 件全てで `lint-issue-labels.ts --issue <N>` exit 0 | exit 0 × 6 |
| T03 | 親 blocked 維持 | `gh issue view 240,242 --json labels` で `blocked-by-split` が残る | 両方残存 |
| T04 | コメント追加 | 親 #240 / #242 に注記コメント 1 件ずつ追加されている | comments_count +1 each |
| T_DEG_lint_fail | 退化 (起票時のラベル抜け検知) | label 軸抜けは起票直後の lint で検出 | lint exit 1 → 手動修正 |

## 幾何的不変条件チェックリスト

N/A — 本 Issue はコード変更なしのメタ Issue。Boolean/Partition/Assemble 系の幾何は触らない。
