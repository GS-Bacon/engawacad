Reading additional input from stdin...
OpenAI Codex v0.121.0 (research preview)
--------
workdir: /home/bacon/mycad
model: gpt-5.4
provider: openai
approval: never
sandbox: read-only
reasoning effort: xhigh
reasoning summaries: none
session id: 019e57c1-eb40-7dc1-8586-a95321e2d761
--------
user
# Codex 設計レビュアー（MyCad CAD カーネル専用）

あなたは Rust 製 B-rep CAD カーネル「MyCad」の設計ドキュメントをレビューする専門家です。
stdin に渡された設計・テスト計画ドキュメントを読み、以下の観点で問題を指摘してください。

## レビュー観点

### 1. 決定性（MyCad の核心）
- `IdGenerator` を使った決定的 ID 生成になっているか
- 同一入力で必ず同一出力（同一 ID・同一座標）が保証されているか
- 非決定的要素（HashMap のイテレーション順、thread_rng など）が混入していないか

### 2. B-rep トポロジー妥当性
- Euler-Poincaré の公式 `V - E + F = 2(S - H)` が成立する設計か（S=殻の数, H=貫通穴）
- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
- Loop/Shell/Solid の入れ子構造が一貫しているか

### 3. 退化幾何の扱い
- ゼロ長エッジ、面積ゼロの Face、縮退した Loop の排除または明示的エラーの設計があるか
- 数値的境界（浮動小数点の epsilon 比較）の方針が明記されているか

### 4. API・型設計
- `Debug, Clone, Serialize, Deserialize` が付与されているか（JsonSchema が必要な型は追記）
- 新しい依存は `[workspace.dependencies]` + `{ workspace = true }` 規約に従っているか
- ライブラリエラーには `thiserror` を使用しているか
- カーネルにレンダリング依存が混入していないか（`TriangleMesh` 生成のみ可）

### 5. テスト計画の充足性
- 決定性反復テスト（T0x 系）があるか
- Euler-Poincaré 検証テストがあるか
- 退化入力エッジケースが十分にあるか
- golden YAML ラウンドトリップテストがあるか

### 6. アーキテクチャ整合性
- Index-based topology（ポインタ不使用、フラット配列 + インデックス参照）を守っているか
- Feature history = source of truth の原則と矛盾しないか
- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか

---

## 出力フォーマット（厳守）

```yaml
issues:
  - id: R01
    severity: critical  # critical | high | medium | low
    section: "設計方針 > 決定性"
    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
    suggestion: "IdGenerator::next() に置き換えること"
  - id: R02
    severity: medium
    ...

verdict: pass  # pass | fail
# fail = Critical または High が1件以上ある
```

`issues` が空の場合は `issues: []` と書く。
**このフォーマット以外の出力は禁止。前置きや後置きの文章は書かない。**

<stdin>
# Issue #12: 円柱プリミティブ + 曲面テッセレーション土台

## Context（なぜやるか）

ROADMAP Phase 3「円柱・球・押し出しが作れる」の最初の一歩。現状カーネルは `make_cuboid` と平面専用の `tessellate_solid` しか持たない。本 Issue で **解析曲面(円柱)を厳密 B-rep として生成し、それを三角形メッシュへ離散化する土台**を確立し、`mycad export examples/cylinder.mycad -o out.stl` を動作させる。

設計の根幹判断（壁打ち済み）:
- 解像度は STL 焼き出し側の関心事。B-rep は厳密なまま。本番出力は将来 STEP 前提なので、解像度は **外部化可能だが既定値で足りる**サブ経路。
- テッセレータは ADR-004 準拠で **variant を直接 `match` しない**。型ごとの分岐は `Surface`/`Curve` の多態メソッド（網羅 match はコンパイラが強制）に閉じ込め、アルゴリズム本体は型非依存に保つ。
- 2D 三角化は今回は自前（円柱は凸領域で自明）。任意トリム面/Boolean 面が出る Phase 3後半/4 でライブラリ(earcutr/spade 等)を差し込む余地を残す。

## 実装対象

- Issue: #12 / Milestone: Phase 3
- 影響ファイル:
  - 新規 `crates/mycad-kernel/src/primitives/cylinder.rs` — `make_cylinder`
  - `crates/mycad-kernel/src/primitives/mod.rs` — `mod`/`pub use` 追加
  - `crates/mycad-kernel/src/geometry/curve.rs` — `Curve::sample_segment` 追加
  - `crates/mycad-kernel/src/geometry/surface.rs` — `Surface::requires_interior_sampling` 追加
  - `crates/mycad-kernel/src/tessellation/mod.rs` — 曲線折れ線化 + 曲面 UV グリッド対応 + `TessellationOptions`
  - `crates/mycad-build/src/lib.rs` — `CreateCylinder` スタブを実差し替え
  - `crates/mycad-cli/src/main.rs` — `export` に任意 `--segments` を追加（既定値あり）
  - 新規 `examples/cylinder.mycad`
  - テスト: `cylinder.rs` 内インライン / `tessellation/mod.rs` 内 / `mycad-build/tests/feature_dispatcher.rs` / `mycad-cli/tests/export.rs`

## 設計方針

### 1. トポロジー（`make_cylinder(radius, height, id_gen) -> Solid`）

軸 = +Z、底面中心を原点、底 z=0 / 上 z=h。`orthonormal_basis(+Z) = (+Y, -X)` なので角度 u=0 は +Y 方向 → シーム頂点は (0, r, z)。

- 頂点 **2**: `v_bot=(0,r,0)`, `v_top=(0,r,h)`
- エッジ **3**:
  - `e_bot`: `Curve::Circle{center(0,0,0), normal +Z, radius r}`, vertices `[v_bot,v_bot]`, t_range `[0,2π]`（閉曲線。evaluate(0)=v_bot）
  - `e_top`: `Curve::Circle{center(0,0,h), normal +Z, radius r}`, vertices `[v_top,v_top]`, t_range `[0,2π]`
  - `e_seam`: `Curve::Line{origin(0,r,0), direction(0,0,h)}`, vertices `[v_bot,v_top]`, t_range `[0,1]`
- 面 **3**（同 `Surface` 既存 variant、enum 追加なし）:
  - 底キャップ: `Surface::Plane{origin(0,0,0), normal -Z, u_axis +X, v_axis +Y}`、loop = `e_bot` の HE 1 本
  - 上キャップ: `Surface::Plane{origin(0,0,h), normal +Z, u_axis +X, v_axis +Y}`、loop = `e_top` の HE 1 本
  - 側面: `Surface::Cylinder{origin(0,0,0), axis +Z, radius r}`、loop = 4 HE: seam上(e_seam,fwd,start v_bot)→top円(e_top,fwd,start v_top)→seam下(e_seam,rev,start v_top)→bot円(e_bot,rev,start v_bot)
- シェル **1**（closed=true）

**マニフォルド規約**: 各エッジは正確に 2 HE で、向きは逆（fwd/rev）。
- `e_seam`: 側面が fwd と rev（シームは同一面が両側で使用＝周期面の標準形）
- `e_bot`: 側面が rev、底キャップが fwd
- `e_top`: 側面が fwd、上キャップが rev
- HE 合計 = 6（= 3 edges × 2）。

**Euler–Poincaré**: V−E+F = 2−3+3 = 2 = 2(S−H), S=1,H=0 → 成立。

**決定性**: ID 採番順を `make_cuboid` と同型に固定。solid → 頂点(v_bot,v_top) → エッジ(e_bot,e_top,e_seam) → 面ごとに[底,上,側]順で HE群→loop→face → shell。すべて `id_gen.next()` を固定順で呼ぶのみ。`HashMap`/乱数等の非決定要素なし。

**退化**: `radius <= 0` または `height <= 0` はゼロ面積/ゼロ長を生むため将来的にエラー化対象だが、現状 `make_cuboid` も正値前提で検証していない。本 Issue は **正値前提**を踏襲し、退化はテストで「正値での妥当性」を担保（負値の明示エラーはスコープ外、必要なら Codex 指摘で追加検討）。

### 2. テッセレータ（ADR-004 準拠の加算拡張）

`tessellate_solid` を次の型非依存アルゴリズムに拡張。**`match Surface::Cylinder` 等は書かない**。

- 解像度の外部化:
  ```rust
  pub struct TessellationOptions { pub angular_segments: usize, pub axial_segments: usize }
  impl Default // angular=32, axial=1
  pub fn tessellate_solid(solid) -> TriangleMesh           // = _with(solid, &default())  ※既存呼び出し・既存テスト不変
  pub fn tessellate_solid_with(solid, &TessellationOptions) -> TriangleMesh
  ```
- 多態フック（型分岐をジオメトリ側の網羅 match に閉じ込め、コンパイラが variant 追加漏れを強制）:
  - `Curve::sample_segment(&self, t_range, segments) -> Vec<Point>`: t0 から t1 手前までの点列（終点は次 HE の始点が担うので除外）。`Line` は `vec![evaluate(t0)]`（1 点）、`Circle` は `segments` 等分点。→ 直線エッジは従来どおり始点 1 点のみ＝**cuboid 出力は不変**。円エッジは折れ線化。
  - `Surface::requires_interior_sampling(&self) -> bool`: `Plane=false`、`Cylinder/Sphere/Cone=true`。
- 面ごとの処理:
  1. 外側 loop を `sample_segment` で折れ線化（rev HE は反転）→ 境界点列。
  2. `surface.requires_interior_sampling()` が **false（平面）** → 境界点列を **頂点0 から fan**（凸前提。cuboid 4 角=従来 2 三角形で不変、円盤キャップ=N角形 fan）。
  3. **true（曲面）** → 境界点を `surface.uv_of` で (u,v) 化し、周期シーム（u が ±π で跳ぶ）を 2π アンラップして連続化 → パラメータ箱 [u_min,u_max]×[v_min,v_max] を取得 → `(angular_segments+1)×(axial_segments+1)` の UV グリッドを `surface.evaluate(u,v)` でサンプル → セルごとに 2 三角形。これで側面が滑らかな多面体に。
  4. 各頂点法線は `surface.normal_at_point`（`same_sense` で反転）— 既存 PR #11 の方式を踏襲。
- **STL 外向き保証**: 各三角形の幾何法線（辺の外積）が、その面の外向き法線（`normal_at_point` を `same_sense` で調整）と逆なら 2 頂点をスワップして向きを揃える。これでキャップ/側面とも STL facet が外向き。cuboid は既に外向きのためスワップ発生せず、出力は実質不変（決定性・三角形数も維持）。

### 3. ディスパッチ / CLI / フォーマット

- `mycad-build/src/lib.rs`: `Feature::CreateCylinder{radius,height}` 分岐を `Ok(make_cylinder(*radius, *height, gen))` に差し替え（`UnsupportedFeature` スタブ削除）。`Feature` enum・`id()`・`KernelError` は既存のまま変更不要。
- `mycad-cli`: `export` に `--segments <N>`（任意、未指定なら `TessellationOptions::default`）。export パスは `tessellate_solid_with` を呼ぶ。
- `examples/cylinder.mycad`:
  ```yaml
  version: "0.1.0"
  root_component:
    name: "Simple Cylinder"
    features:
      - type: create_cylinder
        id: cyl_1
        radius: 5.0
        height: 20.0
  ```

### derive / 依存 / エラー規約

- `TessellationOptions` に `Debug, Clone, Serialize, Deserialize`（既存 `TriangleMesh` と同様）。`Copy` も可。
- 新規 workspace 依存なし（nalgebra のみで完結）。
- 新規エラー型なし（退化の明示エラーは今回スコープ外）。

## テスト計画

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
| T02 | トポロジー | 頂点2・エッジ3・面3・シェル1(closed)・HE6・loop3 | 各 `len()` 一致 |
| T03 | Euler | V−E+F == 2 | `assert_eq!(2-3+3, 2)` 相当の検証 |
| T04 | マニフォルド | 各エッジがちょうど 2 HE に参照され向きが逆 | 集計検証 |
| T05 | 幾何 | シーム頂点が (0,r,0)/(0,r,h)、各 Surface/Curve の係数 | `approx` で一致 |
| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
| T07 | 決定性(tessellate) | 同一入力2回で positions/normals/indices 一致 | `assert_eq!` |
| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index | 既存テスト維持 |
| T09 | 外向き法線 | 全 facet の幾何法線が重心からの外向きと同符号（凸形状なので判定可） | 検証 |
| T10 | ディスパッチ統合 | `examples/cylinder.mycad` を `build_solid_from_features` でビルドし T02 と同じ counts | `feature_dispatcher.rs` |
| T11 | export 統合 | `mycad export examples/cylinder.mycad -o tmp.stl` が成功し、facet 数が T06 と整合、ASCII STL として妥当 | `export.rs`（`CARGO_BIN_EXE_mycad`） |
| T12 | 解像度外部化 | `--segments` / `tessellate_solid_with` で分割数が変わり三角形数が増減 | 2 解像度で差分確認 |

## 検証（エンドツーエンド）

1. `cargo test --workspace` で T01–T12 green。
2. `cargo run -p mycad-cli -- export examples/cylinder.mycad -o /tmp/cyl.stl` 成功。
3. STL を目視/メッシュビューア（Blender/MeshLab）で読み込み、閉じた円柱（側面が多面体近似・上下フタあり・法線外向き）を確認。
4. `cargo xtask ci`（fmt→clippy→test→build）green。

## スコープ外（将来）

- 退化入力（r/h<=0）の明示エラー化、`Sphere`/`Cone`/`Extrude`、任意トリム面の 2D メッシュライブラリ導入、pcurve、ビューアの解析/GPU 適応レンダリング。
</stdin>
codex
設計ドキュメントを MyCad の実装規約に照らして確認します。まず既存の `make_cuboid`、トポロジー型、テッセレーション、ADR とテスト構成を読み、提案内容と矛盾する点を特定します。
exec
/bin/bash -lc 'rg --files' in /home/bacon/mycad
exec
/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
 succeeded in 209ms:
Cargo.toml
ARCHITECTURE.md
README.md
rust-toolchain.toml
CLAUDE.md
.gitignore
ROADMAP.md
.git/packed-refs
crates/mycad-format/Cargo.toml
docs/file-format.md
.git/refs/remotes/origin/HEAD
.git/COMMIT_EDITMSG
features/12-cylinder/design-review.md
docs/decisions/001-brep-representation.md
docs/decisions/002-roadmap-management.md
crates/mycad-format/src/lib.rs
docs/decisions/004-freeform-geometry-commitment.md
crates/mycad-format/src/feature.rs
crates/mycad-format/src/component.rs
crates/mycad-format/src/error.rs
crates/mycad-format/src/document.rs
crates/mycad-format/CLAUDE.md
crates/xtask/Cargo.toml
.cargo/config.toml
.claude/settings.json
.git/info/exclude
.git/refs/remotes/origin/claude/add-claude-guidelines-BKKtD
crates/mycad-viewer/Cargo.toml
crates/mycad-build/Cargo.toml
crates/xtask/src/main.rs
.git/description
Cargo.lock
.git/hooks/sendemail-validate.sample
.git/hooks/applypatch-msg.sample
.git/hooks/pre-commit.sample
.git/hooks/pre-applypatch.sample
.git/hooks/pre-rebase.sample
.git/hooks/pre-merge-commit.sample
.git/opencode
crates/mycad-kernel/Cargo.toml
crates/mycad-viewer/src/lib.rs
.git/hooks/post-update.sample
.git/hooks/pre-receive.sample
.git/hooks/pre-push.sample
.git/hooks/prepare-commit-msg.sample
.git/hooks/commit-msg.sample
.git/hooks/push-to-checkout.sample
.git/hooks/fsmonitor-watchman.sample
.git/hooks/update.sample
crates/mycad-build/src/lib.rs
crates/mycad-cli/Cargo.toml
examples/simple_box.mycad
examples/assembly.mycad
.git/objects/49/c3b468feaf139d9c75a35c7d3a49a65a960a34
.git/refs/heads/claude/add-claude-guidelines-BKKtD
crates/mycad-cli/src/main.rs
.claude/skills/3ai/SKILL.md
crates/mycad-kernel/src/lib.rs
crates/mycad-build/tests/feature_dispatcher.rs
.claude/skills/3ai/agents/glm-implementer.md
.claude/skills/3ai/agents/codex-final-reviewer.md
.claude/skills/3ai/agents/codex-design-reviewer.md
crates/mycad-kernel/tests/cuboid_roundtrip.rs
.git/objects/51/bf8ed0eb7b9b6cecf42c473fb5b19c714683ed
crates/mycad-kernel/CLAUDE.md
.git/objects/da/56d0901c48f54cba1e696480139ba8c64e7f7a
.git/objects/3b/9a6697582a7359e9077d779f27661bf47d3cd5
crates/mycad-kernel/src/tessellation/mod.rs
crates/mycad-kernel/src/tessellation/stl.rs
crates/mycad-cli/tests/export.rs
crates/mycad-kernel/src/primitives/cuboid.rs
crates/mycad-kernel/src/error.rs
crates/mycad-kernel/src/primitives/mod.rs
crates/mycad-kernel/benches/tessellation.rs
.git/objects/b5/fc56c4b80bad66754bdaef1b2053abffd770f1
.git/objects/38/76021c40eea6680c03d67aa21abba119c62f97
.claude/skills/3ai/scripts/guard-crates.sh
.git/objects/27/2970e35a608e9cbde6daef23f439fa7108a35d
.git/objects/pack/pack-d9e6fa1e29ba2b801a71302390018bc93c757528.idx
.git/objects/pack/pack-d9e6fa1e29ba2b801a71302390018bc93c757528.rev
.git/objects/pack/pack-d9e6fa1e29ba2b801a71302390018bc93c757528.pack
.claude/skills/3ai/scripts/dispatch-codex.sh
.claude/skills/3ai/scripts/dispatch-glm.sh
.git/objects/f2/550699a30ac247de8e86e42a7ed6ef0ec68f1f
.git/objects/ee/ef02550dfde1f8e8d78e3bec225d30d2d3fc27
.git/objects/93/416fcabe178a205ed6389e1aab1513f3a84624
.git/objects/ea/c25d13cc45d7da37b3487cdee5277b8b4a4a05
.git/objects/f8/79a384e646d58e8ac33e12939f36ad4cfe1fd6
.git/objects/3c/e3331b1ca62783227769921f9b253a7c7f1ce6
.git/objects/5b/80e283c04cab6f8c9399a797733fb867713b7c
.git/objects/5b/2a7b94a20c4573fae1fd94f905ae00a66f3296
.git/objects/5b/f2f551288dc25718660b3321055b7084248244
.git/objects/07/d9f0998399fe96ad30657a5f33452ca73035da
.git/objects/8a/57aa85df9f51b6a7a30e6b5aea3f0250b1e8a2
.git/objects/e6/3f03657226775892748af8cc4e9e08a7576382
crates/mycad-kernel/src/brep/mod.rs
crates/mycad-kernel/src/brep/topology.rs
.git/logs/refs/remotes/origin/HEAD
.git/objects/3d/a173752990b455d1dc691de79a172d0a64db9c
.git/objects/94/29877508b375ff41a3304575b861073d64cd83
crates/mycad-kernel/src/geometry/surface.rs
crates/mycad-kernel/src/geometry/mod.rs
crates/mycad-kernel/src/geometry/curve.rs
crates/mycad-kernel/src/geometry/math.rs
.git/objects/85/c7712d7caa02c603e9d3578a68e9718d2a2fc2
.git/objects/1c/00df69cd1f2acb22e8b6067b418a07d051b47f
.git/objects/0d/4e25162d94916438174c03612f0023b5a9016f
.git/objects/97/953774ddd8247225e7368643aa031cde3cced9
.git/objects/54/15a0964d76b39cec282b6746803866d0e40ee6
.git/objects/23/a907de7c1c457780c3393da0fbde5893b68de4
.git/objects/d8/3a91bf0faf333faf49ae7889de4844e7960638
.git/config
.git/ORIG_HEAD
.git/index
.git/objects/01/1dc8083a61e1a4283681517e621edd80debe7e
.git/objects/a0/851729e45eeb1cf5b58871b60e7443d29d60fe
.git/objects/a0/20874ac157c5ad937bfb74324d7f445fb0e927
.git/objects/35/37e8ffe05190fc64cb17ae4b5f708fa517b338
.git/objects/ff/677974e6c6e590e3909cf89ed028756419a31f
.git/logs/refs/remotes/origin/claude/add-claude-guidelines-BKKtD
.git/objects/69/811c2cd2f31f63e7c1ddce855f0ec091d841da
.git/objects/14/0f766767eff6dca14ce6d7bf36e3843da17f4a
.git/objects/67/93c94ceae51f8d025b6537d01640bd9263c0e8
.git/objects/4e/22430f743458caba1c7f4c391dc5c8be117d7c
.git/objects/6b/4f560fa674ce568dd735d2298f816b7cdef3bb
.git/objects/02/fc621ea7da109ef42846b74197f5775ece124c
.git/objects/d5/c98d61ad288b808f65a3953abf6575aa3ceb87
.git/objects/c8/d0a83e7b6735cd5568cc39d28be1213d2f280b
.git/objects/a4/16a93868018961c64227d686db4a5a023afbb2
.git/objects/96/b7f38089a05605b9a9c92b57bf8a486bfc2658
.git/objects/b8/b125139776fd7bbb0799362d61c563fff21fc6
.git/objects/88/a701ef888392fd6d773d622ecc8f7e773a1962
.git/objects/a4/af917f103530868e91b3b9bcfedf3902a3a3f1
.git/objects/9e/dca3ddaff8828eba48bfa5007b248221716ede
.git/objects/19/032be60748224982841286ef654905b77ffa21
.git/objects/86/9b8a025c6a26a0d6baabcb7e60e506e8908e2c
.git/objects/c3/9336eedae7858762394902e31fe47408e757e6
.git/logs/refs/heads/claude/add-claude-guidelines-BKKtD
.git/logs/HEAD
.git/objects/cd/e9f18477a427633616ef5385136f44c411ab14
.git/objects/2a/233969062bfaa6d679231fcd91a770aafefcba
.git/HEAD
.git/FETCH_HEAD
.git/objects/6a/96fd640fc3850f2086e19f737d51c3bffd9d46
.git/objects/e3/12b4ef0fa12ba71b7974d6049e67bba346c3b2
.git/objects/e3/67d5288ed80eea015efdfb26c6d8a3505fc2e6
.git/objects/e3/f04758017c63d93adb7b325ba29769cfa70023
.git/objects/0a/579062059d46927b170490912f8ae174dec896
.git/objects/09/079e3c84770ab3b076e78e892aab98a9f93020
.git/objects/f5/c48a6c87abc2dcd1eb3ffdeb67993c4c024487
.git/objects/09/e4b0a14490917bcfa527d5a37ede29baf643ea
.git/objects/ce/0b324280cebfe29f0380d7f87eefed2142816c
.git/objects/40/8d3d65efaf30d74e9d9efb101882c88bd18617
.git/objects/40/a258043ac34a0cb6729f9b54d8431aa30a6c5f
.git/objects/42/855fc4ee4f2e3aed6d109c467f0379cd127369
.git/objects/8b/182ebf45cf0e98ebe2ff1593f32803beedd360
.git/objects/58/5f429fc31ec81a1148129c98725a51fbbeee34
.git/objects/75/2ac98646de4b09bad18047d96478c284161173
.git/objects/75/0fed59cd01a59ddcfc4e7e88f5155f19285708
.git/objects/72/7c6306d9784e19c94e78b0a33bcd73fe7e2aa8
.git/objects/72/10cd99ad1b4ae546ee4dfbaf356d9589cbc2b1
.git/objects/af/eeaa7c36250f659f54fa44e0797888caae9f7e
.git/objects/2c/6a1c7c44bcfd8821c4bfa42cb9f190e676378c
.git/objects/68/148d267aa2e51a49c92b8bda2a81e59a6c7fe2
.git/objects/ab/2958a325e99cd9402a18dcc4c55d816f8eb2ad
.git/objects/ab/2e2d96924b9d1a6a4e028a01c69c0880306148
.git/objects/ab/647dd198a1d32cbd6392e4788e012d517a5a24
.git/objects/fe/5cefc5fca934c4dbac4618bf359b6cb86da03e
.git/objects/6e/54923f857ef695760a2b6505dad7993e6acdae
.git/objects/6e/df172beb62ca9d77ea98777113f8a00e43ddd1
.git/objects/eb/557acee53a52ebe94aafffd472aca1d730b58a
.git/objects/45/e4fbb91bc432be28c5b841783220c40749bb7e
.git/objects/a7/ae11631a5a7afb43171ad7cc73aa386ad65086
.git/objects/03/9a116943cad3c819bae58daa5b9b6c646f6fb3
.git/objects/0e/da9d3dccebc8894764f5126471bb3ce988c576
.git/objects/c9/a87d0c5dc0cd3124752ecfc039e6913295288f
.git/objects/0b/8b10d660bada640047a3fe973908cf0687874a
.git/objects/0b/700d643f2366503a944dd113f367613bb5690f
.git/objects/d3/b572e5310d926e803c088579eb11e4298b5e8c
.git/objects/e5/3883cae824143b7ce9875dce9dfe01ee21a0b3
.git/objects/8f/ebf46db7a41cfa0e85023e6e782931cf136a86
.git/objects/70/09384b80fa2d0fe528b7fe1e309e5a001c104c
.git/objects/83/9d73bc0ab735b8a17ca1b8b92cd7fde13dd057
.git/objects/f1/87785f1ff1a4278e12a8a883aa5f954a3c92a9
.git/objects/ac/7120bd720867f24b98b21423ae79b6fc9b13ab
.git/objects/b6/010273ed46f47ff4af3e7eca7437a79b279c13
.git/objects/ac/b4fcdcbe5649e06c58c01d12598ddf8f7e8f68
.git/objects/e9/271348b9924b586f799f4455fc5a5c9953817d
.git/objects/d4/dd06cc242fe9953846597515fd93d436930a98
.git/objects/44/6f93326a061e16debf8c2496789e5f4aa6aa70
.git/objects/3f/01ea2678c5c4411bee7164e2f5abb82c106682
.git/objects/22/48b7cc3b53c0768c82d8398bb03f9eecb7def6
.git/objects/e0/80419364a7042f83a1723b94dfb2931c968247
.git/objects/56/c5be59ce7658658e6730c229027eba28ab13f6
.git/objects/0f/c322888c190eb4eda47b31e5a5c905948f1484

 succeeded in 234ms:
Cargo.toml:18:[workspace.dependencies]
Cargo.toml:30:thiserror = "2"
crates/mycad-format/Cargo.toml:12:thiserror = { workspace = true }
ARCHITECTURE.md:56:`mycad-kernel` はレンダリングライブラリに一切依存しない。テッセレーション結果（`TriangleMesh`）を生成するのみ。ビューアがそれを消費する。
ARCHITECTURE.md:61:.mycad file → Document → Feature列 → Kernel (B-rep Solid) → Tessellation → TriangleMesh → Viewer
docs/decisions/001-brep-representation.md:29:- `Solid` が最上位エンティティ、内部に Shell → Face → Loop → HalfEdge → Vertex の階層
.claude/skills/3ai/agents/glm-implementer.md:19:- ID 生成は必ず `IdGenerator::next()` を使う。`Uuid::new_v4()`、`rand`、timestamp は禁止
.claude/skills/3ai/agents/glm-implementer.md:25:- HalfEdge: `twin`/`next`/`prev` インデックスが循環的に正しく設定されているか確認
.claude/skills/3ai/agents/glm-implementer.md:33:- 公開型に `#[derive(Debug, Clone, Serialize, Deserialize)]` を付与（スキーマが必要なら `JsonSchema` も）
.claude/skills/3ai/agents/glm-implementer.md:34:- 新規依存は `Cargo.toml` の `[workspace.dependencies]` に追加し、各クレートは `{ workspace = true }` で参照
.claude/skills/3ai/agents/glm-implementer.md:35:- ライブラリエラーは `thiserror` で定義
.claude/skills/3ai/agents/glm-implementer.md:60:- 並行安全（該当する場合）: 複数スレッドから同時に IdGenerator を呼んでも ID が衝突しないか
.claude/skills/3ai/SKILL.md:51:- 決定性要件: (IdGenerator の使い方、同一入力→同一出力の保証方法)
.claude/skills/3ai/SKILL.md:54:- derive 規約: Debug/Clone/Serialize/Deserialize (+JsonSchema が必要か)
.claude/skills/3ai/SKILL.md:55:- エラーハンドリング: thiserror の使い方
.claude/skills/3ai/SKILL.md:56:- workspace.dependencies 規約
CLAUDE.md:27:│   ├── brep/         トポロジー (Solid, Shell, Face, Loop, Edge, HalfEdge, Vertex)
CLAUDE.md:29:│   ├── primitives/   プリミティブ生成 (make_cuboid)
CLAUDE.md:30:│   └── tessellation/ メッシュ化 (tessellate_solid → TriangleMesh)
CLAUDE.md:53:- **決定性**: すべてのEntityIDは `IdGenerator` で決定的に生成。テストでは決定性を検証すること。
CLAUDE.md:54:- **依存管理**: 新しい依存は必ず `[workspace.dependencies]` に追加し、各クレートは `{ workspace = true }` で参照。
CLAUDE.md:55:- **derive規約**: 公開型には `Debug, Clone, Serialize, Deserialize` を付与。スキーマ生成が必要な型は `JsonSchema` も追加。
CLAUDE.md:56:- **エラーハンドリング**: ライブラリエラーには `thiserror` を使用。
CLAUDE.md:57:- **モジュール構造**: 1概念1ファイル。重要な型は `mod.rs` から `pub use` で再エクスポート。
CLAUDE.md:64:- **Kernel has zero rendering deps**: カーネルはレンダリング非依存。`TriangleMesh` を生成し、ビューアが消費する。
.claude/skills/3ai/agents/codex-final-reviewer.md:9:- `IdGenerator` 以外の ID 生成（`Uuid::new_v4()`、`rand`、timestamp 等）が混入していないか
.claude/skills/3ai/agents/codex-final-reviewer.md:14:- HalfEdge の twin/next/prev インデックスが循環的に正しく設定されているか
.claude/skills/3ai/agents/codex-final-reviewer.md:25:- 新規依存が `[workspace.dependencies]` に追加され `{ workspace = true }` で参照されているか
.claude/skills/3ai/agents/codex-final-reviewer.md:26:- エラー型が `thiserror` で定義されているか
.claude/skills/3ai/agents/codex-final-reviewer.md:48:    finding: "HalfEdge の twin インデックスが self を指している（ループ不整合）"
.claude/skills/3ai/agents/codex-final-reviewer.md:49:    suggestion: "make_cuboid の edge 生成ロジックを見直す"
docs/decisions/004-freeform-geometry-commitment.md:1:# ADR-004: 自由曲面・自由曲線を確定要件として扱う
docs/decisions/004-freeform-geometry-commitment.md:43:- 本 ADR 採択に伴う即時対応: `tessellate_solid` が面法線を `normal_at(0.0, 0.0)` で 1 回だけ評価していた平面前提を解消し、`Surface::normal_at_point` による頂点毎評価へ変更した (平面では従来と同一の結果)。
ROADMAP.md:15:- B-rep トポロジー型 (`Solid`/`Shell`/`Face`/`Loop`/`Edge`/`HalfEdge`/`Vertex`) が実装済み
ROADMAP.md:17:- `make_cuboid` → `tessellate_solid` のパイプラインが動作し、決定性テストが通る
ROADMAP.md:60:- `CreateCylinder` / `CreateSphere` が `.mycad` から geometry 生成まで動作する
ROADMAP.md:74:**備考**: 曲面同士の Boolean は交線として自由曲線を、フィレット・面取りは自由曲面を必要とする (交線は一般に円・直線でない)。自由曲面/NURBS は専用 Phase を立てず、本 Phase が要求する範囲から漸進的に導入する。数値モデル (トレラント vs 厳密) もここで決定する。詳細は ADR-004 参照。
features/12-cylinder/design-review.md:22:- `IdGenerator` を使った決定的 ID 生成になっているか
features/12-cylinder/design-review.md:28:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
features/12-cylinder/design-review.md:36:- `Debug, Clone, Serialize, Deserialize` が付与されているか（JsonSchema が必要な型は追記）
features/12-cylinder/design-review.md:37:- 新しい依存は `[workspace.dependencies]` + `{ workspace = true }` 規約に従っているか
features/12-cylinder/design-review.md:38:- ライブラリエラーには `thiserror` を使用しているか
features/12-cylinder/design-review.md:39:- カーネルにレンダリング依存が混入していないか（`TriangleMesh` 生成のみ可）
features/12-cylinder/design-review.md:50:- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか
features/12-cylinder/design-review.md:61:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
features/12-cylinder/design-review.md:62:    suggestion: "IdGenerator::next() に置き換えること"
features/12-cylinder/design-review.md:79:ROADMAP Phase 3「円柱・球・押し出しが作れる」の最初の一歩。現状カーネルは `make_cuboid` と平面専用の `tessellate_solid` しか持たない。本 Issue で **解析曲面(円柱)を厳密 B-rep として生成し、それを三角形メッシュへ離散化する土台**を確立し、`mycad export examples/cylinder.mycad -o out.stl` を動作させる。
features/12-cylinder/design-review.md:83:- テッセレータは ADR-004 準拠で **variant を直接 `match` しない**。型ごとの分岐は `Surface`/`Curve` の多態メソッド（網羅 match はコンパイラが強制）に閉じ込め、アルゴリズム本体は型非依存に保つ。
features/12-cylinder/design-review.md:91:  - `crates/mycad-kernel/src/primitives/mod.rs` — `mod`/`pub use` 追加
features/12-cylinder/design-review.md:95:  - `crates/mycad-build/src/lib.rs` — `CreateCylinder` スタブを実差し替え
features/12-cylinder/design-review.md:125:**決定性**: ID 採番順を `make_cuboid` と同型に固定。solid → 頂点(v_bot,v_top) → エッジ(e_bot,e_top,e_seam) → 面ごとに[底,上,側]順で HE群→loop→face → shell。すべて `id_gen.next()` を固定順で呼ぶのみ。`HashMap`/乱数等の非決定要素なし。
features/12-cylinder/design-review.md:127:**退化**: `radius <= 0` または `height <= 0` はゼロ面積/ゼロ長を生むため将来的にエラー化対象だが、現状 `make_cuboid` も正値前提で検証していない。本 Issue は **正値前提**を踏襲し、退化はテストで「正値での妥当性」を担保（負値の明示エラーはスコープ外、必要なら Codex 指摘で追加検討）。
features/12-cylinder/design-review.md:129:### 2. テッセレータ（ADR-004 準拠の加算拡張）
features/12-cylinder/design-review.md:131:`tessellate_solid` を次の型非依存アルゴリズムに拡張。**`match Surface::Cylinder` 等は書かない**。
features/12-cylinder/design-review.md:137:  pub fn tessellate_solid(solid) -> TriangleMesh           // = _with(solid, &default())  ※既存呼び出し・既存テスト不変
features/12-cylinder/design-review.md:138:  pub fn tessellate_solid_with(solid, &TessellationOptions) -> TriangleMesh
features/12-cylinder/design-review.md:152:- `mycad-build/src/lib.rs`: `Feature::CreateCylinder{radius,height}` 分岐を `Ok(make_cylinder(*radius, *height, gen))` に差し替え（`UnsupportedFeature` スタブ削除）。`Feature` enum・`id()`・`KernelError` は既存のまま変更不要。
features/12-cylinder/design-review.md:153:- `mycad-cli`: `export` に `--segments <N>`（任意、未指定なら `TessellationOptions::default`）。export パスは `tessellate_solid_with` を呼ぶ。
features/12-cylinder/design-review.md:168:- `TessellationOptions` に `Debug, Clone, Serialize, Deserialize`（既存 `TriangleMesh` と同様）。`Copy` も可。
features/12-cylinder/design-review.md:176:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/12-cylinder/design-review.md:181:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
features/12-cylinder/design-review.md:183:| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index | 既存テスト維持 |
features/12-cylinder/design-review.md:187:| T12 | 解像度外部化 | `--segments` / `tessellate_solid_with` で分割数が変わり三角形数が増減 | 2 解像度で差分確認 |
features/12-cylinder/design-review.md:201:設計ドキュメントを MyCad の実装規約に照らして確認します。まず既存の `make_cuboid`、トポロジー型、テッセレーション、ADR とテスト構成を読み、提案内容と矛盾する点を特定します。
features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
crates/mycad-format/src/document.rs:3:use schemars::JsonSchema;
crates/mycad-format/src/document.rs:8:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
docs/decisions/002-roadmap-management.md:54:- フルパイプライン (Feature → Solid → TriangleMesh → 出力) を最短で通すことを優先
docs/decisions/002-roadmap-management.md:56:STL は三角メッシュの単純な ASCII/バイナリ形式で、`tessellate_solid` の出力をそのまま書き出せる。パイプライン確立後、Phase 6 で STEP に拡張する。
.claude/skills/3ai/agents/codex-design-reviewer.md:9:- `IdGenerator` を使った決定的 ID 生成になっているか
.claude/skills/3ai/agents/codex-design-reviewer.md:15:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
.claude/skills/3ai/agents/codex-design-reviewer.md:23:- `Debug, Clone, Serialize, Deserialize` が付与されているか（JsonSchema が必要な型は追記）
.claude/skills/3ai/agents/codex-design-reviewer.md:24:- 新しい依存は `[workspace.dependencies]` + `{ workspace = true }` 規約に従っているか
.claude/skills/3ai/agents/codex-design-reviewer.md:25:- ライブラリエラーには `thiserror` を使用しているか
.claude/skills/3ai/agents/codex-design-reviewer.md:26:- カーネルにレンダリング依存が混入していないか（`TriangleMesh` 生成のみ可）
.claude/skills/3ai/agents/codex-design-reviewer.md:37:- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか
.claude/skills/3ai/agents/codex-design-reviewer.md:48:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
.claude/skills/3ai/agents/codex-design-reviewer.md:49:    suggestion: "IdGenerator::next() に置き換えること"
Cargo.lock:306: "thiserror",
Cargo.lock:317: "thiserror",
Cargo.lock:632:name = "thiserror"
Cargo.lock:637: "thiserror-impl",
Cargo.lock:641:name = "thiserror-impl"
crates/mycad-format/src/component.rs:8:#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
crates/mycad-format/src/component.rs:83:impl schemars::JsonSchema for ComponentRef {
crates/mycad-format/src/component.rs:89:        <String as schemars::JsonSchema>::json_schema(gen)
crates/mycad-format/src/component.rs:96:#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
crates/mycad-format/src/error.rs:1:use thiserror::Error;
crates/mycad-format/src/lib.rs:6:pub use component::Component;
crates/mycad-format/src/lib.rs:7:pub use document::Document;
crates/mycad-format/src/lib.rs:8:pub use error::FormatError;
crates/mycad-format/src/lib.rs:9:pub use feature::Feature;
crates/mycad-format/CLAUDE.md:33:| `CreateBox`    | `make_cuboid`  |
crates/mycad-format/CLAUDE.md:34:| `CreateCylinder` | (未実装)    |
crates/mycad-kernel/Cargo.toml:11:thiserror = { workspace = true }
crates/mycad-format/src/feature.rs:1:use schemars::JsonSchema;
crates/mycad-format/src/feature.rs:5:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
crates/mycad-format/src/feature.rs:14:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
crates/mycad-format/src/feature.rs:28:    CreateCylinder {
crates/mycad-format/src/feature.rs:76:            | Feature::CreateCylinder { id, .. }
crates/mycad-cli/src/main.rs:4:use mycad_kernel::brep::topology::IdGenerator;
crates/mycad-cli/src/main.rs:5:use mycad_kernel::tessellation::{tessellate_solid, to_ascii_stl};
crates/mycad-cli/src/main.rs:43:    let mut gen = IdGenerator::new(0);
crates/mycad-cli/src/main.rs:46:    let mesh = tessellate_solid(&solid);
crates/mycad-build/src/lib.rs:2:use mycad_kernel::brep::topology::{IdGenerator, Solid};
crates/mycad-build/src/lib.rs:4:use mycad_kernel::primitives::make_cuboid;
crates/mycad-build/src/lib.rs:8:    gen: &mut IdGenerator,
crates/mycad-build/src/lib.rs:26:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
crates/mycad-build/src/lib.rs:27:        Feature::CreateCylinder { .. } => Err(KernelError::UnsupportedFeature {
crates/mycad-build/src/lib.rs:30:        Feature::CreateSphere { .. } => Err(KernelError::UnsupportedFeature {
crates/mycad-build/src/lib.rs:33:        Feature::Extrude { .. } => Err(KernelError::UnsupportedFeature { kind: "extrude" }),
crates/mycad-build/src/lib.rs:34:        Feature::Cut { .. } => Err(KernelError::UnsupportedFeature { kind: "cut" }),
crates/mycad-build/src/lib.rs:35:        Feature::Fuse { .. } => Err(KernelError::UnsupportedFeature { kind: "fuse" }),
crates/mycad-build/src/lib.rs:36:        Feature::Intersect { .. } => Err(KernelError::UnsupportedFeature { kind: "intersect" }),
crates/mycad-kernel/src/lib.rs:7:pub use brep::topology;
crates/mycad-kernel/src/lib.rs:8:pub use error::KernelError;
.git/logs/HEAD:11:d5c98d61ad288b808f65a3953abf6575aa3ceb87 eb557acee53a52ebe94aafffd472aca1d730b58a GS-Bacon <bacon0817.ix@gmail.com> 1779521125 +0000	commit: feat(kernel): add ASCII STL writer for TriangleMesh
crates/mycad-build/tests/feature_dispatcher.rs:3:use mycad_kernel::brep::topology::IdGenerator;
crates/mycad-build/tests/feature_dispatcher.rs:15:    let mut g = IdGenerator::new(0);
crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
crates/mycad-kernel/src/tessellation/mod.rs:2:pub use stl::to_ascii_stl;
crates/mycad-kernel/src/tessellation/mod.rs:10:pub struct TriangleMesh {
crates/mycad-kernel/src/tessellation/mod.rs:19:impl TriangleMesh {
crates/mycad-kernel/src/tessellation/mod.rs:34:impl Default for TriangleMesh {
crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
crates/mycad-kernel/src/tessellation/mod.rs:50:    let mut mesh = TriangleMesh::new();
crates/mycad-kernel/src/tessellation/mod.rs:98:    use crate::brep::topology::IdGenerator;
crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
crates/mycad-kernel/src/tessellation/mod.rs:103:        let mut id_gen = IdGenerator::new(0);
crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
crates/mycad-kernel/src/tessellation/mod.rs:105:        let mesh = tessellate_solid(&solid);
crates/mycad-kernel/src/tessellation/mod.rs:117:        let mut id_gen1 = IdGenerator::new(0);
crates/mycad-kernel/src/tessellation/mod.rs:118:        let mut id_gen2 = IdGenerator::new(0);
crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
crates/mycad-kernel/src/tessellation/mod.rs:123:        let mesh1 = tessellate_solid(&solid1);
crates/mycad-kernel/src/tessellation/mod.rs:124:        let mesh2 = tessellate_solid(&solid2);
.git/logs/refs/heads/claude/add-claude-guidelines-BKKtD:7:d5c98d61ad288b808f65a3953abf6575aa3ceb87 eb557acee53a52ebe94aafffd472aca1d730b58a GS-Bacon <bacon0817.ix@gmail.com> 1779521125 +0000	commit: feat(kernel): add ASCII STL writer for TriangleMesh
crates/mycad-kernel/src/tessellation/stl.rs:1:use super::TriangleMesh;
crates/mycad-kernel/src/tessellation/stl.rs:4:pub fn to_ascii_stl(mesh: &TriangleMesh, solid_name: &str) -> String {
crates/mycad-kernel/src/tessellation/stl.rs:51:    use crate::brep::topology::IdGenerator;
crates/mycad-kernel/src/tessellation/stl.rs:52:    use crate::primitives::make_cuboid;
crates/mycad-kernel/src/tessellation/stl.rs:53:    use crate::tessellation::tessellate_solid;
crates/mycad-kernel/src/tessellation/stl.rs:57:        let mut g = IdGenerator::new(0);
crates/mycad-kernel/src/tessellation/stl.rs:58:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
crates/mycad-kernel/src/tessellation/stl.rs:59:        let mesh = tessellate_solid(&solid);
crates/mycad-kernel/src/tessellation/stl.rs:74:            let mut g = IdGenerator::new(0);
crates/mycad-kernel/src/tessellation/stl.rs:75:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
crates/mycad-kernel/src/tessellation/stl.rs:76:            to_ascii_stl(&tessellate_solid(&s), "cube")
crates/mycad-kernel/src/primitives/cuboid.rs:1:use crate::brep::topology::{IdGenerator, Solid};
crates/mycad-kernel/src/primitives/cuboid.rs:16:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
crates/mycad-kernel/src/primitives/cuboid.rs:223:    use crate::brep::topology::IdGenerator;
crates/mycad-kernel/src/primitives/cuboid.rs:227:        let mut id_gen = IdGenerator::new(0);
crates/mycad-kernel/src/primitives/cuboid.rs:228:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
crates/mycad-kernel/src/primitives/cuboid.rs:247:        let mut id_gen = IdGenerator::new(0);
crates/mycad-kernel/src/primitives/cuboid.rs:248:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
crates/mycad-kernel/src/primitives/cuboid.rs:276:        let mut id_gen1 = IdGenerator::new(0);
crates/mycad-kernel/src/primitives/cuboid.rs:277:        let mut id_gen2 = IdGenerator::new(0);
crates/mycad-kernel/src/primitives/cuboid.rs:279:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
crates/mycad-kernel/src/primitives/cuboid.rs:280:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);
crates/mycad-kernel/src/error.rs:1:use thiserror::Error;
crates/mycad-kernel/src/error.rs:12:    UnsupportedFeature { kind: &'static str },
crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
crates/mycad-kernel/src/brep/topology.rs:21:pub struct HalfEdge {
crates/mycad-kernel/src/brep/topology.rs:83:    pub half_edges: Vec<HalfEdge>,
crates/mycad-kernel/src/brep/topology.rs:138:        self.half_edges.push(HalfEdge {
crates/mycad-kernel/src/brep/topology.rs:185:pub struct IdGenerator {
crates/mycad-kernel/src/brep/topology.rs:189:impl IdGenerator {
crates/mycad-kernel/src/brep/topology.rs:208:        let mut gen1 = IdGenerator::new(100);
crates/mycad-kernel/src/brep/topology.rs:209:        let mut gen2 = IdGenerator::new(100);
crates/mycad-kernel/benches/tessellation.rs:1:use mycad_kernel::brep::topology::IdGenerator;
crates/mycad-kernel/benches/tessellation.rs:2:use mycad_kernel::primitives::make_cuboid;
crates/mycad-kernel/benches/tessellation.rs:3:use mycad_kernel::tessellation::tessellate_solid;
crates/mycad-kernel/benches/tessellation.rs:14:        let mut id_gen = IdGenerator::new(i as u64);
crates/mycad-kernel/benches/tessellation.rs:15:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
crates/mycad-kernel/benches/tessellation.rs:16:        let _mesh = tessellate_solid(&solid);
crates/mycad-kernel/tests/cuboid_roundtrip.rs:1:use mycad_kernel::brep::topology::IdGenerator;
crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
crates/mycad-kernel/tests/cuboid_roundtrip.rs:3:use mycad_kernel::tessellation::tessellate_solid;
crates/mycad-kernel/tests/cuboid_roundtrip.rs:8:    let mut id_gen = IdGenerator::new(0);
crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
crates/mycad-kernel/tests/cuboid_roundtrip.rs:17:    let mesh = tessellate_solid(&solid);
crates/mycad-kernel/tests/cuboid_roundtrip.rs:30:        let mut id_gen = IdGenerator::new(42);
crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
crates/mycad-kernel/tests/cuboid_roundtrip.rs:32:        tessellate_solid(&solid)
crates/mycad-kernel/CLAUDE.md:14:- すべてのEdgeは正確に2つのHalfEdgeを持つ
crates/mycad-kernel/CLAUDE.md:16:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
crates/mycad-kernel/CLAUDE.md:17:- Loopは閉じたHalfEdge列（最後のHalfEdgeの終点 = 最初のHalfEdgeの始点）
crates/mycad-kernel/CLAUDE.md:18:- EntityIDは `IdGenerator` で決定的に生成される
crates/mycad-kernel/CLAUDE.md:23:2. 関数シグネチャ: `pub fn make_xxx(params..., id_gen: &mut IdGenerator) -> Solid`
crates/mycad-kernel/CLAUDE.md:24:3. `primitives/mod.rs` に `mod` と `pub use` を追加
crates/mycad-kernel/CLAUDE.md:25:4. 決定性テストを必ず書く（同じ `IdGenerator` 初期値で2回生成し、結果が同一であることを確認）

codex
既存実装の前提を確認できました。次はトポロジー型と現行テッセレータの具体的な制約を見て、提案がそのまま成立しない箇所を潰します。
exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/brep/topology.rs" in /home/bacon/mycad
 succeeded in 0ms:
use crate::geometry::curve::Curve;
use crate::geometry::surface::Surface;
use crate::geometry::Point;
use serde::{Deserialize, Serialize};

/// Unique identifier for topological entities.
/// Generated deterministically by creation order within a feature.
pub type EntityId = u64;

/// A vertex in B-rep — a point in 3D space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vertex {
    pub id: EntityId,
    pub point: Point,
}

/// A half-edge — one side of an edge, used within a loop (wire).
/// Half-edges are oriented: they go from `start_vertex` to the next
/// half-edge's start vertex.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HalfEdge {
    pub id: EntityId,
    /// Index of the start vertex in the parent solid's vertex list.
    pub start_vertex: usize,
    /// Index of the edge this half-edge belongs to.
    pub edge: usize,
    /// Whether this half-edge follows the edge's natural direction.
    pub forward: bool,
}

/// An edge — a curve segment bounded by two vertices.
/// Each edge has exactly two half-edges (one for each adjacent face).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: EntityId,
    /// Indices of the two bounding vertices in the parent solid's vertex list.
    pub vertices: [usize; 2],
    /// The geometric curve this edge lies on.
    pub curve: Curve,
    /// Parameter range [t_start, t_end] on the curve.
    pub t_range: [f64; 2],
}

/// A loop (wire) — a closed sequence of half-edges forming a boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Loop {
    pub id: EntityId,
    /// Ordered indices of half-edges in the parent solid's half-edge list.
    pub half_edges: Vec<usize>,
}

/// A face — a bounded region on a surface.
/// Has one outer loop and zero or more inner loops (holes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Face {
    pub id: EntityId,
    /// The geometric surface this face lies on.
    pub surface: Surface,
    /// Index of the outer loop in the parent solid's loop list.
    pub outer_loop: usize,
    /// Indices of inner loops (holes) in the parent solid's loop list.
    pub inner_loops: Vec<usize>,
    /// Whether the face normal matches the surface normal.
    pub same_sense: bool,
}

/// A shell — a connected set of faces forming a closed or open surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shell {
    pub id: EntityId,
    /// Indices of faces in the parent solid's face list.
    pub faces: Vec<usize>,
    /// Whether this shell is closed (watertight).
    pub closed: bool,
}

/// A solid — the top-level B-rep entity.
/// Contains all topological entities in flat arrays for deterministic ordering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Solid {
    pub id: EntityId,
    pub vertices: Vec<Vertex>,
    pub half_edges: Vec<HalfEdge>,
    pub edges: Vec<Edge>,
    pub loops: Vec<Loop>,
    pub faces: Vec<Face>,
    pub shells: Vec<Shell>,
}

impl Solid {
    /// Create an empty solid with the given ID.
    pub fn new(id: EntityId) -> Self {
        Self {
            id,
            vertices: Vec::new(),
            half_edges: Vec::new(),
            edges: Vec::new(),
            loops: Vec::new(),
            faces: Vec::new(),
            shells: Vec::new(),
        }
    }

    /// Add a vertex and return its index.
    pub fn add_vertex(&mut self, id: EntityId, point: Point) -> usize {
        let idx = self.vertices.len();
        self.vertices.push(Vertex { id, point });
        idx
    }

    /// Add an edge and return its index.
    pub fn add_edge(
        &mut self,
        id: EntityId,
        vertices: [usize; 2],
        curve: Curve,
        t_range: [f64; 2],
    ) -> usize {
        let idx = self.edges.len();
        self.edges.push(Edge {
            id,
            vertices,
            curve,
            t_range,
        });
        idx
    }

    /// Add a half-edge and return its index.
    pub fn add_half_edge(
        &mut self,
        id: EntityId,
        start_vertex: usize,
        edge: usize,
        forward: bool,
    ) -> usize {
        let idx = self.half_edges.len();
        self.half_edges.push(HalfEdge {
            id,
            start_vertex,
            edge,
            forward,
        });
        idx
    }

    /// Add a loop and return its index.
    pub fn add_loop(&mut self, id: EntityId, half_edges: Vec<usize>) -> usize {
        let idx = self.loops.len();
        self.loops.push(Loop { id, half_edges });
        idx
    }

    /// Add a face and return its index.
    pub fn add_face(
        &mut self,
        id: EntityId,
        surface: Surface,
        outer_loop: usize,
        inner_loops: Vec<usize>,
        same_sense: bool,
    ) -> usize {
        let idx = self.faces.len();
        self.faces.push(Face {
            id,
            surface,
            outer_loop,
            inner_loops,
            same_sense,
        });
        idx
    }

    /// Add a shell and return its index.
    pub fn add_shell(&mut self, id: EntityId, faces: Vec<usize>, closed: bool) -> usize {
        let idx = self.shells.len();
        self.shells.push(Shell { id, faces, closed });
        idx
    }
}

/// Counter for generating deterministic entity IDs.
/// Each feature gets its own counter starting from a feature-specific offset.
#[derive(Debug, Clone)]
pub struct IdGenerator {
    next_id: EntityId,
}

impl IdGenerator {
    pub fn new(start: EntityId) -> Self {
        Self { next_id: start }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> EntityId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_generator_is_deterministic() {
        let mut gen1 = IdGenerator::new(100);
        let mut gen2 = IdGenerator::new(100);
        for _ in 0..10 {
            assert_eq!(gen1.next(), gen2.next());
        }
    }

    #[test]
    fn test_solid_add_vertex() {
        let mut solid = Solid::new(0);
        let idx = solid.add_vertex(1, Point::new(1.0, 2.0, 3.0));
        assert_eq!(idx, 0);
        assert_eq!(solid.vertices[0].id, 1);
        assert_eq!(solid.vertices[0].point, Point::new(1.0, 2.0, 3.0));
    }
}

exec
/bin/bash -lc "sed -n '1,340p' crates/mycad-kernel/src/primitives/cuboid.rs" in /home/bacon/mycad
 succeeded in 0ms:
use crate::brep::topology::{IdGenerator, Solid};
use crate::geometry::curve::Curve;
use crate::geometry::surface::Surface;
use crate::geometry::Point;
use crate::geometry::Vec3;

/// Create a cuboid (box) B-rep solid centered at the origin.
///
/// Parameters:
/// - `dx`: width (X dimension)
/// - `dy`: height (Y dimension)
/// - `dz`: depth (Z dimension)
/// - `id_gen`: deterministic ID generator
///
/// The cuboid spans from (-dx/2, -dy/2, -dz/2) to (dx/2, dy/2, dz/2).
pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
    let mut solid = Solid::new(id_gen.next());

    let hx = dx / 2.0;
    let hy = dy / 2.0;
    let hz = dz / 2.0;

    // 8 vertices of the cuboid
    //
    //     6-------7
    //    /|      /|
    //   4-------5 |
    //   | 2-----|-3
    //   |/      |/
    //   0-------1
    //
    // v0 = (-hx, -hy, -hz), v1 = (hx, -hy, -hz), ...
    let v_positions = [
        Point::new(-hx, -hy, -hz), // 0: front-bottom-left
        Point::new(hx, -hy, -hz),  // 1: front-bottom-right
        Point::new(-hx, hy, -hz),  // 2: front-top-left
        Point::new(hx, hy, -hz),   // 3: front-top-right
        Point::new(-hx, -hy, hz),  // 4: back-bottom-left
        Point::new(hx, -hy, hz),   // 5: back-bottom-right
        Point::new(-hx, hy, hz),   // 6: back-top-left
        Point::new(hx, hy, hz),    // 7: back-top-right
    ];

    let v: Vec<usize> = v_positions
        .iter()
        .map(|p| solid.add_vertex(id_gen.next(), *p))
        .collect();

    // 12 edges of the cuboid
    // Each edge is defined by [start_vertex_index, end_vertex_index]
    let edge_defs: [(usize, usize); 12] = [
        (0, 1), // e0:  bottom-front
        (1, 3), // e1:  right-front
        (3, 2), // e2:  top-front
        (2, 0), // e3:  left-front
        (4, 5), // e4:  bottom-back
        (5, 7), // e5:  right-back
        (7, 6), // e6:  top-back
        (6, 4), // e7:  left-back
        (0, 4), // e8:  bottom-left
        (1, 5), // e9:  bottom-right
        (3, 7), // e10: top-right
        (2, 6), // e11: top-left
    ];

    let e: Vec<usize> = edge_defs
        .iter()
        .map(|&(v0, v1)| {
            let p0 = solid.vertices[v[v0]].point;
            let p1 = solid.vertices[v[v1]].point;
            let direction = p1 - p0;
            solid.add_edge(
                id_gen.next(),
                [v[v0], v[v1]],
                Curve::Line {
                    origin: p0,
                    direction,
                },
                [0.0, 1.0],
            )
        })
        .collect();

    // 6 faces of the cuboid
    // Each face is defined by its 4 edges and their orientation (forward/backward),
    // plus the surface definition.
    struct FaceDef {
        // (edge_index, forward, start_vertex_index)
        half_edges: [(usize, bool, usize); 4],
        surface: Surface,
        same_sense: bool,
    }

    let face_defs = [
        // Front face (Z = -hz): vertices 0,1,3,2 — normal -Z
        FaceDef {
            half_edges: [
                (0, true, 0), // e0: 0->1
                (1, true, 1), // e1: 1->3
                (2, true, 3), // e2: 3->2
                (3, true, 2), // e3: 2->0
            ],
            surface: Surface::Plane {
                origin: Point::new(0.0, 0.0, -hz),
                normal: -Vec3::z(),
                u_axis: Vec3::x(),
                v_axis: Vec3::y(),
            },
            same_sense: true,
        },
        // Back face (Z = +hz): vertices 5,4,6,7 — normal +Z
        FaceDef {
            half_edges: [
                (4, false, 5), // e4: 5->4
                (7, false, 4), // e7: 4->6
                (6, false, 6), // e6: 6->7
                (5, false, 7), // e5: 7->5
            ],
            surface: Surface::Plane {
                origin: Point::new(0.0, 0.0, hz),
                normal: Vec3::z(),
                u_axis: -Vec3::x(),
                v_axis: Vec3::y(),
            },
            same_sense: true,
        },
        // Bottom face (Y = -hy): vertices 0,4,5,1 — normal -Y
        FaceDef {
            half_edges: [
                (8, true, 0),  // e8: 0->4
                (4, true, 4),  // e4: 4->5
                (9, false, 5), // e9: 5->1
                (0, false, 1), // e0: 1->0
            ],
            surface: Surface::Plane {
                origin: Point::new(0.0, -hy, 0.0),
                normal: -Vec3::y(),
                u_axis: Vec3::x(),
                v_axis: Vec3::z(),
            },
            same_sense: true,
        },
        // Top face (Y = +hy): vertices 2,3,7,6 — normal +Y
        FaceDef {
            half_edges: [
                (2, false, 2),  // e2: 2->3
                (10, true, 3),  // e10: 3->7
                (6, true, 7),   // e6: 7->6
                (11, false, 6), // e11: 6->2
            ],
            surface: Surface::Plane {
                origin: Point::new(0.0, hy, 0.0),
                normal: Vec3::y(),
                u_axis: Vec3::x(),
                v_axis: Vec3::z(),
            },
            same_sense: true,
        },
        // Left face (X = -hx): vertices 0,2,6,4 — normal -X
        FaceDef {
            half_edges: [
                (3, false, 0), // e3: 0->2
                (11, true, 2), // e11: 2->6
                (7, true, 6),  // e7: 6->4
                (8, false, 4), // e8: 4->0
            ],
            surface: Surface::Plane {
                origin: Point::new(-hx, 0.0, 0.0),
                normal: -Vec3::x(),
                u_axis: Vec3::y(),
                v_axis: Vec3::z(),
            },
            same_sense: true,
        },
        // Right face (X = +hx): vertices 1,5,7,3 — normal +X
        FaceDef {
            half_edges: [
                (9, true, 1),   // e9: 1->5
                (5, true, 5),   // e5: 5->7
                (10, false, 7), // e10: 7->3
                (1, false, 3),  // e1: 3->1
            ],
            surface: Surface::Plane {
                origin: Point::new(hx, 0.0, 0.0),
                normal: Vec3::x(),
                u_axis: Vec3::y(),
                v_axis: Vec3::z(),
            },
            same_sense: true,
        },
    ];

    let mut face_indices = Vec::new();

    for face_def in &face_defs {
        let he_indices: Vec<usize> = face_def
            .half_edges
            .iter()
            .map(|&(edge_idx, forward, start_v)| {
                solid.add_half_edge(id_gen.next(), v[start_v], e[edge_idx], forward)
            })
            .collect();

        let loop_idx = solid.add_loop(id_gen.next(), he_indices);
        let face_idx = solid.add_face(
            id_gen.next(),
            face_def.surface.clone(),
            loop_idx,
            vec![],
            face_def.same_sense,
        );
        face_indices.push(face_idx);
    }

    solid.add_shell(id_gen.next(), face_indices, true);

    solid
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::topology::IdGenerator;

    #[test]
    fn test_cuboid_topology() {
        let mut id_gen = IdGenerator::new(0);
        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);

        assert_eq!(solid.vertices.len(), 8, "cuboid should have 8 vertices");
        assert_eq!(solid.edges.len(), 12, "cuboid should have 12 edges");
        assert_eq!(solid.faces.len(), 6, "cuboid should have 6 faces");
        assert_eq!(solid.shells.len(), 1, "cuboid should have 1 shell");
        assert!(solid.shells[0].closed, "cuboid shell should be closed");
        // 6 faces * 4 half-edges = 24 half-edges
        assert_eq!(
            solid.half_edges.len(),
            24,
            "cuboid should have 24 half-edges"
        );
        // 6 loops (one per face)
        assert_eq!(solid.loops.len(), 6, "cuboid should have 6 loops");
    }

    #[test]
    fn test_cuboid_vertex_positions() {
        let mut id_gen = IdGenerator::new(0);
        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);

        // Check that all vertices are at the expected positions
        let expected = [
            Point::new(-1.0, -2.0, -3.0),
            Point::new(1.0, -2.0, -3.0),
            Point::new(-1.0, 2.0, -3.0),
            Point::new(1.0, 2.0, -3.0),
            Point::new(-1.0, -2.0, 3.0),
            Point::new(1.0, -2.0, 3.0),
            Point::new(-1.0, 2.0, 3.0),
            Point::new(1.0, 2.0, 3.0),
        ];

        for (v, exp) in solid.vertices.iter().zip(expected.iter()) {
            assert!(
                (v.point - exp).norm() < 1e-12,
                "vertex {:?} doesn't match expected {:?}",
                v.point,
                exp
            );
        }
    }

    #[test]
    fn test_cuboid_deterministic() {
        // Building the same cuboid twice with the same ID generator should
        // produce identical results.
        let mut id_gen1 = IdGenerator::new(0);
        let mut id_gen2 = IdGenerator::new(0);

        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);

        assert_eq!(solid1.vertices.len(), solid2.vertices.len());
        for (v1, v2) in solid1.vertices.iter().zip(solid2.vertices.iter()) {
            assert_eq!(v1.id, v2.id);
            assert_eq!(v1.point, v2.point);
        }

        assert_eq!(solid1.edges.len(), solid2.edges.len());
        for (e1, e2) in solid1.edges.iter().zip(solid2.edges.iter()) {
            assert_eq!(e1.id, e2.id);
            assert_eq!(e1.vertices, e2.vertices);
        }

        assert_eq!(solid1.faces.len(), solid2.faces.len());
        for (f1, f2) in solid1.faces.iter().zip(solid2.faces.iter()) {
            assert_eq!(f1.id, f2.id);
        }
    }
}

exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/geometry/curve.rs" in /home/bacon/mycad
 succeeded in 0ms:
use super::math::orthonormal_basis;
use super::{Point, Vec3};
use serde::{Deserialize, Serialize};

/// A geometric curve in 3D space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Curve {
    /// A line defined by an origin point and a direction vector.
    Line { origin: Point, direction: Vec3 },
    /// A circle defined by a center, normal, and radius.
    Circle {
        center: Point,
        normal: Vec3,
        radius: f64,
    },
}

impl Curve {
    /// Evaluate the curve at parameter t.
    /// For a line: origin + t * direction
    /// For a circle: center + radius * (cos(t) * u + sin(t) * v)
    pub fn evaluate(&self, t: f64) -> Point {
        match self {
            Curve::Line { origin, direction } => origin + t * direction,
            Curve::Circle {
                center,
                normal,
                radius,
            } => {
                // Build a local coordinate system from the normal
                let (u, v) = orthonormal_basis(normal);
                center + *radius * (t.cos() * u + t.sin() * v)
            }
        }
    }
}

exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/tessellation/mod.rs" in /home/bacon/mycad
 succeeded in 0ms:
pub mod stl;
pub use stl::to_ascii_stl;

use crate::brep::topology::Solid;
use crate::geometry::Point;
use serde::{Deserialize, Serialize};

/// A triangle mesh for rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriangleMesh {
    /// Vertex positions (x, y, z).
    pub positions: Vec<[f64; 3]>,
    /// Normal vectors per vertex.
    pub normals: Vec<[f64; 3]>,
    /// Triangle indices (every 3 indices form a triangle).
    pub indices: Vec<u32>,
}

impl TriangleMesh {
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Number of triangles in the mesh.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

impl Default for TriangleMesh {
    fn default() -> Self {
        Self::new()
    }
}

/// Tessellate a B-rep solid into a triangle mesh.
///
/// Each face's outer loop is triangulated as a fan from its first vertex.
/// Normals are evaluated per vertex from the underlying surface, so curved
/// faces get correct shading rather than a single per-face normal.
///
/// Not yet handled: interior subdivision of curved faces (uv sampling) — a
/// curved face is still approximated by its boundary fan — and inner loops
/// (holes). Both are deferred to the curved-primitive work (Phase 3).
pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
    let mut mesh = TriangleMesh::new();

    for face in &solid.faces {
        let outer_loop = &solid.loops[face.outer_loop];

        // Collect the vertices of this face's outer loop
        let loop_vertices: Vec<&Point> = outer_loop
            .half_edges
            .iter()
            .map(|&he_idx| {
                let he = &solid.half_edges[he_idx];
                &solid.vertices[he.start_vertex].point
            })
            .collect();

        if loop_vertices.len() < 3 {
            continue;
        }

        // Add vertices and triangulate as a fan from vertex 0.
        // Normals are evaluated per vertex from the surface so curved faces
        // are shaded correctly (planar faces yield a constant normal).
        let base_idx = mesh.positions.len() as u32;

        for &v in &loop_vertices {
            let normal = face.surface.normal_at_point(v);
            let n = if face.same_sense {
                [normal.x, normal.y, normal.z]
            } else {
                [-normal.x, -normal.y, -normal.z]
            };
            mesh.positions.push([v.x, v.y, v.z]);
            mesh.normals.push(n);
        }

        for i in 1..(loop_vertices.len() as u32 - 1) {
            mesh.indices.push(base_idx);
            mesh.indices.push(base_idx + i);
            mesh.indices.push(base_idx + i + 1);
        }
    }

    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::topology::IdGenerator;
    use crate::primitives::make_cuboid;

    #[test]
    fn test_tessellate_cuboid() {
        let mut id_gen = IdGenerator::new(0);
        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
        let mesh = tessellate_solid(&solid);

        // A cuboid has 6 faces, each tessellated as 2 triangles = 12 triangles
        assert_eq!(mesh.triangle_count(), 12);
        // 6 faces * 4 vertices = 24 vertices (not shared across faces for correct normals)
        assert_eq!(mesh.positions.len(), 24);
        assert_eq!(mesh.normals.len(), 24);
        assert_eq!(mesh.indices.len(), 36);
    }

    #[test]
    fn test_tessellation_deterministic() {
        let mut id_gen1 = IdGenerator::new(0);
        let mut id_gen2 = IdGenerator::new(0);

        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);

        let mesh1 = tessellate_solid(&solid1);
        let mesh2 = tessellate_solid(&solid2);

        assert_eq!(mesh1.positions, mesh2.positions);
        assert_eq!(mesh1.normals, mesh2.normals);
        assert_eq!(mesh1.indices, mesh2.indices);
    }
}

exec
/bin/bash -lc "sed -n '1,320p' crates/mycad-kernel/src/geometry/surface.rs" in /home/bacon/mycad
 succeeded in 0ms:
use super::math::orthonormal_basis;
use super::{Point, Vec3};
use serde::{Deserialize, Serialize};

/// A geometric surface in 3D space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Surface {
    /// An infinite plane.
    Plane {
        origin: Point,
        normal: Vec3,
        u_axis: Vec3,
        v_axis: Vec3,
    },
    /// A cylindrical surface.
    Cylinder {
        origin: Point,
        axis: Vec3,
        radius: f64,
    },
    /// A spherical surface.
    Sphere { center: Point, radius: f64 },
    /// A conical surface.
    Cone {
        apex: Point,
        axis: Vec3,
        half_angle: f64,
    },
}

impl Surface {
    /// Evaluate the surface at parameters (u, v).
    pub fn evaluate(&self, u: f64, v: f64) -> Point {
        match self {
            Surface::Plane {
                origin,
                u_axis,
                v_axis,
                ..
            } => origin + u * u_axis + v * v_axis,

            Surface::Cylinder {
                origin,
                axis,
                radius,
            } => {
                let (bu, bv) = orthonormal_basis(axis);
                origin + *radius * (u.cos() * bu + u.sin() * bv) + v * axis.normalize()
            }

            Surface::Sphere { center, radius } => {
                // u = longitude [0, 2π), v = latitude [-π/2, π/2]
                let x = radius * v.cos() * u.cos();
                let y = radius * v.cos() * u.sin();
                let z = radius * v.sin();
                Point::new(center.x + x, center.y + y, center.z + z)
            }

            Surface::Cone {
                apex,
                axis,
                half_angle,
            } => {
                let (bu, bv) = orthonormal_basis(axis);
                let r = v * half_angle.tan();
                apex + v * axis.normalize() + r * (u.cos() * bu + u.sin() * bv)
            }
        }
    }

    /// Get the normal vector at parameters (u, v).
    pub fn normal_at(&self, u: f64, v: f64) -> Vec3 {
        match self {
            Surface::Plane { normal, .. } => normal.normalize(),

            Surface::Cylinder { axis, .. } => {
                let (bu, bv) = orthonormal_basis(axis);
                (u.cos() * bu + u.sin() * bv).normalize()
            }

            Surface::Sphere { center, .. } => {
                let p = self.evaluate(u, v);
                (p - center).normalize()
            }

            Surface::Cone {
                apex,
                axis,
                half_angle,
            } => {
                let p = self.evaluate(u, v);
                let a = axis.normalize();
                let to_point = p - apex;
                let along_axis = to_point.dot(&a) * a;
                let radial = to_point - along_axis;
                if radial.norm() < 1e-12 {
                    return a;
                }
                let radial_n = radial.normalize();
                (radial_n * half_angle.cos() - a * half_angle.sin()).normalize()
            }
        }
    }

    /// Inverse of [`Surface::evaluate`]: find the parameters (u, v) whose
    /// evaluation lands on (or nearest to) `p`.
    ///
    /// Used so that geometry-consuming algorithms (e.g. tessellation) can ask
    /// for the surface normal at a concrete 3D point without assuming the
    /// surface is planar / has a constant normal.
    pub fn uv_of(&self, p: &Point) -> (f64, f64) {
        match self {
            Surface::Plane {
                origin,
                u_axis,
                v_axis,
                ..
            } => {
                let d = p - origin;
                (
                    d.dot(u_axis) / u_axis.dot(u_axis),
                    d.dot(v_axis) / v_axis.dot(v_axis),
                )
            }

            Surface::Cylinder { origin, axis, .. } => {
                let a = axis.normalize();
                let (bu, bv) = orthonormal_basis(axis);
                let d = p - origin;
                let v = d.dot(&a);
                let radial = d - v * a;
                (radial.dot(&bv).atan2(radial.dot(&bu)), v)
            }

            Surface::Sphere { center, radius } => {
                let d = p - center;
                let v = (d.z / radius).clamp(-1.0, 1.0).asin();
                let u = d.y.atan2(d.x);
                (u, v)
            }

            Surface::Cone { apex, axis, .. } => {
                let a = axis.normalize();
                let (bu, bv) = orthonormal_basis(axis);
                let d = p - apex;
                let v = d.dot(&a);
                let radial = d - v * a;
                (radial.dot(&bv).atan2(radial.dot(&bu)), v)
            }
        }
    }

    /// Surface normal at a concrete 3D point lying on (or near) the surface.
    ///
    /// Convenience over [`Surface::uv_of`] + [`Surface::normal_at`] so callers
    /// never have to special-case planar vs curved surfaces.
    pub fn normal_at_point(&self, p: &Point) -> Vec3 {
        let (u, v) = self.uv_of(p);
        self.normal_at(u, v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
        let p = surface.evaluate(u, v);
        let (u2, v2) = surface.uv_of(&p);
        assert_relative_eq!(u, u2, epsilon = 1e-9);
        assert_relative_eq!(v, v2, epsilon = 1e-9);
    }

    #[test]
    fn test_uv_of_roundtrip_plane() {
        let surface = Surface::Plane {
            origin: Point::new(1.0, 2.0, 3.0),
            normal: Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        };
        assert_uv_roundtrip(&surface, 0.7, -1.3);
    }

    #[test]
    fn test_uv_of_roundtrip_cylinder() {
        let surface = Surface::Cylinder {
            origin: Point::new(0.0, 0.0, 0.0),
            axis: Vec3::z(),
            radius: 2.5,
        };
        assert_uv_roundtrip(&surface, 0.9, 1.7);
    }

    #[test]
    fn test_uv_of_roundtrip_sphere() {
        let surface = Surface::Sphere {
            center: Point::new(-1.0, 0.5, 2.0),
            radius: 3.0,
        };
        assert_uv_roundtrip(&surface, 1.1, 0.6);
    }

    #[test]
    fn test_uv_of_roundtrip_cone() {
        let surface = Surface::Cone {
            apex: Point::new(0.0, 0.0, 0.0),
            axis: Vec3::z(),
            half_angle: 0.5,
        };
        assert_uv_roundtrip(&surface, 0.8, 2.0);
    }

    /// A curved surface must report different normals at different points —
    /// the property the tessellator now relies on instead of a single
    /// per-face normal.
    #[test]
    fn test_sphere_normal_varies_per_point() {
        let surface = Surface::Sphere {
            center: Point::origin(),
            radius: 1.0,
        };
        let p1 = surface.evaluate(0.0, 0.0);
        let p2 = surface.evaluate(std::f64::consts::FRAC_PI_2, 0.3);
        let n1 = surface.normal_at_point(&p1);
        let n2 = surface.normal_at_point(&p2);
        // Distinct surface points yield non-parallel outward normals.
        assert!((n1.dot(&n2)).abs() < 0.99);
    }

    #[test]
    fn test_plane_normal_constant_per_point() {
        let surface = Surface::Plane {
            origin: Point::origin(),
            normal: Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        };
        let n1 = surface.normal_at_point(&Point::new(5.0, -2.0, 0.0));
        let n2 = surface.normal_at_point(&Point::new(-3.0, 4.0, 0.0));
        assert_relative_eq!(n1.dot(&n2), 1.0, epsilon = 1e-12);
    }
}

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/src/geometry/math.rs" in /home/bacon/mycad
 succeeded in 0ms:
use super::Vec3;

/// Build an orthonormal basis (u, v) from a normal vector.
///
/// Given a normal vector, computes two perpendicular unit vectors
/// that together with the normal form a right-handed coordinate system.
pub fn orthonormal_basis(normal: &Vec3) -> (Vec3, Vec3) {
    let n = normal.normalize();
    // Choose a vector not parallel to n
    let not_parallel = if n.x.abs() < 0.9 {
        Vec3::x()
    } else {
        Vec3::y()
    };
    let u = n.cross(&not_parallel).normalize();
    let v = n.cross(&u);
    (u, v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_orthonormal_basis_z_axis() {
        let (u, v) = orthonormal_basis(&Vec3::z());
        assert_relative_eq!(u.norm(), 1.0, epsilon = 1e-12);
        assert_relative_eq!(v.norm(), 1.0, epsilon = 1e-12);
        assert_relative_eq!(u.dot(&v), 0.0, epsilon = 1e-12);
        assert_relative_eq!(u.dot(&Vec3::z()), 0.0, epsilon = 1e-12);
    }

    #[test]
    fn test_orthonormal_basis_x_axis() {
        let (u, v) = orthonormal_basis(&Vec3::x());
        assert_relative_eq!(u.norm(), 1.0, epsilon = 1e-12);
        assert_relative_eq!(v.norm(), 1.0, epsilon = 1e-12);
        assert_relative_eq!(u.dot(&v), 0.0, epsilon = 1e-12);
        assert_relative_eq!(u.dot(&Vec3::x()), 0.0, epsilon = 1e-12);
    }
}

codex
トポロジー型を読むと、現行 `HalfEdge` には `twin/next/prev` 自体がありません。提案が「マニフォルド規約」を言語化していても、レビュー観点の要件を満たすにはこのギャップをどう埋めるかが重要です。
exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-cli/src/main.rs" in /home/bacon/mycad
 succeeded in 0ms:
use clap::{Parser, Subcommand};
use mycad_build::build_solid_from_features;
use mycad_format::Document;
use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::tessellation::{tessellate_solid, to_ascii_stl};
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(name = "mycad", version, about = "MyCad CAD kernel CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Convert a .mycad file to STL
    Export {
        /// Input .mycad file
        input: PathBuf,
        /// Output STL file
        #[arg(short, long)]
        output: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Export { input, output } => {
            if let Err(msg) = run_export(&input, &output) {
                eprintln!("error: {msg}");
                process::exit(1);
            }
        }
    }
}

fn run_export(input: &std::path::Path, output: &std::path::Path) -> Result<(), String> {
    let doc = Document::from_path(input)
        .map_err(|e| format!("failed to read {}: {e}", input.display()))?;
    let mut gen = IdGenerator::new(0);
    let solid = build_solid_from_features(&doc.root_component.features, &mut gen)
        .map_err(|e| format!("failed to build solid: {e}"))?;
    let mesh = tessellate_solid(&solid);
    let stl = to_ascii_stl(&mesh, "model");
    std::fs::write(output, stl)
        .map_err(|e| format!("failed to write {}: {e}", output.display()))?;
    Ok(())
}

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-build/src/lib.rs" in /home/bacon/mycad
 succeeded in 0ms:
use mycad_format::Feature;
use mycad_kernel::brep::topology::{IdGenerator, Solid};
use mycad_kernel::error::KernelError;
use mycad_kernel::primitives::make_cuboid;

pub fn build_solid_from_features(
    features: &[Feature],
    gen: &mut IdGenerator,
) -> Result<Solid, KernelError> {
    if features.is_empty() {
        return Err(KernelError::EmptyFeatureList);
    }

    if features.len() > 1 {
        return Err(KernelError::MultipleFeatures {
            count: features.len(),
        });
    }

    match &features[0] {
        Feature::CreateBox {
            width,
            height,
            depth,
            ..
        } => Ok(make_cuboid(*width, *height, *depth, gen)),
        Feature::CreateCylinder { .. } => Err(KernelError::UnsupportedFeature {
            kind: "create_cylinder",
        }),
        Feature::CreateSphere { .. } => Err(KernelError::UnsupportedFeature {
            kind: "create_sphere",
        }),
        Feature::Extrude { .. } => Err(KernelError::UnsupportedFeature { kind: "extrude" }),
        Feature::Cut { .. } => Err(KernelError::UnsupportedFeature { kind: "cut" }),
        Feature::Fuse { .. } => Err(KernelError::UnsupportedFeature { kind: "fuse" }),
        Feature::Intersect { .. } => Err(KernelError::UnsupportedFeature { kind: "intersect" }),
    }
}

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-format/src/feature.rs" in /home/bacon/mycad
 succeeded in 0ms:
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A reference to a topological entity (face, edge, vertex) on a feature's result.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EntityRef {
    /// The feature that created this entity.
    pub feature_id: String,
    /// The role of this entity in the feature (e.g., "top_face", "side_edge_0").
    pub role: String,
}

/// A feature — one step in the operation history.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type")]
pub enum Feature {
    /// Create a box primitive.
    #[serde(rename = "create_box")]
    CreateBox {
        id: String,
        width: f64,
        height: f64,
        depth: f64,
    },

    /// Create a cylinder primitive.
    #[serde(rename = "create_cylinder")]
    CreateCylinder {
        id: String,
        radius: f64,
        height: f64,
    },

    /// Create a sphere primitive.
    #[serde(rename = "create_sphere")]
    CreateSphere { id: String, radius: f64 },

    /// Extrude a sketch profile.
    #[serde(rename = "extrude")]
    Extrude {
        id: String,
        sketch: String,
        depth: f64,
    },

    /// Cut (boolean subtract) one body from another.
    #[serde(rename = "cut")]
    Cut {
        id: String,
        target: String,
        tool: String,
    },

    /// Fuse (boolean union) two bodies.
    #[serde(rename = "fuse")]
    Fuse {
        id: String,
        target: String,
        tool: String,
    },

    /// Intersect two bodies.
    #[serde(rename = "intersect")]
    Intersect {
        id: String,
        target: String,
        tool: String,
    },
}

impl Feature {
    /// Get the feature ID.
    pub fn id(&self) -> &str {
        match self {
            Feature::CreateBox { id, .. }
            | Feature::CreateCylinder { id, .. }
            | Feature::CreateSphere { id, .. }
            | Feature::Extrude { id, .. }
            | Feature::Cut { id, .. }
            | Feature::Fuse { id, .. }
            | Feature::Intersect { id, .. } => id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_serialization() {
        let feature = Feature::CreateBox {
            id: "box_1".to_string(),
            width: 10.0,
            height: 20.0,
            depth: 30.0,
        };

        let yaml = serde_yaml::to_string(&feature).unwrap();
        assert!(yaml.contains("create_box"));
        assert!(yaml.contains("box_1"));

        let deserialized: Feature = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.id(), "box_1");
    }

    #[test]
    fn test_entity_ref_serialization() {
        let entity_ref = EntityRef {
            feature_id: "box_1".to_string(),
            role: "top_face".to_string(),
        };

        let yaml = serde_yaml::to_string(&entity_ref).unwrap();
        let deserialized: EntityRef = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.feature_id, "box_1");
        assert_eq!(deserialized.role, "top_face");
    }
}

exec
/bin/bash -lc "sed -n '1,220p' Cargo.toml" in /home/bacon/mycad
 succeeded in 0ms:
[workspace]
members = [
    "crates/mycad-kernel",
    "crates/mycad-format",
    "crates/mycad-build",
    "crates/mycad-viewer",
    "crates/mycad-cli",
    "crates/xtask",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
rust-version = "1.75"

[workspace.dependencies]
# Math
nalgebra = { version = "0.33", features = ["serde-serialize"] }
# Serialization
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
# Schema generation
schemars = "0.8"
# Testing
approx = "0.5"
tempfile = "3"
# Error handling
thiserror = "2"
# CLI
clap = { version = "4", features = ["derive"] }
# Ordered collections (deterministic iteration)
indexmap = { version = "2", features = ["serde"] }
# Internal crates
mycad-format = { path = "crates/mycad-format" }
mycad-kernel = { path = "crates/mycad-kernel" }
mycad-build = { path = "crates/mycad-build" }

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-build/tests/feature_dispatcher.rs" in /home/bacon/mycad
 succeeded in 0ms:
use mycad_build::build_solid_from_features;
use mycad_format::Document;
use mycad_kernel::brep::topology::IdGenerator;
use std::path::Path;

#[test]
fn simple_box_mycad_builds_solid() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("simple_box.mycad");

    let doc = Document::from_path(&path).expect("load .mycad");
    let mut g = IdGenerator::new(0);
    let solid =
        build_solid_from_features(&doc.root_component.features, &mut g).expect("build solid");

    // simple_box.mycad: width=10, height=20, depth=30 → cuboid topology
    assert_eq!(solid.vertices.len(), 8);
    assert_eq!(solid.edges.len(), 12);
    assert_eq!(solid.faces.len(), 6);
    assert_eq!(solid.shells.len(), 1);
}

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-cli/tests/export.rs" in /home/bacon/mycad
 succeeded in 0ms:
use std::path::PathBuf;
use std::process::Command;

#[test]
fn export_simple_box_produces_12_facets() {
    let bin = env!("CARGO_BIN_EXE_mycad");
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("simple_box.mycad");
    let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");
    let output = tmp.path();

    let status = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(output)
        .status()
        .expect("run mycad export");
    assert!(status.success(), "export should succeed");

    let stl = std::fs::read_to_string(output).expect("read stl");
    let facet_count = stl.matches("facet normal").count();
    assert_eq!(
        facet_count, 12,
        "cuboid should produce 12 facets (6 faces * 2 triangles)"
    );
}

exec
/bin/bash -lc "sed -n '1,220p' docs/decisions/001-brep-representation.md" in /home/bacon/mycad
 succeeded in 0ms:
# ADR-001: B-rep (Boundary Representation) の採用

## Status

Accepted

## Context

3D CAD カーネルの形状表現方式を選択する必要がある。主な候補:

1. **CSG (Constructive Solid Geometry)**: ブーリアン演算のツリーとして形状を表現
2. **B-rep (Boundary Representation)**: 面・辺・頂点で境界を表現
3. **Voxel / Implicit**: 暗黙的な距離関数で表現

## Decision

B-rep を採用する。

## Rationale

- **エッジ・フェイスへの直接アクセス**: フィレット、面取り、面選択等の操作に必要
- **業界標準**: STEP, IGES 等の標準フォーマットが B-rep ベース
- **テッセレーションが直接的**: 面を直接メッシュ化できる
- **CSG との併用可能**: ブーリアン演算の結果を B-rep として保持

## Implementation Details

- トポロジーエンティティはフラット配列に格納（index-based）
- `Solid` が最上位エンティティ、内部に Shell → Face → Loop → HalfEdge → Vertex の階層
- 各エンティティは `EntityId` を持ち、Feature からの参照に使用

exec
/bin/bash -lc "sed -n '1,220p' docs/decisions/004-freeform-geometry-commitment.md" in /home/bacon/mycad
 succeeded in 0ms:
# ADR-004: 自由曲面・自由曲線を確定要件として扱う

## Status

Accepted

## Context

現在の幾何カーネルは解析曲面・解析曲線のみを持つ:

- `Surface` (`crates/mycad-kernel/src/geometry/surface.rs`): `Plane` / `Cylinder` / `Sphere` / `Cone`
- `Curve` (`crates/mycad-kernel/src/geometry/curve.rs`): `Line` / `Circle`

「解析曲面だけで進められないか」を検討したが、ロードマップ上の操作を踏むと自由曲面・自由曲線は構造的に避けられないと結論した:

- **Boolean 演算 (Phase 4)**: 曲面同士の交線は一般に円・直線にならない。例として円筒 ∩ 球の交わりは 4 次の空間曲線であり、`Curve::Line` / `Curve::Circle` では表せない。→ 自由**曲線** (交線・スプライン) が必須。
- **フィレット・面取り (ADR-001 が動機として挙げた操作)**: 一般のフィレット面や掃引面は解析式に乗らない。→ 自由**曲面** (NURBS 等) が必須。

一方、次の理由から「今すぐ実装する必要はない」:

- `Surface` / `Curve` は Rust の enum であり、variant 追加は加算的。`evaluate` / `normal_at` / tessellation などの `match` はコンパイラが網羅性を強制するため、追加漏れは検出される。
- ADR-001 のとおり **Feature history が真実の源で、B-rep は Feature から再生成される**。`.mycad` に永続化されるのは Feature 列であり B-rep ではないため、`Surface` / `Curve` / `Edge` の内部表現は後で改修してもファイル移行が不要。後付けコストが構造的に低い。

## Decision

自由曲面・自由曲線を「いつか検討する」ものではなく **確定要件** として扱う。ただし NURBS 等の実装は今は行わず (現在 Phase 1)、Boolean が要求する範囲から漸進的に導入する。これに伴い、以降の設計で次を守る:

1. **`Surface` / `Curve` enum を唯一の幾何拡張点**とする。`Nurbs` / スプライン variant の追加が加算的であり続けるよう、アルゴリズムは variant 集合を仮定しない。
2. **アルゴリズムは曲面・曲線の型に非依存**であること。平面・直線前提をアルゴリズムへ埋め込まない (例: 面法線は面ごとに 1 回でなく、点ごとに `Surface::normal_at_point` で評価する)。
3. **数値モデル (トレラント方式 vs 厳密方式) は本 ADR では保留**し、曲面 Boolean を実装する Phase 4 着手時に決定する。自由曲面の交線は数値的近似になるため、業界カーネル (Parasolid / ACIS) はエンティティ毎に公差を持つトレラントモデルを採る。これを後から全エンティティ・全比較へ導入するのは大改修であり、Phase 4 で意思決定する論点として明示しておく。なお `CLAUDE.md` の **決定性** 原則とは両立可能 (アルゴリズム固定で再現できる) だが、数値ロバスト性の確保が決定性維持の難所になる点に留意する。

## Rationale

- **要件確定だけ先に行う理由**: 後付けが致命的に痛い横断的決定 (上記 2・3) を「事故的に」決めてしまわないため。実装そのもの (NURBS 評価・Boolean アルゴリズム) は enum + Feature 再生成により安全に後回しでき、先回り実装はむしろ過剰設計になる。
- **専用 Phase を立てない理由**: 自由曲面を独立フェーズにすると、どの操作がそれを必要とするかと切り離されて陳腐化する (ADR-002 の「空想 Issue は作らない」と同趣旨)。Boolean / フィレットが要求した時点で必要分を入れる。

## Implementation Details

- 本 ADR は ADR-001 (B-rep 採用) を拡張する位置づけ。番号 003 は ADR-002 で Viewer スタック用に予約済みのため 004 を使用。
- **未決の横断的論点 (Phase 4 で再検討)**:
  - **pcurve (パラメータ空間曲線)**: トリムされた周期・自由曲面を tessellation / Boolean で安定に扱うには、half-edge ごとに面のパラメータ空間上の 2D 曲線を持たせる構造が事実上必要になりうる (OpenCASCADE 等が保持)。現在の `Edge` は 3D `curve + t_range` のみで pcurve を持たない。
  - **数値モデル (トレラント vs 厳密)**: 上記 Decision 3。
- 本 ADR 採択に伴う即時対応: `tessellate_solid` が面法線を `normal_at(0.0, 0.0)` で 1 回だけ評価していた平面前提を解消し、`Surface::normal_at_point` による頂点毎評価へ変更した (平面では従来と同一の結果)。

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/src/lib.rs" in /home/bacon/mycad
 succeeded in 0ms:
pub mod brep;
pub mod error;
pub mod geometry;
pub mod primitives;
pub mod tessellation;

pub use brep::topology;
pub use error::KernelError;

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/src/geometry/mod.rs" in /home/bacon/mycad
 succeeded in 0ms:
pub mod curve;
pub mod math;
pub mod surface;

use nalgebra::{Point3, Vector3};
use serde::{Deserialize, Serialize};

/// A 3D point with f64 precision.
pub type Point = Point3<f64>;

/// A 3D vector with f64 precision.
pub type Vec3 = Vector3<f64>;

/// A plane defined by an origin point and a normal vector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plane {
    pub origin: Point,
    pub normal: Vec3,
    pub u_axis: Vec3,
    pub v_axis: Vec3,
}

impl Plane {
    pub fn xy() -> Self {
        Self {
            origin: Point::origin(),
            normal: Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        }
    }

    pub fn xz() -> Self {
        Self {
            origin: Point::origin(),
            normal: Vec3::y(),
            u_axis: Vec3::x(),
            v_axis: Vec3::z(),
        }
    }

    pub fn yz() -> Self {
        Self {
            origin: Point::origin(),
            normal: Vec3::x(),
            u_axis: Vec3::y(),
            v_axis: Vec3::z(),
        }
    }
}

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/CLAUDE.md" in /home/bacon/mycad
 succeeded in 0ms:
# mycad-kernel — B-rep 幾何カーネル

## Module Dependency Graph

```
primitives → brep, geometry
tessellation → brep, geometry
brep → geometry
geometry → (nalgebra)
```

## Key Invariants

- すべてのEdgeは正確に2つのHalfEdgeを持つ
- 閉じたShellはwatertight（水密）
- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
- Loopは閉じたHalfEdge列（最後のHalfEdgeの終点 = 最初のHalfEdgeの始点）
- EntityIDは `IdGenerator` で決定的に生成される

## How to Add a New Primitive

1. `src/primitives/` に新ファイルを作成（`cuboid.rs` をテンプレートとして使用）
2. 関数シグネチャ: `pub fn make_xxx(params..., id_gen: &mut IdGenerator) -> Solid`
3. `primitives/mod.rs` に `mod` と `pub use` を追加
4. 決定性テストを必ず書く（同じ `IdGenerator` 初期値で2回生成し、結果が同一であることを確認）

## How to Add a New Surface Type

1. `geometry/surface.rs` の `Surface` enumに新しいバリアントを追加
2. `evaluate(u, v) -> Point` を実装
3. `normal_at(u, v) -> Vec3` を実装
4. 共有数学関数は `geometry/math.rs` に配置

## How to Add a New Curve Type

1. `geometry/curve.rs` の `Curve` enumに新しいバリアントを追加
2. `evaluate(t) -> Point` を実装
3. 共有数学関数は `geometry/math.rs` に配置

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-format/src/document.rs" in /home/bacon/mycad
 succeeded in 0ms:
use crate::component::Component;
use crate::error::FormatError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// The top-level document representing a MyCad design file.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Document {
    /// Kernel version that created this document.
    pub version: String,
    /// The root component (assembly or single part).
    pub root_component: Component,
}

impl Document {
    /// Create a new document with a single empty root component.
    pub fn new(name: &str) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            root_component: Component::new(name),
        }
    }

    /// Serialize to YAML string.
    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }

    /// Deserialize from YAML string.
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    /// Load a Document from a `.mycad` file path.
    pub fn from_path(path: &Path) -> Result<Self, FormatError> {
        let ext = path.extension().and_then(|s| s.to_str());
        if ext != Some("mycad") {
            return Err(FormatError::InvalidExtension(ext.map(str::to_string)));
        }
        let content = std::fs::read_to_string(path)?;
        let doc = Self::from_yaml(&content)?;
        Ok(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::FormatError;
    use crate::feature::Feature;
    use std::path::Path;

    #[test]
    fn test_roundtrip_yaml() {
        let mut doc = Document::new("Test Assembly");
        doc.root_component.features.push(Feature::Extrude {
            id: "box_1".to_string(),
            sketch: "sketch_1".to_string(),
            depth: 10.0,
        });

        let yaml = doc.to_yaml().unwrap();
        let doc2 = Document::from_yaml(&yaml).unwrap();

        assert_eq!(doc.version, doc2.version);
        assert_eq!(doc.root_component.name, doc2.root_component.name);
        assert_eq!(
            doc.root_component.features.len(),
            doc2.root_component.features.len()
        );
    }

    #[test]
    fn test_yaml_is_deterministic() {
        let mut doc = Document::new("Test");
        doc.root_component.features.push(Feature::CreateBox {
            id: "box_1".to_string(),
            width: 10.0,
            height: 20.0,
            depth: 30.0,
        });

        let yaml1 = doc.to_yaml().unwrap();
        let yaml2 = doc.to_yaml().unwrap();
        assert_eq!(yaml1, yaml2, "YAML serialization must be deterministic");
    }

    #[test]
    fn test_from_path_loads_simple_box() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples")
            .join("simple_box.mycad");
        let doc = Document::from_path(&path).unwrap();
        assert_eq!(doc.version, "0.1.0");
        assert_eq!(doc.root_component.name, "Simple Box");
    }

    #[test]
    fn test_from_path_missing_file_returns_io_error() {
        let path = Path::new("does_not_exist.mycad");
        let err = Document::from_path(path).unwrap_err();
        assert!(matches!(err, FormatError::Io(_)));
    }

    #[test]
    fn test_from_path_invalid_extension() {
        let path = Path::new("foo.txt");
        let err = Document::from_path(path).unwrap_err();
        assert!(matches!(err, FormatError::InvalidExtension(Some(ref e)) if e == "txt"));
    }

    #[test]
    fn test_assembly_roundtrip() {
        use crate::component::ComponentRef;

        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples")
            .join("assembly.mycad");
        let doc = Document::from_path(&path).unwrap();

        // Bolt child has a stdlib reference
        let bolt = doc
            .root_component
            .children
            .iter()
            .find(|c| c.name == "Bolt")
            .expect("Bolt component not found");
        assert!(
            matches!(&bolt.reference, Some(ComponentRef::StdLib(p)) if p == "fasteners/jis_b1176/M5x20")
        );

        // Roundtrip: serialize then deserialize yields the same YAML
        let yaml = doc.to_yaml().unwrap();
        let doc2 = Document::from_yaml(&yaml).unwrap();
        assert_eq!(doc.to_yaml().unwrap(), doc2.to_yaml().unwrap());
    }
}

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/tests/cuboid_roundtrip.rs" in /home/bacon/mycad
 succeeded in 0ms:
use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::primitives::make_cuboid;
use mycad_kernel::tessellation::tessellate_solid;

/// Integration test: create a cuboid, tessellate it, and verify the pipeline.
#[test]
fn cuboid_to_mesh_pipeline() {
    let mut id_gen = IdGenerator::new(0);
    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);

    // Verify topology
    assert_eq!(solid.vertices.len(), 8);
    assert_eq!(solid.edges.len(), 12);
    assert_eq!(solid.faces.len(), 6);

    // Tessellate
    let mesh = tessellate_solid(&solid);

    // Verify mesh
    assert_eq!(mesh.triangle_count(), 12); // 6 faces * 2 triangles
    assert_eq!(mesh.positions.len(), 24); // 6 faces * 4 vertices
    assert_eq!(mesh.normals.len(), 24);
    assert_eq!(mesh.indices.len(), 36); // 12 triangles * 3 indices
}

/// Integration test: deterministic pipeline produces identical results.
#[test]
fn cuboid_pipeline_is_deterministic() {
    let run = || {
        let mut id_gen = IdGenerator::new(42);
        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
        tessellate_solid(&solid)
    };

    let mesh1 = run();
    let mesh2 = run();

    assert_eq!(mesh1.positions, mesh2.positions);
    assert_eq!(mesh1.normals, mesh2.normals);
    assert_eq!(mesh1.indices, mesh2.indices);
}

exec
/bin/bash -lc "sed -n '1,220p' docs/file-format.md" in /home/bacon/mycad
 succeeded in 0ms:
# .mycad ファイルフォーマット仕様

## Overview

`.mycad` ファイルは YAML 形式のテキストファイル。

## Structure

```yaml
version: "0.1.0"           # カーネルバージョン
root_component:             # ルートコンポーネント
  name: "Part Name"
  transform:                # (省略可) 変換
    position: [x, y, z]
    rotation: [rx, ry, rz]  # オイラー角（度）
  features: [...]           # (省略可) 操作履歴
  children: [...]           # (省略可) 子コンポーネント
  ref: "path"               # (省略可) 外部参照
```

## Feature Types

### create_box
```yaml
- type: create_box
  id: unique_id
  width: 10.0     # X寸法
  height: 20.0    # Y寸法
  depth: 30.0     # Z寸法
```

### create_cylinder
```yaml
- type: create_cylinder
  id: unique_id
  radius: 5.0
  height: 20.0
```

### create_sphere
```yaml
- type: create_sphere
  id: unique_id
  radius: 10.0
```

### extrude
```yaml
- type: extrude
  id: unique_id
  sketch: sketch_id
  depth: 15.0
```

### Boolean Operations (cut, fuse, intersect)
```yaml
- type: cut          # or fuse, intersect
  id: unique_id
  target: body_id
  tool: body_id
```

## External References

コンポーネントは外部ファイルまたは標準ライブラリを参照可能:

```yaml
ref: "./other_part.mycad"                    # ファイル参照
ref: "stdlib://fasteners/jis_b1176/M5x20"   # 標準ライブラリ参照
```

exec
/bin/bash -lc "sed -n '1,180p' ARCHITECTURE.md" in /home/bacon/mycad
 succeeded in 0ms:
# MyCad Architecture

## Overview

MyCad は Feature-based parametric CAD システム。ユーザーの操作履歴（Feature 列）を真実の源とし、B-rep 形状はそこから再生成される。

## Layer Structure

```
┌─────────────────────┐
│     mycad-cli       │  CLI / UI 層
├─────────────────────┤
│    mycad-viewer     │  可視化層（テッセレーション結果を描画）
├─────────────────────┤
│    mycad-format     │  永続化層（YAML <-> Document/Feature/Component）
├─────────────────────┤
│    mycad-kernel     │  コア層（B-rep, geometry, primitives, tessellation）
└─────────────────────┘
```

## Key Design Decisions

### B-rep (Boundary Representation)

形状を「境界面の集合」として表現する。CSG (Constructive Solid Geometry) より複雑だが、エッジ・フェイスへの直接アクセスが可能で、フィレット・面取り等の操作に適している。

### Index-based Topology

`Solid` 内のトポロジーエンティティ（Vertex, Edge, Face 等）はフラット配列に格納し、ポインタではなくインデックスで相互参照する。

**理由:**
- シリアライズが容易
- 決定性の保証が簡単
- メモリレイアウトがキャッシュフレンドリー
- 所有権問題（循環参照）を回避

### Feature History as Source of Truth

`.mycad` ファイルに保存される Feature 列が設計の唯一の源。B-rep 形状は Feature 列から毎回再生成可能。これにより:
- パラメータ変更時の再生成が可能
- undo/redo が自然に実装できる
- ファイルサイズが小さい（形状データではなく操作手順のみ）

### YAML Format

設計ファイルに YAML を採用。

**理由:**
- 人間が読み書きできる
- git diff でレビューできる
- スキーマバリデーション可能（schemars で JSON Schema 生成）
- 拡張子: `.mycad`

### Kernel の Rendering 非依存

`mycad-kernel` はレンダリングライブラリに一切依存しない。テッセレーション結果（`TriangleMesh`）を生成するのみ。ビューアがそれを消費する。

## Data Flow

```
.mycad file → Document → Feature列 → Kernel (B-rep Solid) → Tessellation → TriangleMesh → Viewer
```

codex
現時点の論点は固まりました。レビューは次の順で確定します。

1. 既存 API と矛盾する設計を抽出する
2. B-rep/退化処理の未充足を分類する
3. テスト計画の穴を既存テスト群と照合する
4. 指摘を指定 YAML 形式に落とす

最優先で見ているのは、提案された `sample_segment` が現行の reversed half-edge と整合しない点と、退化入力・`--segments` 境界値の扱いが未設計な点です。
exec
/bin/bash -lc "sed -n '1,220p' .claude/skills/3ai/agents/codex-design-reviewer.md" in /home/bacon/mycad
 succeeded in 0ms:
# Codex 設計レビュアー（MyCad CAD カーネル専用）

あなたは Rust 製 B-rep CAD カーネル「MyCad」の設計ドキュメントをレビューする専門家です。
stdin に渡された設計・テスト計画ドキュメントを読み、以下の観点で問題を指摘してください。

## レビュー観点

### 1. 決定性（MyCad の核心）
- `IdGenerator` を使った決定的 ID 生成になっているか
- 同一入力で必ず同一出力（同一 ID・同一座標）が保証されているか
- 非決定的要素（HashMap のイテレーション順、thread_rng など）が混入していないか

### 2. B-rep トポロジー妥当性
- Euler-Poincaré の公式 `V - E + F = 2(S - H)` が成立する設計か（S=殻の数, H=貫通穴）
- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
- Loop/Shell/Solid の入れ子構造が一貫しているか

### 3. 退化幾何の扱い
- ゼロ長エッジ、面積ゼロの Face、縮退した Loop の排除または明示的エラーの設計があるか
- 数値的境界（浮動小数点の epsilon 比較）の方針が明記されているか

### 4. API・型設計
- `Debug, Clone, Serialize, Deserialize` が付与されているか（JsonSchema が必要な型は追記）
- 新しい依存は `[workspace.dependencies]` + `{ workspace = true }` 規約に従っているか
- ライブラリエラーには `thiserror` を使用しているか
- カーネルにレンダリング依存が混入していないか（`TriangleMesh` 生成のみ可）

### 5. テスト計画の充足性
- 決定性反復テスト（T0x 系）があるか
- Euler-Poincaré 検証テストがあるか
- 退化入力エッジケースが十分にあるか
- golden YAML ラウンドトリップテストがあるか

### 6. アーキテクチャ整合性
- Index-based topology（ポインタ不使用、フラット配列 + インデックス参照）を守っているか
- Feature history = source of truth の原則と矛盾しないか
- 1概念1ファイル、`mod.rs` から `pub use` で再エクスポートの規約に従っているか

---

## 出力フォーマット（厳守）

```yaml
issues:
  - id: R01
    severity: critical  # critical | high | medium | low
    section: "設計方針 > 決定性"
    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
    suggestion: "IdGenerator::next() に置き換えること"
  - id: R02
    severity: medium
    ...

verdict: pass  # pass | fail
# fail = Critical または High が1件以上ある
```

`issues` が空の場合は `issues: []` と書く。
**このフォーマット以外の出力は禁止。前置きや後置きの文章は書かない。**

codex
issues:
  - id: R01
    severity: critical
    section: "設計方針 > 2. テッセレータ（ADR-004 準拠の加算拡張）"
    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
    suggestion: "`sample_segment` を HalfEdge 向き込みの API に変更し、rev では `t1` から降順にサンプルするか、先頭点を必ず `HalfEdge.start_vertex` から取得すること。reversed line edge を含む cuboid 回帰テストも追加すること。"
  - id: R02
    severity: high
    section: "設計方針 > 1. トポロジー / 2. テッセレータ / 3. ディスパッチ / CLI"
    finding: "退化入力を『正値前提』としてスコープ外にしており、`radius <= 0`/`height <= 0` に加えて公開 API/CLI から入る `angular_segments < 3` や `axial_segments < 1` も無検証のままになっている。ゼロ長 seam、面積ゼロ cap、空または縮退メッシュを生成しうる。"
    suggestion: "primitive 生成時と `TessellationOptions` 構築時に下限チェックを入れ、違反時は `thiserror` ベースの `KernelError` で明示エラーにすること。CLI でも同じ境界値を拒否すること。"
  - id: R03
    severity: high
    section: "設計方針 > 1. トポロジー / テスト計画 > T04"
    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
    suggestion: "`HalfEdge` に隣接インデックスを追加して決定的に構築・検証するか、少なくとも各 loop で consecutive half-edge の終点/始点一致を全件検証する派生テストを追加すること。"
  - id: R04
    severity: medium
    section: "設計方針 > 2. テッセレータ"
    finding: "数値境界の方針が未記載で、`uv_of` の seam アンラップ、幾何法線と `normal_at_point` の向き比較、ゼロ面積三角形の除外条件が実装者任せになっている。決定性はあっても数値ロバスト性の再現条件が固定されない。"
    suggestion: "`geometry`/`tessellation` で共有する epsilon 定数と比較規約を設計に明記し、実装とテストで同じ閾値を使うこと。"
  - id: R05
    severity: medium
    section: "テスト計画"
    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
    suggestion: "`Document::from_path` -> `to_yaml` -> `from_yaml` -> `to_yaml` の equality を `examples/cylinder.mycad` に対して追加し、`create_cylinder` の schema/serialize 回帰を固定すること。"

verdict: fail
tokens used
122,623
issues:
  - id: R01
    severity: critical
    section: "設計方針 > 2. テッセレータ（ADR-004 準拠の加算拡張）"
    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
    suggestion: "`sample_segment` を HalfEdge 向き込みの API に変更し、rev では `t1` から降順にサンプルするか、先頭点を必ず `HalfEdge.start_vertex` から取得すること。reversed line edge を含む cuboid 回帰テストも追加すること。"
  - id: R02
    severity: high
    section: "設計方針 > 1. トポロジー / 2. テッセレータ / 3. ディスパッチ / CLI"
    finding: "退化入力を『正値前提』としてスコープ外にしており、`radius <= 0`/`height <= 0` に加えて公開 API/CLI から入る `angular_segments < 3` や `axial_segments < 1` も無検証のままになっている。ゼロ長 seam、面積ゼロ cap、空または縮退メッシュを生成しうる。"
    suggestion: "primitive 生成時と `TessellationOptions` 構築時に下限チェックを入れ、違反時は `thiserror` ベースの `KernelError` で明示エラーにすること。CLI でも同じ境界値を拒否すること。"
  - id: R03
    severity: high
    section: "設計方針 > 1. トポロジー / テスト計画 > T04"
    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
    suggestion: "`HalfEdge` に隣接インデックスを追加して決定的に構築・検証するか、少なくとも各 loop で consecutive half-edge の終点/始点一致を全件検証する派生テストを追加すること。"
  - id: R04
    severity: medium
    section: "設計方針 > 2. テッセレータ"
    finding: "数値境界の方針が未記載で、`uv_of` の seam アンラップ、幾何法線と `normal_at_point` の向き比較、ゼロ面積三角形の除外条件が実装者任せになっている。決定性はあっても数値ロバスト性の再現条件が固定されない。"
    suggestion: "`geometry`/`tessellation` で共有する epsilon 定数と比較規約を設計に明記し、実装とテストで同じ閾値を使うこと。"
  - id: R05
    severity: medium
    section: "テスト計画"
    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
    suggestion: "`Document::from_path` -> `to_yaml` -> `from_yaml` -> `to_yaml` の equality を `examples/cylinder.mycad` に対して追加し、`create_cylinder` の schema/serialize 回帰を固定すること。"

verdict: fail
