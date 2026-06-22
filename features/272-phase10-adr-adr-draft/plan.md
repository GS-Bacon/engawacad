# Plan: #272 ADR draft — Phase 10 スケッチ基本曲線拡張 + スケッチ編集

## 自律判断ログ (B-3 自律モード)

- **Issue 本文では "ADR-016" と記載されているが番号衝突 (既存 `docs/decisions/016-engawa-cli-naming.md`)。次の空き番号 ADR-017 を採用する。** commit / 本 plan / ADR ファイル名すべて 017 で揃える。ADR-017 frontmatter に `Resolves: Issue #272 (Issue 本文の "ADR-016" は番号取り直し ADR-017)` を明記する。
- **本サイクルでは #272 (ADR draft / light flow) を `#273-#278` よりも先に処理する**。plan.json では batch:kernel groups order=0 が先だが、ADR-006 §1「ADR 決定と実装を混在させない」の運用上、ADR 方針未確定のまま実装に入ると後段で齟齬リスクが大きいため、自律判断で順序を入れ替える。`#273-#278` 各 plan.md は本 ADR-017 を前提として書く。
- 本 Issue は light flow + `intent_check_required: false`。STEP 2/2.5/3/4 を skip し、STEP 5/5.5/6/6.5/6.6/7/7.5 を経由して STEP 8 squash + close する。
- 本 Issue は **deliverable=code** だが実態は `docs/decisions/017-*.md` 1 ファイル追加 + Issue ラベル付与のみ。crates/** には触らない。STEP 5.5 acceptance skeleton は空、STEP 6 GLM core は **skip** (実体無し)、STEP 7/7.5 は docs-only として通す方針。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| ADR-017 draft 作成 (`docs/decisions/017-phase10-sketch-curves-and-edits.md`) | 各曲線の Rust 型定義 (子 #273-#275 で実装) |
| Decision Matrix (Options A/B/C + Trade-off + 採用前提崩壊 trigger) | スケッチ編集オペレーションの実装 (子 #276/#277/#278) |
| 基本曲線 7 種のパラメータ表現方針 | tolerance 値の具体決定 (ADR-004 既存値の流用方針のみ) |
| 退化判定基準 (ε_radius / ε_angle / ε_axis_ratio 等) の値域指針 | 拘束ソルバ・NURBS 統合 (Phase 11/15 で別 ADR) |
| スケッチ編集 7 種の API 抽象方針 | engawa-build dispatch 実装 (子 Issue) |
| 既存 SketchSegment との互換性方針 | SketchElement enum 実装 (子 #273) |
| Issue #272 への `gate:adr-review` ラベル付与 | レビュー実行自体 (L-5.6 / L-1.5 auto-accept チェーンが回す) |
| 親 #195 plan との整合確認 | #195 plan.md の修正 (子 split 完了後の親 close で対応) |

## Non-Goals

- 基本曲線・スケッチ編集の **実コード追加** (子 #273-#278 で分担)
- ADR-005 (Topological Naming) の改訂 (Phase 10 範囲外)
- ADR-004 (tolerance 規約) の改訂 (Phase 10 では既存値域を踏襲)
- Phase 11 拘束ソルバ・Phase 15 NURBS など先回り ADR

## 実装対象

- 影響ファイル: `docs/decisions/017-phase10-sketch-curves-and-edits.md` (新規)
- 既存関数の修正なし (= before/after スニペット不要)
- crates/ には一切触らない (docs-only feature)

## 設計方針 (ADR-017 で固める 4 項目)

### 1. 基本曲線 7 種のパラメータ表現

各曲線は新規 `SketchElement` enum variant として表現する。既存 `SketchSegment` (Line) は `SketchElement::Line` に内包される (= enum 拡張による互換性確保)。

| 曲線 | 必須パラメータ | 退化判定 |
|------|--------------|---------|
| Circle  | `center: [f64; 2]`, `radius: f64` | `radius < ε_radius` |
| Arc     | `center`, `radius`, `start_angle: f64`, `end_angle: f64` | `radius < ε_radius` または `(end - start).abs() < ε_angle` |
| Ellipse | `center`, `major: f64`, `minor: f64`, `rotation: f64` | `minor / major < ε_axis_ratio` または `major < ε_radius` |
| Conic   | `coeffs: [f64; 5]` (Ax² + Bxy + Cy² + Dx + Ey + F=0 を F=-1 で正規化) | `\|B² - 4AC\| < ε_discriminant` で degenerate |
| Rectangle | `corner_min: [f64; 2]`, `corner_max: [f64; 2]` | `width < ε_radius` または `height < ε_radius` |
| Polygon | `center`, `vertex_count: u32`, `circumradius: f64`, `rotation: f64` | `vertex_count < 3` または `circumradius < ε_radius` |
| Slot    | `center_a: [f64; 2]`, `center_b: [f64; 2]`, `radius: f64` | `radius < ε_radius` または `(b - a).norm() < ε_radius` |

退化判定値は ADR-004 既存 `EPS_LENGTH = 1e-9` / `EPS_ANGLE = 1e-9` を踏襲し、新規 `ε_axis_ratio = 1e-6` / `ε_discriminant = 1e-9` を ADR-017 で導入する。

### 2. スケッチ編集 7 種の API 抽象

すべての編集オペレーションは独立した `Feature` enum variant として履歴に残す (純関数モデル: ADR-001「Feature history = source of truth」)。

| 編集 op | feature variant | 主要パラメータ |
|--------|----------------|--------------|
| Trim   | `SketchTrim` | `sketch_ref: EntityRef`, `element_id`, `trim_point: [f64; 2]` |
| Extend | `SketchExtend` | `sketch_ref`, `element_id`, `extend_to: EntityRef \| Point` |
| Offset | `SketchOffset` | `sketch_ref`, `selection: Vec<element_id>`, `distance: f64` |
| Sketch Fillet | `SketchFillet` | `sketch_ref`, `vertex_ref: (e1_id, e2_id)`, `radius: f64` |
| Sketch Chamfer | `SketchChamfer` | `sketch_ref`, `vertex_ref`, `distance_a`, `distance_b` |
| Mirror | `SketchMirror` | `sketch_ref`, `mirror_line: EntityRef \| LineEq`, `selection: Vec<element_id>` |
| Pattern (Rect/Polar) | `SketchPattern` | `sketch_ref`, `kind: Rect \| Polar`, `count_u`, `count_v`, `spacing` |

ID 安定性: 編集後も既存 element ID を保持。分割される場合は `{a}_split_{n}` 派生 ID を決定的に割り当て (ADR-005 Topological Naming と整合)。

参照解決失敗時のエラー: `BuildError::SketchRefNotFound { sketch_id, element_id }` を導入し、`engawa-build` の Feature::apply 内で fail-fast (ADR-015 §1 `FeatureOp::apply_op()` ディスパッチに沿う)。

### 3. 既存 SketchSegment との互換性

`SketchSegment { id, from, to }` は `SketchElement::Line { id, from, to }` に renamed して enum に内包。既存 YAML golden の表現を **完全に保つ** ため、`SketchElement` の serde 表現は `#[serde(tag = "kind", rename_all = "snake_case")]` とし、既存 `from`/`to` をそのまま keep。

migration hook (ADR-015 §3) は **不要**: existing YAML はそのまま新 enum でデコードできるよう serde で互換性確保 (`kind` フィールド欠如時に `Line` を default にマップ)。`schema_version` は v1 のまま据え置く。

### 4. ε 値域指針 (ADR-004 との関係)

ADR-004 で既定:
- `EPS_LENGTH = 1e-9`  (本 ADR では `ε_radius` に流用)
- `EPS_ANGLE = 1e-9`   (本 ADR では `ε_angle` に流用)

ADR-017 で新規追加:
- `ε_axis_ratio = 1e-6`     (楕円の degenerate near-line 判定)
- `ε_discriminant = 1e-9`   (conic の degenerate 判定)
- `ε_polygon_min_edge = 1e-9` (= EPS_LENGTH と同値、可読性のため再定義)

すべて crates レベルでは `engawa-kernel::geometry::tolerances` に追加する想定 (子 #273-#275 で実装)。

## Decision Matrix (ADR-017 本文で展開)

| 項目 | Option A | Option B | Option C | 採用 | Trade-off | 採用前提崩壊 trigger |
|------|----------|----------|----------|------|-----------|--------------------|
| 曲線 enum 構造 | `SketchElement` enum 1 本 | 既存 SketchSegment 残し + 新規 SketchCurve enum 並列 | trait object (`Box<dyn SketchElementTrait>`) | **Option A** | enum は variant 追加が breaking、trait は dyn 越境で型情報失われる。Option A は ADR-015 §1 FeatureOp と同 pattern で一貫 | Phase 12 Refactor Pass 1 で variant 数 > 30 / 拘束ソルバが trait object 必須 |
| Conic 表現 | 一般二次形式 5 自由度 | discriminator 別 enum | NURBS 統一 | **Option A** | NURBS は Phase 15 先取り過剰、別 enum は退化判定で分岐増 | Phase 15 NURBS 導入で conic を NURBS で再表現する選択肢 |
| 編集モデル | 各編集 op を独立 Feature variant (純関数) | 既存 sketch を in-place mutation | edit DAG (sketch ごと局所履歴) | **Option A** | in-place は ADR-001 と矛盾、DAG は実装複雑 | 1 sketch 当たり編集 op > 100 (Phase 11 拘束ソルバ自動編集) |
| ID 安定性 | 編集後も既存 ID 保持 + 派生 ID | 毎編集で全再採番 | UUID 完全分離 | **Option A** | 再採番は ADR-005 と衝突、UUID は決定性要件と衝突 | ADR-005 改訂 (TN 編集後再計算規約変更) |
| SketchSegment 互換 | `SketchElement::Line` 内包 + serde default | breaking (schema v1 → v2 + MigrationHook) | dual-write 移行期間 | **Option A** | breaking は既存 example golden 全更新、dual-write は遷移期間ロジック複雑 | YAML パーサが `kind` default 扱えない問題発覚 |
| ε 値域 | ADR-004 既存流用 + Phase 10 新規 ε 追加 | Phase 10 専用 ε 別 crate | runtime config 化 | **Option A** | 別 crate は over-eng、runtime config は決定性と相性悪い | Phase 11 拘束ソルバで tolerance を user 指定にする必要 |

## テスト計画 (light flow / docs-only)

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | 本 Issue は docs-only (crates/ 変更なし) のためテスト追加なし。`cargo xtask ci` green を STEP 8 直前に確認するのみ | meta / N/A |
| T02_degen_doc | meta | ADR-017 本文に退化判定基準 (ε_radius / ε_angle / ε_axis_ratio / ε_discriminant) の全曲線分が明記されているか | grep で "ε_" の出現を 5 件以上確認 |
| T03_boundary_doc | meta | ADR-017 本文に Decision Matrix 全 6 行が存在するか | grep で "Option A" を 6 件以上確認 |

→ 退化/境界ケース ID は本 Issue では docs lint 的 meta テスト (`T02_degen_doc` / `T03_boundary_doc`) で代替。STEP 5.5 acceptance skeleton は空 (テストファイル追加なし) で済ませる。

## 幾何的不変条件チェックリスト

- N/A (本 Issue は ADR draft のみ、partition/assemble 系の幾何コードを触らない)

## 完了条件

1. `docs/decisions/017-phase10-sketch-curves-and-edits.md` を `Status: Proposed` で commit
2. ADR 内に上記 6 行の Decision Matrix が入っている
3. Issue #272 に `gate:adr-review` ラベル付与 → 次サイクル L-1.5 で auto-accept rescan
4. 親 #195 split 計画書と整合確認 (= ADR §Context で #195 を参照)

## 関連

- 親 #195 (blocked-by-split)
- ADR-006 §1 粒度ガード
- ADR-013 auto-accept フロー
- ADR-004 (tolerance 規約)
- ADR-005 (Topological Naming)
- ADR-015 (Phase 9 設計基盤 — 本 ADR は Phase 10 起点として同等位置づけ)
