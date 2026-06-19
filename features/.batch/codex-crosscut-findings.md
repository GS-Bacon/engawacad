# Batch B-6 Codex 横断レビュー findings (non-blocking)

## 2026-06-19 cycle (#264 + #265)

**batch_start_sha**: f745d85164cbe340b94efac3e02780be5e86d4c3
**verdict**: pass (blocking=0, medium=1, low=0)
**scope**: #264 (CreateSketch.plane_ref=Entity body-lifetime tracking) + #265 (pre-existing history broken ref defensive validate)

### F01 (medium, non-blocking) — self-reference check が implicit body refs を見ない

- 場所: `crates/engawa-build/src/feature_crud.rs:267` (check_self_reference)
- 指摘: `check_self_reference` が `feature_sketch_refs` + direct `feature_body_refs` のみ。#264 で追加した `CreateSketch.plane_ref=PlaneRef::Entity(Named/Derived → self.id)` の implicit body ref を自己参照判定に含めていない。
- 影響: plane_ref で自分自身を参照する CreateSketch は `SelfReference` ではなく後段で `BodyNotFound` として落ちる。既存の自己退化エラー契約 (`SelfReference { ref_kind: "body" }`) と新 implicit ref 経路の意味論がずれる。
- 推奨: `check_self_reference` に `feature_implicit_body_refs(f)` も通し、`plane_ref` 経由で self を参照する CreateSketch を `SelfReference` で拒否する回帰テストを追加。
- **判定**: non-blocking (medium) のため B-6 では記録のみ。#264/#266/#267 の implicit ref 系 follow-up と束ねて将来対応。

---

## post-#241 / #243 cycle

**Cycle**: post-#241 / #243
**batch_start_sha**: c5995cb8da46bee83ab79b3442f00471f0295696
**verdict**: pass (blocking=0)

## Medium findings (フォローアップ候補)

### F01 — docs/QUALITY_TOOLS.md fuzz corpus 相対パス誤り

`cd crates/engawa-format/fuzz` 直後の `cargo fuzz run from_yaml fuzz/corpus/from_yaml/` で相対パスが 1 段深く `crates/engawa-format/fuzz/fuzz/corpus/...` を参照して失敗する。

修正案: `cargo fuzz run from_yaml corpus/from_yaml/` または repo ルートから実行する前提に統一。

### F02 — xtask の Variable.ts golden 比較未追加

`xtask` の ts-rs 回帰テストが `Document.ts` / `Feature.ts` の golden 比較に留まり、新規公開 DTO `web/src/generated/Variable.ts` を検証していない。今回追加した生成ファイルが未生成・署名ドリフトしても CI で検出できない。

修正案: `Variable.ts` の golden 文字列を追加し既存生成物テストへ組み込む。

→ #241 STEP 7.5 R5 で同主旨 (M-F02 medium) が指摘済で棄却 (別 Issue 対応)。本 cross-cut でも再指摘されたため、後続 Issue として優先度を上げる候補。

## アクション

両件とも medium / 非 blocking で本バッチの merge を妨げない。本サイクルでは記録のみとし、後続 Issue (#249 などとあわせて) で集約処理する。

---

## 2026-06-19 cycle 44 (#266 + #267)

**batch_start_sha**: 2228c3479e605d04b27429d682fc4241953bcffa
**batch commits**: 8621b54 (#266 feat), 78be9aa (#266 sweep), c3d04b1 (#267 feat), 9d75961 (#267 sweep)
**verdict**: fail (blocking=1, high=1, medium=1, low=0)

### F01 (high — defer to #269)

**指摘**: `refs_resolve_in_state` は CreateSketch の `plane_ref` だけを見ており、Extrude/ExtrudeCut は sketch ID が残っている限り executed 扱いのまま。`[box_1, sk(plane=box_1), Cut(box_1,...), e1(sketch=sk)]` のように sketch 作成後に plane_ref の body が dead になる history でも e1 が executed_at_* に残る。

**判断**: #267 で実装した CreateSketch self check (plane_ref body が live なら sk 実行) と、Extrude/ExtrudeCut が **使う側** での transitive 反映は独立した修正。#268 spec の残り半分。**#269 で起票して next cycle で消化**。

**autonomous mode 規約適合性**: B-6 spec 「2 回ループ後も critical ≥ 1 が残る場合のみユーザーにエスカレーション; critical = 0 なら Claude 裁量で受け切る」に従う。本 batch では critical=0 / high=1 のため受け切り可。loop policy `project-3ailoop-policy` の "他 Issue を block しない" 方針とも整合。

### F02 (medium — findings only)

**指摘**: T11-T15 (#266) の `assert!(["sk", "e1"].contains(&displaced_feature_id))` で sk と e1 のどちらでも通るため、transitive sketch-user 検出 (e1 hit) を強制できていない。T14 も同様。

**判断**: 改善余地はあるが medium / non-blocking。fix-it Issue として next cycle 以降に起票するかどうかは F01 修正 (#269) の副作用で T14 fixture が変わる可能性もあるため、#269 完了後に再判断する。本 batch では受容。

---

## 2026-06-19 cycle 45 (#269)

**batch_start_sha**: af5c5c88ec9342953d70a6a71f369956eb8b454a
**batch commits**: 6fa7eef (#269 feat squash), a6ac470 (finalize), bc2912c (B-6 r1 fix: success-path determinism + minimal transitive check), 4e85f03 (B-6 r2 fix: rename to test_269_*)
**rounds**: r1 verdict=fail (critical=1, high=1), r2 verdict=fail (critical=1), r3 verdict=pass (blocking=0, medium=2, low=1)

### r3 残 medium/low (記録のみ、non-blocking)

#### F01 (medium) — test-summary.json が stale で committed

- 指摘: feature folder の `test-summary.json` (committed snapshot) が `total_added: 0` / `determinism: 0` のまま。実際には r2 fix 後の再生成では `total_added=2, determinism=1, boundary=1` になっている。
- 判断: B-7 reconciliation で sweep 時に最新版が commit される予定。記録のみ。

#### F02 (medium) — t_269_edge_roundtrip_yaml_serialize_deserialize の検証が浅い

- 場所: `crates/engawa-build/tests/feature_crud_prefix_validate_acceptance.rs:1742`
- 指摘: GLM が追加した YAML roundtrip テストが CreateBox 1件目のみ検証しており、本 Issue の主題 (CreateSketch.plane_ref/profile/offset) 経路を検証していない。
- 判断: Issue scope 越境 (engawa-build acceptance に YAML serialize 検証を入れたこと自体が C-F02/M-F02 でも指摘済)。後続 Issue で format 側に移動する判断と連動。本 batch では受容。

#### F03 (low) — EDGE_269_* ブロックの scope 越境

- 場所: `crates/engawa-build/tests/feature_crud_prefix_validate_acceptance.rs:1803`
- 指摘: NaN/Inf/empty id/negative zero の serialize 検証が engawa-format 仕様への結合を engawa-build acceptance に持ち込んでいる。
- 判断: F02 と同根。format 側へ移動の follow-up Issue 起票候補として記録。本 batch では受容。

### auto-fix loop 経過

- **F01 (r1 critical) 採用 + 修正済**: slug-matched ファイルに 2 件 #[test] 追加 → r2 で coverage_hints heuristic 名前不一致が判明 → r2 で rename → r3 で解消。
- **F02 (r1 high) 採用 + 修正済**: success-path determinism test (byte-equal YAML 比較) を `test_269_success_path_determinism_byte_equal` として追加 → r3 で消化済。
- **F01 (r2 critical) 採用 + 修正済**: extractor regex `(?:test_|t\d+_)` に合うよう `t_269_*` → `test_269_*` リネーム → r3 で解消。

### autonomous mode 規約適合性

B-6 spec「4. 再レビューで blocking == 0 になれば完了」を満たす。r1 → r2 → r3 で blocking 解消。loop policy 通り。
