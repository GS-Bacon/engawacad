## 修正対象
`crates/xtask/src/main.rs` の clippy エラー 2種を修正する。

## エラー 1: "very complex type" (line ~50)
Vec の型注釈が複雑すぎると判定される。

## エラー 2: "redundant closure" (line ~54-58)
`|c| Type::export_all(c)` は `Type::export_all` と等価な冗長クロージャ。

## 修正内容
`gen_ts()` 関数内の該当箇所を以下に書き換える:

変更前:
```rust
let roots: Vec<(
    &str,
    Box<dyn FnOnce(&ts_rs::Config) -> Result<(), ts_rs::ExportError>>,
)> = vec![
    ("Document", Box::new(|c| Document::export_all(c))),
    ("EntityRef", Box::new(|c| EntityRef::export_all(c))),
    ("TriangleMesh", Box::new(|c| TriangleMesh::export_all(c))),
    ("MeshRequest", Box::new(|c| MeshRequest::export_all(c))),
    ("ErrorResponse", Box::new(|c| ErrorResponse::export_all(c))),
];
```

変更後:
```rust
type ExportFn = Box<dyn FnOnce(&ts_rs::Config) -> Result<(), ts_rs::ExportError>>;
let roots: Vec<(&str, ExportFn)> = vec![
    ("Document", Box::new(Document::export_all)),
    ("EntityRef", Box::new(EntityRef::export_all)),
    ("TriangleMesh", Box::new(TriangleMesh::export_all)),
    ("MeshRequest", Box::new(MeshRequest::export_all)),
    ("ErrorResponse", Box::new(ErrorResponse::export_all)),
];
```

## 完了条件
1. 上記 Edit を適用する
2. `cargo clippy --workspace -- -D warnings` が通る
3. `cargo xtask ci` が green になる
4. 結果を glm-result.json に書き出す: { "status": "success|failed", "ci_passed": true|false, "summary": "...", "failed_reason": "..." }

## 禁止
- git commit/push は行わない
