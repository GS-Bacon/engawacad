# Phase 10 起点 Issue — 分割計画書

> **自律判断ログ (B-3)**: 本 Issue は body に `loop-split-detector で粒度に合うように分割される想定` と明記された Phase 起点メタ Issue。intent-check も `aligned: skip (split-detector parent)` で yes 等価。/3ai 単 Issue の正規フロー (実装→テスト→マージ) は起点 Issue にマッチしないため、#194 (Phase 9 起点 → 7 子へ分割) / #206 (Phase 8 起点 → 5 子へ分割) と同じパターンで **plan.md を分割計画書として書き、`split_proposal` 経由で 7 子 Issue を起票、親は `blocked-by-split` で待機** する運用を採用する。
>
> 採用根拠: ADR-006 §1 (1 Issue = GLM 1 サイクルで通せる粒度、ADR 決定と実装を混在させない)、ROADMAP Phase 10 完了条件 2 項目 (スケッチ基本曲線 7 種 + スケッチ編集 7 種、合計 14 機能) を独立 Issue に分解する。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| Phase 10 完了条件を満たす 7 子 Issue の分割案を提示し、`loop-split-detector` で起票する | 子 Issue の実装そのもの (各子 Issue で別サイクル消化) |
| 親 #195 を `blocked-by-split` で待機させ、子全 close で自動 close される運用に乗せる | 既存 ADR の改訂、Phase 11 以降のスコープ (拘束ソルバ等) |

## Non-Goals

- 親 Issue 内での実装コード変更 (`crates/**` への commit はゼロ)
- 子 Issue の plan.md / GLM 設計レビュー (各子 Issue 着手時に /3ai サイクルで実施)
- ADR-016 (本 Phase の方針 ADR) draft 本体 (それ自体を子 Issue として起票)
- スケッチ拘束ソルバ (Phase 11)、3D 曲線/曲面拡張 (Phase 12 以降)

## 分割案: Phase 10 → 7 子 Issue

ROADMAP Phase 10 完了条件 2 項目 (基本曲線 7 種 + スケッチ編集 7 種) を以下に分解する。実装順は依存関係に従って ADR 系を先頭、curve 系を中段 (2→3→4)、editing 系を後段 (5→6→7) に配置する。Editing 系は最低 1 curve 子 (子 2 = Circle/Arc) が closed であれば着手可能。

### 子 1: ADR-016 draft (スケッチ基本曲線拡張 + スケッチ編集の方針)

- type: `type: foundation` + `batch:skill`
- 内容: Phase 10 全体の設計方針を ADR-016 にまとめる。
  - 基本曲線 7 種 (Circle / Arc / Ellipse / Conic / Rectangle / Polygon / Slot) のパラメータ表現 (中心+半径 / 焦点+離心率 / 中心+a/b 軸 / 一般二次形式 5 係数 / 2 corner / 中心+頂点数+外接円半径 / 2 中心点+半径)
  - 退化判定基準 (ε_radius, ε_angle, ε_axis_ratio, 多角形最小辺長)
  - スケッチ編集 7 種 (Trim / Extend / Offset / Sketch Fillet / Sketch Chamfer / Mirror / Pattern) の API 抽象 (in-place vs 純関数、ID-stable 性、参照解決失敗時のエラー伝播)
  - 既存 Line ベース sketch との互換性 (`SketchElement` enum 拡張 vs 別 type)
  - Options A/B/C + Trade-off + 採用前提崩壊 trigger + 既存 ADR (-004/-005) との関係
- 子 5/6/7 (editing 系) のブロッカー。子 2/3/4 (curve 系) は ADR-016 approved を前提に着手。

### 子 2: 円 + 弧 (Circle / Arc) を engawa-format / engawa-build に実装

- type: `type: feature` + `batch:kernel`
- 内容: 最も基本的な 2 曲線。中心 + 半径 (+ 弧の場合は start/end angle) パラメータ。tessellation で polyline 化、退化 (半径 0 / 角度差 0) はエラー。
- 前提: ADR-016 approved
- テスト: 決定性 / 半径ゼロ退化 / 弧の 0°/360° 境界 / golden YAML

### 子 3: 楕円 + Conic (Ellipse / Conic) を engawa-format / engawa-build に実装

- type: `type: feature` + `batch:kernel`
- 内容: 楕円は中心 + 長軸半径 a + 短軸半径 b + 回転角。Conic は一般二次形式 Ax² + Bxy + Cy² + Dx + Ey + F = 0 の 5 自由度。退化判定 (a=b → 円扱い、b=0 → 線分扱い、conic discriminant が双曲線/放物線)。
- 前提: ADR-016 approved
- テスト: 決定性 / 楕円→円縮退 / conic 判別式境界 / golden YAML

### 子 4: 矩形 + 多角形 + Slot (Rectangle / Polygon / Slot) を engawa-format / engawa-build に実装

- type: `type: feature` + `batch:kernel`
- 内容: 3 つの複合形状。Rectangle は 2 corner、Polygon は中心+頂点数+外接円半径 (regular polygon)、Slot は 2 中心点+半径 (semicircle + line + semicircle + line の合成)。それぞれを構成 line/arc として展開しつつ「論理的単一要素」として ID 管理。
- 前提: ADR-016 approved
- テスト: 決定性 / 多角形 n=2 退化 / Slot の 2 中心点一致 / golden YAML

### 子 5: Trim + Extend を engawa-format / engawa-build に実装

- type: `type: feature` + `batch:kernel`
- 内容: 既存 sketch element の切断/延長。Trim は与えた intersection 点で element を 2 つに分割し片方削除。Extend は与えた boundary 要素まで element を伸長。intersection 計算は基本曲線 (line/circle/arc) 同士のみ対応 (楕円/conic は ADR-016 で defer 判断)。
- 前提: ADR-016 approved + 子 2 (Circle/Arc) closed
- テスト: 決定性 / 交点なし退化 / 接線交差 / golden YAML

### 子 6: Offset + Sketch Fillet + Sketch Chamfer を engawa-format / engawa-build に実装

- type: `type: feature` + `batch:kernel`
- 内容: 3 つの形状改変操作。Offset は element を法線方向に距離 d だけ平行移動 (closed curve は内/外 offset)。Sketch Fillet は 2 element 交点に半径 r の円弧を挿入。Sketch Chamfer は同様に長さ c の line を挿入。
- 前提: ADR-016 approved + 子 2 (Circle/Arc) closed
- テスト: 決定性 / Offset 距離ゼロ退化 / Fillet 半径過大 (収まらない) / golden YAML

### 子 7: Mirror + Pattern を engawa-format / engawa-build に実装

- type: `type: feature` + `batch:kernel`
- 内容: 2 つの複製操作。Mirror は与えた線対称軸で element 群を鏡像複製。Pattern は線形 (n 個 × 距離 d 方向 v) または円形 (n 個 × 中心 c × 全角 θ) の配列複製。複製された element 群は新しい ID で生成。
- 前提: ADR-016 approved + 子 2 (Circle/Arc) closed
- テスト: 決定性 / 軸上要素の Mirror (自己重複) 退化 / Pattern n=1 境界 / golden YAML

## 親 Issue の閉鎖条件

- 上記 7 子 Issue が**全て closed** になった時点で、loop-phase-close-check の split-parent auto-close (#271) が親 #195 を自動 close する
- 子のいずれかが `needs-human` 退避された場合、親は `blocked-by-split` のまま待機

## 実装対象

本 Issue 自体に実装はない (起点 Issue)。各子 Issue で別サイクル消化。

## 設計方針

本 Issue 自体に設計はない (起点 Issue)。各子の plan.md / 各 ADR で記述。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01_split_processed | normal | `loop-split-detector` が `split-proposal.yaml` を読み 7 子 Issue を起票、親 #195 に `blocked-by-split` を付与 | 7 子 open + 親 blocked-by-split |
| T_DEG_proposal_empty | degenerate | `split_proposal: []` (空) で起点 Issue を作るとエラー (本 Issue では発生しない、参照のみ) | エラー検出 |

> ※ 起点 Issue のため通常の「実装/テスト」ループは走らない。state.json の全 STEP を `skipped_split_parent` でマークし、`split_processed` と `artifacts_committed` のみ `passed` に倒す。

## 幾何的不変条件チェックリスト

- N/A (起点 Issue、幾何処理なし)
