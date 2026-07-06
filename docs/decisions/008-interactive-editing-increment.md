# ADR-008: 対話編集の背骨 — Phase 6 設計決定

**Date**: 2026-06-07
**Status**: Accepted
**Related**: ADR-003 (viewer/app architecture), ADR-005 (topological naming), ADR-006 (issue decomposition)

---

## Context

Phase 0–5 の完了により、MyCad は「決定的 B-rep カーネル + YAML 手書き入力 + 閲覧専用ビューア」として動作する。
ADR-003 の最終ビジョン「Fusion/FreeCAD 的な対話 CAD」との間には次のギャップが残っている:

- `TriangleMesh` は `positions / normals / indices` のみで**面の所属情報を持たない** → ブラウザで面を選べない
- API は `GET /api/v0/mesh` 1 本のみ → 書き込み操作ができない
- スケッチはすべて `.mycad` YAML への手書き → 人が触る UI が存在しない

Phase 6 では「対話ループの背骨を縦に薄く一本通す」ことを目標とする。
最初の操作（first gesture）は**既存の平面/モデル面を選んで押出または押出カット**。
新しい幾何を作らなくても現行機能だけで対話ループを通せる最小スライスである。

本 ADR はこの背骨を通すために必要な **3 つの設計判断**を記録する。実装の詳細は各 Issue の `plan.md` に委ねる。

---

## Decision 1: 押出 / 押出カットを別 Feature variant とする

### 決定

`Extrude`（加算）と `ExtrudeCut`（除算）を **別の Feature variant** として `mycad-format` の `Feature` enum に定義する。

```
Feature::Extrude    { id, sketch, depth, … }   // ソリッドに体積を加える
Feature::ExtrudeCut { id, sketch, depth, … }   // ソリッドから体積を引く
```

### 採用しなかった案

```
Feature::Extrude { id, sketch, depth, operation: Add | Cut }
```

### 理由

1. **意図が一読でわかる**: `.mycad` を人が読むとき、`ExtrudeCut` と書いてあれば「これは除去」と即判別できる。
   `operation: Cut` が後ろに隠れる構造は可読性が落ちる。
2. **ADR-003「変更モデル一本化」との整合**: Feature ログが設計意図の記録である以上、"何をしたか" を
   型レベルで表現することが望ましい。加算と除算は概念的に異なる操作。
3. **将来の拡張への対応**: `ExtrudeCut` に固有のパラメータ（例: "穴の深さを"最後まで"とする `ThroughAll` フラグ）
   が将来追加される可能性がある。混合型では区別が難しくなる。
4. **実装コスト**: `ExtrudeCut` の本体は `make_extrusion` + Boolean `Cut`（Phase 4 で実装済み）の合成で
   実現できるため、別 variant にしてもカーネル側の追加工数は最小。

### 影響範囲

- `crates/mycad-format/src/feature.rs`: `Feature` enum に `ExtrudeCut` を追加
- `crates/mycad-build/src/lib.rs`: `Feature::ExtrudeCut` のディスパッチを追加
- `web/src/generated/Feature.ts`: ts-rs による自動再生成（手編集不要）

---

## Decision 2: TriangleMesh に面の安定参照を焼き込む

### 決定

`TriangleMesh` に `face_ids: Vec<String>` を追加し、各三角形が属する面の `EntityRef` 文字列表現を
triangle ごとに記録する。生 index は露出しない（ADR-005 準拠）。

```rust
pub struct TriangleMesh {
    pub positions: Vec<[f64; 3]>,
    pub normals:   Vec<[f64; 3]>,
    pub indices:   Vec<u32>,
    /// 三角形 i が属する面の安定参照 (EntityRef の文字列表現).
    /// 長さは indices.len() / 3 と一致する.
    pub face_ids:  Vec<String>,
}
```

`BodyMesh` / TS 型は `face_ids` を含む形で自動再生成される。

### 理由

1. **ピッキングの前提**: ブラウザの raycaster が返すのは三角形インデックスのみ。
   三角形 → 面 の対応が無ければ「どの面を選んだか」が決定できない。
2. **ADR-005 準拠**: 生の Face index をフロントに返すと、Feature 再生成後に index が変わり
   参照が壊れる。`EntityRef`（安定命名）を使うことでトポロジー変化後も面参照が安定する。
3. **設計の整合**: ADR-003 が「`TriangleMesh` への face グループ情報付与はピッキングの前提」
   と明記しており、本 ADR でその宿題を実施する。

### 採用しなかった案: face_index: Vec<u32>

生 Face index をフロントに渡す案は実装が簡単だが、ADR-005 の保証を破るため不採用。

### 影響範囲

- `crates/mycad-kernel/src/tessellation/mod.rs`: テッセレーション時に `face_ids` を充填
- `crates/mycad-api/src/transport/body_mesh.rs`: `BodyMesh` 型の変更なし（`TriangleMesh` を透過的に含む）
- `web/src/generated/TriangleMesh.ts`: ts-rs による自動再生成

---

## Decision 3: 書き込み API の契約

### 決定

ADR-003「変更モデル一本化」を Phase 6 の最小スコープで具体化する。

#### エンドポイント

```
POST /api/v0/features      Feature を 1 個 .mycad に追加し、再テッセレーション結果を返す
```

レスポンスは既存の `GET /api/v0/mesh` と同じ `Vec<BodyMesh>` 形式。
フロントはレスポンスを受け取り、Three.js シーンを差し替える（WebSocket は Phase 7 以降）。

#### transient / committed の境界

| 操作 | 分類 | 実装 |
|---|---|---|
| カメラ位置・ズーム・回転 | transient | クライアント内のみ。サーバに送らない |
| 面の選択・ホバーハイライト | transient | クライアント内のみ |
| `Extrude` / `ExtrudeCut` の実行 | committed | `POST /api/v0/features` で `.mycad` に積む |

#### push / preview について

Phase 6 では WebSocket を導入しない（複雑さを最小化）。
`POST` の同期レスポンスでモデルを更新するシンプルな request/response モデルを採用する。
リアルタイムプレビュー（ドラッグ中の即時反映）は Phase 7 以降の拡張とする。

#### ファイル永続化

`POST /api/v0/features` は `.mycad` をメモリ上で更新した上でディスクに書き戻す。
（`mycad view` は単一ファイルを開くシングルユーザーサーバのため、競合問題は発生しない）

### 理由

- Phase 6 の目的は「配管の疎通確認」。WebSocket を加えると実証と無関係な複雑さが増える。
- 同期 POST はフロントの実装が最もシンプルで、疎通確認に集中できる。
- ADR-003 の将来発展（WebSocket/push preview）はアーキテクチャ上の位置づけを変えず、
  追加エンドポイントとして後付けできる。

---

## GLM ペルソナ構成（ADR-006 §GLM ペルソナ表への追記）

| ペルソナ | 適用 Phase | Phase 6 での観点 |
|---|---|---|
| SCOPE    | 全 Phase | In-Scope 表 / Issue 整合 / 粒度 |
| INVARIANT | 全 Phase | 決定性 / `face_ids` 長さ不変量 / 既存 Feature 副作用 |
| AMBIG    | 全 Phase | 曖昧表現 / 数値判断丸投げ |
| API      | Phase 6+  | エンドポイント契約 / transient 境界 / TS 型整合 |

`API` ペルソナは Phase 6 で新設。観点: HTTP 契約が Decision 3 と一致しているか、
フロント TS 型が自動生成で最新化されているか、`face_ids` の長さ不変量がテストで保証されているか。

---

## Issue 分解マトリクス

ADR-006 §3 に従い、実装 Issue を「1 軸 × 1–2 op」に分解する。起票は `check-issue-granularity.ts` の粒度チェック (d6f103b で Codex intent-check から移行) 後。

| Issue | スラグ | ラベル | 内容 |
|---|---|---|---|
| [#90](https://github.com/GS-Bacon/mycad/issues/90) | `viewer-render-loop`    | `viewer`, `type: refactor`, `batch:viewer` | render loop を rAF に統一し damping + DPR を設定 |
| [#91](https://github.com/GS-Bacon/mycad/issues/91) | `viewer-camera-controls` | `viewer`, `type: refactor`, `batch:viewer` | 初期フィット表示 + Front/Top/Iso 標準ビューボタン |
| [#92](https://github.com/GS-Bacon/mycad/issues/92) | `mesh-face-ids`    | `kernel`, `type: foundation`, `batch:kernel` | `TriangleMesh.face_ids` の実装 + BodyMesh/TS 型反映 |
| [#93](https://github.com/GS-Bacon/mycad/issues/93) | `write-api`        | `cli`, `type: feature`, `batch:kernel` | `POST /api/v0/features` エンドポイント + ディスク書き戻し（依存: #92） |
| [#94](https://github.com/GS-Bacon/mycad/issues/94) | `viewer-pick`      | `viewer`, `type: feature`, `batch:viewer` | raycaster によるピッキング + 選択ハイライト（依存: #92） |
| [#95](https://github.com/GS-Bacon/mycad/issues/95) | `extrude-op`       | `viewer`+`kernel`, `type: feature`, `batch:viewer` | 選択面から `Extrude` を UI 実行（依存: #93 #94） |
| [#96](https://github.com/GS-Bacon/mycad/issues/96) | `cut-op`           | `format`+`kernel`+`viewer`, `type: feature`, `batch:kernel` | `ExtrudeCut` variant 追加 + UI 実行（依存: #95） |

処理順（依存関係）: #90/#91/#92 → #93/#94 → #95 → #96

`type: feature` Issue（#93 / #94 / #95 / #96）が全 closed になった時点で
Phase 6 完了（ADR-002 完了手続き: ROADMAP ✅ + Milestone close）。
#90/#91（refactor）と #92（foundation）は完了判定の対象外。
