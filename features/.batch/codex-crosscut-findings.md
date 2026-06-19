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
