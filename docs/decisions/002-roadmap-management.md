# ADR-002: ロードマップ・タスク管理方式の選定

## Status

Accepted

## Context

Phase 0 完了後、今後の開発を継続するためにロードマップとタスク管理の仕組みを整備する必要が生じた。主な候補:

1. **in-repo Markdown のみ** (`ROADMAP.md`, `TODO.md` 等)
2. **GitHub Issues + Milestones**
3. **GitHub Projects (カンバンボード)**
4. **Linear / Notion 等の外部ツール**

## Decision

**二層構成を採用する**:

- `ROADMAP.md` (in-repo): フェーズ定義と長期ビジョン (静的・安定)
- GitHub Issues + Milestones: 個別タスク管理 (動的・PR と連結)
- `docs/decisions/` (ADR): 技術選定の理由記録 (既存運用を継続)

## Rationale

### なぜ二層か

- `ROADMAP.md` は AI (Claude Code) が毎セッション即読できる静的文書。フェーズの「なぜこの順か」という文脈を git 履歴とともに保存できる
- GitHub Issues は PR と `Closes #N` で自動連結でき、`gh` CLI 経由で AI が open issue を参照して自律的に着手できる
- GitHub Projects はソロ開発には過剰。カンバン UI の維持コストが便益を上回る
- 外部ツール (Linear 等) はリポジトリとの連携コストが高く、公開 OSS に不向き

### なぜ solo なのに Issues か

Claude Code が「1セッション = 1 Issue = 1 PR」のループで動くことを想定している。Issues がないと次に着手するものの決定に毎セッション議論コストがかかる。

### Phase 順序の理由

| 順序 | Phase | 理由 |
|---|---|---|
| Phase 1 | STL export | Feature → geometry パイプラインが初めて繋がる。CLI ツールとして動く最小構成 |
| Phase 2 | Viewer (Web ベース) | box 1個でも viewer ができれば、以降の primitive・Boolean を追加するたびに視覚確認できる。Web ベースビューアを採用。詳細は [ADR-003](003-viewer-and-app-architecture.md) |
| Phase 3 | Primitives | Viewer があるため実装 → 即視覚確認のループが回る |
| Phase 4 | Boolean | B-rep 交差演算は難度が高いため primitives の後 |
| Phase 5 | Assembly | 依存: Boolean が安定していること、部品ライブラリが存在すること |
| Phase 6 | STEP | 別途記述 |

### Phase 1 で STL を選んだ理由 (STEP でなく)

STEP (ISO 10303-21) は CAD 業界標準で、B-rep を直接表現できるため MyCad との親和性は高い。しかし:

- 仕様が巨大 (AP203, AP214 等の Application Protocol が多数存在)
- Rust の STEP パーサー/ライターは成熟したものが限られる (2025 時点)
- フルパイプライン (Feature → Solid → TriangleMesh → 出力) を最短で通すことを優先

STL は三角メッシュの単純な ASCII/バイナリ形式で、`tessellate_solid` の出力をそのまま書き出せる。パイプライン確立後、Phase 6 で STEP に拡張する。

### ラベル運用: 2 軸ラベル制

**crate 軸** (どのコードに触るか): `kernel` / `format` / `cli` / `viewer` / `docs`

**type 軸** (作業種別): `type: feature` / `type: refactor` / `type: foundation`

- `type: feature` — ユーザから見た機能追加。**Phase 完了判定の対象**。
- `type: refactor` — 内部リファクタ。完了判定に含めない。
- `type: foundation` — 設計・横断的な土台作業。完了判定に含めない。
- `bug` / `docs` ラベルは既存のものを流用。

solo 開発でも type 軸を設けた理由: 保守・設計・差し込み作業が機能 Milestone に混入すると Phase が偽 closed になる (Phase 2 Milestone がこれで崩れた)。type ラベルにより「Phase 完了 = その Phase の `type: feature` Issue が全 closed」と明確に定義できる。

### 着手 Phase のみ Issue 化する理由

先の Phase の実装詳細は知見が揃っていないため陳腐化する。**着手するフェーズの Issue のみを作成する** (空想 Issue 禁止)。現在は Phase 2/3 が active なので両 Phase の Issue を管理対象にする。

差し込み作業 (保守・設計・リファクタ) は奉仕する Phase の Milestone に入れ `type:` ラベルで区別する。これにより作業の置き場に迷わず、かつ Phase 完了判定を汚さない。

## Implementation Details

- 1 フェーズ = 1 GitHub Milestone
- Phase 0 Milestone は作成後すぐ close (完了済みの記録)
- branch 命名規則: `claude/issue-<N>-<slug>`
- PR description に `Closes #N` を含めて Issue を自動 close
- viewer スタック選定 → ADR-003 として記録済み (003-viewer-and-app-architecture.md)
- Phase 完了判定: 「その Phase の `type: feature` Issue が全て closed」。`type: refactor` / `type: foundation` / `bug` は判定に含めない
