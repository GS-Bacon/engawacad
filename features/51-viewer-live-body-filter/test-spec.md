# test-spec.md — Issue #51 live-body フィルタ回帰テスト

## 不足テスト（plan 計画分）

T19 は GLM がコアモードで実装済み（`#[ignore]` 解除済み）。
→ **plan 計画のテストは全て実装済み。追加不要。**

## 実装差分から追加すべきテスト

変更差分は `mesh_api.rs` への T19 追加のみ。production コードの変更なし。
追加実装を要するテストなし。

## エッジケース・退化入力

| ケース | 対処状況 |
|--------|----------|
| Fuse / Intersect も live のみ返す | handler.rs は op 種別に依存しない。`all()→live()` は Boolean op 全体に適用。T19 (cut) で経路を保証できる。追加テスト不要（out-of-scope 明記済み）。 |
| Boolean 後に複数 live body が残るケース | two_bodies.mycad（既存 T12）でカバー。追加不要。 |

## 数値境界

- 該当なし（body 数の整数比較のみ）。

## 決定性

- 既存 T03 / T14 でカバー。追加不要。

## 結論

追加 GLM 作業は不要。STEP 6.6 は空振り確認として dispatch する。
