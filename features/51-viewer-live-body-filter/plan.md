## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|---|---|
| `mesh_api.rs` に Boolean example の live-body フィルタ回帰テストを追加 | handler.rs の変更（修正は #35 で完了済み、触らない） |
| Boolean cut 結果が API で 1 body のみ返ることを検証 | viewer フロントエンド（TS/Three.js）の変更 |
| | 他の Boolean op（fuse/intersect）の網羅テスト（cut 1 本で経路を保証できる） |

## Non-Goals
- production コード（`crates/**/src/`）の変更は行わない。修正は #35 で完了済み。
- Boolean 全 op・全 example の API テスト網羅はしない（handler.rs の live() 経路を 1 本で保証）。
- viewer フロント（消費済み body の描画ロジック）の検証はしない。

## 実装対象
<!-- Issue: #51 -->
<!-- 影響クレート: mycad-api (tests のみ) -->

**追加ファイル**: `crates/mycad-api/tests/mesh_api.rs`（末尾に T19 を追記）

新規追加のみ（既存関数の修正なし）。before/after スニペット不要。

追加するテスト:
```rust
// T19: Boolean cut — API returns only the live result body (consumed bodies excluded).
// Regression guard for #51 (handler.rs was using bodies.all() instead of bodies.live()).
#[tokio::test]
async fn t19_boolean_cut_live_bodies_only() {
    let file = example_path("boolean_box_cut.mycad");
    let (status, body) = send_mesh_request(file).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let bodies: Vec<BodyMesh> = serde_json::from_str(&body).unwrap();
    // boolean_box_cut.mycad: target + tool + cut1 → consumed=2, live=1
    assert_eq!(bodies.len(), 1, "only the live result body should be returned");
    assert_eq!(bodies[0].feature_id, "cut1");
    assert!(!bodies[0].mesh.positions.is_empty(), "result mesh non-empty");
}
```

## 設計方針

- **決定性**: 変更なし（テスト追加のみ）。
- **B-rep トポロジー妥当性**: 変更なし。
- **退化幾何の扱い**: 変更なし。
- **derive/workspace.dependencies**: 変更なし。
- **エラーハンドリング**: 変更なし。
- **回帰検出力**: handler.rs が `all()` に戻ると len==3 になり本テストが fail する。

### 数値モデル
- 該当なし（body 数の整数比較のみ）。

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T19 | 回帰 | boolean_box_cut.mycad を API 経由でリクエスト、live body のみ返ることを検証 | status==200, len==1, feature_id=="cut1", mesh 非空 |

## 幾何的不変条件チェックリスト
- N/A（テスト追加のみ。B-rep/partition/assemble に触れない）
