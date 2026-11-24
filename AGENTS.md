# EngawaCAD agent guidance

## Authorization

- 回答・説明・レビュー・診断・検討の依頼では、関連する情報を確認して結果を報告する。明示的な依頼がない限り、ファイル変更や計画成果物の作成は行わない。
- 変更・実装・修正の依頼では、範囲内のローカル変更と非破壊的な検証を実施する。
- 作業範囲内で完了した変更は、エージェントが適切と判断した時点で、追加確認なしに commit および現在のブランチへ push してよい。
- 特定 Issue の実装依頼は、その Issue に必要なローカル変更、commit、メインブランチへの直接 push、Issue close、および必要な Phase 完了処理までの包括承認として扱う。
- 上記の包括承認に含まれない外部への書き込み、破壊的操作、購入、依頼範囲の実質的な拡大は事前に確認する。

## Communication

- 回答・説明・レビュー・壁打ちでは、結論または要点を先に示す。
- 内容に応じて、確認できた事実、解釈・懸念、選択肢または次の一歩を分けて示す。
- 短い事実回答には固定テンプレートや過剰な見出しを強制せず、必要な構造だけを使う。
- 事実・推測・価値判断を混同せず、判断には対応する根拠を添える。
- `/3ai` スキルでの提案・説明は、専門用語を避けた平易なトーンにする。具体例・身近な比喩・番号付き手順を用い、初学者にも分かるように説明する。

## Exploration

ユーザーが「考えたい」「整理したい」「壁打ち」「検討したい」と示したとき:

- 計画ファイルの作成、ファイル編集、外部操作を始めない。
- 判断すべきこと、重要な前提、最も影響の大きい不確実性を整理する。
- 判断を変え得る場合に限り、前提を点検し、代替案またはトレードオフを示す。
- 推測で作業しない。事実と仮説を区別し、検索・調査・既存実装の確認に基づいて判断する。
- ワークスペースや適切なツールで確認できる事実は、質問する前に確認する。
- 次の行動を実質的に変える質問だけを、1回に最大2つ行う。
- 具体的な次の行動、小さな検証、または決めるべき事項を1つ提示して終える。一般的な確認依頼で終えない。

## Evidence and records

- 修正・完了を報告するときは、関連するテスト、確認結果、ログ、または未検証の制約を示す。
- 議論を保存するときは、決定、理由、未解決事項を残す。通常の往復や試行錯誤の全記録は残さない。

## GLM delivery

- ユーザーが確定プランの実行を明示承認した場合は、GLMワーカーを手動起動せず `$glm-delivery` Skill を使用する。
- `$glm-delivery` の長時間ランは `$async-agent-delegation` Skill を通して起動し、終了通知後に同じCodexスレッドを再開する。
- Skillが定める実装・盲検検証・修正・再検証・Codex最終レビューの順序を省略または変更しない。
- SkillまたはGLM-5.2の疎通確認に失敗した場合は実装を開始せず、失敗を報告する。

## External AI delegation

- GLM、Claude、Geminiその他の外部AIエージェントへ長時間タスクを渡す場合は、エージェント固有の手順に加えて必ず `$async-agent-delegation` Skill を使用する。
- 現在のターンではジョブをキューへ登録し、返された委譲マーカーを最終メッセージへ含めてターンを終了する。Codexのturn-complete通知からジョブを起動し、プロセス終了後のコールバックで同じスレッドを再開する。
- 実行中の `write_stdin`、プロセス一覧・状態ファイルの反復確認、sleepループ、定期的な進捗確認は禁止する。「継続中」を確認するためのツール呼び出しもポーリングとして扱う。
- 成功、レート制限、異常終了のいずれも終了通知として扱う。再開後は保存済みの終了コード、状態、ログを一度だけ確認して次の処理を行う。

## セッションと GitHub 運用

セッション開始時には、次を確認する。

- `ROADMAP.md` を読み、現在の Phase と完了条件を把握する。
- 現在 Phase の `type: feature` Issue がすべて closed なら、ROADMAP の表示（🚧→✅）と milestone の close が反映済みか確認し、漏れは ADR-002 の Phase 完了手続きに従う。
- `gh issue list --state open` で open issue を確認する。
- ユーザーが特定 Issue を指定しない場合、現在 Phase の milestone に紐づく未着手 Issue を提案する。

Issue に着手する場合:

- commit message に `Closes #N` を含め、メインブランチへ直接 push する。branch・PR は不要。
- PR を作る場合は、作成直後にセルフマージして branch を削除する。
- 実装完了後は、Issue が自動 close されたことを確認し、されていなければ close する。Phase の `type: feature` Issue がすべて closed になったら、ADR-002 に従って ROADMAP を ✅ に更新し、milestone を close する。

## 新 Phase・新 Issue

Issue を新規作成する前に、ADR-006（`docs/decisions/006-issue-decomposition.md`）の粒度ガードを適用する。

1. ADR-006 §1 の粒度チェックリストをすべて満たす。
2. 次のコマンドで粒度チェックを実行する。

   ```bash
   bun .claude/skills/3ai/scripts/check-issue-granularity.ts --issue-draft <draft.md> --result <intent.yaml>
   ```

   `aligned: yes` ならラベル決定へ進む。`aligned: no` なら、`split_proposal` に従って分割するか、type軸ラベル・In-Scope表・`enhancement` の使用を修正して再チェックする。

3. ラベルを2軸で決定する。
   - type軸（必須、1つ）: `type: feature`、`type: refactor`、`type: foundation`、`bug`、`docs`。
   - batch軸: `type: feature` 以外では `batch:kernel`、`batch:data`、`batch:viewer`、`batch:skill` のいずれかを必ず追加する。
   - `enhancement` は正規のtype軸ラベルではない。機能拡張には `type: foundation` を使う。
4. `gh issue create --label "<type>,<batch>"` で起票し、直後に `bun .claude/skills/3ai/scripts/lint-issue-labels.ts --issue <N>` を実行して exit 0 を確認する。
5. 親Issueから子Issueを作るときは、親の milestone を必ず `--milestone "Phase N: ..."` で継承する。
6. Phase 着手時は、当該Phaseの ADR にペルソナ構成（Common 3 + Phase オプション）を記載する。

`batch:*` またはtype軸ラベルが欠けたIssueは `/3ai` の自動選定から漏れる。Phase の `type: feature` 子Issueに milestone がない場合も、phase-feature tier に乗らない。

## プロジェクト概要

EngawaCAD は Rust 製オープンソース B-rep（Boundary Representation）CADカーネルである。設計原則は次のとおり。

- **決定性**: 同じ入力は常に同じ出力を生成する。
- **パラメトリック履歴**: feature history を真実の源とする。
- **YAML形式**: 人間が読みやすく、差分を確認しやすい形式を用いる。

主要クレートは `engawa-kernel`（B-rep・幾何・プリミティブ・メッシュ化）、`engawa-format`（`.engawa` YAMLスキーマ）、`engawa-build`（formatからkernelへのディスパッチ）、`engawa-cli`（CLI）、`engawa-viewer`（将来の3Dビューア）、`xtask`（自動化）である。

## 実装規約

- すべての Entity ID は `IdGenerator` で決定的に生成し、テストで決定性を検証する。
- 新しい依存は必ず `[workspace.dependencies]` に追加し、各クレートでは `{ workspace = true }` で参照する。
- 公開型には `Debug, Clone, Serialize, Deserialize` を付与し、スキーマ生成が必要な型には `JsonSchema` も付与する。
- ライブラリエラーには `thiserror` を使う。
- 1概念1ファイルとし、重要な型は `mod.rs` から `pub use` で再エクスポートする。
- 単体テストは `#[cfg(test)] mod tests` としてインラインに置き、統合テストは `tests/` に置く。

アーキテクチャ上、`Solid` のトポロジーエンティティはフラット配列に格納し、インデックスで参照する。`engawa-format` の Feature 列が設計の真実の源であり、B-rep はそこから再生成する。カーネルにはレンダリング依存を入れず、`TriangleMesh` を生成してビューアが消費する。クレートは `crates/` 直下にフラットに配置し、深いネストを避ける。

## 検証コマンド

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
cargo xtask ci
```
