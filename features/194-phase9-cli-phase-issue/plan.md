# Phase 9 起点 Issue — 分割計画書

> **自律判断ログ (B-3)**: 本 Issue は body に `loop-split-detector で分割される想定` と明記された Phase 起点メタ Issue。intent-check も `aligned: skip (split-detector parent)` で yes 等価。/3ai 単 Issue の正規フロー (実装→テスト→マージ) は起点 Issue にマッチしないため、#206 (Phase 8 起点 → #214-#218 へ分割) と同じパターンで **plan.md を分割計画書として書き、`split_proposal` 経由で 7 子 Issue を起票、親は `blocked-by-split` で待機** する運用を採用する。
>
> 採用根拠: ADR-006 §1 (1 Issue = GLM 1 サイクルで通せる粒度、ADR 決定と実装を混在させない)、ROADMAP Phase 9 完了条件 5 項目 + 前提 ADR 2 件 (ADR-014/015) を独立 Issue に分解する。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| Phase 9 完了条件を満たす 7 子 Issue の分割案を提示し、`loop-split-detector` で起票する | 子 Issue の実装そのもの (各子 Issue で別サイクル消化) |
| 親 #194 を `blocked-by-split` で待機させ、子全 close で自動 close される運用に乗せる | 既存 ADR の改訂、Phase 10 以降のスコープ |

## Non-Goals

- 親 Issue 内での実装コード変更 (`crates/**` への commit はゼロ)
- 子 Issue の plan.md / GLM 設計レビュー (各子 Issue 着手時に /3ai サイクルで実施)
- ADR-014 / ADR-015 の draft 本体 (それ自体を子 Issue として起票)

## 分割案: Phase 9 → 7 子 Issue

ROADMAP Phase 9 完了条件 5 項目 + 前提 ADR 2 件 (ADR-014: 履歴 CRUD 抽象 + Variable スコープ + schema_version + 品質基盤、ADR-015: CLI 命名規約) を以下に分解する。実装順は依存関係に従って ADR 系を先頭、CLI を最後尾に配置する。

### 子 1: ADR-014 draft (履歴 CRUD 抽象 + Variable スコープ + schema_version + 品質基盤の方針決定)

- type: `type: foundation` + `batch:skill`
- 内容: Phase 9 全体の設計方針を ADR-014 にまとめる。Feature CRUD API 抽象、Variable の 2 段スコープ (Document / Sketch) 定義、`schema_version` 命名規則と migration hook 形、品質基盤 (proptest / criterion / cargo-fuzz / cargo-llvm-cov / Playwright) の選定理由と最小 setup 範囲を Decision Matrix で記録。
- 子 2-6 着手前のブロッカー。

### 子 2: ADR-015 draft (engawa CLI 命名規約)

- type: `type: foundation` + `batch:skill`
- 内容: `engawa entry add/edit/remove/reorder/suppress` を含む CLI 全般の命名規約 (動詞/名詞順序、サブコマンド階層、`--feature-id` などの共通フラグ命名) を ADR-015 にまとめる。子 5 着手前のブロッカー。

### 子 3: Feature CRUD (Edit / Roll back / Suppress / Reorder / Delete / Insert) の engawa-build 実装

- type: `type: feature` + `batch:kernel`
- 内容: `engawa-build` の Feature ディスパッチャに CRUD 操作を追加。`.engawa` の Feature 列に対する Edit / Roll back / Suppress / Reorder / Delete / Insert を ADR-014 で確定した API に従って実装。決定性テスト + golden YAML + 退化ケース。

### 子 4: Document 全域 + Sketch 内 2 段スコープ Variable / Equation

- type: `type: feature` + `batch:kernel`
- 内容: `engawa-format` に Variable 型 + Equation 評価エンジン (依存解決 + 循環検出) を追加。Document スコープと Sketch スコープの 2 段名前空間 (ADR-014 で確定) を実装し、Sketch 内 Variable が Document Variable を上書き可能。決定性テスト + Equation 評価エラー系。

### 子 5: `engawa entry add/edit/remove/reorder/suppress` 統一 CLI

- type: `type: feature` + `batch:kernel`
- 内容: `engawa-cli` に ADR-015 命名規約に基づいた entry 系サブコマンドを追加。子 3 の Feature CRUD API を呼び出して `.engawa` を更新する。golden file テスト + 非正常系 (存在しない entry ID 等)。

### 子 6: `schema_version` フィールド + migration hook 入口を engawa-format に追加

- type: `type: feature` + `batch:kernel`
- 内容: `engawa-format` の Document トップレベルに `schema_version: u32` を追加 + migration hook trait (将来 v2 へ移行する空入口) を定義。ADR-014 の schema_version 規約に従う。読込時に未指定なら v1 fallback、未知 version は明示エラー。

### 子 7: 品質基盤 (proptest + criterion + cargo-fuzz + cargo-llvm-cov + Playwright) 最小 setup + main bench baseline

- type: `type: foundation` + `batch:kernel`
- 内容: 5 ツールを workspace に最小 setup (各 1 件動くサンプルテスト)。`cargo xtask ci` には組み込まず手動実行できる状態にする。criterion で main の bench baseline を 1 回取得し `.benchmarks/baseline-phase9.json` 等に保存。Playwright は将来の Viewer 用なので最小 hello-world のみ。

## 親 Issue の閉鎖条件

子 1-7 すべてが close されたら親 #194 を自動 close (ROADMAP Phase 9 完了条件と一致)。`blocked-by-split` ラベルが残っている間は `loop-actionable` から除外され、batch-select で skip される。

## テスト計画 (本 Issue では実装なしのため N/A)

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T_DEG_split | 起票 | `split_proposal` 7 子 → loop-split-detector で 7 件起票 + 親に blocked-by-split | gh issue list で 7 件 OPEN + 親に label |

## 幾何的不変条件チェックリスト

- N/A (本 Issue では幾何コード変更なし、子 3/4/6 で個別に検証)

## 設計方針

- 本 Issue は **メタ Issue として分割計画のみ実施**。コード変更ゼロ、ADR commit ゼロ。
- 子 Issue 起票時のラベル組み合わせは ADR-002 / ADR-006 §1 に従い `lint-issue-labels.ts` で検証する。
- 子 1 (ADR-014) は他子のブロッカーで、`gate:adr-review` を経て ADR-013 auto-accept フローに乗る。
