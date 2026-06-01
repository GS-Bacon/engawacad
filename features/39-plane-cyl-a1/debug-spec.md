# Debug Spec — MissingEntityName (test_impl ラウンド)

## 現在の CI 失敗状況

A1 acceptance tests が全て `MissingEntityName` で fail している。

```
thread 'a1_build_manifold_euler' panicked at crates/mycad-build/tests/feature_dispatcher.rs:1785:43:
A1: build should succeed: MissingEntityName
```

---

## 根本原因

`boolean()` 入力バリデーション (`validate_boolean_input` @ booleans/mod.rs:82-95) が、
入力 Solid の全 face/edge/vertex に name が必要と検査する。

`make_cuboid` と `make_sphere` は `fid = "cuboid"` / `fid = "sphere"` でエンティティに名前を付けるが、
**`make_cylinder` だけが全エンティティに `name: None` を渡している**。

そのため `boolean(box_solid, cyl_solid, Cut)` の呼び出し時に
`validate_boolean_input(tool="cyl_solid")` が `MissingEntityName` を返す。

---

## 修正場所

### `crates/mycad-kernel/src/primitives/cylinder.rs`

`make_cylinder` の先頭に `let fid = "cylinder";` を追加し、
各エンティティに `EntityRef::try_named(fid, kind, role).ok()` を渡す。

```rust
use mycad_format::{EntityKind, EntityRef};  // ← use 文を追加

pub fn make_cylinder(radius: f64, height: f64, id_gen: &mut IdGenerator) -> Result<Solid, KernelError> {
    // ... 既存バリデーション ...

    let fid = "cylinder";  // ← 追加
    let mut solid = Solid::new(id_gen.next());

    let v_bot = solid.add_vertex(
        id_gen.next(),
        Point::new(0.0, radius, 0.0),
        EntityRef::try_named(fid, EntityKind::Vertex, "seam_bot").ok(),  // ← None → named
    );
    let v_top = solid.add_vertex(
        id_gen.next(),
        Point::new(0.0, radius, height),
        EntityRef::try_named(fid, EntityKind::Vertex, "seam_top").ok(),  // ← None → named
    );

    let e_bot = solid.add_edge(
        id_gen.next(),
        [v_bot, v_bot],
        Curve::Circle { ... },
        [0.0, 2.0 * std::f64::consts::PI],
        EntityRef::try_named(fid, EntityKind::Edge, "bot_circle").ok(),  // ← None → named
    );

    let e_top = solid.add_edge(
        id_gen.next(),
        [v_top, v_top],
        Curve::Circle { ... },
        [0.0, 2.0 * std::f64::consts::PI],
        EntityRef::try_named(fid, EntityKind::Edge, "top_circle").ok(),  // ← None → named
    );

    let e_seam = solid.add_edge(
        id_gen.next(),
        [v_bot, v_top],
        Curve::Line { ... },
        [0.0, 1.0],
        EntityRef::try_named(fid, EntityKind::Edge, "seam").ok(),  // ← None → named
    );

    // f_bot:
    let f_bot = solid.add_face(
        id_gen.next(), surface, loop_bot, vec![], true,
        EntityRef::try_named(fid, EntityKind::Face, "bot_face").ok(),  // ← None → named
    );

    // f_top:
    let f_top = solid.add_face(
        id_gen.next(), surface, loop_top, vec![], true,
        EntityRef::try_named(fid, EntityKind::Face, "top_face").ok(),  // ← None → named
    );

    // f_lat:
    let f_lat = solid.add_face(
        id_gen.next(), surface, loop_lat, vec![], true,
        EntityRef::try_named(fid, EntityKind::Face, "lat_face").ok(),  // ← None → named
    );
```

`EntityRef::try_named(fid, EntityKind::Vertex/Edge/Face, role)` は sphere.rs や cuboid.rs で使われているパターンと同じ。

---

## 既存テストへの影響

`test_cylinder_determinism` / `test_cylinder_topology` など既存テストは通る想定。
既存テストで `assert!(e.name.is_none())` のような名前の「無い」チェックは cylinder.rs 内に存在しない。

ただし修正後に `test_cylinder_entity_names` テスト (未実装) を追加して
全 entity に名前があることを assert すること:

```rust
#[test]
fn test_cylinder_entity_names() {
    let mut gen = IdGenerator::new(0);
    let s = make_cylinder(2.0, 6.0, &mut gen).unwrap();
    for v in &s.vertices { assert!(v.name.is_some(), "vertex missing name"); }
    for e in &s.edges    { assert!(e.name.is_some(), "edge missing name"); }
    for f in &s.faces    { assert!(f.name.is_some(), "face missing name"); }
}
```

---

## A1 tests の example ファイル確認

`examples/boolean_cut_cylinder_hole.mycad` が存在することを確認し、なければ作成:

```yaml
schema_version: 1
version: "0.1.0"
root_component:
  name: "Box Cut Cylinder (Blind Hole)"
  features:
    - type: create_box
      id: box1
      width: 10.0
      height: 10.0
      depth: 10.0
    - type: create_cylinder
      id: cyl1
      radius: 2.0
      height: 6.0
    - type: cut
      id: cut1
      target: box1
      tool: cyl1
```

---

## t05_cut_now_supported の更新

`t05_cut_now_supported` のコメントに「may fail with internal error」とあるが、
修正後は `result.expect("cylinder cut should now succeed")` に書き換える。
ただし `a1_build_manifold_euler` が pass すれば t05 は自動的に pass するので、
t05 の書き換えは lower priority。

---

## 修正後の確認手順

```bash
cargo xtask ci
```

全テスト (a1_build_manifold_euler, a1_determinism, a1_intersection_edge_is_circle,
a1_tessellation_succeeds, a1_stl_export_succeeds, a1_top_face_has_inner_loop) が PASS になること。

---

## 注意

- `crates/**` のみ変更する (cylinder.rs, feature_dispatcher.rs テスト内)
- `make_cylinder` の既存シグネチャは変更しない
- `EntityRef::try_named(fid, kind, role)` は sphere.rs のパターンを踏襲すること
