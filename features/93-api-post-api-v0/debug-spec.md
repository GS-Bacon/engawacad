# Debug Spec — #93 Codex round 2 指摘

## Codex F01 (high): ensure_loaded の外部ファイル更新非検知が未ドキュメント

### 仮説
`AppState::ensure_loaded` は初回ロード後にキャッシュを返す。外部プロセスが `.mycad` を書き換えても
サーバは気づかない。この制約自体は ADR-008 §Decision 3「`mycad view` はシングルユーザーサーバのため
競合問題なし」で明示的に許容済みだが、コードに記載がなく Codex が懸念。

### 修正方針
`crates/mycad-api/src/state.rs` の `ensure_loaded` に ADR-008 根拠コメントを追記する（挙動変更なし、
ドキュメント修正のみ）:

```rust
/// doc 未ロードなら from_path で読み込む。読込済みなら参照を返す。
///
/// # キャッシュ戦略
/// 初回ロード後はメモリ常駐（再読み込みなし）。
/// ADR-008 §Decision 3「mycad view はシングルユーザーサーバのため競合問題は発生しない」に基づき、
/// 外部プロセスによる .mycad の並行更新は非サポートシナリオとして明示的に除外する。
pub fn ensure_loaded(&mut self) -> Result<&mut Document, ApiError> {
```

## Codex F02 (medium): Json<Feature> rejection が ErrorResponse JSON でなく plain-text

### 仮説
axum の `Json<T>` エクストラクタはデシリアライズ失敗時に `JsonRejection` を返す。`JsonRejection` は
`IntoResponse` を実装するが、`ErrorResponse { error }` JSON ではなく axum 既定の plain-text を返す。
これで `/api/v0/features` のエラーレスポンス形式が `/api/v0/mesh` と不整合になる。

### 修正方針
`crates/mycad-api/src/handler.rs` の `post_feature` で `Json<Feature>` エクストラクタを使う代わりに
`JsonRejection` を捕捉する:

```rust
use axum::extract::rejection::JsonRejection;

pub(crate) async fn post_feature(
    State(state): State<SharedState>,
    feature_result: Result<Json<Feature>, JsonRejection>,
) -> Result<Json<Vec<BodyMesh>>, ApiError> {
    let Json(feature) = feature_result.map_err(|e| ApiError::Unprocessable(e.body_text()))?;
    // ... 以降は同じ
```

これで JSON 解析失敗時も `ApiError::Unprocessable` → 422 + `{"error": "..."}` JSON を返す。

## Codex F03 (medium): E03/E04 テストが 500 も許容している

### 仮説
`edge_zero_width_box_returns_error` / `edge_negative_dimension_box_returns_error` で
`status == UNPROCESSABLE_ENTITY || status == INTERNAL_SERVER_ERROR` を許容している。
しかし `make_cuboid` でゼロ幅・負次元は `KernelError` を返し、それは既存の
`From<KernelError> for ApiError` が `ApiError::Unprocessable` → 422 に写像する。
500 が返ることはありえないので、許容は過剰でテストの弱体化。

### 修正方針
E03/E04 を `assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY)` のみに絞る:

```rust
// E03: zero-width box
assert_eq!(
    status, StatusCode::UNPROCESSABLE_ENTITY,
    "zero-width box must return 422"
);

// E04: negative dimension
assert_eq!(
    status, StatusCode::UNPROCESSABLE_ENTITY,
    "negative dimension must return 422"
);
```

実際に `make_cuboid(0.0, ...)` が `KernelError` を返すかを `cargo test` で確認後、
`assert_eq!` に変更すること。もし `make_cuboid(0.0,...)` が `Ok(solid)` を返し
tessellation で失敗する場合も `TessellationError` → 422 の経路が存在するため、
いずれにせよ 422 が保証される。

## 試した修正と結果
- (まだなし — これが初回 debug-spec)

## 次にやること
1. F01: `state.rs` に ADR-008 根拠コメントを追記
2. F02: `handler.rs` の `post_feature` で `JsonRejection` を捕捉し `ApiError::Unprocessable` に変換
3. F03: E03/E04 テストを `assert_eq!` UNPROCESSABLE_ENTITY のみに変更
4. `cargo xtask ci` green を確認
