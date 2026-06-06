## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `count_naked_edges(mesh)` ヘルパー関数の実装 | 既存 naked edge 問題の修正（別 Issue） |
| 新規テストファイル `bool_naked_edge_acceptance.rs` の作成 | 既存 acceptance テストファイルの編集 |
| box−sphere void / cyl∩sph / box−sphere cut の naked_edge=0 テスト | Boolean 以外プリミティブへの naked edge チェック追加 |
| 既知 naked edge 失敗ケース（box−cylinder cut/fuse）を `#[ignore]` で文書化 | `TriangleMesh` 構造体や API の変更 |

## Non-Goals
- Boolean テッセレーションの既存バグ修正（#56 以降の残課題は別 Issue で対応）
- `mycad-kernel` の公開 API 追加
- 数値公差のチューニング

## 実装対象
- Issue: #60
- 影響クレート: `mycad-kernel`
- 新規ファイル: `crates/mycad-kernel/tests/bool_naked_edge_acceptance.rs`
- 既存ファイルへの変更: なし（guard により Claude が既存ファイルを直接編集不可）

新規ファイルに含める内容:
1. `count_naked_edges(mesh: &TriangleMesh) -> usize` — 位置ベース頂点ウェルディング後に undirected edge の出現回数が 1 の edge 数を返す
2. T01〜T04 の各テスト関数

## 設計方針
- **決定性**: テスト内で `IdGenerator::new(0)` を使い再現性を担保する
- **位置ベースウェルディング**: テッセレーションは Face 単位で独立してメッシュを生成するため頂点インデックスを共有しない。位置を `1e-10` で量子化してウェルド後に edge カウントする
- **naked edge の定義**: ウェルド後の undirected edge で出現回数が 1 のもの
- **既知の失敗ケース**: box−cylinder cut/fuse は seam vertex mismatch（#56 out-of-scope）で naked edge > 0 が既知。`#[ignore = "known seam mismatch: tracked separately"]` で文書化する
- **数値モデル**: 不要（削除）
- **derive 規約**: 新規型追加なし
- **エラーハンドリング**: テストコード内のみ、`expect()` で十分
- **workspace.dependencies 規約**: 新規依存追加なし

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01_determinism | 決定性 | box(10³)−sphere(r=3,origin) Cut を 2 回実行し mesh が byte-identical | assert_eq! |
| T02_box_sphere_void_naked_edge | 正常系 | box(10³)−sphere(r=3,origin) Cut の naked_edge = 0 | count == 0 |
| T03_cyl_sph_intersect_naked_edge | 正常系 | cylinder(r=3,h=20)∩sphere(r=4,c=(0,0,10)) の naked_edge = 0 | count == 0 |
| T04_boundary_degen_cut_cyl | 境界（known fail） | box(10³)−cylinder(r=2,h=6) Cut は naked_edge > 0 の既知問題（ignore） | #[ignore] |

## 幾何的不変条件チェックリスト
- N/A: テスト追加のみで、Boolean/Partition/Assemble の実装コードに変更なし
