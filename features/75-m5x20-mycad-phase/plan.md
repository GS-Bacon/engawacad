## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| `stdlib/fasteners/jis_b1176/M5x20.mycad` の作成（ADR-007 §4 仕様通り） | ネジ山・六角形状の精密モデリング |
| `Document::from_path` でパースできることを統合テストで確認 | 他サイズ・種類の標準部品追加 |
| `cargo xtask ci` 通過 | stdlib フォルダ規約ドキュメント化（ADR-007 で決定済み） |
| examples_smoke.rs にエントリ追加（#73 で build_assembly が使えるため） | |

## Non-Goals
- 精密なボルト形状（Phase 5 完了条件は「正しい位置に見える」まで）
- 他の標準部品の追加
- ネジ山・六角頭の形状
- M5x20.mycad に ComponentRef 参照は含まない（シンプルな 2 部品インライン定義）

## 実装対象
<!-- Issue: #75 -->
影響クレート/ファイル:
- `stdlib/fasteners/jis_b1176/M5x20.mycad` — **新規 YAML ファイル**
- `crates/mycad-build/tests/examples_smoke.rs` — M5x20.mycad のパース確認エントリを追加（または新規統合テスト）

**M5x20.mycad の内容（ADR-007 §4 準拠）:**
```yaml
schema_version: 1
version: "0.1.0"
root_component:
  name: "M5x20 Bolt (JIS B 1176)"
  features:
    - type: create_cylinder
      id: shaft
      radius: 2.5
      height: 20.0
      origin: [0.0, 0.0, 0.0]
    - type: create_cylinder
      id: head
      radius: 4.5
      height: 3.0
      origin: [0.0, 20.0, 0.0]
```

> 単位: mm。軸: 直径 5mm / 長さ 20mm。頭: 直径 9mm / 高さ 3mm（原点 Z=20）。

**examples_smoke.rs へのエントリ追加:**
既存 `smoke(yaml: &str)` ヘルパー（parse → `build_bodies_from_features`）を再利用:
```rust
#[test]
fn m5x20_bolt() {
    smoke(include_str!("../../../stdlib/fasteners/jis_b1176/M5x20.mycad"));
}
```
> `smoke()` は parse + build_bodies_from_features まで確認するため、T01/T02 をカバー。
> T03_boundary_head_origin の厳密な origin 確認は plan.md のみに記載し、GLM テスト実装で追加する。

## 設計方針
- **決定性**: YAML ファイルは静的テキストのため常に同一。パース結果の決定性は `Document` デシリアライズの実装に依存し、追加実装は不要。
- **derive 規約**: 新規 Rust 型なし（YAML のみ）。
- **エラーハンドリング**: `Document::from_yaml` でパース失敗は unwrap で検出（テストでのみ使用）。
- **workspace.dependencies**: 新規依存なし。
- **スキーマ確認**: 既存の `create_cylinder` 定義（mycad-format/src/feature.rs）が `radius`, `height`, `origin: [f64; 3]` フィールドを持つことを実装前に確認する。

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 正常系(parse) | M5x20.mycad を `Document::from_yaml` でパース | Ok, features.len() == 2 |
| T02 | 正常系(feature) | shaft の feature が CreateCylinder (radius=2.5, height=20.0) | フィールド値一致 |
| T03_boundary_head_origin | 境界 | head の origin が [0.0, 20.0, 0.0] であること | origin 値一致 |

## 幾何的不変条件チェックリスト
- [x] N/A — 本 Issue は YAML ファイル作成のみ。幾何型は `CreateCylinder` プリミティブの既存実装に委譲。
