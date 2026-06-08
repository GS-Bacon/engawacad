# #101 plan: refactor(api): AppState.doc を非公開化 — validate バイパス経路を閉じる

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|---|---|
| `state.rs`: `pub doc` → `doc`（非公開） | handler.rs 以外のモジュール追加 |
| `state.rs`: `ensure_loaded()` → `&Document`（読み取り専用）に変更 | 新規エンドポイントの追加 |
| `state.rs`: `pub(crate) fn snapshot()` 追加（`ensure_loaded` + clone） | ユーザー認証・マルチユーザー対応 |
| `state.rs`: `pub(crate) fn commit(doc: Document)` 追加 | パフォーマンス最適化 |
| `handler.rs`: `post_feature` の candidate 操作を `snapshot`/`commit` 経由に変更 | Axum State 型の変更 |
| `crates/mycad-api/tests/state_invariants.rs`: acceptance スケルトン | ファイル永続化ロジックの変更 |

## Non-Goals

- `get_mesh` / `get_features` の動作変更（ensure_loaded が &Document を返すだけで十分）
- `AppState::new` / `base_dir` の変更
- `write_atomic` の変更
- ロールバック・トランザクション機能の追加

## 実装対象

### crates/mycad-api/src/state.rs — Before / After

**フィールド公開範囲:**
```rust
// Before
pub doc: Option<Document>,

// After
doc: Option<Document>,
```

**ensure_loaded の戻り値:**
```rust
// Before
pub fn ensure_loaded(&mut self) -> Result<&mut Document, ApiError> {
    if self.doc.is_none() {
        self.doc = Some(Document::from_path(&self.path)?);
    }
    Ok(self.doc.as_mut().unwrap())
}

// After
pub fn ensure_loaded(&mut self) -> Result<&Document, ApiError> {
    if self.doc.is_none() {
        self.doc = Some(Document::from_path(&self.path)?);
    }
    Ok(self.doc.as_ref().unwrap())
}
```

**追加メソッド:**
```rust
/// ロード済み Document の clone を返す（mutation の元にする）。
pub(crate) fn snapshot(&mut self) -> Result<Document, ApiError> {
    self.ensure_loaded()?;
    Ok(self.doc.as_ref().unwrap().clone())
}

/// 検証済み Document でインメモリ状態を更新する（呼び出し側が validate を通す責務を持つ）。
pub(crate) fn commit(&mut self, doc: Document) {
    self.doc = Some(doc);
}
```

### crates/mycad-api/src/handler.rs — post_feature の変更

```rust
// Before
g.ensure_loaded()?;
let mut candidate = g.doc.as_ref().unwrap().clone();
// ...
*g.doc.as_mut().unwrap() = candidate;

// After
let mut candidate = g.snapshot()?;
// ...
g.commit(candidate);
```

## 設計方針

- `doc` を private にしても inline `#[cfg(test)] mod tests` は同モジュール内なので引き続き `state.doc` に直接アクセス可能（Rust のスコープ規則）
- `ensure_loaded()` の戻り値を `&Document` にすることで `get_mesh` / `get_features` は変更不要（これらは読み取りのみ）
- `post_feature` で `g.doc.as_ref()/.as_mut().unwrap()` を直接叩くパターンを `snapshot/commit` に集約し、validate バイパスを構造的に不可能にする
- `commit` は validate を呼ばない（呼び出し側責務）。IN01 順序不変条件はコメントで明記する

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|---|---|---|---|
| T01 | 正常系 | `snapshot()` でクローン取得 → 元の doc が変わらないこと | state 内部 doc ≠ candidate のポインタ |
| T02 | 正常系 | `commit()` で state.doc が更新されること | commit 後 snapshot が新値を返す |
| T03 | 正常系 | `ensure_loaded()` が読み取り専用 `&Document` を返すこと | コンパイル通過（mut 不要） |
| T01_degen_commit_without_load | 境界 | ロード前に `commit()` を呼んでも panic しない | `snapshot()` がその値を返す |

## 幾何的不変条件チェックリスト

N/A（B-rep 演算なし、API 状態管理のみ）
