## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `crates/mycad-api/src/handler.rs:37` の `bodies.all()` を `bodies.live()` に変更 | その他の変更一切 |

## Non-Goals

- 他のファイルへの変更なし

## 実装対象

**ファイル**: `crates/mycad-api/src/handler.rs`

**変更箇所** (line 36-38):

before:
```rust
    let out: Vec<BodyMesh> = bodies
        .all()
        .iter()
```

after:
```rust
    let out: Vec<BodyMesh> = bodies
        .live()
        .iter()
```

`BodyStore::live()` は `crates/mycad-build/src/lib.rs:37-40` で定義済み。
`BodyStore::all()` の代わりに `live()` を使うことで consumed な body を除外する。

## 設計方針

1 行の変更のみ。他のコード・テストへの影響なし。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | ビルド | `cargo build --workspace` が通る | exit 0 |
| T02 | CI | `cargo xtask ci` が通る | exit 0 |
