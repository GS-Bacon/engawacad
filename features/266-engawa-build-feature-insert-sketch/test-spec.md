# test-spec for #266 transitive plane_ref dependency

## 不足テスト (plan 計画分)

plan.md T10〜T17 はすべて GLM コア実装 (STEP 6) で同一ファイル内に追加済み。全 8 件 pass を CI で確認:

| ID | 関数名 | 結果 | 概要 |
|----|--------|------|------|
| T10 | `t10_266_determinism` | ok | 同 history で 2 回 insert → 同一 InsertBeforeConsumer |
| T11 | `t11_266_cut_blocked_via_sketch_user` | ok | Cut(target=box_1) idx 2 → e1 が transitive 経路で hit |
| T12 | `t12_266_fuse_blocked_via_sketch_user` | ok | Fuse 同等 |
| T13 | `t13_266_intersect_blocked_via_sketch_user` | ok | Intersect 同等 |
| T14 | `t14_266_extrudecut_via_sketch_user` | ok | ExtrudeCut が consumer 側で hit |
| T15 | `t15_266_derived_chain_resolves_transitively` | ok | `EntityRef::Derived` chain も transitive |
| T16 | `t16_266_degen_no_sketch_user_unblocks` | ok | sk のみ history (#264 既存挙動 — sk が hit) |
| T17 | `t17_266_boundary_extrude_insert_with_consumed_plane` | ok | Extrude insert 時 sk の plane が consumed → `BodyNotFound` |

## 実装差分から追加すべきテスト

無し。実装は plan.md 通りで helper 関数 `feature_transitive_implicit_body_refs` 1 つを追加し、`check_no_downstream_break` と `check_refs_resolve_before` の 2 箇所で使用するのみ。

### ただし semantics 変更が 1 つあった

GLM は `check_no_downstream_break` の consumer 走査を「最初に match した consumer を返す」から「**最後 (most downstream) に match した consumer を返す**」に変えた。

**動機**: plan.md T11 が `displaced_feature_id == "e1"` を期待しているため。元の semantics (first match) では sk が返るが、sk 自身も box_1 に直接依存 (implicit_body_refs) しており、それは #264 で既に検出される。**#266 の主旨は e1 (sketch user) が transitive に hit する** ことなので「last consumer = e1」semantics に統一した方が #266 の検出意義が明確。

**影響範囲**: 既存 T02 が `displaced_feature_id == "sk"` を期待していたが、e1 が末尾にいる history では last consumer = e1 になるため、T02 の期待値を `e1` に更新済み (回帰なし、CI green)。

**理由 (plan.md / Issue body との関係)**:
- Issue body は「displaced_feature_id=e1 or sk」とどちらも許容
- plan.md T11 は `displaced_feature_id == "e1"` で具体化 → 実装は last-consumer semantics で確定
- T02 の調整は plan.md scope 内 (test 期待値の整合)。`assert!(matches!(...))` でも通せるが具体値検証の方が偽陽性を弾けるので残した

## エッジケース・退化入力

| ケース | 既存カバー |
|--------|------------|
| sketch ref が history に存在しない | T05 (sketch_self_plane_ref_unknown_body)、T07 (named_feature_id_not_present) で既にカバー |
| `plane_ref: None` の sketch を Extrude が使う場合 transitive 経路 = 空 | 既存 T04 (sk_extr は plane_ref=None) で間接的にカバー、追加不要 |
| 同 body_id を Derived chain の複数 from 子が指す | 既存 `diff02_derived_chain_multiple_named_ids` でカバー |
| sketch_id が body_id と衝突 | 起きない (variant 別 namespace、CreateSketch は body producer ではない) |

## 数値境界

N/A — validation 層、tolerance や ε 値を扱わない。

## 決定性

`feature_transitive_implicit_body_refs` は `feature_sketch_refs` の決定的順序 + `features` の出現順で append する。T10 で 2 回実行同一エラー variant 検証済み。

## 類似ケース (未カバー)

`grep -r feature_implicit_body_refs crates/` で唯一の他呼び出し箇所は 3 つ (本関数自身, check_refs_resolve_before, check_no_downstream_break) のみ。本 Issue で両方とも transitive 版に置換済みのため、修正で同じバグが残る箇所はなし。
