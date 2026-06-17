## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| `batch-select.ts` の foundation-batch tier フィルタに milestone gating を追加 | 他の tier (bug-batch / enh-batch / phase-feature) の挙動変更 |
| `--batch foundation` 経路の同等修正 | Phase 起点 Issue 自体の loop-split-detector 挙動変更 |
| `isFutureMilestoneTitle` 関数の export + unit test | tier 優先順位ラダーそのものの再設計 |

## Non-Goals
- ROADMAP.md の Phase 完了マーカー検出ロジック (`detectCurrentPhase`) の改変
- `loop-split-detector.ts` 側のロジック変更 (これは別 Issue で扱う)
- Phase 起点 Issue (#194-#206 の「Phase N 起点 Issue」) の自動分割パイプライン整備 (本 Issue は selector の混入排除のみ)
- foundation 以外の tier への gating 拡張

## 実装対象
- Issue: #210
- 影響ファイル:
  - `.claude/skills/3ai/scripts/batch-select.ts` (修正)
  - `.claude/skills/3ai/scripts/__tests__/batch-select.test.ts` (新規)

### 既存関数の修正

**1) `batch-select.ts` 自律モード `foundation-batch` tier フィルタ (L297-299)**

before:
```ts
const foundationBatch = tryTier(
  allIssues.filter(i => isFoundationTier(labelNames(i)) && hasBatchLabel(i)),
);
```

after:
```ts
const foundationBatch = tryTier(
  // #210: 将来 Phase milestone (Phase N で N > currentPhase) は除外
  allIssues.filter(
    i => isFoundationTier(labelNames(i)) && hasBatchLabel(i) && !isFutureMilestone(i),
  ),
);
```

**2) `batch-select.ts` `--batch foundation` 経路 (L250-253)**

before:
```ts
} else if (batchArg === "foundation") {
  selected = allIssues.filter(i => isFoundationTier(labelNames(i)) && hasBatchLabel(i));
  tier = "foundation-batch";
```

after:
```ts
} else if (batchArg === "foundation") {
  selected = allIssues.filter(
    i => isFoundationTier(labelNames(i)) && hasBatchLabel(i) && !isFutureMilestone(i),
  );
  tier = "foundation-batch";
```

**3) `isFutureMilestone` / `isFutureMilestoneTitle` の新設**

`main()` 内の closure `isFutureMilestone(issue)` は title 抽出を上位関数 `isFutureMilestoneTitle(title, currentPhase)` に委譲。後者は top-level に export してテスト可能にする。

**4) `main()` の import guard 追加**

before: `main().catch(...)` を無条件で実行
after: `if (import.meta.main) { main().catch(...) }`

これによりテストファイルから batch-select.ts を import しても `gh issue list` が起動しない。

## 設計方針
- **決定性**: gating ロジックは pure function (title + currentPhase) のみで判定。副作用なし、同一入力で同一出力。
- **後方互換**:
  - `--batch foundation` 明示指定経路でも将来 Phase は除外する (今回 fix の対象)。
  - milestone なしの foundation Issue (#167-169 のような基盤改修) は今まで通り選ばれる。
- **fall-through 動作**: foundation-batch が空なら自動的に `phase-feature` tier に fall-through する (既存挙動)。これにより #197/#201/#205 除外後、Phase 8 milestone の `type: feature` Issue (#206) が拾われる。
- **誤検出ガード**: `Phase N` 以外の milestone title (例: "Backlog", "Release 1.0") は除外対象外 (=選ばれ続ける) として扱う。これは過剰除外を避けるため。

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 (pure function) | `isFutureMilestoneTitle(null, 8)` を 2 回 → 両方 false | 同一結果 |
| T02 | 正常系 | `("Phase 12: Quality Pass", 8)` → true | true |
| T03 | 正常系 | `("Phase 8: モデル面上のスケッチ", 8)` → false | false (現 Phase) |
| T04 | 正常系 | `("Phase 4: Boolean", 8)` → false | false (過去 Phase 振り返り) |
| T05_boundary_next_phase | 境界 | `("Phase 9: ...", 8)` → true | true (境界の次) |
| T06_boundary_current_phase | 境界 | `("Phase 8: ...", 8)` → false | false |
| T07_degen_null_milestone | 退化 | milestoneTitle = null → false | false |
| T08_degen_null_phase | 退化 | currentPhase = null → false | false (判定不能) |
| T09_non_phase_title | 退化 | "Backlog" や "Release 1.0" → false | false (過剰除外しない) |

## 幾何的不変条件チェックリスト
N/A (Boolean/Partition/Assemble 系ではなく skill スクリプトの修正のため)

## 自律判断ログ
- **修正方針の選択**: batch-select.ts の tier 優先順位ラダーは「bug → enh → foundation → phase-feature」で確立済み。tier 順序自体を変えるのではなく、foundation-batch tier の **フィルタに milestone gating を追加**する最小修正を選択した。tier 順を逆転すると bug-batch (緊急修正) より phase-feature が優先されてしまい、別の問題を引き起こす。
- **Phase 起点 Issue の扱い**: #206 (Phase 8 起点) も「Phase 起点 Issue」だが、これは現 Phase milestone なので phase-feature tier に該当する。本 Issue ではこの分類のまま受け入れ、loop-split-detector による分割は別経路で動作する想定。
- **light フローで Claude 直接実装**: batch:skill ラベルかつ TS スクリプトのみの変更のため、crates/** ノータッチ。GLM dispatch 不要。memory `project_3ailoop_implementation_style` に従い Claude 直接 + 後段で Codex 独立レビューする。
