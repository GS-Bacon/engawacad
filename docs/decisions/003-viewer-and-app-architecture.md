# ADR-003: ビューア・アプリ全体アーキテクチャの選定

## Status

Accepted

## Context

Phase 2 (Viewer) 着手にあたり、最終形を見据えたアーキテクチャを決定する必要が生じた。

**最終形のビジョン**:
- Fusion/FreeCAD 的な対話 CAD アプリ (人間が直感的に操作)
- CLI/API で AI・スクリプトも操作可能
- OSS として配布可能。ローカル完結でも self-hosted でも動く

**開発制約**:
- 現在はリモート Linux 環境で開発。ローカルは Ubuntu デスクトップ。将来 Windows / macOS でも動かしたい。
- ローカル完結 (デスクトップアプリ) と self-hosted Onshape 的利用の両取りが理想。

## Decision

**Rust core + Rust API 層 + TS フロント (型は Rust から自動生成) + 二配備 (Tauri / server)**

```
[ Rust Core: kernel / format / build ]  ← 既存。据え置き。Feature 履歴が真実。
          ▲                   ▲
   embed  │            HTTP/WS │ 公開
┌─────────┴──────┐   ┌─────────┴──────────┐
│ Tauri アプリ    │   │  サーバモード        │
│ ローカル完結    │   │  ブラウザ/出先       │
│ 1 クリック起動  │   │  複数人接続可        │
│ native shortcut│   │  self-hosted 可      │
└─────────┬──────┘   └─────────┬──────────┘
          └──── 同じ TS フロント ────┘
                  (型は Rust から自動生成)
```

CLI / AI / スクリプトはすべて同じ **API 層**を経由する。GUI も API の一クライアントに過ぎない。

## Rationale

### なぜ Rust backend を維持するか (TS backend を採用しない理由)

- kernel は既に Rust で稼働しており、B-rep トポロジ・決定的 ID・曲面テッセレーションまで実装済みの主要資産を廃棄することになる。
- CAD kernel は性能要求が高い (テッセレーション、Boolean、大規模アセンブリ)。Rust ネイティブが圧倒的に有利。
- プロジェクト第一原則「決定性」は Rust + nalgebra で制御しやすい。
- 「型統一」の旨味は **ts-rs / JsonSchema → TS による型自動生成**で代替できる (Rust 型定義を真実とした型安全な TS 型)。

### なぜ純ブラウザ (サーバ必須) を採用しないか

- ローカル完結で使うには「サーバ起動 + ブラウザ起動」2 プロセスが必要。
- ブラウザは複雑な CAD ショートカットキーとの相性が悪い。
- ローカルファイルへの直接アクセスが制限される。
- Tauri はこれら 3 点を解消しつつ、同じ TS フロントをそのまま使える。

### なぜ Tauri + server の両取りが可能か

- TS フロントは 1 つ。Tauri は embedded Rust core を直接呼ぶ (追加サーバプロセス不要)。
- server モードは同じ Rust core を axum 等で公開し、同じ TS フロントを配信する。
- Windows / macOS 配布は Tauri のクロスビルドで対応可能。
- server の multi-user 対応: `.mycad` は YAML で Git 管理可能な文字列 Feature 履歴。realtime 協調編集 (OT/CRDT) が不要 (Git で合流) なため、サーバ実装を大幅に簡略化できる。

## 変更モデル: Feature コマンド一本化

GUI / CLI / AI / スクリプトを問わず、**モデルを変える操作はすべて Feature コマンドとして履歴に積む**。

| 種別 | 扱い |
|---|---|
| Feature の追加・編集・削除 (`.mycad` が変わる操作) | 履歴に積む。undo スタック対象。 |
| カメラ位置・ズーム・選択/ホバー | transient 状態。セッション内のみ。`.mycad` にも undo にも含めない。 |
| ドラッグ中プレビュー | transient。ジェスチャー確定時に 1 Feature として coalesce する (毎フレーム積まない)。 |

この一本化により undo/redo・スクリプト記録・AI 操作・決定的リプレイ・増分再生成キャッシュが自然に成立する。

## API 契約 v0 の方針

- **transport**: HTTP (REST) をベースとし、push/preview 用に WebSocket を追加。
- **versioning**: URL prefix `/api/v0/`。破壊的変更時は `/v1/` に移行。
- **操作**: Feature コマンドベース (CRUD on feature list) + tessellation/query エンドポイント。
- **TS 型**: `ts-rs` または JSON Schema → TS で Rust 型から自動生成。フロントは Rust 型定義を真実とする。

## 関連懸念と委譲先

| 懸念 | 委譲先 |
|---|---|
| トポロジカル・ネーミング (生 index 参照禁止、Phase 3 前提) | **ADR-005** |
| `schema_version` / kernel version スタンプ / migration | Issue I-2 |
| 単位系・グローバル公差 | ADR-004 (一部)、Issue I-4 |
| API 契約詳細 | 本 ADR の方針に基づき Phase 2 実装時に確定 |
| 増分再生成 (Feature 純関数保証) | 変更モデルの必然的帰結。将来最適化フェーズで有効化 |
| Tauri 包装・server 認証 | Issue 化は当該 Phase 着手時 (空想 Issue 作らない) |

## Implementation Details

- Phase 2 の最小実装: Rust HTTP サーバ + TS フロント (Three.js) の骨組み。`mycad view` = サーバ起動 + ブラウザ自動起動。
- `TriangleMesh` への face グループ情報付与 (ピッキングの前提) は Phase 3 の `Extrude` 実装前に対応する。
- Tauri 包装は Phase 2 完了後の発展的ステップ。Issue 化は着手時。
