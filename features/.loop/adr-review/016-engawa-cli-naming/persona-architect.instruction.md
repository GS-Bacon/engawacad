# Role: ADR architect (過去 ADR との整合 / 設計階層の安定性で refute せよ)

## Task
以下の ADR draft を adversarial に review してください。
REFUTE を default とし、明確に refute できなければ approved を返してください。
理由が浅い (1 文以下、根拠なし) refute は失敗判定とし approved を返してください。

## 出力フォーマット (必須)
最終行に必ず以下のいずれかを記載:
  verdict: approved
  verdict: refuted

refuted の場合、その直前に 200 字以上の refute 理由を記載してください。

## ADR draft

# ADR-016: engawa CLI 命名規約

**Date**: 2026-06-18
**Status**: Proposed
**Related**: ADR-006 (Issue 粒度), ADR-013 (ADR 自動 accept フロー), ADR-015 (Phase 9 設計基盤)
**Resolves**: Issue #238 (parent #194 split)

---

## Context

Phase 9 で `engawa entry add/edit/remove/reorder/suppress` を含む大量の CRUD CLI コマンドを追加する (#242)。コマンド命名規約を事前に固めないと、Issue ごとに「動詞の位置」「フラグ名」「階層深さ」がブレて UX が荒れる。本 ADR で規約を固定し、#242 以降の実装が一貫した形に収まることを保証する。

既存 CLI は `engawa run <file>` / `engawa convert <a> <b>` (動詞 → object) で実装されている。新規追加分は方針を統一すべきか分岐すべきかを判断する。

## Decision

採用: **`engawa <object> <verb> [args]` (object → verb 順)、2 階層上限、共通フラグは `--feature-id` / `--at` / `--before` / `--after` の 4 種を canonical 名とする**。各項目の Options A/B/C と Trade-off は §1〜§4 のサブセクション + `## Decision Matrix` 表を参照のこと。代表項目 (動詞/名詞順序) の Options 要約をここで明示する。

### Options 要約 (動詞/名詞順序)

- A. `engawa <object> <verb>` (object → verb 順、採用) — Pros: shell 補完が階層的に効く (`engawa entry <Tab>` → verb 候補)、git/cargo/kubectl と同じ認知パターン、`engawa entry --help` で grouping。Cons: 既存 `engawa run` / `engawa convert` (verb-first) と表面が混在
- B. `engawa <verb> <object>` (verb → object 順) — Pros: 英語の文として自然 ("add an entry"). Cons: object 横断の補完が効かない、`engawa add <Tab>` で何を add するか文脈不明
- C. フラット (`engawa-entry-add`) — Pros: shell history 検索が直接ヒット。Cons: コマンド数が線形に増えてエコシステム (補完/man/help) が肥大化

**Trade-off**: Option A は既存 verb-first コマンドとの混在を許容する代わりに、補完 UX と認知負荷で Option B/C より優位 (A 採用)。

### 1. 動詞/名詞順序

`engawa entry add` / `engawa entry edit` / `engawa entry remove` / `engawa entry reorder` / `engawa entry suppress`

理由:
- object 名 (`entry`/`sketch`/`component`) で grouping すると `engawa entry --help` で「entry に対して何ができるか」が一覧できる
- shell の補完が階層的に効く (`engawa <Tab>` → object 候補、`engawa entry <Tab>` → verb 候補)
- `git`/`cargo`/`kubectl` 等の主要 CLI が object → verb を採用しているため認知負荷が低い

既存 `engawa run` / `engawa convert` は object なしの "top-level verb" として残し、本 ADR の例外とする (= 後方互換)。

### 2. サブコマンド階層深さ上限

上限: **2 階層 (`engawa <object> <verb>`)**

理由:
- 3 階層 (`engawa entry feature add`) は CLI として深すぎて記憶が困難
- object のグルーピングが不足する場合は object 名を細分化する (`entry-set` / `entry-list` などのフラットな構成)

### 3. 共通フラグ命名

canonical 名 (本 ADR で固定する):

| フラグ | 意味 | 例 |
|--------|------|-----|
| `--feature-id <id>` | 操作対象 feature の id | `engawa entry edit --feature-id box_1 --width 20` |
| `--at <index>` | リスト中の絶対位置 (0-based) | `engawa entry add --at 3` |
| `--before <id>` | 指定 feature の直前に挿入 | `engawa entry add --before extrude_1` |
| `--after <id>` | 指定 feature の直後に挿入 | `engawa entry add --after extrude_1` |
| `--dry-run` | 実行せず副作用予定だけ表示 | `engawa entry remove --feature-id x --dry-run` |

`--at` / `--before` / `--after` は **mutually exclusive** (同時指定はエラー)。

避ける名前: `--index` (`--at` と意味が同じ、片方に統一する)、`--id` (object 種が不明)、`--position` (動詞含意なし)

### 4. 出力フォーマット

- 標準: 人間向けの 1 行サマリ (色付き、`stdout`)
- `--json`: 機械可読 JSON (色なし、`stdout`)
- エラー: `stderr` に「`error: <message>`」形

## Decision Matrix

| 項目 | Option A | Option B | Option C | 採用 | Trade-off | 採用前提崩壊 trigger |
|------|----------|----------|----------|------|-----------|--------------------|
| 動詞/名詞順序 | `engawa <object> <verb>` (object→verb) | `engawa <verb> <object>` (verb→object) | フラット (`engawa-entry-add`) | **Option A** | A は補完が階層的、B は文として読みやすい、C は git-style preferences と乖離 | git-style (`git add`/`git commit` のような verb→object) が CAD 業界デフォルトと判明した場合 |
| 階層深さ上限 | 2 階層 (`engawa entry add`) | 3 階層 (`engawa entry feature add`) | 1 階層 (`engawa-entry-add`) | **Option A** | 2 階層が記憶限界。3 階層は kubectl 級 CLI でも稀 | object の種類が 20 種以上に増えて grouping が必要になった時 |
| 共通フラグ命名 | `--feature-id` (Object 種を明示) | `--id` (短いが曖昧) | `--target` (汎用) | **Option A** | A は冗長だがエラー時のメッセージで何の id か明確 | object 種が 1 種に固定された場合 (= `--id` で十分) |
| 位置指定 | `--at` / `--before` / `--after` の 3 種 | `--index` 単独 (negative index で末尾参照) | 順序 op 専用コマンド (`engawa entry move`) | **Option A** | A は意図が明示的、B は negative index が誤解されやすい、C は CRUD 統一感欠落 | Pythonic な negative index UX 要求が強かった場合 |
| 出力フォーマット | 1 行サマリ + `--json` フラグ | デフォルト JSON | YAML | **Option A** | A はインタラクティブ操作の認知負荷が低い、B は機械読み取り前提、C は YAML 構造化過剰 | スクリプト用途が主流になり JSON デフォルトの方が便利になった時 |

## Trade-off

- **object → verb 採用**: 既存 `engawa run` / `engawa convert` (verb-first) と不整合。後方互換のためレガシーは残すが、新規はすべて object-first 化する。混在は UX をわずかに損なうが、Phase 9 以降の新コマンドが圧倒的多数になるため許容。
- **2 階層上限**: 将来 grouping が必要になっても 3 階層に拡張せず、object 名を細分化する (`entry-set`/`entry-list`)。これは CLI 表面が広がる代償。
- **`--feature-id` 命名**: 冗長で打鍵が増えるが、エラーメッセージで「どの id か」を明示できる UX 利点が勝る。

## 採用前提崩壊 trigger

- **動詞順序**: ユーザーアンケート / GitHub issue で「git-style の方が直感的」が累積 3 件以上 → Option B 移行を検討
- **階層深さ**: object 種が 20 以上に膨らんで grouping が破綻 → 3 階層 (Option B) 許容を検討
- **`--feature-id` 命名**: CLI 利用ログで `--id` の誤入力 (補完失敗) が累積 → 後方互換 alias で `--id` も受ける
- **位置指定**: Pythonic negative index 需要が累積 → `--at -1` (末尾) を許容するエスケープハッチを追加
- **出力フォーマット**: スクリプト用途比率 > 70% → JSON デフォルトに転換

## 既存 ADR との関係

- **ADR-006 (Issue 粒度)**: 本 ADR は ADR-006 §1 で扱う Issue 粒度の規約ではなく、コマンド命名の規約。直交
- **ADR-013 (ADR 自動 accept フロー)**: 本 ADR は `gate:adr-review` 経由で auto-accept される対象
- **ADR-015 (Phase 9 設計基盤)**: ADR-015 §1 で Feature CRUD API 抽象を `FeatureOp` enum で決定したが、本 ADR は CLI 表面の規約を定める。enum variant 1 つ ↔ CLI verb 1 つ で対応

## Open Questions

- **Q1**: `engawa-cli` のサブコマンド clap 構造で object 階層を `Subcommand` enum でネストすべきか、`subcommand_required = true` で flat にすべきか → #242 実装で判断
- **Q2**: shell completion script (bash/zsh/fish) の自動生成は本 Phase で必須か、Phase 12 (Refactor Pass 1) 以降か → Phase 末で UX レビュー
- **Q3**: `--dry-run` 時の出力が「実行された場合の効果プレビュー」をどこまで詳細に出すか (どの feature が remove されるか、affected B-rep など) → UI/UX 仕様として別 Issue で
