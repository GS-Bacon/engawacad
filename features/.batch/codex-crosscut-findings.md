# B-6 横断 Codex レビュー結果

verdict: fail | critical=0, high=1, medium=1, blocking=1

## F01 (high) — viewer.ts `setView("top")` camera.up 未設定

**ファイル**: `web/src/viewer.ts` L230  
**内容**: `top` ビューで `camera.up` を切り替えずに `lookAt()` を呼ぶため、視線方向と up ベクトルが平行になる。Three.js の特例分岐で補正されるが厳密な真上視点にならない。  
**修正案**: `camera.up.set(0, 0, -1)` を `lookAt()` 前に追加。T09 で forward/up も検証する。

## F02 (medium) — state.rs T01 assertion が weak

**ファイル**: `crates/mycad-api/src/state.rs` L116  
**内容**: `features` フィールドのアドレス比較のみで shallow clone と区別できない。`snapshot returns clone, not alias` の証明として不十分。  
**修正案**: `candidate` 変更後に再度 `snapshot()` して state 内部が不変であることを確認するか `features.as_ptr()` 比較。

---
escalated: B-6 blocking ≥ 1 → ユーザー確認待ち

---

## B-6 R2 結果 (codex-crosscut-r2.yaml)

verdict: fail | critical=0, high=1, medium=1, blocking=1

### F01 (high) — main.ts usedFeatureIds 初期化不足 → **棄却 (false positive)**

Codex 主張: "body 一覧からのみ初期化" → 実際は `fetchAllFeatureIds()` で文書全 feature ID を取得している (`main.ts:48`)。

### F02 (medium) — state.rs doc フィールド公開 → **棄却 (false positive)**

Codex 主張: "`pub doc`" → 実際は `doc: Option<Document>` (pub なし、private)。
`ensure_loaded()` も `&Document` を返す（`&mut` ではない）。

### 判定

critical=0, 全 findings が false positive → Claude 自律裁量で B-6 passed とする。

---

## B-6 R3 (バッチ:viewer 追加分) — codex-crosscut.yaml

verdict: fail | critical=1, medium=1, blocking=1

### F01 (critical) — acceptance_extrude.spec.ts 固定 ID → **採用・修正済み**

問題: 再実行時に DuplicateFeatureId (422) でクラッシュ。  
対応: `const RUN = Date.now().toString(36)` を追加し全 ID を `${RUN}_sk_tXX` 形式でプレフィックス。

### F02 (medium) — fuzz_features.rs T03-T05 アサートが甘い → **採用・修正済み**

問題: T03 が NaN/Infinity を実際に送っていない、T04/T05 の assert が緩い。  
対応: T03 に生バイト NaN/Infinity + 422 assert を追加。T04 は 422 を assert。T05 は 4xx を assert。

### 修正後確認

- `cargo build --workspace` → OK
- `cargo clippy --workspace -- -D warnings` → OK
- `cd web && npx tsc --noEmit` → OK

→ Codex 再レビュー (R3-r2) → r3 → r4 まで継続

---

## B-6 R4

verdict: fail | critical=1, medium=0, high=1, blocking=2

### F01 (critical) — features/124-api-http/test-summary.json の coverage_hints が全 0 → **採用・修正済み（docs-only）**

実際の T01-T05 に合わせて total_added=5, determinism=1, degenerate=3, edge_case=1 に更新。

### F02 (high) — reuseExistingServer: !process.env.CI → **棄却**

理由:
- CI では `!process.env.CI = false` → `reuseExistingServer = false` → 毎回フレッシュ起動
- ローカルで false にすると 120s startup が繰り返されて開発速度が大幅低下
- playwright.config.ts はすでに `/tmp/mycad-test-server.mycad` を使いワークツリー汚染は解消済み
- 既存の `reuseExistingServer` パターンはプロジェクト全体（frontend server 側も同様）で一貫して使用されている

### 判定

B-6 ループ上限 2 回を超過。critical F01 は docs-only として修正済み（コードファイル変更なし）。
F02 (high) は棄却。→ Claude 裁量で B-6 完了とする。

---

## B-6 R5 (バッチ: bug-batch #135 + #136) — codex-crosscut.yaml (2026-06-11)

verdict: **pass** | critical=0, high=0, medium=1, low=0, blocking=0

**batch_start_sha**: `7ee1dbf`
**対象コミット**: #135 (transform.rotation 配線) + #136 (trimmed UV face u shift) + 各 features/* artifacts

### F01 (medium) — `total_rotation != IDENTITY3` の exact 比較

**file**: `crates/mycad-build/src/lib.rs:438`

> `total_rotation != IDENTITY3` と `total_offset != Vec3::zeros()` を exact な `f64` 比較で分岐しているため、親 `+45°` / 子 `-45°` のように変換が数学的に相殺される階層でも near-identity の回転・平行移動が適用され、不要な座標ノイズが B-rep に焼き込まれる。

**状態**: **#135 STEP 7.5 でも同内容を Codex が medium 指摘 → `features/135-build-component-transform-rotation/codex-findings.md` に既記録**。本 B-6 で再確認 (重複指摘) されたことで、別 Issue 起票の優先度が上がる候補。

**判定**: blocking=0 のため B-6 pass、本バッチでは対応せず記録のみ。将来 Issue 起票候補 (snap or `Tolerance` newtype 導入と合わせて検討、ADR-004 §段階移行プラン Issue #34 と並行可)。

---

# 2026-06-13 B-6 (#147 + #153 + skill batch) round 1

verdict: fail | critical=0, high=2, medium=1, blocking=2

## F01 (high, 採用→修正済み): t_degen_offset_axis_circ_center_rejected で perp ガード単独の回帰になっていない

**file**: `crates/mycad-kernel/tests/trim_sphere_circ_normal_acceptance.rs:1116`

> `signed_offset = 0` なのに `circ_radius = 4.0` を入れており、期待半径 `sqrt(5^2 - 0^2) = 5.0` との不一致だけで `InvalidTrimCircle` になる。perp.norm の新規ガードを外してもこのテストは通るため、軸直交ずれ回帰を固定できていない。

**修正**: `circ_radius = 4.0` → `circ_radius = radius` (期待値 5.0) に変更し、`circ_center` だけが reject 要因になる形にした。

## F02 (high, 採用→修正済み): mycad_api_serial_acceptance.rs で TIME_WAIT 待機がない

**file**: `crates/mycad-api/tests/mycad_api_serial_acceptance.rs:7`

> `#[file_serial]` は同時実行は防げるが TIME_WAIT は解消しない。新規テストは即座に bind しており、別 binary 実行直後に EADDRINUSE で flaky になり得る。

**修正**: startup_log_acceptance.rs と同等の `wait_port_free(Duration::from_secs(10))` を本ファイル内に追加。

## F03 (medium, 記録のみ・将来対応): T04_strict での face name 一意性保証不足

**file**: `crates/mycad-kernel/tests/trim_sphere_circ_normal_acceptance.rs:748`

> `face.name.as_ref().map(|n| n.canonical_name()).unwrap_or_default()` で未命名 face があると `""` に潰れて `extract_face_triangles()` が複数 face を混在させる。face 名が非一意でも同様で、T04_strict が意図した 2 face 間の共有境界だけを比較している保証がない。

**現状**: `boolean_cut_sphere_dimple` の出力 face は実装上 name が一意に付与されている前提で T04 は green。

**判定**: medium で blocking=0、本バッチでは対応せず記録のみ。将来 face name が無い primitive を T04 系のターゲットに使う Issue が出てきたら別 Issue で「tessellate 前に対象 face に一時 name を付ける補助」を実装する。

---

## Round 2: 1 high (F01)

### F01 (high, 採用→修正済み): T04_strict での face name 一意性が保証されていない

- 同 round 1 の F03 を high に昇格しての再指摘。修正: T04 内で tessellate 前に `result.faces[sphere_face_idx].name` / `result.faces[adj_face_idx].name` に `EntityRef::Named("t04_strict_sphere_face" / "t04_strict_adj_face", ...)` を設定し、他 face との canonical_name 衝突を assert で固定した。

---

## Round 3: 2 high (F01 + F02)

### F01 (high, 採用→修正済み): post-rust-fmt.ts と settings.json が `/home/bacon/mycad` 絶対パスを固定している

- 修正: `post-rust-fmt.ts` で `import.meta.dir` から 4 階層上を repo root として導出する形に変更。`.claude/settings.json` の hook command も `bun .claude/skills/3ai/scripts/post-rust-fmt.ts` 相対パス化。これで任意のチェックアウト先でも動作する。

### F02 (high, partial 採用→将来別 Issue): T04 helper の twin / ring 構築が strict ではない

- 指摘: `find_twin_halfedge` は `half_edges[he_idx].twin` を使わず同 `edge` を線形探索しているため、保存済み twin index との相互参照を assert していない。`try_build_ordered_ring_mesh` は最初の閉サイクルで return するため、non-manifold や複数サイクルの共有境界でも通過してしまう。
- **判定**: #147 の本 Issue 主目的 (validation + 共有境界の基本検証) はすでに固定済みで、現状の test は `boolean_cut_sphere_dimple` の単一閉境界には十分。Codex 提案の「twin index 相互参照 assert」「ring 全消費 assert」は別 Issue で `tests/` ヘルパとして共通化して適用する候補。
- **本バッチでは対応せず記録のみ**。自律モードの skill 規定「2 回ループ後も critical = 0 なら Claude 裁量で受け切る」に従い B-6 を pass 扱いとする (critical=0、すべて high で test 厳密性向上の余地)。

---

## 2026-06-16 B-6 Phase 7 viewer-batch (#163/#164/#165/#166) — codex-crosscut-r4.yaml

verdict: **pass** | critical=0, high=0, medium=1, low=0, blocking=0

**batch_start_sha**: `9ccdbbe`

経緯:
- r1: high 2 (F01 in-flight ガード / F02 end-state 検証不足) → 採用・修正
- r2: high 1 (F01 sketchId 中間成功 orphan) → 採用、`pendingPostedSketchId` 導入
- r3: high 1 (F01 pendingPostedSketchId クリア漏れ) + medium 1 (F02 D01 mock 形式) → 採用、btn-start-sketch / Escape で `pendingPostedSketchId = null`、D01 mock `positions/normals` を `Array<[n,n,n]>` 形式に
- r4: medium 1 (F01) のみ・blocking=0 で pass

### F01 (medium) — extrude/extrude_cut 成功後の sketch-canvas data-state リセット漏れ

- ファイル: `web/src/main.ts` L271 付近
- 内容: 送信成功時に `clearSketchOverlay()` + `lastFinalizedSketch = null` + `refreshSketchButtonState()` までは呼ぶが、`updateSketchCanvasState()` を呼ばないので `sketch-canvas` の `data-state` が `"closed"` のまま残る。「送信成功でリセット」の UI 状態契約と乖離。
- 判定: medium / blocking=0。本 PR ではコード変更を行わず記録のみ。本指摘は **Phase 8 で sketch UI のリファクタ Issue を起票する際に解消する候補**。動作上の bug ではなく state 整合の品質改善項目。

---

## 2026-06-17 B-6 Phase 8 batch (#216/#217/#218) — codex-crosscut-r2.yaml

verdict: **pass** | critical=0, high=0, medium=2, low=0, blocking=0

**batch_start_sha**: `adc564e`

経緯:
- r1: high 1 (#218 T02 が tool extrusion 単独でも通る) → 採用、z range [-5, 5] を T02_topology から T02_plumbing へ移動
- r2: medium 2 (#217 T02 に validate_manifold 不足 / #217 T11 が full snapshot でない) → 採用・修正

### R1 F01 (high, 採用→修正済み): #218 T02_plumbing が boolean Cut 結果 vs tool extrusion を識別不能

- ファイル: `crates/engawa-build/tests/face_sketch_extrudecut_acceptance.rs:79`
- 内容: `bodies.len()==1` + `feature_id` + `validate_manifold` + face basis sanity (4 vertex) では tool extrusion 単独も通る。target cuboid z=[-5, 5] vs tool z=[5, 10] の区別なし
- 修正: T02_plumbing に `z_min == -5 && z_max == 5` assertion を追加 (T02_topology の冗長 assert は削除)。commit `98c2fe0`

### R2 F01 (medium, 採用→修正済み): #217 T02 が validate_manifold() を呼んでいない

- ファイル: `crates/engawa-build/tests/face_sketch_extrude_acceptance.rs:44`
- 内容: euler_poincare() のみで half-edge twin/next, loop closure, shell composition の整合は gate されない
- 修正: `extrude.solid.validate_manifold()` を T02 末尾に追加。commit `1579d85`

### R2 F02 (medium, 採用→修正済み): #217 T11 が頂点座標のみ比較で half_edges/loops/shells/names を見ていない

- ファイル: `crates/engawa-build/tests/face_sketch_extrude_acceptance.rs:551`
- 内容: 決定性が幾何だけで判定され、B-rep 接続/shell/name 順序の非決定的揺れを見逃す
- 修正: serde_json snapshot に切り替えて full Solid (vertices/half_edges/edges/loops/faces/shells/names + IDs) を比較。commit `1579d85`

### 関連 follow-up

- **#220**: kernel boolean MultipleOuterShellsResult 制限解消 (#218 の real hole drilling + euler_poincare == 0 化)
- **#218 T10**: medium F01 (codex-final-r6) で「depth=1e-6 境界が ignored のまま」と指摘済 (#220 完了後に active 化候補)

