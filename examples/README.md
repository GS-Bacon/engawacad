# Examples

`.engawa` (YAML) サンプル集。各ファイルは `engawa export` で STL 化、または `engawa view` でブラウザ 3D 表示できる。

```bash
# STL に変換
./target/release/engawa export examples/<file>.engawa -o out.stl

# ブラウザでリアルタイム 3D 表示
./target/release/engawa view examples/<file>.engawa
```

## Phase 7 代表: スケッチを引いて押し出す

[`sketch_via_refplane.engawa`](sketch_via_refplane.engawa) — Phase 7 で完成した「スケッチ→押出」の最小例。Front 参照平面 (xy) 上に 10×5 の矩形プロファイルを描き、深さ 8 で押し出している。

```bash
./target/release/engawa view examples/sketch_via_refplane.engawa
```

ブラウザ上で参照平面の選択 → スケッチキャンバスでの線分描画 → 押出までを対話で行いたい場合は、`simple_box.engawa` などを開いた状態で UI 側の Sketch / Extrude パネルを操作する。

## スケッチ + 押出 (Phase 3 / 7)

| ファイル | 何を見せるか |
| --- | --- |
| [`sketch_via_refplane.engawa`](sketch_via_refplane.engawa) | Phase 7 代表。`create_sketch` + `plane_ref: Front` + `extrude` |
| [`extruded_rect.engawa`](extruded_rect.engawa) | Phase 3 由来の素朴な矩形押出 |
| [`two_bodies.engawa`](two_bodies.engawa) | スケッチ 2 枚 × 押出 2 回で独立した 2 ボディを作る |

## プリミティブ (Phase 0 / 3)

| ファイル | 何を見せるか |
| --- | --- |
| [`simple_box.engawa`](simple_box.engawa) | 最小例。`create_box` のみ |
| [`cylinder.engawa`](cylinder.engawa) | `create_cylinder` |
| [`sphere.engawa`](sphere.engawa) | `create_sphere` |
| [`cylinder_offset.engawa`](cylinder_offset.engawa) | 原点からオフセットした円柱 |
| [`sphere_offset.engawa`](sphere_offset.engawa) | 原点からオフセットした球 |

## Boolean 演算 (Phase 4)

| ファイル | 何を見せるか |
| --- | --- |
| [`boolean_box_cut.engawa`](boolean_box_cut.engawa) | Box から Cylinder を `cut` |
| [`boolean_box_fuse.engawa`](boolean_box_fuse.engawa) | Box ＋ Box を `fuse` |
| [`boolean_box_intersect.engawa`](boolean_box_intersect.engawa) | Box ∩ Cylinder の `intersect` |
| [`boolean_box_void.engawa`](boolean_box_void.engawa) | Box の中に球を `cut` してボイドを作る |
| [`boolean_cut_cylinder_hole.engawa`](boolean_cut_cylinder_hole.engawa) | Box に円柱穴を貫通 |
| [`boolean_cut_sphere_dimple.engawa`](boolean_cut_sphere_dimple.engawa) | Box に球の窪み (ディンプル) |
| [`boolean_fuse_box_cyl.engawa`](boolean_fuse_box_cyl.engawa) | Box ＋ Cylinder を `fuse` |
| [`boolean_intersect_box_cyl.engawa`](boolean_intersect_box_cyl.engawa) | Box ∩ Cylinder |
| [`boolean_intersect_cyl_sphere.engawa`](boolean_intersect_cyl_sphere.engawa) | Cylinder ∩ Sphere |

## アセンブリ (Phase 5)

| ファイル | 何を見せるか |
| --- | --- |
| [`assembly.engawa`](assembly.engawa) | 子コンポーネント階層 + `stdlib://` 部品参照 (M5 ボルト) |

## Phase と機能の対応

各 Phase で何が増えたかは [`ROADMAP.md`](../ROADMAP.md) を参照。サンプルは Phase が積み上がる順 (プリミティブ → 押出 → Boolean → アセンブリ → スケッチ対話) に試すと、`.engawa` フォーマットでできることが体感しやすい。
