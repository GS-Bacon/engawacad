# ADR-010: スケッチ入力モデル — Phase 7 設計決定

**Date**: 2026-06-13
**Status**: Accepted
**Related**: ADR-002 (ロードマップ管理), ADR-003 (viewer/app architecture), ADR-005 (topological naming), ADR-006 (issue decomposition), ADR-008 (対話編集の背骨 / Phase 6)

---

## Context

Phase 6 までに「面クリック → 押出/押出カット」の対話ループが通り、`POST /api/v0/features` による Feature 追加と再テッセレーション返却の同期 request/response 経路が完成した。
Phase 7 「スケッチ描画 (正準平面)」は **ブラウザ上に 2D 線分プロファイルを直接描く** ことを完了条件とする (ROADMAP.md §Phase 7)。

Phase 7 開始時点で既に揃っている基盤:

- `engawa-format::Feature::CreateSketch { id, plane: SketchPlane, offset, profile: Vec<SketchSegment> }`
- `engawa-format::Feature::Extrude { id, sketch, depth, fuse_target }` (`sketch` フィールドは `CreateSketch.id` 参照)
- `engawa-build` の `validate_profile_closed`: 閉ループ検証 (`LENGTH_TOLERANCE = 1e-9`)
- `engawa-kernel::make_extrusion`: 閉プロファイル → ソリッド
- Phase 6 acceptance テスト (`web/tests/acceptance_extrude.spec.ts`) が `makeSketch()` ヘルパで CreateSketch + Extrude を API に投げる経路を既に検証している

つまり Phase 7 の残作業は **「ブラウザの 2D 入力 UI で `SketchSegment` を構築し、既存の API 経路に流す」** ことが中心。
ただし UI を実装し始めると以下 4 点で判断が必要になり、判断を後回しにすると UI / kernel / format を行き来した修正が頻発する:

1. 閉ループでない線分列が UI から発生した場合の振る舞い
2. 「最初に描く平面」をどう選ばせるか (空ドキュメントから始められるか)
3. `SketchPlane.offset` を UI に露出するか
4. 描画中 (ドラッグ・マウス移動) のプレビューをどこまで持つか

本 ADR はこの 4 点に対する設計判断を記録する。実装の詳細は各 Issue の `plan.md` に委ねる。

---

## Decision 1: 閉ループ違反時はフロントで自動スナップ + 視覚警告で防ぐ

### 決定

- バックエンド (`validate_profile_closed`) の閉ループ判定 (`LENGTH_TOLERANCE = 1e-9`) は**変更しない**。
- フロント側で以下 2 段の防御を入れる:
  1. **端点自動スナップ**: ユーザーが新しい線を引き終えるとき、終点が既存セグメントの端点 (とくに最初の点) と一定距離以内にあれば、その既存端点に座標を吸い付ける (`SketchSegment` の `to` を既存頂点座標に正規化)。
  2. **視覚警告 + 操作ロック**: 全セグメントの端点が一周つながっていない (= 1e-9 で閉じない) 場合は、スケッチ canvas を赤ハイライトし、Extrude / ExtrudeCut の確定ボタンを `disabled` にする。
- スナップ対象は **「線分の端点」のみ**。中点・中心 (将来円が入った時)・延長線・グリッド等のスナップは Phase 7 では実装しない。

### 採用しなかった案

- **A. 「閉じる」ボタンの明示押下**: ユーザーが UI で「ここで閉じる」と宣言する。
  → 操作回数が増え、CAD 慣れしたユーザーの想定外挙動 (Solidworks / Fusion / Onshape はいずれも自動スナップ)。
- **B. 自由描画 + 1e-9 厳格判定**: スナップなしでバックエンドに送る。
  → マウス入力の浮動小数誤差で 1e-9 はほぼ通らず、Phase 7 完了条件「ブラウザで矩形を描いて押出」が事実上達成不能になる。

### 理由

1. **バックエンドの厳密性を保持しつつ Phase 7 を達成可能にする**: 1e-9 は決定性 (ADR-005) と数値モデル (ADR-004) を守る境界条件として妥当。これを緩めず、フロント側で「閉じる意図」を座標レベルで実現する。
2. **将来のスナップ拡張に影響しない**: 中点・中心・延長線スナップは「マウス座標を別座標に寄せる」という同じ抽象の拡張なので、`SketchSegment` のスキーマもバックエンドの API も変更不要で後付けできる。
3. **拘束ソルバとは別概念**: 「スナップ」は入力時点の座標補正、「拘束」は描画後にも保持される幾何関係 (水平 / 平行 / 同心等)。後者は連立方程式の数値解法を要する独立した実装テーマで、ROADMAP の将来 Phase (拘束ソルバ) に属する。

### 影響範囲

- `web/src/sketch.ts` (新規): スナップ判定と SketchSegment 構築ロジック
- `web/src/main.ts`: Extrude / ExtrudeCut ボタンの enabled 状態を「閉ループ成立」と連動
- バックエンドへの変更なし

---

## Decision 2: `engawa-format` に `RefPlane` 概念を導入し、Document 初期化時に Front/Top/Right を自動配置

### 決定

- `engawa-format` に新しい型 `RefPlane` を追加する。最小スキーマ案 (詳細は Issue `format-refplane` の `plan.md` で確定):

  ```rust
  pub struct RefPlane {
      pub id: String,                // "Front" / "Top" / "Right" / 将来はユーザー命名
      pub plane: SketchPlane,        // Xy / Xz / Yz
      #[serde(default, skip_serializing_if = "is_zero")]
      pub offset: f64,               // Phase 7 では 0 固定 (Phase 8 で UI 露出)
  }
  ```

- `Document` (または root `Component`) に `ref_planes: Vec<RefPlane>` フィールドを追加する。
- **Document を新規作成した時点で Front (Xy)/ Top (Xz) / Right (Yz) の 3 枚が自動的に含まれる**。これは Solidworks / Fusion 360 / Onshape の挙動に合わせる。
- フロントは Three.js シーンに 3 枚の半透明 quad として描画し、Phase 6 で実装済みの raycaster 経路 (`web/src/viewer.ts:182-200`) でクリック検出する。クリックされた RefPlane は「次に作るスケッチの基準平面」として扱う。
- `CreateSketch` Feature に **新しいフィールド `plane_ref: String` を追加** (= 選ばれた RefPlane の `id` を参照)。既存の `plane: SketchPlane` + `offset: f64` フィールドは互換のため当面残すが、新規入力経路 (UI から起こされた CreateSketch) は `plane_ref` を使う。`plane`/`offset` と `plane_ref` の整合性ルール (どちらが優先か、両方あった場合の挙動) は Issue `format-refplane` の `plan.md` で確定する。
- Phase 6 で実装した「既存形状の面をクリック → スケッチ平面を自動推定」経路はそのまま残す (RefPlane と面ピックの両方が入口として併存)。

### 採用しなかった案

- **A. 3 ボタン UI ("Xy / Xz / Yz")** をフロントだけに追加 (format 無変更):
  - 真っ白なドキュメントから始められる点では同じだが、スケッチが「どの参照平面に作られたか」を `.engawa` に記録できず、Phase 8 (モデル面上のスケッチ / 任意平面追加) で必ず追加することになる `RefPlane` 概念を二度作ることになる。
- **B. 既存の面ピックだけに頼る** (format も UI も無変更):
  - 空ドキュメントから何も始められない。Phase 7 完了条件「ブラウザ上のスケッチキャンバスで線分を描いてプロファイルを作れる」が達成不能。

### 理由

1. **Solidworks 準拠の UX を最小コストで実現**: Front / Top / Right の 3 枚を最初から見せる流儀は CAD 業界標準で、初学者にも馴染みやすい。
2. **Phase 8 への布石**: 「モデル面上にスケッチ」は本質的に「面から導出される新しい RefPlane を追加する」操作。Phase 7 で `RefPlane` を導入しておけば、Phase 8 は **ユーザーが追加できる RefPlane** という単純な拡張で実現でき、二度作りを避けられる。
3. **`.engawa` ファイルの履歴可読性**: スケッチの基準平面が ID 参照になることで、`.engawa` を読んだ人が「このスケッチは Front プレーンに作られた」と一目で分かる。
4. **既存 SketchPlane の互換維持**: `plane` + `offset` フィールドを当面残すことで、Phase 6 までに書かれたサンプル `.engawa` ファイルとテストが壊れない。

### 影響範囲

- `crates/engawa-format/src/feature.rs`: `RefPlane` 型追加、`CreateSketch` への `plane_ref` 追加
- `crates/engawa-format/src/document.rs` (もしくは `component.rs`): `ref_planes` フィールド追加 + 初期化時の Front/Top/Right 自動補填
- `crates/engawa-build/src/lib.rs`: CreateSketch の `plane_ref` → 内部 `Plane` 変換 (既存の `base_plane.translate(...)` 経路を踏襲)
- `web/src/generated/Feature.ts`, `web/src/generated/RefPlane.ts`: ts-rs による自動再生成
- `web/src/viewer.ts`: RefPlane の半透明描画 + raycaster でのクリック検出
- `web/src/extrude.ts` / `web/src/main.ts`: 新規スケッチの基準として RefPlane の id を渡す経路

---

## Decision 3: `SketchPlane.offset` は Phase 7 では UI に露出せず、`ExtrudeCut` の offset 無視バグは独立 Issue で修正

### 決定

- Phase 7 で配置される RefPlane (Front/Top/Right) は **全て `offset = 0` 固定**。UI から offset の値を変更する手段は提供しない。
- offset 入力欄が必要になるのは **Phase 8 (モデル面上のスケッチ / ユーザー定義 RefPlane の追加)** であり、その時点で RefPlane エディタの一部として導入する。
- `engawa-build/src/lib.rs` の現状で **`ExtrudeCut` のディスパッチが `CreateSketch.offset` を `_offset` で抑制し読まずに捨てている** 不整合は仕様バグ。本 ADR と並行して、独立した `type: bug` Issue で修正する。

### 採用しなかった案

- **A. Phase 7 で offset 入力欄を出す** (正準平面ボタン選択時):
  → RefPlane 概念を導入する Decision 2 と二重表現になる。ユーザーが「Front プレーン + offset 50」を選んだ場合と「offset 50 の独自 RefPlane」を作った場合の意味が UI 上区別できず混乱を招く。
- **B. ExtrudeCut の offset バグを Phase 7 内で修正**:
  → Phase 7 の RefPlane は offset=0 のみなので、Phase 7 中はバグの実害が出ない。Phase 7 スコープと独立して扱う方が ADR-006 §2「設計/実装の分離」と整合。

### 理由

1. **概念の二重持ちを避ける**: 「正準平面 + offset」と「独自 RefPlane」の両方が UI に存在すると、ユーザー視点で意味の重なる入口が 2 つになる。Phase 7 では「3 枚の RefPlane を選ぶ」一本に絞る。
2. **Phase 8 で必要なものを Phase 8 で出す**: Phase 7 を最小スコープで達成しつつ、Phase 8 着手時に offset 入力欄を RefPlane エディタの自然な一部として導入できる。
3. **既存バグを温存しない**: Phase 7 で実害が出なくても、`ExtrudeCut` で offset が読まれない仕様不整合はテストカバレッジの隙であり、`type: bug` の独立 Issue として明示的に潰す。

### 影響範囲

- `crates/engawa-build/src/lib.rs`: `ExtrudeCut` ディスパッチで `_offset` を `offset` として読み、`Plane::translate` 適用 (独立 Issue で対応)
- UI: 変更なし (Phase 7 では offset 入力欄を出さない)

---

## Decision 4: 描画中プレビューはフロント完結。API には preview エンドポイントを追加しない

### 決定

- 線分描画中の中間状態 (「次のクリックで引かれる線」の表示 = ラバーバンド) は **`web/src/` 内で Three.js (または HTML Canvas) シーンに描く** ことで完結する。
- 確定 (右クリック・Enter キー・閉ループ達成時) で初めて `POST /api/v0/features` に `CreateSketch` + `Extrude`/`ExtrudeCut` をまとめて送る (Phase 6 の流儀を踏襲)。
- API には preview / readonly 計算用エンドポイントを **追加しない**。

### 採用しなかった案

- **A. preview エンドポイント追加**: `POST /api/v0/preview` で確定せず形状を計算しレスポンスのみ返す。
  → Phase 7 のラバーバンドは「次に引かれる予定の半透明線」を見せたいだけで、kernel 側で何の計算もしない。バックエンドへのラウンドトリップが入ると逆に視覚レスポンスが鈍る。

### 理由

1. **ADR-008 Decision 3 と整合**: Phase 6 で「同期 request/response で Phase 7 以降の preview 拡張も追加エンドポイントで対応する」と決めた。Phase 7 のラバーバンド程度では API 拡張は必要ない。
2. **kernel 計算が不要**: ラバーバンドは確定前の入力候補を見せるだけで、トポロジー計算は一切走らない。フロント完結でレイテンシゼロにできる。
3. **将来の preview と互換**: Phase 9 以降で「ドラッグ中に kernel 計算結果をプレビュー」のような重い preview が必要になった時、その時に preview エンドポイントを追加すればよい。

### 影響範囲

- `web/src/sketch.ts` (新規): ラバーバンド描画 + 確定操作の検出
- バックエンドへの変更なし

---

## Out of scope (本 ADR では扱わない)

以下は Phase 7 では実装しない。混乱を避けるため明示的に除外する:

- **幾何拘束ソルバ** (水平 / 垂直 / 平行 / 同心 / 同一長さ / 角度固定 等): ROADMAP の将来 Phase「円/円弧 + 拘束ソルバ」に属する。
- **円・円弧・スプライン要素**: Phase 7 は線分 (`SketchSegment`) のみ。曲線要素は拘束ソルバ Phase の前提。
- **中点 / 中心 / 延長線 / グリッド スナップ**: 「点スナップ」の自然な拡張だが、Phase 7 では端点スナップに絞る。データモデル無変更で後付け可能。
- **任意平面 (モデル面 / ユーザー定義 RefPlane の追加)**: Phase 8 「モデル面上のスケッチ」に属する。
- **スケッチ編集 (作成済みスケッチの線分追加・削除・移動)**: 「履歴編集 + undo/redo」フェーズに属する。Phase 7 は作成のみ。
- **スケッチ寸法表示**: 同上、将来の拡張。

---

## Affects

- **ADR-002** (ロードマップ管理): Phase 7 完了条件 (ROADMAP.md §Phase 7) はそのまま。本 ADR は完了条件の解釈を確定するもの。
- **ADR-003** (viewer/app architecture): preview エンドポイントを追加しないことで、Phase 6 で決めた同期 request/response モデルを継続。
- **ADR-005** (topological naming): バックエンドの 1e-9 閉ループ判定を維持することで、ID 生成の決定性を保つ。
- **ADR-008** (Phase 6 設計): RefPlane の raycaster 検出は Phase 6 の `face_ids` ピック経路を流用。

---

## Issue 分解マトリクス

ADR-006 §1 の粒度ガードに沿い、Issue を「1 軸 × 1〜2 op」に分解する。起票は各 Issue ごとに `check-issue-granularity.ts` (d6f103b で `dispatch-codex-intent.ts` から移行) で `aligned: yes` を取り、`gh issue create --label "<type>,<batch>"` 後に `bun .claude/skills/3ai/scripts/lint-issue-labels.ts --issue <N>` で検証する。

| # | スラグ | ラベル | 内容 | 依存 |
|---|---|---|---|---|
| 1 | `format-refplane`        | `type: feature`, `batch:kernel` | `engawa-format` に `RefPlane` 型追加 + Document 初期化で Front/Top/Right 自動配置 + `CreateSketch.plane_ref` 追加 (旧 `plane`/`offset` 互換維持) | — |
| 2 | `viewer-refplane-pick`   | `type: feature`, `batch:viewer` | フロントで RefPlane を半透明描画 + raycaster でクリック選択 | #1 |
| 3 | `viewer-sketch-canvas`   | `type: feature`, `batch:viewer` | 2D スケッチ描画 (線分入力 + ラバーバンド + 端点自動スナップ + 開ループ赤ハイライト) | #2 |
| 4 | `viewer-sketch-extrude`  | `type: feature`, `batch:viewer` | スケッチ確定 → Extrude / ExtrudeCut の UI 接続 (Phase 6 経路に `sketch_id` 連動) | #3 |
| 5 | `e2e-phase7-acceptance`  | `type: feature`, `batch:viewer` | Phase 7 完了 E2E (Playwright で「3 RefPlane × Extrude/ExtrudeCut」マトリクス) | #4 |
| 6 | `fix-extrudecut-offset`  | `bug`, `batch:kernel` | `ExtrudeCut` ディスパッチで `CreateSketch.offset` が読まれていない不整合を修正 | (独立) |

クリティカルパス: **#1 → #2 → #3 → #4 → #5**。**#6** は独立、Phase 7 進行中いつでも処理可。

`type: feature` Issue (#1 / #2 / #3 / #4 / #5) が全 closed になった時点で Phase 7 完了 (ADR-002 完了手続き: ROADMAP ✅ + Milestone close)。
**#6** (`bug`) は Phase 完了判定の対象外。
