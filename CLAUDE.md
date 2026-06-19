# EngawaCAD — Claude Code ガイドライン

## AI作業ルール

- 推測による作業を禁止。必ず検索や調査を行い、事実に基づいて作業すること。
- プランモードではユーザーの許可があるまでプランファイルの作成を禁止。
- プランモードではまず議論や壁打ちを行い、適宜質問や反論を通じて客観的な視点から議論すること。
- `/3ai` スキルでの提案・説明は平易なトーンで提示する: 専門用語を避けて噛み砕き、具体例・身近な比喩・番号付き手順を用い、初学者でも理解できるように説明する。

## セッション開始時のチェックリスト

- `ROADMAP.md` を読み、現在の Phase と完了条件を把握する
- 現在 Phase の `type: feature` Issue が全 closed なら、ROADMAP 表記 (🚧→✅) とマイルストーン close が反映済みか突合し、漏れていれば ADR-002 の Phase 完了手続きに従って反映する
- `gh issue list --state open` で open issue を確認する
- ユーザーが特定 Issue を指定しない場合は、現在 Phase の Milestone に紐づく未着手 Issue を提案する
- Issue に着手する際は commit message に `Closes #N` を含めてメインブランチへ直接 push する (branch・PR は不要)
- ただし PR を作る場合は作成直後にセルフマージしてブランチを削除する
- 実装完了後は Issue・マイルストーンの後処理を自己判断で行う: Issue が自動クローズされているか確認し、されていなければ閉じる。Phase の `type: feature` Issue が全 closed になったら ADR-002 の手続きに従い ROADMAP を ✅ に更新してマイルストーンを close する

## 新 Phase 着手 / 新 Issue 起票時のチェックリスト

**Issue を新たに作成する前に** ADR-006 の粒度ガードを適用する (`docs/decisions/006-issue-decomposition.md`)。
主なルール: 1 Issue = GLM 1 サイクルで通せるサイズ / ADR 決定と実装を混在させない / 軸 × op マトリクスで分解する。

1. ADR-006 §1 の粒度チェックリストを全項目 ✓ にする
2. `dispatch-codex-intent.ts` で Codex intent-check を実施:
   - `aligned: yes` → 次の step 3 (ラベル決定) へ
   - `aligned: no` → ユーザーと相談して Issue 案を修正 → 再チェック
3. **ラベルを 2 軸決定する** (ADR-002 / ADR-006 §1):
   - **type 軸 (必須・1 つ)**: `type: feature` / `type: refactor` / `type: foundation` / `bug` / `docs` のいずれか
   - **batch 軸**: `type: feature` 以外なら `batch:kernel` / `batch:data` / `batch:viewer` / `batch:skill` のいずれかを**必ず**追加
   - `enhancement` は ADR-002 type 軸の正規ラベルではない (機能拡張系は `type: foundation` を使う)
4. `gh issue create --label "<type>,<batch>"` で起票。起票直後に必ず `bun .claude/skills/3ai/scripts/lint-issue-labels.ts --issue <N>` で検証 (exit 0 を確認)
5. **親 Issue から派生する子 Issue を起票する場合は、親 milestone を必ず継承する** (#261 — `--milestone "Phase N: ..."` を必ず渡す)。継承漏れだと batch-select の phase-feature tier で弾かれて永遠に actionable 化しない
6. Phase 着手時は当 Phase の ADR にペルソナ構成を記載する (Common 3 + Phase オプション)

⚠️ **ラベル抜けの代償**: `batch:*` または type 軸のいずれかが欠けた Issue は、`/3ai` auto 選定 (`batch-select.ts` の bug-batch / enh-batch / foundation-batch / refactor-batch ラダー) から漏れて取り残される

⚠️ **milestone 抜けの代償**: Phase N 子 Issue (`type: feature`) に milestone が無いと phase-feature tier に乗らない (#261 で観測、#244 子の #255-#260 が手動付与待ちになった)

## Project Overview

EngawaCADはRust製のオープンソースB-rep (Boundary Representation) CADカーネル。
設計原則: **決定性** (同じ入力は常に同じ出力)、**パラメトリック履歴** (feature historyが真実の源)、**YAML-based format** (人間可読・diff可能)。

## Project Map

```
crates/
├── engawa-kernel/     B-rep幾何カーネル
│   ├── brep/         トポロジー (Solid, Shell, Face, Loop, Edge, HalfEdge, Vertex)
│   ├── geometry/     幾何型 (Point, Vec3, Plane, Curve, Surface) + 共有数学ユーティリティ
│   ├── primitives/   プリミティブ生成 (make_cuboid)
│   └── tessellation/ メッシュ化 (tessellate_solid → TriangleMesh)
├── engawa-format/     .engawaファイルのYAMLスキーマ・パーサー
│   ├── document.rs   Document (トップレベル)
│   ├── feature.rs    Feature enum (操作履歴の1ステップ)
│   └── component.rs  Component (設計階層)
├── engawa-build/      Featureディスパッチャー (format → kernel の橋渡し)
├── engawa-cli/        CLIバイナリ (engawa)
├── engawa-viewer/     3Dビューア (未実装)
└── xtask/            ビルド自動化タスク
```

## Build & Test Commands

```bash
cargo build --workspace          # 全クレートビルド
cargo test --workspace           # 全テスト実行
cargo clippy --workspace -- -D warnings  # lint
cargo fmt --all -- --check       # フォーマットチェック
cargo xtask ci                   # 上記すべてを順次実行
```

## Conventions

- **決定性**: すべてのEntityIDは `IdGenerator` で決定的に生成。テストでは決定性を検証すること。
- **依存管理**: 新しい依存は必ず `[workspace.dependencies]` に追加し、各クレートは `{ workspace = true }` で参照。
- **derive規約**: 公開型には `Debug, Clone, Serialize, Deserialize` を付与。スキーマ生成が必要な型は `JsonSchema` も追加。
- **エラーハンドリング**: ライブラリエラーには `thiserror` を使用。
- **モジュール構造**: 1概念1ファイル。重要な型は `mod.rs` から `pub use` で再エクスポート。
- **テスト**: 単体テストは `#[cfg(test)] mod tests` でインライン。統合テストは `tests/` ディレクトリ。

## Architecture Principles

- **Index-based topology**: `Solid` 内のトポロジーエンティティはフラット配列に格納し、インデックスで参照（ポインタ不使用）。
- **Feature history = source of truth**: `engawa-format` の Feature 列が設計の真実の源。B-rep は Feature から再生成される。
- **Kernel has zero rendering deps**: カーネルはレンダリング非依存。`TriangleMesh` を生成し、ビューアが消費する。
- **Flat crates layout**: `crates/` 直下にフラットに配置。深いネストは作らない。
