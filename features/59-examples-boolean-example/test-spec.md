# Test Spec — Issue #59 examples-boolean-example

## 不足テスト（plan 計画分）
T01 golden_roundtrip: crates/mycad-format/tests/golden_examples.rs の以下 5 テストを新 YAML に合わせて更新。

## GLM への具体的指示

`crates/mycad-format/tests/golden_examples.rs` の以下の関数を更新すること。

### golden_boolean_box_fuse
```rust
concat!(
    "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Boolean Box Fuse\n  features:\n",
    "  - type: create_box\n    id: box_a\n    width: 6.0\n    height: 2.0\n    depth: 2.0\n",
    "  - type: create_box\n    id: box_b\n    width: 2.0\n    height: 6.0\n    depth: 2.0\n",
    "  - type: fuse\n    id: fuse1\n    target: box_a\n    tool: box_b\n",
)
```

### golden_boolean_box_cut
```rust
concat!(
    "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Boolean Box Cut\n  features:\n",
    "  - type: create_box\n    id: target\n    width: 4.0\n    height: 4.0\n    depth: 4.0\n",
    "  - type: create_cylinder\n    id: tool\n    radius: 1.0\n    height: 6.0\n    origin:\n    - 0.0\n    - 0.0\n    - -3.0\n",
    "  - type: cut\n    id: cut1\n    target: target\n    tool: tool\n",
)
```

### golden_boolean_box_intersect
```rust
concat!(
    "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Boolean Box Intersect\n  features:\n",
    "  - type: create_box\n    id: box_a\n    width: 4.0\n    height: 4.0\n    depth: 2.0\n",
    "  - type: create_cylinder\n    id: cyl_b\n    radius: 1.5\n    height: 6.0\n    origin:\n    - 0.0\n    - 0.0\n    - -3.0\n",
    "  - type: intersect\n    id: int1\n    target: box_a\n    tool: cyl_b\n",
)
```

### golden_boolean_box_void
```rust
concat!(
    "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Boolean Box Void Shell\n  features:\n",
    "  - type: create_box\n    id: outer\n    width: 6.0\n    height: 6.0\n    depth: 6.0\n",
    "  - type: create_sphere\n    id: inner\n    radius: 2.0\n    center:\n    - 0.0\n    - 0.0\n    - 3.5\n",
    "  - type: cut\n    id: cut1\n    target: outer\n    tool: inner\n",
)
```

### golden_boolean_cut_sphere_dimple
```rust
concat!(
    "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Box Cut Sphere (Dimple)\n  features:\n",
    "  - type: create_box\n    id: box1\n    width: 10.0\n    height: 10.0\n    depth: 10.0\n",
    "  - type: create_sphere\n    id: sphere1\n    radius: 3.0\n    center:\n    - 0.0\n    - 0.0\n    - 6.0\n",
    "  - type: cut\n    id: cut1\n    target: box1\n    tool: sphere1\n",
)
```

## 実装差分から追加すべきテスト
なし。変更は YAML データと golden 文字列のみ。

## エッジケース・退化入力
T03_degen_boundary: boolean_box_cut の cylinder が box を完全貫通（z方向にはみ出す）→ smoke テストで確認済み。

## 決定性
T01: golden round-trip で決定性を保証（YAML は静的）。
