# mycad-format — .mycad ファイルフォーマット

## Overview

`.mycad` ファイルはYAML形式。`Document` がトップレベルで、`root_component` にComponent階層を持つ。各Componentは `features` (操作履歴) と `children` (子コンポーネント) を持てる。

## File Format Example

```yaml
version: "0.1.0"
root_component:
  name: "My Part"
  features:
    - type: create_box
      id: box_1
      width: 10.0
      height: 20.0
      depth: 30.0
```

## How to Add a New Feature Type

1. `feature.rs` の `Feature` enum に新バリアントを追加
2. `#[serde(rename = "snake_case_name")]` を付ける
3. 必ず `id: String` フィールドを含める
4. `Feature::id()` の match arm を更新
5. テストを追加: シリアライズ → デシリアライズのラウンドトリップ

## Feature → Kernel Mapping

| Feature variant | Kernel function |
|----------------|----------------|
| `CreateBox`    | `make_cuboid`  |
| `CreateCylinder` | (未実装)    |
| `CreateSphere` | (未実装)      |
| `Extrude`      | (未実装)      |
| `Cut`          | (未実装)      |
| `Fuse`         | (未実装)      |
| `Intersect`    | (未実装)      |

## Conventions

- シリアライズは決定的でなければならない（同じDocumentを2回シリアライズして同一結果）
- `#[serde(skip_serializing_if = ...)]` でデフォルト値を省略し、YAMLを簡潔に保つ
- `indexmap` を使用して挿入順序を保持（将来のMap型フィールド用）
