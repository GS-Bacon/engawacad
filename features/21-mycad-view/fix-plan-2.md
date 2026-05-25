# F01 + F03 修正

## F01(High): /api/v0/<未知パス> が static fallback を継承する問題

### 問題
`crates/mycad-api/src/router.rs` で `.nest("/api/v0", ...)` に明示的 fallback がないため、
Axum の仕様上 `/api/v0/<未知パス>` が外側の `.fallback(static_handler)` を継承し、
`index.html` 200 を返してしまう。API 契約上 `/api/v0/` 配下の未知パスは JSON 404 であるべき。

### 修正
`/api/v0` ネスト用 Router に明示的な 404 JSON fallback を追加する:

```rust
Router::new()
    .route("/mesh", get(get_mesh))
    .fallback(|| async {
        (
            StatusCode::NOT_FOUND,
            axum::Json(crate::ErrorResponse { error: "not found".to_string() }),
        )
    })
```

外側の `app()` Router は `.fallback(static_handler)` を持つが、
ネスト内の fallback が優先されるため `/api/v0/unknown` は JSON 404 を返す。

### テスト追加
`crates/mycad-api/tests/` に以下を追加(既存 `mesh_api.rs` か新ファイル):
- `GET /api/v0/unknown` → 404, content-type `application/json`
- `GET /api/v0/mesh/` → 404, content-type `application/json`

## F03(Low): features/20-ts-frontend-skeleton/state.json の差分除去

### 問題
GLM が前のパスで `features/20-ts-frontend-skeleton/state.json` に `merge: passed` を
書き込んでしまい、Issue 21 の差分に無関係なメタデータが混入している。

### 修正
`features/20-ts-frontend-skeleton/state.json` を元の状態に戻す。
現在リポジトリの HEAD(claude/add-claude-guidelines-BKKtD ブランチ)での内容:
```bash
git show claude/add-claude-guidelines-BKKtD:features/20-ts-frontend-skeleton/state.json
```
の出力に書き戻す。

## 修正後
`cargo xtask ci` が green であること。
