# xtask から MeshRequest 参照を削除

## 問題

`mycad-api` から `MeshRequest` 型が削除されたが、
`crates/xtask/src/main.rs` がまだ `MeshRequest` を import・使用しているためビルドが失敗する。

## 修正

`crates/xtask/src/main.rs` の以下を全て削除・修正する:

1. `use mycad_api::{ErrorResponse, MeshRequest};` → `use mycad_api::ErrorResponse;`
   (ファイル内に2箇所ある場合は両方)

2. gen_ts の export リストから `MeshRequest` の行を削除:
   ```rust
   ("MeshRequest", Box::new(MeshRequest::export_all)),  // 削除
   ```

3. テスト内の `MeshRequest::export_all` 呼び出しを全て削除

4. テスト内の `"MeshRequest.ts"` を検証しているアサーションを全て削除
   (ファイル存在確認、内容比較、goldenテスト等)

5. `web/src/generated/MeshRequest.ts` ファイルが存在する場合は削除する
   (git rm または fs::remove_file でよい)

## 完了条件

`cargo xtask ci` が green になること。
