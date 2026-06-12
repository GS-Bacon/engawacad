# ADR-006: Issue 分解規約と Phase 移行時の設計ガイドライン

**Date**: 2026-06-03  
**Status**: Accepted  
**Supersedes**: (なし)  
**Related**: ADR-002 (ロードマップ管理), ADR-004 (数値モデル), ADR-005 (トポロジカル命名)

---

## 背景

Phase 4 (Boolean) の作業データを分析した結果、以下の法則が観測された:

| Issue サイズ | 代表例 | 所要時間 | design_loops |
|---|---|---|---|
| 大粒度 (ADR+実装混在) | #31 (pcurve+ADR-004) | 55h | 4 |
| 大粒度 (複数形状×複数op) | #34 (曲面 Boolean 全部) | — | 5 |
| 小粒度 (1 軸 × 1 op) | #41 (Plane×Sphere Cut) | 5h | 1 |
| 小粒度 (1 軸 × 2 op) | #42 (Plane×Cyl Fuse+Isect) | 11h | 3 |

遅延の主因は GLM の能力でも課題の難しさでもなく、**Issue 粒度 × 設計/実装の混在 × ガードレール未整備**だった。

---

## 規約

### 1. Issue 粒度ガード

> **1 Issue = GLM が 1 サイクル (core_impl ≤ 3 runs) で通せるサイズ**

判定チェックリスト (Phase 着手時、Issue 起票前に確認):

- [ ] この Issue は「1 軸 × 1〜2 op」または「単一コンポーネントの改善」に収まるか?
- [ ] ADR の決定と実装が混在していないか? (ADR 決定は別 Issue)
- [ ] 完了条件が「テストが通る / export で確認できる」で計測可能か?
- [ ] 「前提として必要な別 Issue」はすでに closed か?
- [ ] `type:feature` 以外 (`bug` / `enhancement` 等) なら `batch:{kernel|data|viewer|skill}` のいずれかを付けたか? (`batch:*` 無しの Light tier Issue は `/3ai` auto 選定の bug-batch / enh-batch ラダー (`.claude/skills/3ai/scripts/batch-select.ts`) に乗らず取り残される)

**NG パターン:**
- "曲面 Boolean 全部" → (Plane×Cyl A1) + (Plane×Sphere A2) + (Cyl×Sphere A3) に分割
- "ADR-004 + pcurve 実装" → ADR 改訂と実装を別 Issue に

### 2. 設計/実装の分離

- **ADR-only Issue**: 意思決定のみ、コード変更なし。完了条件は ADR ファイルの commit。
- **impl Issue**: ADR は参照するが変更しない。ADR が closed かつ方針確定を前提に着手。
- 例外: 実装中に ADR の小修正が必要になった場合は、Issue 本文に「実装中追記」セクションを追加 (→ §5 参照)。

### 3. 分解パターン: 軸 × op マトリクス

曲面 Boolean のような多次元問題は表でマトリクス化し、セルごとに Issue を切る:

```
          | Cut | Fuse | Intersect
-----------+-----+------+----------
Plane×Cyl  | #39 | #42  |   #42
Plane×Sph  | #41 | (A2.1)|  (A2.1)
Cyl×Sph    | (A3)|      |
```

**作り方**: Phase ADR に上記のような表を書き、未着手セルを Issue として起票する。

### 4. Phase 着手チェックリスト

新 Phase を開始する際に実施:

1. Phase のロードマップと完了条件を読む (`ROADMAP.md`)
2. 当 Phase の ADR が存在するか確認 (なければ最初に ADR-only Issue を起票)
3. Issue 分解マトリクスを草案 (Claude が作成、Codex intent-check を通す)
4. 各 Issue を起票: §1 の粒度チェックリストを全項目 ✓ にしてから `gh issue create`
5. Milestone に紐付ける
6. Codex intent-check: `aligned: yes` → 起票確定。`aligned: no` → Claude がユーザーと再設計

Codex intent-check の実施:
```bash
bun .claude/skills/3ai/scripts/dispatch-codex-intent.ts \
  --issue <N> \
  --issue-draft <issue-draft.md> \
  --result <issue-N-intent.yaml>
```

### 5. Issue 更新プロトコル

実装中に Issue 本文を変更する必要が生じた場合:

| 変更の重さ | 内容例 | 対応 |
|---|---|---|
| 軽微 (実装詳細の決定) | ε 値の選定、関数名の確定 | Issue 本文末尾「実装中追記」セクションに追記。Claude 判断で追加可。Codex 再投入不要 |
| 中 (scope 内の追加判断) | 想定外のエッジケース処理方針 | Issue 本文を編集して scope 内であることを明記。Codex intent-check を再実施 |
| 重大 (scope 境界の変更) | 当初 Out-of-Scope だったケースを追加 | Issue を close → sub-Issue を再起票 (§4 Phase 着手と同じ手順) |

---

## plan.md 必須セクション

`features/$N-$SLUG/plan.md` に以下のセクションが揃っていること (GLM SCOPE ペルソナが検証):

```markdown
## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| (本 Issue で実装するもの) | (明示的に除外するもの) |

## Non-Goals
- (Out-of-Scope と同内容でも重複 OK、dispatch-codex-auto.ts の guard が参照)

## 設計方針
...

### 数値モデル (Phase 4/6+ 必須)
- tolerance: ε = (値)
- 退化判定基準: (値と条件)
- ADR-004 準拠方針: (tolerant vs exact のどちらを採用するか)
```

`## In-Scope / Out-of-Scope` がない場合、GLM SCOPE ペルソナが critical 指摘を出す。
`### 数値モデル` がない場合、GLM NUMERIC ペルソナが high 指摘を出す (Phase 4/6+)。
テスト計画 ID 表に退化/境界ケース専用の ID（`_degen_` / `_boundary_` / `_degenerate_` を含む ID、または `T_DEG` 系）が最低 1 件ない場合、STEP 5.5 で警告が出る（Phase 4 で退化バグ #46/#50–#55 が acceptance 初回をすり抜けた教訓）。

---

## GLM 設計レビューペルソナ

### 構成

| ペルソナ | 対象 Phase | 観点 |
|---|---|---|
| SCOPE | 全 Phase | In-Scope 表 / Issue 整合 / 粒度 |
| INVARIANT | 全 Phase | 決定性 / トポロジー整合 / 既存 Feature 副作用 |
| AMBIG | 全 Phase | 曖昧表現 / 数値判断の実装者丸投げ |
| NUMERIC | Phase 4, Phase 6+ | tolerance/ε / 退化ケース / ADR-004 準拠 |
| ASSEMBLY | Phase 5 予定 | transform 整合 / 参照解決 (Phase 5 ADR で追加定義) |

### 深さ設定

| Issue サイズ | ペルソナ | Max iterations |
|---|---|---|
| Heavy (新規面型・ADR 改訂含む) | Common 3 + Phase オプション | 10 |
| Standard (sub-Issue 1 軸 1-2 op) | Common 3 + Phase オプション | 5 |
| Light (微修正・bug fix) | SCOPE + AMBIG のみ | 3 |

サイズ判定: §1 のチェックリストで "1 軸 × 1-2 op" に収まるなら Standard 以下。

### 収束条件

- 2 連続 round で C/H 件数がゼロ → DONE
- 2 連続 round で全指摘を棄却 (adopted = 0) → early-stop: エスカレーション
- Max iterations 到達 → ユーザーに確認

---

## Codex の役割

Codex は以下の **2 つの独立ゲート** を担当する:

### ① Issue 起票時の intent-check（従来通り）

1. Claude が Issue 案を作成 (§1 粒度チェックを確認済み)
2. Codex に「Issue 本文の意図・スコープが明確か」だけを判定させる
3. `aligned: yes` → 自動起票
4. `aligned: no` → Claude がユーザーに相談 → Issue 修正 → Codex 再投入

### ② STEP 7.5 マージ前の独立技術最終レビュー（Phase 4 教訓から追加）

GLM final レビュー (STEP 7) 通過後・squash マージ (STEP 8) 前に、`git diff <base>...HEAD` 全体を Codex が技術的観点でレビューする。

**背景**: 実装者 GLM とレビュアー GLM が同系であることによる相関盲点を、別モデル系 (Codex/gpt-5.4) の独立視点で破る設計。Phase 4 (Boolean) では曲面 Boolean の退化バグ #46/#50–#55 が GLM レビューを通過してしまった教訓による。

**設計段階**の技術検証は GLM 多ペルソナ (SCOPE/INVARIANT/AMBIG/NUMERIC) が担当し、**マージ前の最終独立検証**は別モデル系の Codex が担当する（分担の明確化）。

**ポリシー**:
- severity **critical/high** → merge ブロック・GLM 実装へ差し戻し（ループ上限 `codex_loops` = 2）
- severity **medium/low** → `codex-findings.md` に記録のみ（非 block）
- GLM 指摘と Codex 指摘が衝突した場合は Claude が既存の judgment-summary 方式で裁定

dispatch: `dispatch-codex.ts --mode review --instruction agents/codex-final-reviewer.md`

---

## リスクと緩和策

| Risk | 緩和策 |
|---|---|
| GLM SCOPE が Issue ↔ plan drift を見逃す | SCOPE テンプレに「Issue 本文の核と In-Scope 表が一致しているか」を明示項目化 |
| NUMERIC ペルソナが plan テンプレ不備で空振り | plan.md の `### 数値モデル` セクション必須化 |
| Phase オプションペルソナの追加忘れ | Codex intent-check に「数値判断を含むか」チェックを追加。yes なら NUMERIC 強制 |
| Issue 更新の軽微/中の判定が曖昧 | 本 ADR の §5 表を参照。迷ったら中 (Codex 再投入) を選ぶ |
| Codex 最終ゲートで throughput 低下 | severity 閾値 block (critical/high のみ) と `codex_loops` 上限 2 で抑制 |
| バッチモードで粒度ガードが崩れる | バッチモードは機械的 Issue に light フロー、`type:feature` に full フローを適用し、**§1 粒度ガード (1 Issue = GLM 1 サイクル) を Issue 単位で維持する**。batch:kernel の light Issue は STEP 7.5 (Codex 個別ゲート) を保持し幾何不変量を守る |
