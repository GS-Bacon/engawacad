## 自律判断ログ (B-3 front-load)

- **scope 決定**: 親 Issue 本文は「ADR-005 本格運用」として全 primitives (cuboid/cylinder/sphere/extrusion) の feature_id 連動を示唆するが、**本 Issue は cuboid のみ**に絞る。理由: (1) 親 #215 で実害が出たのは `make_cuboid` 由来の Face name のみ。(2) ADR-006 §1 粒度ガード「1 Issue = GLM 1 サイクル」を満たすために 1 primitive に限定。(3) cylinder/sphere/extrusion の同等修正は別 Issue として後続させる方が変更面が読みやすく、相互干渉も避けられる (各 primitive のテスト assert が独立)。
- **API 形**: `make_cuboid(dx, dy, dz, feature_id: &str, id_gen)` の **必須パラメタ追加** を採用。`Option<&str>` でデフォルト "cuboid" にする案も検討したが、(a) 呼び出し側で「忘れて hardcoded fid に戻る」回帰を許してしまう、(b) 既存テストは `"cuboid"` を明示渡しすれば全部維持できるため、必須パラメタの方がコンパイラ強制で安全。
- **既存テストの扱い**: `EntityRef::try_named("cuboid", ...)` で face を引いている既存テスト (`find_face_by_entity_ref_acceptance.rs` 等) は **意味的に「cuboid feature の face を引く」** ものであり、呼び出し側で `make_cuboid(..., "cuboid", &mut gen)` を渡せば挙動完全互換。よって既存 assert は変更しない (= 既存 behavior の回帰防止)。
- **example YAML の扱い**: `examples/sketch_via_face_entity_ref.engawa` の workaround コメント (#46-47) を解除し、`feature_id: cuboid` → `feature_id: box_1` に修正。

## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| `make_cuboid` シグネチャに `feature_id: &str` を必須追加し、内部 `fid` 変数の hardcode を引数化 | cylinder / sphere / extrusion 同等修正 (別 Issue として親 #219 close 後に起票) |
| `engawa-build` の `Feature::CreateBox` dispatcher で `id.as_str()` を `make_cuboid` に渡す | `make_cuboid` の決定性 / 形状 / トポロジーロジック変更 |
| `examples/sketch_via_face_entity_ref.engawa` の `feature_id: cuboid` → `feature_id: box_1` (workaround コメント削除) | `EntityRef` / `EntityKind` 自体の構造変更 |
| 既存 `make_cuboid` callsite (主に tests/ と benches) で `"cuboid"` を明示渡しして既存 behavior を維持 | sphere/cylinder/extrusion の test 側 `EntityRef::try_named("sphere"/"cylinder"/"extrusion", ...)` |
| T01: CreateBox(id="box_1") → kernel solid の Face.name の feature_id が "box_1" であることの assert | parametric history の root-cause refactor (#219 の延長で起こすべきでない) |

## Non-Goals
- cylinder / sphere / extrusion の同等修正 — 個別 Issue で実施
- `Feature::CreateBox.id` の semantic 変更 (validation 強化など)
- 旧 example YAML 以外の workaround コメントの掃除 (該当箇所が他にあれば別 Issue)
- ADR-005 本格運用の章追加 / ADR-005 改訂 — 本 Issue は scope minimal の fix
- 該当なし以外、上記列挙で網羅

## 実装対象
Issue: #219

影響クレート / ファイル:
- `crates/engawa-kernel/src/primitives/cuboid.rs` — シグネチャ変更 + `fid` 引数化
- `crates/engawa-build/src/lib.rs` (line ~342-350) — `CreateBox` dispatcher で `id.as_str()` を渡す
- `crates/engawa-kernel/tests/*.rs` (約 20 ファイル) — `make_cuboid(...)` 呼び出しに `"cuboid"` を引数追加 (mechanical)
- `crates/engawa-kernel/benches/tessellation.rs` — 同上
- `crates/engawa-build/tests/*.rs` — `make_cuboid(...)` を直接呼ぶテストに `"cuboid"` を追加
- `examples/sketch_via_face_entity_ref.engawa` — `feature_id: cuboid` → `feature_id: box_1`、workaround コメント (#46-47) 削除

### before/after スニペット

**`crates/engawa-kernel/src/primitives/cuboid.rs` (line 18-23, 61-64)**

before:
```rust
pub fn make_cuboid(
    dx: f64,
    dy: f64,
    dz: f64,
    id_gen: &mut IdGenerator,
) -> Result<Solid, KernelError> {
    ...
    // We'll use a fixed feature_id "cuboid" for the EntityRef names since the actual
    // feature_id is not available inside the kernel. The build dispatcher will need to
    // re-name these if needed. For now this gives named entities for boolean operations.
    let fid = "cuboid";
```

after:
```rust
pub fn make_cuboid(
    dx: f64,
    dy: f64,
    dz: f64,
    feature_id: &str,
    id_gen: &mut IdGenerator,
) -> Result<Solid, KernelError> {
    ...
    let fid = feature_id;
```

**`crates/engawa-build/src/lib.rs` (line 342-350)**

before:
```rust
Feature::CreateBox {
    id: _,
    width,
    height,
    depth,
} => {
    let solid = make_cuboid(*width, *height, *depth, gen)?;
    built.register(id.to_string(), solid);
}
```

after:
```rust
Feature::CreateBox {
    id: _,
    width,
    height,
    depth,
} => {
    let solid = make_cuboid(*width, *height, *depth, id.as_str(), gen)?;
    built.register(id.to_string(), solid);
}
```

(注: enclosing match の outer `id` が Feature の id バインディング。`id: _` は struct field の destructure 側であり enclosing scope 経由でアクセス可能。GLM 実装時に正確な束縛変数名を kernel 側コードで再確認すること。)

**`examples/sketch_via_face_entity_ref.engawa`**

before:
```yaml
      # NOTE: 現状 make_cuboid は内部で fid="cuboid" を固定で使うため (#219 参照)、
      # CreateBox の id (box_1) ではなく "cuboid" を指定する必要がある。
      face_ref:
        feature_id: cuboid
```

after:
```yaml
      face_ref:
        feature_id: box_1
```

## 設計方針

- **決定性要件**: シグネチャに追加するのは `&str` のみ。`fid` は EntityRef 名前文字列に転写されるだけで `IdGenerator.next()` 呼び出し順序に影響しない → 既存 ID 決定性テスト (`test_cuboid_deterministic`) は変わらず pass する。
- **B-rep トポロジー妥当性**: トポロジー (V=8, E=12, F=6, V-E+F=2) は変更なし。`fid` は Vertex/Edge/Face の name にのみ影響。
- **退化幾何**: 既存の `dx<=0` / `!is_finite` チェックは変更しない。`feature_id: &str` の空文字列 (`""`) は `EntityRef::try_named` 側の検証に委ねる (kernel 側で追加 validation はしない)。
- **derive 規約**: 型変更なし。
- **エラーハンドリング**: `KernelError` バリアント追加なし。
- **workspace.dependencies**: 変更なし。

### 数値モデル
不要 (シグネチャ・名前空間の変更のみ、数値演算は変更なし)。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | `make_cuboid(10,20,30,"cuboid", &mut IdGenerator::new(0))` を 2 回呼ぶ | 全 vertex/edge/face id・position・name が一致 |
| T02 | 正常系 (fid propagation) | `CreateBox { id: "box_1", width:1, height:1, depth:1 }` を build → solid.faces[i].name.feature_id == "box_1" を全 6 face で assert | OK |
| T03 | 正常系 (異なる fid) | `make_cuboid(1,1,1,"my_box", ...)` で生成 → Face name.feature_id が "my_box" / Edge name.feature_id が "my_box" / Vertex name.feature_id が "my_box" であることを assert | OK |
| T04_degen_empty_fid | 退化/境界 | `make_cuboid(1,1,1,"", ...)` を呼ぶ → `EntityRef::try_named` が空文字列を許可するかに依存。挙動を確認 (assert に固定) | 既存 EntityRef 仕様準拠 (空でも Ok か Err かを観察し test 化) |
| T05_boundary_long_fid | 退化/境界 | 長い fid (`"a".repeat(256)`) で `make_cuboid` を呼ぶ | エラーにならず Face.name に該当文字列が入る (truncate 等の意図しない処理がないことを担保) |
| T06_example_smoke | 統合 | `examples/sketch_via_face_entity_ref.engawa` を build → run、`feature_id: box_1` で Face 解決できることを smoke で確認 | examples_smoke が green |

(plan のテスト計画では `_degen_` / `_boundary_` 専用 ID を含めることが STEP 5.5 ガード条件。T04_degen_empty_fid + T05_boundary_long_fid で 2 件確保済み)

## 幾何的不変条件チェックリスト

- [N/A] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか — Boolean/Partition 系ではない
- [N/A] 各プリミティブの face ごとの outer_loop 2D 向き — 形状は変更しない (既存テスト test_cuboid_topology / test_cuboid_vertex_positions が回帰検出)
- [N/A] flip_normals / same_sense の意味論 — 変更なし
- [N/A] pslg_subdivide の出力向き — 関係なし
