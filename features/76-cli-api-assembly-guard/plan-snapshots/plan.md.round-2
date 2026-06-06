## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| `mycad-cli/src/main.rs` の拒否ガード削除と `build_assembly` 配線 | 回転 transform 対応 (#77) |
| `mycad-api/src/handler.rs` の同様の変更 | ビューアの目視確認 (#78) |
| CLI export テスト: `examples/assembly.mycad` が exit 0 で STL 出力 | 複数 Component のソリッド識別子管理 |
| API テスト: `triangles` 非空 + HTTP 200 の統合テスト | |
| `cargo xtask ci` 通過 | |

## Non-Goals
- 回転 transform の対応（#77 で実施）
- ビューアの目視確認（#78 close-gate で実施）
- `empty features` ガードの変更（assembly では意味が変わるため `bodies.is_empty()` 判定に切り替える）

## 実装対象
<!-- Issue: #76 -->
影響クレート/ファイル:
- `crates/mycad-cli/src/main.rs` — `run_export` 関数の拒否ガード削除 + `build_assembly` 配線
- `crates/mycad-api/src/handler.rs` — `get_mesh` の拒否ガード削除 + `build_assembly` 配線

### mycad-cli/src/main.rs の変更

**before:**
```rust
use mycad_build::build_bodies_from_features;
// run_export 内:
    let root = &doc.root_component;
    if root.reference.is_some() || !root.children.is_empty() {
        return Err("assembly/reference documents are not supported".to_string());
    }
    let mut gen = IdGenerator::new(0);
    let bodies = build_bodies_from_features(&root.features, &mut gen)
        .map_err(|e| format!("failed to build solid: {e}"))?;
    let meshes: Vec<_> = bodies
        .live()
        .map(|b| tessellate_solid_with(&b.solid, &opts))
        .collect::<Result<_, _>>()
        .map_err(|e| format!("failed to tessellate: {e}"))?;
```

**after:**
```rust
use mycad_build::build_assembly;
// run_export 内:
    let base_dir = input.parent().unwrap_or(std::path::Path::new("."));
    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, base_dir, &mut gen)
        .map_err(|e| format!("failed to build assembly: {e}"))?;
    let meshes: Vec<_> = bodies
        .iter()
        .map(|b| tessellate_solid_with(&b.solid, &opts))
        .collect::<Result<_, _>>()
        .map_err(|e| format!("failed to tessellate: {e}"))?;
```

### mycad-api/src/handler.rs の変更

**before:**
```rust
use mycad_build::build_bodies_from_features;
// get_mesh 内:
    let root = &doc.root_component;
    if root.reference.is_some() || !root.children.is_empty() {
        return Err(ApiError::Unprocessable(
            "assembly/reference documents are not supported in v0".to_string(),
        ));
    }
    if root.features.is_empty() {
        return Err(ApiError::Unprocessable(
            "empty part: document has no features".to_string(),
        ));
    }
    let mut gen = IdGenerator::new(0);
    let bodies = build_bodies_from_features(&root.features, &mut gen)?;
    let out: Vec<BodyMesh> = bodies
        .live()
        .map(|b| -> Result<BodyMesh, TessellationError> { ... })
        .collect::<Result<_, _>>()?;
```

**after:**
```rust
use mycad_build::build_assembly;
// get_mesh 内:
    let base_dir = file.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| std::path::PathBuf::from("."));
    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, &base_dir, &mut gen)?;
    if bodies.is_empty() {
        return Err(ApiError::Unprocessable(
            "empty assembly: no bodies built".to_string(),
        ));
    }
    let out: Vec<BodyMesh> = bodies
        .iter()
        .map(|b| -> Result<BodyMesh, TessellationError> {
            Ok(BodyMesh { feature_id: b.feature_id.clone(), mesh: tessellate_solid_with(&b.solid, &V0_TESSELLATION)? })
        })
        .collect::<Result<_, _>>()?;
```

> `KernelError → ApiError` の From impl は既存済み（`crates/mycad-api/src/error.rs` 確認済み）。

## 設計方針
- **決定性**: `build_assembly` 内で `IdGenerator::new(0)` を使用。同一入力 → 同一 EntityID。
- **base_dir**: `input.parent()` / `file.parent()` が None の場合は `.` にフォールバック。
- **import 整理**: CLI は `build_bodies_from_features` → `build_assembly` に差し替え。API も同様。
- **workspace.dependencies**: 新規依存なし（`mycad_build` は両クレートで既使用）。

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | `examples/assembly.mycad` を 2 回 export、STL バイト列が同一 | 完全一致 |
| T02 | 正常系(CLI) | `examples/assembly.mycad` を export → STL が非空 | exit 0, STL bytes > 0 |
| T03 | 正常系(API) | `examples/assembly.mycad` を API get_mesh → HTTP 200, bodies 非空 | 200, `out.len() > 0` |
| T04_boundary_empty_assembly | 境界 | features/children/reference すべて空の Document を API に渡す → 422 | `bodies.is_empty()` ガード動作 |
| T05_boundary_simple_part | 境界 | features のみ（children なし）の通常 .mycad も引き続き動作する | exit 0, STL bytes > 0 |

## 幾何的不変条件チェックリスト
- [x] N/A — 本 Issue はパイプライン配線のみ。幾何演算なし。transform 合成は #74 で検証済み。
