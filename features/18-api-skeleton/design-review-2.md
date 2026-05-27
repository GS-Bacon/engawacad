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
session id: 019e5dbc-18e2-7360-a289-2a836eea3bb6
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
# Issue #18: API 層スケルトン (mycad-api / axum)

## Context

Phase 2「Web ベースビューア基盤」の土台として、CLI / GUI / AI 共通の操作面 v0 を Rust HTTP サーバとして立てる。`.mycad` を読み込み、テッセレーションした `TriangleMesh` を JSON で返す最小 API を `crates/mycad-api/` に新規実装する。後続 Issue #20(Three.js フロント)・#21(`mycad view`)がこの API を消費する前提。

既存 CLI の `run_export`(`crates/mycad-cli/src/main.rs:47-67`)が同じ Document→Solid→Mesh フローを既に持つため、STL 化を除いて再利用する。

> Codex 設計レビュー(round 1, verdict=fail)反映済み。R01/R03/R04 受諾、R02 は kernel 検証追加で対応(ユーザー決定)、R05 は部分受諾(下記)。

## 実装対象

- 新規クレート `crates/mycad-api/`(lib + bin 構成)+ `make_cuboid` の入力検証(kernel)
- Issue: #18 / Milestone: Phase 2

### v0 スコープ明確化(R03)
- 対応するのは **`root_component` に単一 part feature を持つ文書のみ**(Box / Cylinder)。
- assembly(`children`/`reference` を持ち root features が空)や複数フィーチャは v0 非対応。明確なエラー + 422 系で返す(クラッシュさせない)。

### 影響ファイル

| ファイル | 変更 |
|----------|------|
| `crates/mycad-kernel/src/primitives/cuboid.rs` | **(R02)** `make_cuboid` を `-> Result<Solid, KernelError>` 化。`dx/dy/dz` が非有限 or `<= 0` なら `InvalidParameter { kind }`(`make_cylinder` cylinder.rs:16-22 と対称)。インライン test の呼び出しを `.unwrap()` 化 |
| `crates/mycad-build/src/lib.rs` | CreateBox 分岐を `Ok(make_cuboid(...))` → `make_cuboid(...)`(`?` 伝播)に変更 |
| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
| `Cargo.toml`(ルート) | members に `"crates/mycad-api"` 追加。`[workspace.dependencies]` に `axum`/`tokio`/`serde_json`/`tower`(dev) 追加 |
| `crates/mycad-api/Cargo.toml` | 新規。`version.workspace = true` 等の既存規約。`[[bin]]` name=`mycad-api` |
| `crates/mycad-api/src/lib.rs` | 新規。**(R04)** `pub use` 中心の薄い再エクスポートのみ |
| `crates/mycad-api/src/router.rs` | 新規。`pub fn app() -> Router`(`/api/v0` を `nest`) |
| `crates/mycad-api/src/handler.rs` | 新規。`get_mesh` ハンドラ + パス解決ロジック |
| `crates/mycad-api/src/error.rs` | 新規。`ApiError` + `IntoResponse` |
| `crates/mycad-api/src/main.rs` | 新規。`#[tokio::main]`、`127.0.0.1:3000` バインド |
| `crates/mycad-api/tests/mesh_api.rs` | 新規。tower `oneshot` 統合テスト + fixtures |
| `crates/mycad-api/tests/fixtures/*.mycad` | 新規。退化/未対応ケース用 fixture(R02/R03 テスト用) |

### シグネチャ

```rust
// router.rs
pub fn app() -> axum::Router;

// handler.rs
#[derive(serde::Deserialize)]
struct MeshQuery { file: String }
async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;

// error.rs
enum ApiError { BadExtension(String), NotFound, Unsupported(String), Build(String), Tessellate(String), Internal(String) }
impl IntoResponse for ApiError { /* status + {"error": "..."} */ }
```

## 設計方針

### 再利用(新規ロジックを書かない)
- `Document::from_path`(`crates/mycad-format/src/document.rs:36`)。`.mycad` 拡張子を強制(InvalidExtension)。
- `build_solid_from_features`(`crates/mycad-build/src/lib.rs:6`)。単一フィーチャ・Box/Cylinder のみ。
- `tessellate_solid_with`(`crates/mycad-kernel/src/tessellation/mod.rs:90`)。v0 は `TessellationOptions::default()` 固定。
- `TriangleMesh`(mod.rs:16, Serialize 済み)→ `Json(mesh)`。
- ハンドラ本体は CLI `run_export`(main.rs:47-67)から STL 化を除き末尾を `Ok(Json(mesh))` に置換。

### パス解決の契約(R01)
- `file` クエリは **絶対パスならそのまま、相対パスならサーバプロセスの CWD(`std::env::current_dir`)基準**で解決する、と明文化。
- bin は起動時に effective base(CWD)をログ出力。
- テストは `env!("CARGO_MANIFEST_DIR")` 起点で絶対パスを組み立て(`crates/mycad-api` から `../../examples/...`)、CWD 非依存にする。
- E2E 例(`examples/...` 相対)はリポジトリルートを CWD として起動する前提と注記。

### エンドポイント / バージョニング
- `GET /api/v0/mesh?file=<path>`。`Router::nest("/api/v0", ...)`。クエリは `file` のみ(v0 スコープ)。解像度は default 固定。

### パス安全性(多層防御 — 任意位置の .mycad 読取とセキュリティを両立)
1. **CWD ジェイルなし** → 任意パスの `.mycad` を読める(要望)。
2. **`.mycad` 拡張子限定**(`Document::from_path` が既に強制)→ 機密ファイル読取を遮断。
3. **`canonicalize` + 実ファイル存在確認** → 無ければ 404、ディレクトリ等は 400。
4. **`127.0.0.1` のみバインド** → ネットワークから到達不可。
5. **Host ヘッダ検証ミドルウェア** → loopback 以外は 403(DNS リバインディング対策)。

### エラーハンドリング
- `FormatError`/`KernelError`/`TessellationError` は `Serialize` 非対応 → `ApiError` に `.to_string()` で集約し `IntoResponse` で status + `{"error": "..."}`。
- status マッピング: ファイル不在 → 404 / 非.mycad 拡張子・不正パラメータ → 400 / `InvalidParameter`(退化寸法)・`UnsupportedFeature`・`MultipleFeatures`・`EmptyFeatureList`・未対応サーフェス → 422 / その他 → 500。

### 依存・規約
- `[workspace.dependencies]` に新規登録: `axum = "0.8"`、`tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }`、`serde_json = "1"`、`tower = "0.5"`(dev、`oneshot`)。各クレートは `{ workspace = true }`。
- derive 規約・`thiserror`・`version.workspace = true` を踏襲。clippy `-D warnings` 通過必須。

## テスト計画

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 正常系(box) | `GET /api/v0/mesh?file=<abs>/examples/simple_box.mycad` を oneshot | 200、`TriangleMesh` deser 可、`positions`/`indices` 非空、`indices.len() % 3 == 0` |
| T02 | 正常系(cylinder)(R05) | 同上 `examples/cylinder.mycad`(周期面経路) | 200、mesh 非空 |
| T03 | 決定性 | T01/T02 を 2 回実行 | 返る JSON バイト列が完全一致 |
| T04 | 404 | 存在しない `file` | 404、body `{"error": ...}` |
| T05 | 400 | 非 `.mycad` 拡張子(`Cargo.toml`) | 400(InvalidExtension) |
| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
| T07 | 422 未対応 feature(R03) | fixture `create_sphere` 単一 feature | 422(UnsupportedFeature) |
| T08 | 422 退化寸法(R02) | fixture `create_box` width=0(または負値) | 422(InvalidParameter) |
| T09 | 422 assembly 非対応(R03) | fixture: root features 空 + children あり | 422(EmptyFeatureList、クラッシュしない) |
| K01 | kernel 単体(R02) | `make_cuboid(0.0, 1.0, 1.0, ..)` 等 | `Err(InvalidParameter { kind: "..." })` |

> **R05 反論**: Euler-Poincaré 検証は kernel(`cylinder.rs:180-195`)で、golden YAML ラウンドトリップは format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保済み。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。API 層は HTTP 契約(status・JSON 形・決定性・cylinder 経路疎通)に集中する。

## 検証(end-to-end)

```bash
cargo xtask ci                      # fmt → clippy(-D warnings) → test --workspace → build
cargo run -p mycad-api --bin mycad-api &   # 127.0.0.1:3000(repo ルートを CWD に)
curl -s 'http://127.0.0.1:3000/api/v0/mesh?file=examples/simple_box.mycad' | head -c 300   # TriangleMesh JSON
curl -s 'http://127.0.0.1:3000/api/v0/mesh?file=examples/cylinder.mycad'    | head -c 300   # 同上(曲面)
curl -s -o /dev/null -w '%{http_code}\n' 'http://127.0.0.1:3000/api/v0/mesh?file=examples/nope.mycad'  # 404
```

## 3ai フロー上の注記

- **STEP 3(Codex 設計レビュー)はプランモードを抜けずに実行する**(Bash dispatch は plan mode でも通る。deny は `crates/**` の Edit/Write のみ)。本改訂後に再レビュー(round 2/3)し、Critical/High 解消後に STEP 4 `ExitPlanMode` で最終承認。
- 承認後: STEP 5(ブランチ `cad/18-api-skeleton`)→ STEP 6(GLM 実装、`crates/**` は Claude 編集禁止)→ STEP 7(Codex 最終レビュー)→ STEP 8(squash マージ, `Closes #18`)。
</stdin>
codex
設計レビューとして、まず参照されている既存実装と規約が文書の主張と整合しているかを確認します。`run_export`、`make_cuboid`、build/tessellation、workspace 依存規約、既存テスト配置を先に見ます。
exec
/bin/bash -lc 'rg --files .' in /home/bacon/mycad
 succeeded in 0ms:
./Cargo.toml
./ARCHITECTURE.md
./README.md
./ROADMAP.md
./.git/ORIG_HEAD
./.git/index
./.git/packed-refs
./crates/mycad-format/Cargo.toml
./.git/refs/remotes/origin/HEAD
./.git/FETCH_HEAD
./.git/COMMIT_EDITMSG
./docs/file-format.md
./.git/config
./.git/refs/remotes/origin/claude/add-claude-guidelines-BKKtD
./crates/mycad-format/src/lib.rs
./crates/mycad-format/src/feature.rs
./crates/mycad-format/src/component.rs
./crates/mycad-format/src/error.rs
./crates/mycad-format/src/document.rs
./crates/mycad-format/CLAUDE.md
./.git/HEAD
./.git/info/exclude
./.git/logs/HEAD
./crates/xtask/Cargo.toml
./docs/decisions/001-brep-representation.md
./crates/mycad-build/Cargo.toml
./CLAUDE.md
./docs/decisions/003-viewer-and-app-architecture.md
./.claude/skills/3ai/SKILL.md
./docs/decisions/004-freeform-geometry-commitment.md
./rust-toolchain.toml
./crates/xtask/src/main.rs
./docs/decisions/002-roadmap-management.md
./crates/mycad-build/src/lib.rs
./.claude/skills/3ai/agents/glm-implementer.md
./.claude/skills/3ai/agents/codex-final-reviewer.md
./.claude/skills/3ai/agents/codex-design-reviewer.md
./crates/mycad-viewer/Cargo.toml
./.git/refs/heads/claude/add-claude-guidelines-BKKtD
./crates/mycad-build/tests/feature_dispatcher.rs
./.git/hooks/update.sample
./.gitignore
./.git/hooks/fsmonitor-watchman.sample
./.git/hooks/commit-msg.sample
./.git/hooks/push-to-checkout.sample
./.git/hooks/post-update.sample
./.git/hooks/pre-receive.sample
./.git/hooks/pre-push.sample
./.git/hooks/prepare-commit-msg.sample
./.git/hooks/sendemail-validate.sample
./.git/hooks/applypatch-msg.sample
./.git/hooks/pre-commit.sample
./.git/hooks/pre-applypatch.sample
./.git/hooks/pre-rebase.sample
./.git/hooks/pre-merge-commit.sample
./.git/opencode
./.git/objects/0d/4e25162d94916438174c03612f0023b5a9016f
./.git/logs/refs/remotes/origin/HEAD
./.git/objects/35/37e8ffe05190fc64cb17ae4b5f708fa517b338
./crates/mycad-viewer/src/lib.rs
./.claude/skills/3ai/scripts/guard-crates.sh
./.claude/skills/3ai/scripts/dispatch-codex.sh
./.claude/skills/3ai/scripts/state.sh
./.claude/skills/3ai/scripts/dispatch-glm.sh
./.claude/settings.json
./Cargo.lock
./.git/objects/54/15a0964d76b39cec282b6746803866d0e40ee6
./features/12-cylinder/design-review.md
./crates/mycad-kernel/Cargo.toml
./.git/objects/32/6b630e5579772cacd0a6a71f732f2c7a0d6e14
./.git/description
./features/12-cylinder/design-review-2.md
./.git/logs/refs/heads/claude/add-claude-guidelines-BKKtD
./features/12-cylinder/final-review-2.md
./features/12-cylinder/final-review.md
./features/12-cylinder/design-review-3.md
./.git/objects/a4/16a93868018961c64227d686db4a5a023afbb2
./.git/objects/a4/af917f103530868e91b3b9bcfedf3902a3a3f1
./.git/objects/38/76021c40eea6680c03d67aa21abba119c62f97
./.git/objects/3b/9a6697582a7359e9077d779f27661bf47d3cd5
./.git/objects/d8/3a91bf0faf333faf49ae7889de4844e7960638
./.git/objects/4e/dbe09f32289798cf0432213d231a349515b2aa
./crates/mycad-cli/Cargo.toml
./.git/objects/4e/22430f743458caba1c7f4c391dc5c8be117d7c
./features/18-api-skeleton/design-review.md
./features/18-api-skeleton/design-review-2.md
./features/18-api-skeleton/plan.md
./features/18-api-skeleton/state.json
./.git/objects/5e/44e212ad81548b5198b747289d03403caa104b
./examples/cylinder.mycad
./examples/assembly.mycad
./examples/simple_box.mycad
./.git/objects/3c/e3331b1ca62783227769921f9b253a7c7f1ce6
./.git/objects/3c/b43d4f8ab6eceacd675a2a76c2b8935b959671
./crates/mycad-kernel/src/lib.rs
./.git/objects/7c/83ec1f3c0e9ca44bad182fd228b8cbd82874dc
./.git/objects/5b/80e283c04cab6f8c9399a797733fb867713b7c
./.cargo/config.toml
./crates/mycad-kernel/src/error.rs
./.git/objects/dd/e130d4cabd8ca91b3d2207a7b1a2ae1d00d2db
./.git/objects/27/2970e35a608e9cbde6daef23f439fa7108a35d
./.git/objects/b7/caddebb669828c9282e8413bf326b98a7d42b9
./.git/objects/96/8c75683941bfe1301f6a4aad41aafd60f134df
./.git/objects/9f/4ac6ed6d6fa01ed94c0ff204efdc52d559a51d
./.git/objects/9f/d72bbdb9b9ee2423a192d48877cc646f512515
./crates/mycad-cli/src/main.rs
./.git/objects/93/416fcabe178a205ed6389e1aab1513f3a84624
./crates/mycad-kernel/src/tessellation/mod.rs
./crates/mycad-kernel/src/tessellation/stl.rs
./.git/logs/refs/remotes/origin/claude/add-claude-guidelines-BKKtD
./.git/objects/96/b7f38089a05605b9a9c92b57bf8a486bfc2658
./.git/objects/c8/d0a83e7b6735cd5568cc39d28be1213d2f280b
./.git/objects/ea/c25d13cc45d7da37b3487cdee5277b8b4a4a05
./crates/mycad-kernel/benches/tessellation.rs
./.git/objects/5b/2a7b94a20c4573fae1fd94f905ae00a66f3296
./.git/objects/5b/f2f551288dc25718660b3321055b7084248244
./.git/objects/07/d9f0998399fe96ad30657a5f33452ca73035da
./.git/objects/67/93c94ceae51f8d025b6537d01640bd9263c0e8
./.git/objects/ff/677974e6c6e590e3909cf89ed028756419a31f
./.git/objects/1c/00df69cd1f2acb22e8b6067b418a07d051b47f
./.git/objects/9e/dca3ddaff8828eba48bfa5007b248221716ede
./.git/objects/97/953774ddd8247225e7368643aa031cde3cced9
./crates/mycad-kernel/src/geometry/surface.rs
./crates/mycad-kernel/src/geometry/mod.rs
./crates/mycad-kernel/src/geometry/curve.rs
./crates/mycad-kernel/src/geometry/math.rs
./.git/objects/6b/4f560fa674ce568dd735d2298f816b7cdef3bb
./crates/mycad-kernel/tests/cuboid_roundtrip.rs
./crates/mycad-kernel/CLAUDE.md
./.git/objects/2f/9170d8518206f2f86745fc69f0d96ee7e653f8
./.git/objects/85/c7712d7caa02c603e9d3578a68e9718d2a2fc2
./.git/objects/85/bad72d071f01f6128ad62002522a27a49e17e8
./crates/mycad-cli/tests/export.rs
./.git/objects/28/d9e8d71b0b4c87717f39e8b34baf3d3c893725
./.git/objects/51/bf8ed0eb7b9b6cecf42c473fb5b19c714683ed
./.git/objects/80/0a5a55e47af0e783d167ef48b99cba7e540937
./.git/objects/a0/20874ac157c5ad937bfb74324d7f445fb0e927
./.git/objects/01/1dc8083a61e1a4283681517e621edd80debe7e
./crates/mycad-kernel/src/brep/mod.rs
./.git/objects/fe/5cefc5fca934c4dbac4618bf359b6cb86da03e
./crates/mycad-kernel/src/brep/topology.rs
./.git/objects/8a/57aa85df9f51b6a7a30e6b5aea3f0250b1e8a2
./.git/objects/a0/851729e45eeb1cf5b58871b60e7443d29d60fe
./.git/objects/69/811c2cd2f31f63e7c1ddce855f0ec091d841da
./.git/objects/69/39a727b2f2612fefa62b33e9953f9ac7fd2699
./.git/objects/94/29877508b375ff41a3304575b861073d64cd83
./.git/objects/pack/pack-d9e6fa1e29ba2b801a71302390018bc93c757528.rev
./crates/mycad-kernel/src/primitives/mod.rs
./.git/objects/b5/adb8414f06985e70d682ca41dd7498a07f0616
./crates/mycad-kernel/src/primitives/cuboid.rs
./.git/objects/b5/fc56c4b80bad66754bdaef1b2053abffd770f1
./crates/mycad-kernel/src/primitives/cylinder.rs
./.git/objects/7e/cfd05d37449d4c99db3a152c437a384006dd32
./.git/objects/pack/pack-d9e6fa1e29ba2b801a71302390018bc93c757528.idx
./.git/objects/b8/b125139776fd7bbb0799362d61c563fff21fc6
./.git/objects/e6/a9a63fce84611088f59b9daa9a9f5b1da5c0ea
./.git/objects/e6/3f03657226775892748af8cc4e9e08a7576382
./.git/objects/b8/b22b880f8187eab9e3936a0c5ee3c62da99630
./.git/objects/46/c30c6fb372efd7e4c74171134a6434f5ff687e
./.git/objects/f5/c48a6c87abc2dcd1eb3ffdeb67993c4c024487
./.git/objects/02/fc621ea7da109ef42846b74197f5775ece124c
./.git/objects/pack/pack-d9e6fa1e29ba2b801a71302390018bc93c757528.pack
./.git/objects/f2/550699a30ac247de8e86e42a7ed6ef0ec68f1f
./.git/objects/71/c7fdaa466bb5a8608fe5d55ed2d1afd795f7cc
./.git/objects/e3/67d5288ed80eea015efdfb26c6d8a3505fc2e6
./.git/objects/e3/12b4ef0fa12ba71b7974d6049e67bba346c3b2
./.git/objects/e3/f04758017c63d93adb7b325ba29769cfa70023
./.git/objects/49/c3b468feaf139d9c75a35c7d3a49a65a960a34
./.git/objects/57/2a245caa7428946206c816c36c75258dd4c7a2
./.git/objects/0a/579062059d46927b170490912f8ae174dec896
./.git/objects/f4/a7c52f36fb75473e39a27f024a760c53f1f74e
./.git/objects/da/56d0901c48f54cba1e696480139ba8c64e7f7a
./.git/objects/95/12066b9a38f0ae26303824d5691cba4e196ef8
./.git/objects/2a/233969062bfaa6d679231fcd91a770aafefcba
./.git/objects/2a/9347045996aff0cf34ae8d91694b0df74a1a54
./.git/objects/2a/10604b441bbbc51fba5b657d3f0bb169108e47
./.git/objects/8b/182ebf45cf0e98ebe2ff1593f32803beedd360
./.git/objects/a8/8b1887fd14932733480557824e50f1c6226bf9
./.git/objects/c0/75ce3a3dfb33efdfa152fc3c413f3f7e4962b0
./.git/objects/bf/99a65e77a06a29065debb903f763c8bd5c23c7
./.git/objects/bf/248d636a20fb99d441cc25e77ae7e5f99b989c
./.git/objects/88/a701ef888392fd6d773d622ecc8f7e773a1962
./.git/objects/6a/96fd640fc3850f2086e19f737d51c3bffd9d46
./.git/objects/cd/e9f18477a427633616ef5385136f44c411ab14
./.git/objects/ce/0b324280cebfe29f0380d7f87eefed2142816c
./.git/objects/23/a907de7c1c457780c3393da0fbde5893b68de4
./.git/objects/58/5f429fc31ec81a1148129c98725a51fbbeee34
./.git/objects/f9/f86451736f06b8dce7ed37224df4d14eba12f4
./.git/objects/23/634201cd4c0f8c7421c47c499d1c226c31b93c
./.git/objects/75/0fed59cd01a59ddcfc4e7e88f5155f19285708
./.git/objects/5d/c824c3982538c158b527e104a7403f0158c945
./.git/objects/5d/8e707c630dd18212d53fdd492894675b84df9d
./.git/objects/75/2ac98646de4b09bad18047d96478c284161173
./.git/objects/fd/25fd0d6c3cfb657b95273102cab52cce8db03c
./.git/objects/b2/df39b7bb20b7a2b77745d5c66ba5a300554bd5
./.git/objects/36/0f324d35e1753537b2c6325cb86fa5328eea78
./.git/objects/09/079e3c84770ab3b076e78e892aab98a9f93020
./.git/objects/09/e4b0a14490917bcfa527d5a37ede29baf643ea
./.git/objects/c3/9336eedae7858762394902e31fe47408e757e6
./.git/objects/72/6345cf3747babdd713c6166294e6aaad133b47
./.git/objects/72/10cd99ad1b4ae546ee4dfbaf356d9589cbc2b1
./.git/objects/72/7c6306d9784e19c94e78b0a33bcd73fe7e2aa8
./.git/objects/86/9b8a025c6a26a0d6baabcb7e60e506e8908e2c
./.git/objects/42/855fc4ee4f2e3aed6d109c467f0379cd127369
./.git/objects/d5/c98d61ad288b808f65a3953abf6575aa3ceb87
./.git/objects/86/04dae66752d7eebc80950865e0f7f5862e71a4
./.git/objects/c5/954c871b0d1561c89ab7cff32b5d67774e5581
./.git/objects/0b/7f6dc8d75d94ed57aa82c28a769279194df81c
./.git/objects/0b/ad8252323ac183dca4052acbef5e0b1a092a99
./.git/objects/0b/8b10d660bada640047a3fe973908cf0687874a
./.git/objects/81/9a91ef3a8c09ecf5c367ae20ca282f0b1d7ef1
./.git/objects/0b/700d643f2366503a944dd113f367613bb5690f
./.git/objects/2c/6a1c7c44bcfd8821c4bfa42cb9f190e676378c
./.git/objects/2c/46221afd4538207633cd7dc899e4fb46433b3f
./.git/objects/68/148d267aa2e51a49c92b8bda2a81e59a6c7fe2
./.git/objects/16/39e244d83dc8e4f760a2042994568d4af8c77b
./.git/objects/0e/da9d3dccebc8894764f5126471bb3ce988c576
./.git/objects/14/0f766767eff6dca14ce6d7bf36e3843da17f4a
./.git/objects/c9/7b61c1963142b75205f9e2119acaed740527d2
./.git/objects/c9/a87d0c5dc0cd3124752ecfc039e6913295288f
./.git/objects/f1/87785f1ff1a4278e12a8a883aa5f954a3c92a9
./.git/objects/29/e77e42d661d9d5939f83ecba22fab8e6bcdd64
./.git/objects/44/6f93326a061e16debf8c2496789e5f4aa6aa70
./.git/objects/16/6f24ec1c7c565d412dfd373b7f515e1165c35f
./.git/objects/b6/010273ed46f47ff4af3e7eca7437a79b279c13
./.git/objects/45/e4fbb91bc432be28c5b841783220c40749bb7e
./.git/objects/6e/54923f857ef695760a2b6505dad7993e6acdae
./.git/objects/6e/df172beb62ca9d77ea98777113f8a00e43ddd1
./.git/objects/6e/48c33009478e68b9c91d9f053ea74a336cbd81
./.git/objects/2b/f230fa056889403412a9d8fe02608816c48f46
./.git/objects/8f/ebf46db7a41cfa0e85023e6e782931cf136a86
./.git/objects/83/7a5277fd53eadf14d81742b554050b0771c857
./.git/objects/83/9d73bc0ab735b8a17ca1b8b92cd7fde13dd057
./.git/objects/e9/271348b9924b586f799f4455fc5a5c9953817d
./.git/objects/70/09384b80fa2d0fe528b7fe1e309e5a001c104c
./.git/objects/22/48b7cc3b53c0768c82d8398bb03f9eecb7def6
./.git/objects/d3/b572e5310d926e803c088579eb11e4298b5e8c
./.git/objects/de/6726a7708fa5153ff377528993bcab44fe6a46
./.git/objects/9c/bd8bc8184b2830ad7e59824a59d802979299c9
./.git/objects/ac/b4fcdcbe5649e06c58c01d12598ddf8f7e8f68
./.git/objects/ac/7120bd720867f24b98b21423ae79b6fc9b13ab
./.git/objects/a2/2846ba5f8b0eb8423cf92a44c5944c96b2159c
./.git/objects/a2/e33b522bea20039f8fbc8570ba4597fab99ea8
./.git/objects/ab/2958a325e99cd9402a18dcc4c55d816f8eb2ad
./.git/objects/eb/557acee53a52ebe94aafffd472aca1d730b58a
./.git/objects/ee/ef02550dfde1f8e8d78e3bec225d30d2d3fc27
./.git/objects/f8/79a384e646d58e8ac33e12939f36ad4cfe1fd6
./.git/objects/40/daf94337fec045dc6e125f12fbf058775f417d
./.git/objects/40/8d3d65efaf30d74e9d9efb101882c88bd18617
./.git/objects/40/a258043ac34a0cb6729f9b54d8431aa30a6c5f
./.git/objects/a7/ae11631a5a7afb43171ad7cc73aa386ad65086
./.git/objects/ab/647dd198a1d32cbd6392e4788e012d517a5a24
./.git/objects/ab/2e2d96924b9d1a6a4e028a01c69c0880306148
./.git/objects/e5/3883cae824143b7ce9875dce9dfe01ee21a0b3
./.git/objects/e5/d8a79710c5efe7999fe0c1b0b5e7e6c72b441a
./.git/objects/3d/a173752990b455d1dc691de79a172d0a64db9c
./.git/objects/19/032be60748224982841286ef654905b77ffa21
./.git/objects/5f/a2fa2194d71d0465ed2c3f7130fda908e2d640
./.git/objects/5f/201cf7649fb3a6526c884897abfb35720ebb21
./.git/objects/0f/c322888c190eb4eda47b31e5a5c905948f1484
./.git/objects/03/9a116943cad3c819bae58daa5b9b6c646f6fb3
./.git/objects/e0/80419364a7042f83a1723b94dfb2931c968247
./.git/objects/e7/4950aa277997e68ca83eb3bf3d08e7b3ee53ea
./.git/objects/78/c97853cbe14a2952a90a3b8fa3f4171565fb2c
./.git/objects/3f/74bb095cf5ada6e6f3dfe7d4b06d3572c24f61
./.git/objects/3f/01ea2678c5c4411bee7164e2f5abb82c106682
./.git/objects/11/ed22c11b29675fbe585960171ded2dd1196c59
./.git/objects/26/9b350f53cfa0b68a104deb8569495666629ef2
./.git/objects/11/9bcc4290dd7e92f564599fa794ae6704cef980
./.git/objects/11/a1000fd898ef612b6c3bcf0fa8920decbfc8bc
./.git/objects/af/32de8aa0c209e31dc2159c88a2b83a7e67a34c
./.git/objects/10/285c12b3a8a1f4d9ca72a70647b0f82e586f4d
./.git/objects/af/eeaa7c36250f659f54fa44e0797888caae9f7e
./.git/objects/56/c5be59ce7658658e6730c229027eba28ab13f6
./.git/objects/be/c65f4b216eb675db283c406afad4b3393a95b1
./.git/objects/06/35cd062b0d7a1b77d02f2865e163196e19c4e9
./.git/objects/d4/dd06cc242fe9953846597515fd93d436930a98

exec
/bin/bash -lc 'rg -n "run_export|make_cuboid|tessellate_solid_with|test_assembly_roundtrip|workspace.dependencies|thiserror|TriangleMesh|build_solid_from_features|Document::from_path|InvalidExtension|make_cylinder|Euler|HalfEdge|Shell|Loop|IdGenerator" -S .' in /home/bacon/mycad
 succeeded in 0ms:
./Cargo.toml:18:[workspace.dependencies]
./Cargo.toml:30:thiserror = "2"
./ARCHITECTURE.md:56:`mycad-kernel` はレンダリングライブラリに一切依存しない。テッセレーション結果（`TriangleMesh`）を生成するのみ。ビューアがそれを消費する。
./ARCHITECTURE.md:61:.mycad file → Document → Feature列 → Kernel (B-rep Solid) → Tessellation → TriangleMesh → Viewer
./README.md:7:- **B-rep カーネル**: Solid, Shell, Face, Edge, Vertex によるトポロジー表現
./ROADMAP.md:15:- B-rep トポロジー型 (`Solid`/`Shell`/`Face`/`Loop`/`Edge`/`HalfEdge`/`Vertex`) が実装済み
./ROADMAP.md:17:- `make_cuboid` → `tessellate_solid` のパイプラインが動作し、決定性テストが通る
./.claude/skills/3ai/SKILL.md:58:- 決定性要件: (IdGenerator の使い方、同一入力→同一出力の保証方法)
./.claude/skills/3ai/SKILL.md:59:- B-rep トポロジー妥当性: (Euler-Poincaré が成立するか: V - E + F = 2)
./.claude/skills/3ai/SKILL.md:62:- エラーハンドリング: thiserror の使い方
./.claude/skills/3ai/SKILL.md:63:- workspace.dependencies 規約
./docs/decisions/001-brep-representation.md:29:- `Solid` が最上位エンティティ、内部に Shell → Face → Loop → HalfEdge → Vertex の階層
./.claude/skills/3ai/agents/glm-implementer.md:19:- ID 生成は必ず `IdGenerator::next()` を使う。`Uuid::new_v4()`、`rand`、timestamp は禁止
./.claude/skills/3ai/agents/glm-implementer.md:25:- HalfEdge: `twin`/`next`/`prev` インデックスが循環的に正しく設定されているか確認
./.claude/skills/3ai/agents/glm-implementer.md:26:- 完成後に Euler-Poincaré の公式 `V - E + F = 2` が成立することを確認（単純立体の場合）
./.claude/skills/3ai/agents/glm-implementer.md:34:- 新規依存は `Cargo.toml` の `[workspace.dependencies]` に追加し、各クレートは `{ workspace = true }` で参照
./.claude/skills/3ai/agents/glm-implementer.md:35:- ライブラリエラーは `thiserror` で定義
./.claude/skills/3ai/agents/glm-implementer.md:60:- 並行安全（該当する場合）: 複数スレッドから同時に IdGenerator を呼んでも ID が衝突しないか
./.claude/skills/3ai/agents/glm-implementer.md:73:  "summary": "make_cylinder を実装。テスト22件通過（コア8、エッジ14）。Euler-Poincaré 確認済み。",
./docs/decisions/003-viewer-and-app-architecture.md:96:- `TriangleMesh` への face グループ情報付与 (ピッキングの前提) は Phase 3 の `Extrude` 実装前に対応する。
./.claude/skills/3ai/agents/codex-final-reviewer.md:9:- `IdGenerator` 以外の ID 生成（`Uuid::new_v4()`、`rand`、timestamp 等）が混入していないか
./.claude/skills/3ai/agents/codex-final-reviewer.md:14:- HalfEdge の twin/next/prev インデックスが循環的に正しく設定されているか
./.claude/skills/3ai/agents/codex-final-reviewer.md:15:- Euler-Poincaré の不変条件が保たれているか
./.claude/skills/3ai/agents/codex-final-reviewer.md:16:- Face の Loop リスト、Shell の Face リストが整合しているか
./.claude/skills/3ai/agents/codex-final-reviewer.md:25:- 新規依存が `[workspace.dependencies]` に追加され `{ workspace = true }` で参照されているか
./.claude/skills/3ai/agents/codex-final-reviewer.md:26:- エラー型が `thiserror` で定義されているか
./.claude/skills/3ai/agents/codex-final-reviewer.md:48:    finding: "HalfEdge の twin インデックスが self を指している（ループ不整合）"
./.claude/skills/3ai/agents/codex-final-reviewer.md:49:    suggestion: "make_cuboid の edge 生成ロジックを見直す"
./crates/mycad-format/Cargo.toml:12:thiserror = { workspace = true }
./.claude/skills/3ai/agents/codex-design-reviewer.md:9:- `IdGenerator` を使った決定的 ID 生成になっているか
./.claude/skills/3ai/agents/codex-design-reviewer.md:14:- Euler-Poincaré の公式 `V - E + F = 2(S - H)` が成立する設計か（S=殻の数, H=貫通穴）
./.claude/skills/3ai/agents/codex-design-reviewer.md:15:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
./.claude/skills/3ai/agents/codex-design-reviewer.md:16:- Loop/Shell/Solid の入れ子構造が一貫しているか
./.claude/skills/3ai/agents/codex-design-reviewer.md:19:- ゼロ長エッジ、面積ゼロの Face、縮退した Loop の排除または明示的エラーの設計があるか
./.claude/skills/3ai/agents/codex-design-reviewer.md:24:- 新しい依存は `[workspace.dependencies]` + `{ workspace = true }` 規約に従っているか
./.claude/skills/3ai/agents/codex-design-reviewer.md:25:- ライブラリエラーには `thiserror` を使用しているか
./.claude/skills/3ai/agents/codex-design-reviewer.md:26:- カーネルにレンダリング依存が混入していないか（`TriangleMesh` 生成のみ可）
./.claude/skills/3ai/agents/codex-design-reviewer.md:30:- Euler-Poincaré 検証テストがあるか
./.claude/skills/3ai/agents/codex-design-reviewer.md:48:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
./.claude/skills/3ai/agents/codex-design-reviewer.md:49:    suggestion: "IdGenerator::next() に置き換えること"
./docs/decisions/002-roadmap-management.md:54:- フルパイプライン (Feature → Solid → TriangleMesh → 出力) を最短で通すことを優先
./Cargo.lock:306: "thiserror",
./Cargo.lock:317: "thiserror",
./Cargo.lock:632:name = "thiserror"
./Cargo.lock:637: "thiserror-impl",
./Cargo.lock:641:name = "thiserror-impl"
./features/12-cylinder/design-review.md:22:- `IdGenerator` を使った決定的 ID 生成になっているか
./features/12-cylinder/design-review.md:27:- Euler-Poincaré の公式 `V - E + F = 2(S - H)` が成立する設計か（S=殻の数, H=貫通穴）
./features/12-cylinder/design-review.md:28:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
./features/12-cylinder/design-review.md:29:- Loop/Shell/Solid の入れ子構造が一貫しているか
./features/12-cylinder/design-review.md:32:- ゼロ長エッジ、面積ゼロの Face、縮退した Loop の排除または明示的エラーの設計があるか
./features/12-cylinder/design-review.md:37:- 新しい依存は `[workspace.dependencies]` + `{ workspace = true }` 規約に従っているか
./features/12-cylinder/design-review.md:38:- ライブラリエラーには `thiserror` を使用しているか
./features/12-cylinder/design-review.md:39:- カーネルにレンダリング依存が混入していないか（`TriangleMesh` 生成のみ可）
./features/12-cylinder/design-review.md:43:- Euler-Poincaré 検証テストがあるか
./features/12-cylinder/design-review.md:61:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
./features/12-cylinder/design-review.md:62:    suggestion: "IdGenerator::next() に置き換えること"
./features/12-cylinder/design-review.md:79:ROADMAP Phase 3「円柱・球・押し出しが作れる」の最初の一歩。現状カーネルは `make_cuboid` と平面専用の `tessellate_solid` しか持たない。本 Issue で **解析曲面(円柱)を厳密 B-rep として生成し、それを三角形メッシュへ離散化する土台**を確立し、`mycad export examples/cylinder.mycad -o out.stl` を動作させる。
./features/12-cylinder/design-review.md:90:  - 新規 `crates/mycad-kernel/src/primitives/cylinder.rs` — `make_cylinder`
./features/12-cylinder/design-review.md:102:### 1. トポロジー（`make_cylinder(radius, height, id_gen) -> Solid`）
./features/12-cylinder/design-review.md:123:**Euler–Poincaré**: V−E+F = 2−3+3 = 2 = 2(S−H), S=1,H=0 → 成立。
./features/12-cylinder/design-review.md:125:**決定性**: ID 採番順を `make_cuboid` と同型に固定。solid → 頂点(v_bot,v_top) → エッジ(e_bot,e_top,e_seam) → 面ごとに[底,上,側]順で HE群→loop→face → shell。すべて `id_gen.next()` を固定順で呼ぶのみ。`HashMap`/乱数等の非決定要素なし。
./features/12-cylinder/design-review.md:127:**退化**: `radius <= 0` または `height <= 0` はゼロ面積/ゼロ長を生むため将来的にエラー化対象だが、現状 `make_cuboid` も正値前提で検証していない。本 Issue は **正値前提**を踏襲し、退化はテストで「正値での妥当性」を担保（負値の明示エラーはスコープ外、必要なら Codex 指摘で追加検討）。
./features/12-cylinder/design-review.md:137:  pub fn tessellate_solid(solid) -> TriangleMesh           // = _with(solid, &default())  ※既存呼び出し・既存テスト不変
./features/12-cylinder/design-review.md:138:  pub fn tessellate_solid_with(solid, &TessellationOptions) -> TriangleMesh
./features/12-cylinder/design-review.md:152:- `mycad-build/src/lib.rs`: `Feature::CreateCylinder{radius,height}` 分岐を `Ok(make_cylinder(*radius, *height, gen))` に差し替え（`UnsupportedFeature` スタブ削除）。`Feature` enum・`id()`・`KernelError` は既存のまま変更不要。
./features/12-cylinder/design-review.md:153:- `mycad-cli`: `export` に `--segments <N>`（任意、未指定なら `TessellationOptions::default`）。export パスは `tessellate_solid_with` を呼ぶ。
./features/12-cylinder/design-review.md:168:- `TessellationOptions` に `Debug, Clone, Serialize, Deserialize`（既存 `TriangleMesh` と同様）。`Copy` も可。
./features/12-cylinder/design-review.md:176:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
./features/12-cylinder/design-review.md:178:| T03 | Euler | V−E+F == 2 | `assert_eq!(2-3+3, 2)` 相当の検証 |
./features/12-cylinder/design-review.md:183:| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index | 既存テスト維持 |
./features/12-cylinder/design-review.md:185:| T10 | ディスパッチ統合 | `examples/cylinder.mycad` を `build_solid_from_features` でビルドし T02 と同じ counts | `feature_dispatcher.rs` |
./features/12-cylinder/design-review.md:187:| T12 | 解像度外部化 | `--segments` / `tessellate_solid_with` で分割数が変わり三角形数が増減 | 2 解像度で差分確認 |
./features/12-cylinder/design-review.md:201:設計ドキュメントを MyCad の実装規約に照らして確認します。まず既存の `make_cuboid`、トポロジー型、テッセレーション、ADR とテスト構成を読み、提案内容と矛盾する点を特定します。
./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
./features/12-cylinder/design-review.md:404:Cargo.toml:18:[workspace.dependencies]
./features/12-cylinder/design-review.md:405:Cargo.toml:30:thiserror = "2"
./features/12-cylinder/design-review.md:406:crates/mycad-format/Cargo.toml:12:thiserror = { workspace = true }
./features/12-cylinder/design-review.md:407:ARCHITECTURE.md:56:`mycad-kernel` はレンダリングライブラリに一切依存しない。テッセレーション結果（`TriangleMesh`）を生成するのみ。ビューアがそれを消費する。
./features/12-cylinder/design-review.md:408:ARCHITECTURE.md:61:.mycad file → Document → Feature列 → Kernel (B-rep Solid) → Tessellation → TriangleMesh → Viewer
./features/12-cylinder/design-review.md:409:docs/decisions/001-brep-representation.md:29:- `Solid` が最上位エンティティ、内部に Shell → Face → Loop → HalfEdge → Vertex の階層
./features/12-cylinder/design-review.md:410:.claude/skills/3ai/agents/glm-implementer.md:19:- ID 生成は必ず `IdGenerator::next()` を使う。`Uuid::new_v4()`、`rand`、timestamp は禁止
./features/12-cylinder/design-review.md:411:.claude/skills/3ai/agents/glm-implementer.md:25:- HalfEdge: `twin`/`next`/`prev` インデックスが循環的に正しく設定されているか確認
./features/12-cylinder/design-review.md:413:.claude/skills/3ai/agents/glm-implementer.md:34:- 新規依存は `Cargo.toml` の `[workspace.dependencies]` に追加し、各クレートは `{ workspace = true }` で参照
./features/12-cylinder/design-review.md:414:.claude/skills/3ai/agents/glm-implementer.md:35:- ライブラリエラーは `thiserror` で定義
./features/12-cylinder/design-review.md:415:.claude/skills/3ai/agents/glm-implementer.md:60:- 並行安全（該当する場合）: 複数スレッドから同時に IdGenerator を呼んでも ID が衝突しないか
./features/12-cylinder/design-review.md:416:.claude/skills/3ai/SKILL.md:51:- 決定性要件: (IdGenerator の使い方、同一入力→同一出力の保証方法)
./features/12-cylinder/design-review.md:418:.claude/skills/3ai/SKILL.md:55:- エラーハンドリング: thiserror の使い方
./features/12-cylinder/design-review.md:419:.claude/skills/3ai/SKILL.md:56:- workspace.dependencies 規約
./features/12-cylinder/design-review.md:420:CLAUDE.md:27:│   ├── brep/         トポロジー (Solid, Shell, Face, Loop, Edge, HalfEdge, Vertex)
./features/12-cylinder/design-review.md:421:CLAUDE.md:29:│   ├── primitives/   プリミティブ生成 (make_cuboid)
./features/12-cylinder/design-review.md:422:CLAUDE.md:30:│   └── tessellation/ メッシュ化 (tessellate_solid → TriangleMesh)
./features/12-cylinder/design-review.md:423:CLAUDE.md:53:- **決定性**: すべてのEntityIDは `IdGenerator` で決定的に生成。テストでは決定性を検証すること。
./features/12-cylinder/design-review.md:424:CLAUDE.md:54:- **依存管理**: 新しい依存は必ず `[workspace.dependencies]` に追加し、各クレートは `{ workspace = true }` で参照。
./features/12-cylinder/design-review.md:426:CLAUDE.md:56:- **エラーハンドリング**: ライブラリエラーには `thiserror` を使用。
./features/12-cylinder/design-review.md:428:CLAUDE.md:64:- **Kernel has zero rendering deps**: カーネルはレンダリング非依存。`TriangleMesh` を生成し、ビューアが消費する。
./features/12-cylinder/design-review.md:429:.claude/skills/3ai/agents/codex-final-reviewer.md:9:- `IdGenerator` 以外の ID 生成（`Uuid::new_v4()`、`rand`、timestamp 等）が混入していないか
./features/12-cylinder/design-review.md:430:.claude/skills/3ai/agents/codex-final-reviewer.md:14:- HalfEdge の twin/next/prev インデックスが循環的に正しく設定されているか
./features/12-cylinder/design-review.md:431:.claude/skills/3ai/agents/codex-final-reviewer.md:25:- 新規依存が `[workspace.dependencies]` に追加され `{ workspace = true }` で参照されているか
./features/12-cylinder/design-review.md:432:.claude/skills/3ai/agents/codex-final-reviewer.md:26:- エラー型が `thiserror` で定義されているか
./features/12-cylinder/design-review.md:433:.claude/skills/3ai/agents/codex-final-reviewer.md:48:    finding: "HalfEdge の twin インデックスが self を指している（ループ不整合）"
./features/12-cylinder/design-review.md:434:.claude/skills/3ai/agents/codex-final-reviewer.md:49:    suggestion: "make_cuboid の edge 生成ロジックを見直す"
./features/12-cylinder/design-review.md:437:ROADMAP.md:15:- B-rep トポロジー型 (`Solid`/`Shell`/`Face`/`Loop`/`Edge`/`HalfEdge`/`Vertex`) が実装済み
./features/12-cylinder/design-review.md:438:ROADMAP.md:17:- `make_cuboid` → `tessellate_solid` のパイプラインが動作し、決定性テストが通る
./features/12-cylinder/design-review.md:441:features/12-cylinder/design-review.md:22:- `IdGenerator` を使った決定的 ID 生成になっているか
./features/12-cylinder/design-review.md:442:features/12-cylinder/design-review.md:28:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
./features/12-cylinder/design-review.md:444:features/12-cylinder/design-review.md:37:- 新しい依存は `[workspace.dependencies]` + `{ workspace = true }` 規約に従っているか
./features/12-cylinder/design-review.md:445:features/12-cylinder/design-review.md:38:- ライブラリエラーには `thiserror` を使用しているか
./features/12-cylinder/design-review.md:446:features/12-cylinder/design-review.md:39:- カーネルにレンダリング依存が混入していないか（`TriangleMesh` 生成のみ可）
./features/12-cylinder/design-review.md:448:features/12-cylinder/design-review.md:61:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
./features/12-cylinder/design-review.md:449:features/12-cylinder/design-review.md:62:    suggestion: "IdGenerator::next() に置き換えること"
./features/12-cylinder/design-review.md:450:features/12-cylinder/design-review.md:79:ROADMAP Phase 3「円柱・球・押し出しが作れる」の最初の一歩。現状カーネルは `make_cuboid` と平面専用の `tessellate_solid` しか持たない。本 Issue で **解析曲面(円柱)を厳密 B-rep として生成し、それを三角形メッシュへ離散化する土台**を確立し、`mycad export examples/cylinder.mycad -o out.stl` を動作させる。
./features/12-cylinder/design-review.md:454:features/12-cylinder/design-review.md:125:**決定性**: ID 採番順を `make_cuboid` と同型に固定。solid → 頂点(v_bot,v_top) → エッジ(e_bot,e_top,e_seam) → 面ごとに[底,上,側]順で HE群→loop→face → shell。すべて `id_gen.next()` を固定順で呼ぶのみ。`HashMap`/乱数等の非決定要素なし。
./features/12-cylinder/design-review.md:455:features/12-cylinder/design-review.md:127:**退化**: `radius <= 0` または `height <= 0` はゼロ面積/ゼロ長を生むため将来的にエラー化対象だが、現状 `make_cuboid` も正値前提で検証していない。本 Issue は **正値前提**を踏襲し、退化はテストで「正値での妥当性」を担保（負値の明示エラーはスコープ外、必要なら Codex 指摘で追加検討）。
./features/12-cylinder/design-review.md:458:features/12-cylinder/design-review.md:137:  pub fn tessellate_solid(solid) -> TriangleMesh           // = _with(solid, &default())  ※既存呼び出し・既存テスト不変
./features/12-cylinder/design-review.md:459:features/12-cylinder/design-review.md:138:  pub fn tessellate_solid_with(solid, &TessellationOptions) -> TriangleMesh
./features/12-cylinder/design-review.md:460:features/12-cylinder/design-review.md:152:- `mycad-build/src/lib.rs`: `Feature::CreateCylinder{radius,height}` 分岐を `Ok(make_cylinder(*radius, *height, gen))` に差し替え（`UnsupportedFeature` スタブ削除）。`Feature` enum・`id()`・`KernelError` は既存のまま変更不要。
./features/12-cylinder/design-review.md:461:features/12-cylinder/design-review.md:153:- `mycad-cli`: `export` に `--segments <N>`（任意、未指定なら `TessellationOptions::default`）。export パスは `tessellate_solid_with` を呼ぶ。
./features/12-cylinder/design-review.md:462:features/12-cylinder/design-review.md:168:- `TessellationOptions` に `Debug, Clone, Serialize, Deserialize`（既存 `TriangleMesh` と同様）。`Copy` も可。
./features/12-cylinder/design-review.md:463:features/12-cylinder/design-review.md:176:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
./features/12-cylinder/design-review.md:465:features/12-cylinder/design-review.md:183:| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index | 既存テスト維持 |
./features/12-cylinder/design-review.md:466:features/12-cylinder/design-review.md:187:| T12 | 解像度外部化 | `--segments` / `tessellate_solid_with` で分割数が変わり三角形数が増減 | 2 解像度で差分確認 |
./features/12-cylinder/design-review.md:467:features/12-cylinder/design-review.md:201:設計ドキュメントを MyCad の実装規約に照らして確認します。まず既存の `make_cuboid`、トポロジー型、テッセレーション、ADR とテスト構成を読み、提案内容と矛盾する点を特定します。
./features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
./features/12-cylinder/design-review.md:471:docs/decisions/002-roadmap-management.md:54:- フルパイプライン (Feature → Solid → TriangleMesh → 出力) を最短で通すことを優先
./features/12-cylinder/design-review.md:473:.claude/skills/3ai/agents/codex-design-reviewer.md:9:- `IdGenerator` を使った決定的 ID 生成になっているか
./features/12-cylinder/design-review.md:474:.claude/skills/3ai/agents/codex-design-reviewer.md:15:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
./features/12-cylinder/design-review.md:476:.claude/skills/3ai/agents/codex-design-reviewer.md:24:- 新しい依存は `[workspace.dependencies]` + `{ workspace = true }` 規約に従っているか
./features/12-cylinder/design-review.md:477:.claude/skills/3ai/agents/codex-design-reviewer.md:25:- ライブラリエラーには `thiserror` を使用しているか
./features/12-cylinder/design-review.md:478:.claude/skills/3ai/agents/codex-design-reviewer.md:26:- カーネルにレンダリング依存が混入していないか（`TriangleMesh` 生成のみ可）
./features/12-cylinder/design-review.md:480:.claude/skills/3ai/agents/codex-design-reviewer.md:48:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
./features/12-cylinder/design-review.md:481:.claude/skills/3ai/agents/codex-design-reviewer.md:49:    suggestion: "IdGenerator::next() に置き換えること"
./features/12-cylinder/design-review.md:482:Cargo.lock:306: "thiserror",
./features/12-cylinder/design-review.md:483:Cargo.lock:317: "thiserror",
./features/12-cylinder/design-review.md:484:Cargo.lock:632:name = "thiserror"
./features/12-cylinder/design-review.md:485:Cargo.lock:637: "thiserror-impl",
./features/12-cylinder/design-review.md:486:Cargo.lock:641:name = "thiserror-impl"
./features/12-cylinder/design-review.md:491:crates/mycad-format/src/error.rs:1:use thiserror::Error;
./features/12-cylinder/design-review.md:496:crates/mycad-format/CLAUDE.md:33:| `CreateBox`    | `make_cuboid`  |
./features/12-cylinder/design-review.md:498:crates/mycad-kernel/Cargo.toml:11:thiserror = { workspace = true }
./features/12-cylinder/design-review.md:504:crates/mycad-cli/src/main.rs:4:use mycad_kernel::brep::topology::IdGenerator;
./features/12-cylinder/design-review.md:506:crates/mycad-cli/src/main.rs:43:    let mut gen = IdGenerator::new(0);
./features/12-cylinder/design-review.md:508:crates/mycad-build/src/lib.rs:2:use mycad_kernel::brep::topology::{IdGenerator, Solid};
./features/12-cylinder/design-review.md:509:crates/mycad-build/src/lib.rs:4:use mycad_kernel::primitives::make_cuboid;
./features/12-cylinder/design-review.md:510:crates/mycad-build/src/lib.rs:8:    gen: &mut IdGenerator,
./features/12-cylinder/design-review.md:511:crates/mycad-build/src/lib.rs:26:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
./features/12-cylinder/design-review.md:520:.git/logs/HEAD:11:d5c98d61ad288b808f65a3953abf6575aa3ceb87 eb557acee53a52ebe94aafffd472aca1d730b58a GS-Bacon <bacon0817.ix@gmail.com> 1779521125 +0000	commit: feat(kernel): add ASCII STL writer for TriangleMesh
./features/12-cylinder/design-review.md:521:crates/mycad-build/tests/feature_dispatcher.rs:3:use mycad_kernel::brep::topology::IdGenerator;
./features/12-cylinder/design-review.md:522:crates/mycad-build/tests/feature_dispatcher.rs:15:    let mut g = IdGenerator::new(0);
./features/12-cylinder/design-review.md:523:crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
./features/12-cylinder/design-review.md:525:crates/mycad-kernel/src/tessellation/mod.rs:10:pub struct TriangleMesh {
./features/12-cylinder/design-review.md:526:crates/mycad-kernel/src/tessellation/mod.rs:19:impl TriangleMesh {
./features/12-cylinder/design-review.md:527:crates/mycad-kernel/src/tessellation/mod.rs:34:impl Default for TriangleMesh {
./features/12-cylinder/design-review.md:528:crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/design-review.md:529:crates/mycad-kernel/src/tessellation/mod.rs:50:    let mut mesh = TriangleMesh::new();
./features/12-cylinder/design-review.md:530:crates/mycad-kernel/src/tessellation/mod.rs:98:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review.md:531:crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
./features/12-cylinder/design-review.md:532:crates/mycad-kernel/src/tessellation/mod.rs:103:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review.md:533:crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
./features/12-cylinder/design-review.md:535:crates/mycad-kernel/src/tessellation/mod.rs:117:        let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/design-review.md:536:crates/mycad-kernel/src/tessellation/mod.rs:118:        let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/design-review.md:537:crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
./features/12-cylinder/design-review.md:538:crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
./features/12-cylinder/design-review.md:541:.git/logs/refs/heads/claude/add-claude-guidelines-BKKtD:7:d5c98d61ad288b808f65a3953abf6575aa3ceb87 eb557acee53a52ebe94aafffd472aca1d730b58a GS-Bacon <bacon0817.ix@gmail.com> 1779521125 +0000	commit: feat(kernel): add ASCII STL writer for TriangleMesh
./features/12-cylinder/design-review.md:542:crates/mycad-kernel/src/tessellation/stl.rs:1:use super::TriangleMesh;
./features/12-cylinder/design-review.md:543:crates/mycad-kernel/src/tessellation/stl.rs:4:pub fn to_ascii_stl(mesh: &TriangleMesh, solid_name: &str) -> String {
./features/12-cylinder/design-review.md:544:crates/mycad-kernel/src/tessellation/stl.rs:51:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review.md:545:crates/mycad-kernel/src/tessellation/stl.rs:52:    use crate::primitives::make_cuboid;
./features/12-cylinder/design-review.md:547:crates/mycad-kernel/src/tessellation/stl.rs:57:        let mut g = IdGenerator::new(0);
./features/12-cylinder/design-review.md:548:crates/mycad-kernel/src/tessellation/stl.rs:58:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
./features/12-cylinder/design-review.md:550:crates/mycad-kernel/src/tessellation/stl.rs:74:            let mut g = IdGenerator::new(0);
./features/12-cylinder/design-review.md:551:crates/mycad-kernel/src/tessellation/stl.rs:75:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
./features/12-cylinder/design-review.md:553:crates/mycad-kernel/src/primitives/cuboid.rs:1:use crate::brep::topology::{IdGenerator, Solid};
./features/12-cylinder/design-review.md:554:crates/mycad-kernel/src/primitives/cuboid.rs:16:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
./features/12-cylinder/design-review.md:555:crates/mycad-kernel/src/primitives/cuboid.rs:223:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review.md:556:crates/mycad-kernel/src/primitives/cuboid.rs:227:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review.md:557:crates/mycad-kernel/src/primitives/cuboid.rs:228:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
./features/12-cylinder/design-review.md:558:crates/mycad-kernel/src/primitives/cuboid.rs:247:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review.md:559:crates/mycad-kernel/src/primitives/cuboid.rs:248:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
./features/12-cylinder/design-review.md:560:crates/mycad-kernel/src/primitives/cuboid.rs:276:        let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/design-review.md:561:crates/mycad-kernel/src/primitives/cuboid.rs:277:        let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/design-review.md:562:crates/mycad-kernel/src/primitives/cuboid.rs:279:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
./features/12-cylinder/design-review.md:563:crates/mycad-kernel/src/primitives/cuboid.rs:280:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);
./features/12-cylinder/design-review.md:564:crates/mycad-kernel/src/error.rs:1:use thiserror::Error;
./features/12-cylinder/design-review.md:566:crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
./features/12-cylinder/design-review.md:567:crates/mycad-kernel/src/brep/topology.rs:21:pub struct HalfEdge {
./features/12-cylinder/design-review.md:568:crates/mycad-kernel/src/brep/topology.rs:83:    pub half_edges: Vec<HalfEdge>,
./features/12-cylinder/design-review.md:569:crates/mycad-kernel/src/brep/topology.rs:138:        self.half_edges.push(HalfEdge {
./features/12-cylinder/design-review.md:570:crates/mycad-kernel/src/brep/topology.rs:185:pub struct IdGenerator {
./features/12-cylinder/design-review.md:571:crates/mycad-kernel/src/brep/topology.rs:189:impl IdGenerator {
./features/12-cylinder/design-review.md:572:crates/mycad-kernel/src/brep/topology.rs:208:        let mut gen1 = IdGenerator::new(100);
./features/12-cylinder/design-review.md:573:crates/mycad-kernel/src/brep/topology.rs:209:        let mut gen2 = IdGenerator::new(100);
./features/12-cylinder/design-review.md:574:crates/mycad-kernel/benches/tessellation.rs:1:use mycad_kernel::brep::topology::IdGenerator;
./features/12-cylinder/design-review.md:575:crates/mycad-kernel/benches/tessellation.rs:2:use mycad_kernel::primitives::make_cuboid;
./features/12-cylinder/design-review.md:577:crates/mycad-kernel/benches/tessellation.rs:14:        let mut id_gen = IdGenerator::new(i as u64);
./features/12-cylinder/design-review.md:578:crates/mycad-kernel/benches/tessellation.rs:15:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
./features/12-cylinder/design-review.md:580:crates/mycad-kernel/tests/cuboid_roundtrip.rs:1:use mycad_kernel::brep::topology::IdGenerator;
./features/12-cylinder/design-review.md:581:crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
./features/12-cylinder/design-review.md:583:crates/mycad-kernel/tests/cuboid_roundtrip.rs:8:    let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review.md:584:crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
./features/12-cylinder/design-review.md:586:crates/mycad-kernel/tests/cuboid_roundtrip.rs:30:        let mut id_gen = IdGenerator::new(42);
./features/12-cylinder/design-review.md:587:crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
./features/12-cylinder/design-review.md:589:crates/mycad-kernel/CLAUDE.md:14:- すべてのEdgeは正確に2つのHalfEdgeを持つ
./features/12-cylinder/design-review.md:590:crates/mycad-kernel/CLAUDE.md:16:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
./features/12-cylinder/design-review.md:591:crates/mycad-kernel/CLAUDE.md:17:- Loopは閉じたHalfEdge列（最後のHalfEdgeの終点 = 最初のHalfEdgeの始点）
./features/12-cylinder/design-review.md:592:crates/mycad-kernel/CLAUDE.md:18:- EntityIDは `IdGenerator` で決定的に生成される
./features/12-cylinder/design-review.md:593:crates/mycad-kernel/CLAUDE.md:23:2. 関数シグネチャ: `pub fn make_xxx(params..., id_gen: &mut IdGenerator) -> Solid`
./features/12-cylinder/design-review.md:595:crates/mycad-kernel/CLAUDE.md:25:4. 決定性テストを必ず書く（同じ `IdGenerator` 初期値で2回生成し、結果が同一であることを確認）
./features/12-cylinder/design-review.md:622:pub struct HalfEdge {
./features/12-cylinder/design-review.md:647:pub struct Loop {
./features/12-cylinder/design-review.md:670:pub struct Shell {
./features/12-cylinder/design-review.md:684:    pub half_edges: Vec<HalfEdge>,
./features/12-cylinder/design-review.md:686:    pub loops: Vec<Loop>,
./features/12-cylinder/design-review.md:688:    pub shells: Vec<Shell>,
./features/12-cylinder/design-review.md:739:        self.half_edges.push(HalfEdge {
./features/12-cylinder/design-review.md:751:        self.loops.push(Loop { id, half_edges });
./features/12-cylinder/design-review.md:778:        self.shells.push(Shell { id, faces, closed });
./features/12-cylinder/design-review.md:786:pub struct IdGenerator {
./features/12-cylinder/design-review.md:790:impl IdGenerator {
./features/12-cylinder/design-review.md:809:        let mut gen1 = IdGenerator::new(100);
./features/12-cylinder/design-review.md:810:        let mut gen2 = IdGenerator::new(100);
./features/12-cylinder/design-review.md:829:use crate::brep::topology::{IdGenerator, Solid};
./features/12-cylinder/design-review.md:844:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
./features/12-cylinder/design-review.md:1051:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review.md:1055:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review.md:1056:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
./features/12-cylinder/design-review.md:1075:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review.md:1076:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
./features/12-cylinder/design-review.md:1104:        let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/design-review.md:1105:        let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/design-review.md:1107:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
./features/12-cylinder/design-review.md:1108:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);
./features/12-cylinder/design-review.md:1181:pub struct TriangleMesh {
./features/12-cylinder/design-review.md:1190:impl TriangleMesh {
./features/12-cylinder/design-review.md:1205:impl Default for TriangleMesh {
./features/12-cylinder/design-review.md:1220:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/design-review.md:1221:    let mut mesh = TriangleMesh::new();
./features/12-cylinder/design-review.md:1269:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review.md:1270:    use crate::primitives::make_cuboid;
./features/12-cylinder/design-review.md:1274:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review.md:1275:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
./features/12-cylinder/design-review.md:1288:        let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/design-review.md:1289:        let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/design-review.md:1291:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
./features/12-cylinder/design-review.md:1292:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
./features/12-cylinder/design-review.md:1598:トポロジー型を読むと、現行 `HalfEdge` には `twin/next/prev` 自体がありません。提案が「マニフォルド規約」を言語化していても、レビュー観点の要件を満たすにはこのギャップをどう埋めるかが重要です。
./features/12-cylinder/design-review.md:1603:use mycad_build::build_solid_from_features;
./features/12-cylinder/design-review.md:1605:use mycad_kernel::brep::topology::IdGenerator;
./features/12-cylinder/design-review.md:1633:            if let Err(msg) = run_export(&input, &output) {
./features/12-cylinder/design-review.md:1641:fn run_export(input: &std::path::Path, output: &std::path::Path) -> Result<(), String> {
./features/12-cylinder/design-review.md:1642:    let doc = Document::from_path(input)
./features/12-cylinder/design-review.md:1644:    let mut gen = IdGenerator::new(0);
./features/12-cylinder/design-review.md:1645:    let solid = build_solid_from_features(&doc.root_component.features, &mut gen)
./features/12-cylinder/design-review.md:1658:use mycad_kernel::brep::topology::{IdGenerator, Solid};
./features/12-cylinder/design-review.md:1660:use mycad_kernel::primitives::make_cuboid;
./features/12-cylinder/design-review.md:1662:pub fn build_solid_from_features(
./features/12-cylinder/design-review.md:1664:    gen: &mut IdGenerator,
./features/12-cylinder/design-review.md:1682:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
./features/12-cylinder/design-review.md:1839:[workspace.dependencies]
./features/12-cylinder/design-review.md:1851:thiserror = "2"
./features/12-cylinder/design-review.md:1864:use mycad_build::build_solid_from_features;
./features/12-cylinder/design-review.md:1866:use mycad_kernel::brep::topology::IdGenerator;
./features/12-cylinder/design-review.md:1877:    let doc = Document::from_path(&path).expect("load .mycad");
./features/12-cylinder/design-review.md:1878:    let mut g = IdGenerator::new(0);
./features/12-cylinder/design-review.md:1880:        build_solid_from_features(&doc.root_component.features, &mut g).expect("build solid");
./features/12-cylinder/design-review.md:1954:- `Solid` が最上位エンティティ、内部に Shell → Face → Loop → HalfEdge → Vertex の階層
./features/12-cylinder/design-review.md:2086:- すべてのEdgeは正確に2つのHalfEdgeを持つ
./features/12-cylinder/design-review.md:2087:- 閉じたShellはwatertight（水密）
./features/12-cylinder/design-review.md:2088:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
./features/12-cylinder/design-review.md:2089:- Loopは閉じたHalfEdge列（最後のHalfEdgeの終点 = 最初のHalfEdgeの始点）
./features/12-cylinder/design-review.md:2090:- EntityIDは `IdGenerator` で決定的に生成される
./features/12-cylinder/design-review.md:2095:2. 関数シグネチャ: `pub fn make_xxx(params..., id_gen: &mut IdGenerator) -> Solid`
./features/12-cylinder/design-review.md:2097:4. 決定性テストを必ず書く（同じ `IdGenerator` 初期値で2回生成し、結果が同一であることを確認）
./features/12-cylinder/design-review.md:2153:            return Err(FormatError::InvalidExtension(ext.map(str::to_string)));
./features/12-cylinder/design-review.md:2210:        let doc = Document::from_path(&path).unwrap();
./features/12-cylinder/design-review.md:2218:        let err = Document::from_path(path).unwrap_err();
./features/12-cylinder/design-review.md:2225:        let err = Document::from_path(path).unwrap_err();
./features/12-cylinder/design-review.md:2226:        assert!(matches!(err, FormatError::InvalidExtension(Some(ref e)) if e == "txt"));
./features/12-cylinder/design-review.md:2230:    fn test_assembly_roundtrip() {
./features/12-cylinder/design-review.md:2238:        let doc = Document::from_path(&path).unwrap();
./features/12-cylinder/design-review.md:2261:use mycad_kernel::brep::topology::IdGenerator;
./features/12-cylinder/design-review.md:2262:use mycad_kernel::primitives::make_cuboid;
./features/12-cylinder/design-review.md:2268:    let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review.md:2269:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
./features/12-cylinder/design-review.md:2290:        let mut id_gen = IdGenerator::new(42);
./features/12-cylinder/design-review.md:2291:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
./features/12-cylinder/design-review.md:2435:`mycad-kernel` はレンダリングライブラリに一切依存しない。テッセレーション結果（`TriangleMesh`）を生成するのみ。ビューアがそれを消費する。
./features/12-cylinder/design-review.md:2440:.mycad file → Document → Feature列 → Kernel (B-rep Solid) → Tessellation → TriangleMesh → Viewer
./features/12-cylinder/design-review.md:2463:- `IdGenerator` を使った決定的 ID 生成になっているか
./features/12-cylinder/design-review.md:2468:- Euler-Poincaré の公式 `V - E + F = 2(S - H)` が成立する設計か（S=殻の数, H=貫通穴）
./features/12-cylinder/design-review.md:2469:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
./features/12-cylinder/design-review.md:2470:- Loop/Shell/Solid の入れ子構造が一貫しているか
./features/12-cylinder/design-review.md:2473:- ゼロ長エッジ、面積ゼロの Face、縮退した Loop の排除または明示的エラーの設計があるか
./features/12-cylinder/design-review.md:2478:- 新しい依存は `[workspace.dependencies]` + `{ workspace = true }` 規約に従っているか
./features/12-cylinder/design-review.md:2479:- ライブラリエラーには `thiserror` を使用しているか
./features/12-cylinder/design-review.md:2480:- カーネルにレンダリング依存が混入していないか（`TriangleMesh` 生成のみ可）
./features/12-cylinder/design-review.md:2484:- Euler-Poincaré 検証テストがあるか
./features/12-cylinder/design-review.md:2502:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
./features/12-cylinder/design-review.md:2503:    suggestion: "IdGenerator::next() に置き換えること"
./features/12-cylinder/design-review.md:2520:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
./features/12-cylinder/design-review.md:2521:    suggestion: "`sample_segment` を HalfEdge 向き込みの API に変更し、rev では `t1` から降順にサンプルするか、先頭点を必ず `HalfEdge.start_vertex` から取得すること。reversed line edge を含む cuboid 回帰テストも追加すること。"
./features/12-cylinder/design-review.md:2526:    suggestion: "primitive 生成時と `TessellationOptions` 構築時に下限チェックを入れ、違反時は `thiserror` ベースの `KernelError` で明示エラーにすること。CLI でも同じ境界値を拒否すること。"
./features/12-cylinder/design-review.md:2530:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
./features/12-cylinder/design-review.md:2531:    suggestion: "`HalfEdge` に隣接インデックスを追加して決定的に構築・検証するか、少なくとも各 loop で consecutive half-edge の終点/始点一致を全件検証する派生テストを追加すること。"
./features/12-cylinder/design-review.md:2541:    suggestion: "`Document::from_path` -> `to_yaml` -> `from_yaml` -> `to_yaml` の equality を `examples/cylinder.mycad` に対して追加し、`create_cylinder` の schema/serialize 回帰を固定すること。"
./features/12-cylinder/design-review.md:2550:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
./features/12-cylinder/design-review.md:2551:    suggestion: "`sample_segment` を HalfEdge 向き込みの API に変更し、rev では `t1` から降順にサンプルするか、先頭点を必ず `HalfEdge.start_vertex` から取得すること。reversed line edge を含む cuboid 回帰テストも追加すること。"
./features/12-cylinder/design-review.md:2556:    suggestion: "primitive 生成時と `TessellationOptions` 構築時に下限チェックを入れ、違反時は `thiserror` ベースの `KernelError` で明示エラーにすること。CLI でも同じ境界値を拒否すること。"
./features/12-cylinder/design-review.md:2560:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
./features/12-cylinder/design-review.md:2561:    suggestion: "`HalfEdge` に隣接インデックスを追加して決定的に構築・検証するか、少なくとも各 loop で consecutive half-edge の終点/始点一致を全件検証する派生テストを追加すること。"
./features/12-cylinder/design-review.md:2571:    suggestion: "`Document::from_path` -> `to_yaml` -> `from_yaml` -> `to_yaml` の equality を `examples/cylinder.mycad` に対して追加し、`create_cylinder` の schema/serialize 回帰を固定すること。"
./crates/mycad-format/src/component.rs:12:    /// Rotation as Euler angles in degrees [rx, ry, rz].
./crates/mycad-format/src/error.rs:1:use thiserror::Error;
./crates/mycad-format/src/error.rs:12:    InvalidExtension(Option<String>),
./.git/logs/HEAD:5:49c3b468feaf139d9c75a35c7d3a49a65a960a34 752ac98646de4b09bad18047d96478c284161173 GS-Bacon <bacon0817.ix@gmail.com> 1779519904 +0000	commit: feat(format): add Document::from_path and FormatError
./.git/logs/HEAD:11:d5c98d61ad288b808f65a3953abf6575aa3ceb87 eb557acee53a52ebe94aafffd472aca1d730b58a GS-Bacon <bacon0817.ix@gmail.com> 1779521125 +0000	commit: feat(kernel): add ASCII STL writer for TriangleMesh
./features/12-cylinder/design-review-2.md:22:- `IdGenerator` を使った決定的 ID 生成になっているか
./features/12-cylinder/design-review-2.md:27:- Euler-Poincaré の公式 `V - E + F = 2(S - H)` が成立する設計か（S=殻の数, H=貫通穴）
./features/12-cylinder/design-review-2.md:28:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
./features/12-cylinder/design-review-2.md:29:- Loop/Shell/Solid の入れ子構造が一貫しているか
./features/12-cylinder/design-review-2.md:32:- ゼロ長エッジ、面積ゼロの Face、縮退した Loop の排除または明示的エラーの設計があるか
./features/12-cylinder/design-review-2.md:37:- 新しい依存は `[workspace.dependencies]` + `{ workspace = true }` 規約に従っているか
./features/12-cylinder/design-review-2.md:38:- ライブラリエラーには `thiserror` を使用しているか
./features/12-cylinder/design-review-2.md:39:- カーネルにレンダリング依存が混入していないか（`TriangleMesh` 生成のみ可）
./features/12-cylinder/design-review-2.md:43:- Euler-Poincaré 検証テストがあるか
./features/12-cylinder/design-review-2.md:61:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
./features/12-cylinder/design-review-2.md:62:    suggestion: "IdGenerator::next() に置き換えること"
./features/12-cylinder/design-review-2.md:79:ROADMAP Phase 3「円柱・球・押し出しが作れる」の最初の一歩。現状カーネルは `make_cuboid` と平面専用の `tessellate_solid` しか持たない。本 Issue で **解析曲面(円柱)を厳密 B-rep として生成し、それを三角形メッシュへ離散化する土台**を確立し、`mycad export examples/cylinder.mycad -o out.stl` を動作させる。
./features/12-cylinder/design-review-2.md:90:  - 新規 `crates/mycad-kernel/src/primitives/cylinder.rs` — `make_cylinder`
./features/12-cylinder/design-review-2.md:102:### 1. トポロジー（`make_cylinder(radius, height, id_gen) -> Solid`）
./features/12-cylinder/design-review-2.md:123:**Euler–Poincaré**: V−E+F = 2−3+3 = 2 = 2(S−H), S=1,H=0 → 成立。
./features/12-cylinder/design-review-2.md:125:**決定性**: ID 採番順を `make_cuboid` と同型に固定。solid → 頂点(v_bot,v_top) → エッジ(e_bot,e_top,e_seam) → 面ごとに[底,上,側]順で HE群→loop→face → shell。すべて `id_gen.next()` を固定順で呼ぶのみ。`HashMap`/乱数等の非決定要素なし。
./features/12-cylinder/design-review-2.md:127:**退化入力の検証（R02）**: `.mycad` 由来のユーザー入力は境界で弾く。`KernelError` に `InvalidParameter { kind: &'static str }` を追加し、`build_solid_from_features` の `CreateCylinder` 分岐で `radius > 0.0 && height > 0.0` を検証（非正なら `InvalidParameter` を返し `make_cylinder` を呼ばない）。`make_cylinder` 自体は `make_cuboid` と同じ `-> Solid` シグネチャを保ち、正値前提（検証は呼び出し側＝境界に集約）。解像度の退化は `TessellationOptions` のクランプ（前述）で吸収。
./features/12-cylinder/design-review-2.md:138:  pub fn tessellate_solid(solid) -> TriangleMesh           // = _with(solid, &default())  ※既存呼び出し・既存テスト不変
./features/12-cylinder/design-review-2.md:139:  pub fn tessellate_solid_with(solid, &TessellationOptions) -> TriangleMesh  // 内部でも angular.max(3)/axial.max(1) で防御
./features/12-cylinder/design-review-2.md:147:  1. 外側 loop の各 HE について、`forward` なら `(t0,t1)`、`!forward` なら `(t1,t0)` を `sample_segment` に渡して折れ線化 → 連結して境界点列。**先頭点が必ず `HalfEdge.start_vertex` に一致**するため、逆向き Line を含む cuboid でも従来と同一の角点列＝**平面出力不変**。
./features/12-cylinder/design-review-2.md:155:- `mycad-build/src/lib.rs`: `Feature::CreateCylinder{radius,height}` 分岐で `radius>0 && height>0` を検証し、OK なら `Ok(make_cylinder(*radius, *height, gen))`、NG なら `Err(KernelError::InvalidParameter{..})`（`UnsupportedFeature` スタブ削除）。`Feature` enum・`id()` は既存のまま変更不要。
./features/12-cylinder/design-review-2.md:156:- `mycad-cli`: `export` に `--segments <N>`（任意、未指定なら `TessellationOptions::default`）。指定時は `TessellationOptions::new(N, 1)` でクランプ。export パスは `tessellate_solid_with` を呼ぶ。
./features/12-cylinder/design-review-2.md:171:- `TessellationOptions` に `Debug, Clone, Serialize, Deserialize`（既存 `TriangleMesh` と同様）。`Copy` も可。
./features/12-cylinder/design-review-2.md:173:- `KernelError`（既存 `thiserror`）に `InvalidParameter { kind: &'static str }` を 1 variant 追加（R02 の境界検証用）。
./features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
./features/12-cylinder/design-review-2.md:187:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
./features/12-cylinder/design-review-2.md:189:| T03 | Euler | V−E+F == 2 | `assert_eq!(2-3+3, 2)` 相当の検証 |
./features/12-cylinder/design-review-2.md:194:| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index。cuboid は逆向き Line HE を含むため R01 の向き対応も同時に担保 | 既存テスト維持 + positions が従来値と一致 |
./features/12-cylinder/design-review-2.md:196:| T10 | ディスパッチ統合 | `examples/cylinder.mycad` を `build_solid_from_features` でビルドし T02 と同じ counts | `feature_dispatcher.rs` |
./features/12-cylinder/design-review-2.md:198:| T12 | 解像度外部化+クランプ | `tessellate_solid_with` で分割数が変わり三角形数が増減。`TessellationOptions::new(2,0)` が `angular=3/axial=1` にクランプされる | 2 解像度で差分 + クランプ検証 (R02) |
./features/12-cylinder/design-review-2.md:200:| T14 | 退化入力エラー | `radius<=0` / `height<=0` の `create_cylinder` を `build_solid_from_features` に渡すと `KernelError::InvalidParameter` | `assert!(matches!(..))` (R02) |
./features/12-cylinder/design-review-2.md:211:- `Sphere`/`Cone`/`Extrude`、任意トリム面の 2D メッシュライブラリ導入、pcurve、ビューアの解析/GPU 適応レンダリング、`HalfEdge` への隣接(twin/next/prev)構造導入（R03、Boolean/フィレットが要求した時点で別途）。
./features/12-cylinder/design-review-2.md:214:設計レビューとして、まず既存カーネルのトポロジー型・`make_cuboid`・テッセレーション実装・エラー/依存規約を確認します。文書内の前提が既存設計と噛み合っているかを見てから、重大度付きで指摘を整理します。
./features/12-cylinder/design-review-2.md:244:既存実装の骨格が見えました。次に `make_cuboid`、B-rep 型、テッセレータ、`build`/CLI の現状を読み、今回の設計がどこで破綻しうるかを詰めます。
./features/12-cylinder/design-review-2.md:248:use crate::brep::topology::{IdGenerator, Solid};
./features/12-cylinder/design-review-2.md:263:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
./features/12-cylinder/design-review-2.md:470:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review-2.md:474:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:475:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
./features/12-cylinder/design-review-2.md:494:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:495:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
./features/12-cylinder/design-review-2.md:514:pub use cuboid::make_cuboid;
./features/12-cylinder/design-review-2.md:568:pub struct TriangleMesh {
./features/12-cylinder/design-review-2.md:577:impl TriangleMesh {
./features/12-cylinder/design-review-2.md:592:impl Default for TriangleMesh {
./features/12-cylinder/design-review-2.md:607:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/design-review-2.md:608:    let mut mesh = TriangleMesh::new();
./features/12-cylinder/design-review-2.md:656:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review-2.md:657:    use crate::primitives::make_cuboid;
./features/12-cylinder/design-review-2.md:661:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:662:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
./features/12-cylinder/design-review-2.md:675:        let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:676:        let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:678:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
./features/12-cylinder/design-review-2.md:679:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
./features/12-cylinder/design-review-2.md:939:`make_cuboid` は `HalfEdge.start_vertex` の順序だけでループを表現しており、現状テッセレータもそこを前提にしています。次は B-rep 型定義と `build` 側の feature/history の扱いを見ます。
./features/12-cylinder/design-review-2.md:944:use mycad_kernel::brep::topology::{IdGenerator, Solid};
./features/12-cylinder/design-review-2.md:946:use mycad_kernel::primitives::make_cuboid;
./features/12-cylinder/design-review-2.md:948:pub fn build_solid_from_features(
./features/12-cylinder/design-review-2.md:950:    gen: &mut IdGenerator,
./features/12-cylinder/design-review-2.md:968:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
./features/12-cylinder/design-review-2.md:986:use mycad_build::build_solid_from_features;
./features/12-cylinder/design-review-2.md:988:use mycad_kernel::brep::topology::IdGenerator;
./features/12-cylinder/design-review-2.md:1016:            if let Err(msg) = run_export(&input, &output) {
./features/12-cylinder/design-review-2.md:1024:fn run_export(input: &std::path::Path, output: &std::path::Path) -> Result<(), String> {
./features/12-cylinder/design-review-2.md:1025:    let doc = Document::from_path(input)
./features/12-cylinder/design-review-2.md:1027:    let mut gen = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1028:    let solid = build_solid_from_features(&doc.root_component.features, &mut gen)
./features/12-cylinder/design-review-2.md:1038:/bin/bash -lc "rg -n \"struct Solid|struct HalfEdge|struct Loop|struct Face|struct Shell|struct Edge|struct Vertex|IdGenerator|same_sense|inner_loops\" crates/mycad-kernel/src -g '"'!target'"'" in /home/bacon/mycad
./features/12-cylinder/design-review-2.md:1041:crates/mycad-kernel/src/tessellation/mod.rs:98:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review-2.md:1042:crates/mycad-kernel/src/tessellation/mod.rs:103:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1043:crates/mycad-kernel/src/tessellation/mod.rs:117:        let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1044:crates/mycad-kernel/src/tessellation/mod.rs:118:        let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1045:crates/mycad-kernel/src/tessellation/stl.rs:51:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review-2.md:1046:crates/mycad-kernel/src/tessellation/stl.rs:57:        let mut g = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1047:crates/mycad-kernel/src/tessellation/stl.rs:74:            let mut g = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1049:crates/mycad-kernel/src/brep/topology.rs:21:pub struct HalfEdge {
./features/12-cylinder/design-review-2.md:1051:crates/mycad-kernel/src/brep/topology.rs:46:pub struct Loop {
./features/12-cylinder/design-review-2.md:1055:crates/mycad-kernel/src/brep/topology.rs:69:pub struct Shell {
./features/12-cylinder/design-review-2.md:1061:crates/mycad-kernel/src/brep/topology.rs:185:pub struct IdGenerator {
./features/12-cylinder/design-review-2.md:1062:crates/mycad-kernel/src/brep/topology.rs:189:impl IdGenerator {
./features/12-cylinder/design-review-2.md:1063:crates/mycad-kernel/src/brep/topology.rs:208:        let mut gen1 = IdGenerator::new(100);
./features/12-cylinder/design-review-2.md:1064:crates/mycad-kernel/src/brep/topology.rs:209:        let mut gen2 = IdGenerator::new(100);
./features/12-cylinder/design-review-2.md:1065:crates/mycad-kernel/src/primitives/cuboid.rs:1:use crate::brep::topology::{IdGenerator, Solid};
./features/12-cylinder/design-review-2.md:1066:crates/mycad-kernel/src/primitives/cuboid.rs:16:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
./features/12-cylinder/design-review-2.md:1076:crates/mycad-kernel/src/primitives/cuboid.rs:223:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review-2.md:1077:crates/mycad-kernel/src/primitives/cuboid.rs:227:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1078:crates/mycad-kernel/src/primitives/cuboid.rs:247:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1079:crates/mycad-kernel/src/primitives/cuboid.rs:276:        let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1080:crates/mycad-kernel/src/primitives/cuboid.rs:277:        let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1102:[workspace.dependencies]
./features/12-cylinder/design-review-2.md:1114:thiserror = "2"
./features/12-cylinder/design-review-2.md:1147:pub struct HalfEdge {
./features/12-cylinder/design-review-2.md:1172:pub struct Loop {
./features/12-cylinder/design-review-2.md:1195:pub struct Shell {
./features/12-cylinder/design-review-2.md:1209:    pub half_edges: Vec<HalfEdge>,
./features/12-cylinder/design-review-2.md:1211:    pub loops: Vec<Loop>,
./features/12-cylinder/design-review-2.md:1213:    pub shells: Vec<Shell>,
./features/12-cylinder/design-review-2.md:1264:        self.half_edges.push(HalfEdge {
./features/12-cylinder/design-review-2.md:1276:        self.loops.push(Loop { id, half_edges });
./features/12-cylinder/design-review-2.md:1303:        self.shells.push(Shell { id, faces, closed });
./features/12-cylinder/design-review-2.md:1311:pub struct IdGenerator {
./features/12-cylinder/design-review-2.md:1315:impl IdGenerator {
./features/12-cylinder/design-review-2.md:1334:        let mut gen1 = IdGenerator::new(100);
./features/12-cylinder/design-review-2.md:1335:        let mut gen2 = IdGenerator::new(100);
./features/12-cylinder/design-review-2.md:1354:use thiserror::Error;
./features/12-cylinder/design-review-2.md:1381:thiserror = { workspace = true }
./features/12-cylinder/design-review-2.md:1405:thiserror = { workspace = true }
./features/12-cylinder/design-review-2.md:1467:crates/mycad-format/src/document.rs:96:        let doc = Document::from_path(&path).unwrap();
./features/12-cylinder/design-review-2.md:1469:crates/mycad-format/src/document.rs:104:        let err = Document::from_path(path).unwrap_err();
./features/12-cylinder/design-review-2.md:1471:crates/mycad-format/src/document.rs:111:        let err = Document::from_path(path).unwrap_err();
./features/12-cylinder/design-review-2.md:1472:crates/mycad-format/src/document.rs:124:        let doc = Document::from_path(&path).unwrap();
./features/12-cylinder/design-review-2.md:1477:crates/mycad-build/tests/feature_dispatcher.rs:14:    let doc = Document::from_path(&path).expect("load .mycad");
./features/12-cylinder/design-review-2.md:1655:            return Err(FormatError::InvalidExtension(ext.map(str::to_string)));
./features/12-cylinder/design-review-2.md:1712:        let doc = Document::from_path(&path).unwrap();
./features/12-cylinder/design-review-2.md:1720:        let err = Document::from_path(path).unwrap_err();
./features/12-cylinder/design-review-2.md:1727:        let err = Document::from_path(path).unwrap_err();
./features/12-cylinder/design-review-2.md:1728:        assert!(matches!(err, FormatError::InvalidExtension(Some(ref e)) if e == "txt"));
./features/12-cylinder/design-review-2.md:1732:    fn test_assembly_roundtrip() {
./features/12-cylinder/design-review-2.md:1740:        let doc = Document::from_path(&path).unwrap();
./features/12-cylinder/design-review-2.md:1763:use mycad_build::build_solid_from_features;
./features/12-cylinder/design-review-2.md:1765:use mycad_kernel::brep::topology::IdGenerator;
./features/12-cylinder/design-review-2.md:1776:    let doc = Document::from_path(&path).expect("load .mycad");
./features/12-cylinder/design-review-2.md:1777:    let mut g = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1779:        build_solid_from_features(&doc.root_component.features, &mut g).expect("build solid");
./features/12-cylinder/design-review-2.md:1789:/bin/bash -lc 'rg -n "golden|roundtrip|tessellate_solid_with|TessellationOptions|export" crates/mycad-cli tests crates/mycad-kernel crates/mycad-build' in /home/bacon/mycad
./features/12-cylinder/design-review-2.md:1796:crates/mycad-cli/src/main.rs:32:            if let Err(msg) = run_export(&input, &output) {
./features/12-cylinder/design-review-2.md:1797:crates/mycad-cli/src/main.rs:40:fn run_export(input: &std::path::Path, output: &std::path::Path) -> Result<(), String> {
./features/12-cylinder/design-review-2.md:1863:./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
./features/12-cylinder/design-review-2.md:1875:./features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
./features/12-cylinder/design-review-2.md:1883:./features/12-cylinder/design-review.md:523:crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
./features/12-cylinder/design-review-2.md:1885:./features/12-cylinder/design-review.md:525:crates/mycad-kernel/src/tessellation/mod.rs:10:pub struct TriangleMesh {
./features/12-cylinder/design-review-2.md:1886:./features/12-cylinder/design-review.md:526:crates/mycad-kernel/src/tessellation/mod.rs:19:impl TriangleMesh {
./features/12-cylinder/design-review-2.md:1887:./features/12-cylinder/design-review.md:527:crates/mycad-kernel/src/tessellation/mod.rs:34:impl Default for TriangleMesh {
./features/12-cylinder/design-review-2.md:1888:./features/12-cylinder/design-review.md:528:crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/design-review-2.md:1889:./features/12-cylinder/design-review.md:529:crates/mycad-kernel/src/tessellation/mod.rs:50:    let mut mesh = TriangleMesh::new();
./features/12-cylinder/design-review-2.md:1890:./features/12-cylinder/design-review.md:530:crates/mycad-kernel/src/tessellation/mod.rs:98:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review-2.md:1891:./features/12-cylinder/design-review.md:531:crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
./features/12-cylinder/design-review-2.md:1892:./features/12-cylinder/design-review.md:532:crates/mycad-kernel/src/tessellation/mod.rs:103:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1893:./features/12-cylinder/design-review.md:533:crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
./features/12-cylinder/design-review-2.md:1895:./features/12-cylinder/design-review.md:535:crates/mycad-kernel/src/tessellation/mod.rs:117:        let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1896:./features/12-cylinder/design-review.md:536:crates/mycad-kernel/src/tessellation/mod.rs:118:        let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1897:./features/12-cylinder/design-review.md:537:crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
./features/12-cylinder/design-review-2.md:1898:./features/12-cylinder/design-review.md:538:crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
./features/12-cylinder/design-review-2.md:1901:./features/12-cylinder/design-review.md:566:crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
./features/12-cylinder/design-review-2.md:1924:./crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
./features/12-cylinder/design-review-2.md:1928:./crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
./features/12-cylinder/design-review-2.md:1936:./features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
./features/12-cylinder/design-review-2.md:1941:./features/12-cylinder/design-review-2.md:514:pub use cuboid::make_cuboid;
./features/12-cylinder/design-review-2.md:1945:./features/12-cylinder/design-review-2.md:1041:crates/mycad-kernel/src/tessellation/mod.rs:98:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review-2.md:1946:./features/12-cylinder/design-review-2.md:1042:crates/mycad-kernel/src/tessellation/mod.rs:103:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1947:./features/12-cylinder/design-review-2.md:1043:crates/mycad-kernel/src/tessellation/mod.rs:117:        let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:1948:./features/12-cylinder/design-review-2.md:1044:crates/mycad-kernel/src/tessellation/mod.rs:118:        let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:2016:レビュー項目はほぼ固まりました。既存コードとの照合上、重大なのは「汎用曲面テッセレーションの抽象が cylinder 専用の仮定を漏らしている点」と「Euler テストが実体を検証していない点」です。最終的に YAML だけ返すので、最後に指摘を絞ります。
./features/12-cylinder/design-review-2.md:2048:- `Solid` が最上位エンティティ、内部に Shell → Face → Loop → HalfEdge → Vertex の階層
./features/12-cylinder/design-review-2.md:2116:/bin/bash -lc "rg -n \"決定性|determin|same input|IdGenerator|HashMap|thread_rng|cross-platform|float\" CLAUDE.md crates/mycad-kernel/CLAUDE.md docs -g '*.md'" in /home/bacon/mycad
./features/12-cylinder/design-review-2.md:2118:crates/mycad-kernel/CLAUDE.md:18:- EntityIDは `IdGenerator` で決定的に生成される
./features/12-cylinder/design-review-2.md:2119:crates/mycad-kernel/CLAUDE.md:23:2. 関数シグネチャ: `pub fn make_xxx(params..., id_gen: &mut IdGenerator) -> Solid`
./features/12-cylinder/design-review-2.md:2120:crates/mycad-kernel/CLAUDE.md:25:4. 決定性テストを必ず書く（同じ `IdGenerator` 初期値で2回生成し、結果が同一であることを確認）
./features/12-cylinder/design-review-2.md:2122:CLAUDE.md:53:- **決定性**: すべてのEntityIDは `IdGenerator` で決定的に生成。テストでは決定性を検証すること。
./features/12-cylinder/design-review-2.md:2141:- すべてのEdgeは正確に2つのHalfEdgeを持つ
./features/12-cylinder/design-review-2.md:2142:- 閉じたShellはwatertight（水密）
./features/12-cylinder/design-review-2.md:2143:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
./features/12-cylinder/design-review-2.md:2144:- Loopは閉じたHalfEdge列（最後のHalfEdgeの終点 = 最初のHalfEdgeの始点）
./features/12-cylinder/design-review-2.md:2145:- EntityIDは `IdGenerator` で決定的に生成される
./features/12-cylinder/design-review-2.md:2150:2. 関数シグネチャ: `pub fn make_xxx(params..., id_gen: &mut IdGenerator) -> Solid`
./features/12-cylinder/design-review-2.md:2152:4. 決定性テストを必ず書く（同じ `IdGenerator` 初期値で2回生成し、結果が同一であることを確認）
./features/12-cylinder/design-review-2.md:2172:use super::TriangleMesh;
./features/12-cylinder/design-review-2.md:2175:pub fn to_ascii_stl(mesh: &TriangleMesh, solid_name: &str) -> String {
./features/12-cylinder/design-review-2.md:2222:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review-2.md:2223:    use crate::primitives::make_cuboid;
./features/12-cylinder/design-review-2.md:2228:        let mut g = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:2229:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
./features/12-cylinder/design-review-2.md:2245:            let mut g = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:2246:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
./features/12-cylinder/design-review-2.md:2272:        let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:2273:        let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/design-review-2.md:2275:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
./features/12-cylinder/design-review-2.md:2276:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);
./features/12-cylinder/design-review-2.md:2307:    finding: "「cuboid は既に外向きなのでスワップは発生しない／平面出力不変」という前提が既存 `make_cuboid` と整合していない。現状の cuboid ループ順では front face `0→1→3→2` などの幾何法線が outward normal と逆向きで、提案どおりの法線整合スワップを入れると既存 STL/indices は変わる。"
./features/12-cylinder/design-review-2.md:2312:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
./features/12-cylinder/design-review-2.md:2316:    section: "設計方針 > 1. トポロジー（`make_cylinder`） > 退化入力の検証（R02）"
./features/12-cylinder/design-review-2.md:2317:    finding: "退化入力の排除が `build_solid_from_features` 境界にしかなく、公開 API `make_cylinder(radius, height, id_gen) -> Solid` 自体は `radius<=0` / `height<=0` を受けても不正 B-rep を返せてしまう。`mycad-build` を経由しない kernel 利用では不変条件を守れない。"
./features/12-cylinder/design-review-2.md:2332:    finding: "「cuboid は既に外向きなのでスワップは発生しない／平面出力不変」という前提が既存 `make_cuboid` と整合していない。現状の cuboid ループ順では front face `0→1→3→2` などの幾何法線が outward normal と逆向きで、提案どおりの法線整合スワップを入れると既存 STL/indices は変わる。"
./features/12-cylinder/design-review-2.md:2337:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
./features/12-cylinder/design-review-2.md:2341:    section: "設計方針 > 1. トポロジー（`make_cylinder`） > 退化入力の検証（R02）"
./features/12-cylinder/design-review-2.md:2342:    finding: "退化入力の排除が `build_solid_from_features` 境界にしかなく、公開 API `make_cylinder(radius, height, id_gen) -> Solid` 自体は `radius<=0` / `height<=0` を受けても不正 B-rep を返せてしまう。`mycad-build` を経由しない kernel 利用では不変条件を守れない。"
./crates/mycad-format/src/document.rs:39:            return Err(FormatError::InvalidExtension(ext.map(str::to_string)));
./crates/mycad-format/src/document.rs:96:        let doc = Document::from_path(&path).unwrap();
./crates/mycad-format/src/document.rs:104:        let err = Document::from_path(path).unwrap_err();
./crates/mycad-format/src/document.rs:111:        let err = Document::from_path(path).unwrap_err();
./crates/mycad-format/src/document.rs:112:        assert!(matches!(err, FormatError::InvalidExtension(Some(ref e)) if e == "txt"));
./crates/mycad-format/src/document.rs:116:    fn test_assembly_roundtrip() {
./crates/mycad-format/src/document.rs:124:        let doc = Document::from_path(&path).unwrap();
./crates/mycad-format/CLAUDE.md:33:| `CreateBox`    | `make_cuboid`  |
./features/12-cylinder/final-review-2.md:56: use thiserror::Error;
./features/12-cylinder/final-review-2.md:64: pub struct TriangleMesh {
./features/12-cylinder/final-review-2.md:73: impl TriangleMesh {
./features/12-cylinder/final-review-2.md:88: impl Default for TriangleMesh {
./features/12-cylinder/final-review-2.md:127:     NonManifoldLoop,
./features/12-cylinder/final-review-2.md:131: pub fn tessellate_solid(solid: &Solid) -> Result<TriangleMesh, TessellationError> {
./features/12-cylinder/final-review-2.md:132:     tessellate_solid_with(solid, &TessellationOptions::default())
./features/12-cylinder/final-review-2.md:136: pub fn tessellate_solid_with(
./features/12-cylinder/final-review-2.md:139: ) -> Result<TriangleMesh, TessellationError> {
./features/12-cylinder/final-review-2.md:140:     let mut mesh = TriangleMesh::new();
./features/12-cylinder/final-review-2.md:167:     mesh: &mut TriangleMesh,
./features/12-cylinder/final-review-2.md:203:     lp: &crate::brep::topology::Loop,
./features/12-cylinder/final-review-2.md:232:     mesh: &mut TriangleMesh,
./features/12-cylinder/final-review-2.md:317: fn push_triangle(mesh: &mut TriangleMesh, i0: u32, i1: u32, i2: u32) {
./features/12-cylinder/final-review-2.md:353:     use crate::brep::topology::IdGenerator;
./features/12-cylinder/final-review-2.md:357:         let mesh_low = tessellate_solid_with(&solid, &TessellationOptions::new(8, 1)).unwrap();
./features/12-cylinder/final-review-2.md:358:         let mesh_hi = tessellate_solid_with(&solid, &TessellationOptions::new(64, 2)).unwrap();
./features/12-cylinder/final-review-2.md:411:         let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/final-review-2.md:412:         let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
./features/12-cylinder/final-review-2.md:423:         let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/final-review-2.md:424:         let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/final-review-2.md:426:         let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
./features/12-cylinder/final-review-2.md:427:         let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
./features/12-cylinder/final-review-2.md:437:+    /// (public fields bypass new() clamping, so tessellate_solid_with must guard at point of use).
./features/12-cylinder/final-review-2.md:440:+        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review-2.md:441:+        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review-2.md:446:+        let mesh = tessellate_solid_with(&solid, &opts).unwrap();
./features/12-cylinder/final-review-2.md:769:use crate::brep::topology::{IdGenerator, Solid};
./features/12-cylinder/final-review-2.md:779:pub fn make_cylinder(
./features/12-cylinder/final-review-2.md:782:    id_gen: &mut IdGenerator,
./features/12-cylinder/final-review-2.md:904:        let mut gen1 = IdGenerator::new(0);
./features/12-cylinder/final-review-2.md:905:        let mut gen2 = IdGenerator::new(0);
./features/12-cylinder/final-review-2.md:906:        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
./features/12-cylinder/final-review-2.md:907:        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
./features/12-cylinder/final-review-2.md:932:        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review-2.md:933:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review-2.md:948:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
./features/12-cylinder/final-review-2.md:951:        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review-2.md:952:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review-2.md:963:            "Euler-Poincaré V-E+F = 2(S-H), S={shells}, H=0"
./features/12-cylinder/final-review-2.md:971:        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review-2.md:972:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review-2.md:1060:   156	    lp: &crate::brep::topology::Loop,
./features/12-cylinder/final-review-2.md:1089:   185	    mesh: &mut TriangleMesh,
./features/12-cylinder/final-review-2.md:1174:pub struct HalfEdge {
./features/12-cylinder/final-review-2.md:1199:pub struct Loop {
./features/12-cylinder/final-review-2.md:1222:pub struct Shell {
./features/12-cylinder/final-review-2.md:1236:    pub half_edges: Vec<HalfEdge>,
./features/12-cylinder/final-review-2.md:1238:    pub loops: Vec<Loop>,
./features/12-cylinder/final-review-2.md:1240:    pub shells: Vec<Shell>,
./features/12-cylinder/final-review-2.md:1291:        self.half_edges.push(HalfEdge {
./features/12-cylinder/final-review-2.md:1303:        self.loops.push(Loop { id, half_edges });
./features/12-cylinder/final-review-2.md:1330:        self.shells.push(Shell { id, faces, closed });
./features/12-cylinder/final-review-2.md:1338:pub struct IdGenerator {
./features/12-cylinder/final-review-2.md:1342:impl IdGenerator {
./features/12-cylinder/final-review-2.md:1361:        let mut gen1 = IdGenerator::new(100);
./features/12-cylinder/final-review-2.md:1362:        let mut gen2 = IdGenerator::new(100);
./features/12-cylinder/final-review-2.md:1379:/bin/bash -lc "rg -n \"triangle_count\\(|make_cylinder|tessellate_solid_with\\(&solid, &TessellationOptions|zero_segments|partial_revolution\" crates/mycad-kernel/src/tessellation/mod.rs crates/mycad-kernel/src/primitives -g'*.rs'" in /home/bacon/mycad
./features/12-cylinder/final-review-2.md:1382:crates/mycad-kernel/src/tessellation/mod.rs:306:    use crate::primitives::make_cylinder;
./features/12-cylinder/final-review-2.md:1384:crates/mycad-kernel/src/tessellation/mod.rs:312:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review-2.md:1387:crates/mycad-kernel/src/tessellation/mod.rs:333:        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
./features/12-cylinder/final-review-2.md:1388:crates/mycad-kernel/src/tessellation/mod.rs:334:        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
./features/12-cylinder/final-review-2.md:1390:crates/mycad-kernel/src/tessellation/mod.rs:378:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review-2.md:1392:crates/mycad-kernel/src/tessellation/mod.rs:432:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review-2.md:1393:crates/mycad-kernel/src/tessellation/mod.rs:435:        let mesh_low = tessellate_solid_with(&solid, &TessellationOptions::new(8, 1)).unwrap();
./features/12-cylinder/final-review-2.md:1394:crates/mycad-kernel/src/tessellation/mod.rs:436:        let mesh_hi = tessellate_solid_with(&solid, &TessellationOptions::new(64, 2)).unwrap();
./features/12-cylinder/final-review-2.md:1401:crates/mycad-kernel/src/tessellation/mod.rs:519:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review-2.md:1404:crates/mycad-kernel/src/primitives/mod.rs:5:pub use cylinder::make_cylinder;
./features/12-cylinder/final-review-2.md:1405:crates/mycad-kernel/src/primitives/cylinder.rs:11:pub fn make_cylinder(
./features/12-cylinder/final-review-2.md:1406:crates/mycad-kernel/src/primitives/cylinder.rs:138:        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
./features/12-cylinder/final-review-2.md:1407:crates/mycad-kernel/src/primitives/cylinder.rs:139:        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
./features/12-cylinder/final-review-2.md:1408:crates/mycad-kernel/src/primitives/cylinder.rs:165:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review-2.md:1409:crates/mycad-kernel/src/primitives/cylinder.rs:184:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review-2.md:1410:crates/mycad-kernel/src/primitives/cylinder.rs:204:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review-2.md:1411:crates/mycad-kernel/src/primitives/cylinder.rs:258:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review-2.md:1412:crates/mycad-kernel/src/primitives/cylinder.rs:305:        let err = make_cylinder(0.0, 20.0, &mut gen).unwrap_err();
./features/12-cylinder/final-review-2.md:1413:crates/mycad-kernel/src/primitives/cylinder.rs:312:        let err = make_cylinder(-5.0, 20.0, &mut gen).unwrap_err();
./features/12-cylinder/final-review-2.md:1414:crates/mycad-kernel/src/primitives/cylinder.rs:319:        let err = make_cylinder(f64::NAN, 20.0, &mut gen).unwrap_err();
./features/12-cylinder/final-review-2.md:1415:crates/mycad-kernel/src/primitives/cylinder.rs:326:        let err = make_cylinder(f64::INFINITY, 20.0, &mut gen).unwrap_err();
./features/12-cylinder/final-review-2.md:1416:crates/mycad-kernel/src/primitives/cylinder.rs:333:        let err = make_cylinder(5.0, 0.0, &mut gen).unwrap_err();
./features/12-cylinder/final-review-2.md:1417:crates/mycad-kernel/src/primitives/cylinder.rs:340:        let err = make_cylinder(5.0, -10.0, &mut gen).unwrap_err();
./features/12-cylinder/final-review-2.md:1418:crates/mycad-kernel/src/primitives/cylinder.rs:347:        let err = make_cylinder(5.0, f64::NAN, &mut gen).unwrap_err();
./features/12-cylinder/final-review-2.md:1419:crates/mycad-kernel/src/primitives/cylinder.rs:354:        let err = make_cylinder(5.0, f64::INFINITY, &mut gen).unwrap_err();
./.git/logs/refs/heads/claude/add-claude-guidelines-BKKtD:7:d5c98d61ad288b808f65a3953abf6575aa3ceb87 eb557acee53a52ebe94aafffd472aca1d730b58a GS-Bacon <bacon0817.ix@gmail.com> 1779521125 +0000	commit: feat(kernel): add ASCII STL writer for TriangleMesh
./features/12-cylinder/design-review-3.md:22:- `IdGenerator` を使った決定的 ID 生成になっているか
./features/12-cylinder/design-review-3.md:27:- Euler-Poincaré の公式 `V - E + F = 2(S - H)` が成立する設計か（S=殻の数, H=貫通穴）
./features/12-cylinder/design-review-3.md:28:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
./features/12-cylinder/design-review-3.md:29:- Loop/Shell/Solid の入れ子構造が一貫しているか
./features/12-cylinder/design-review-3.md:32:- ゼロ長エッジ、面積ゼロの Face、縮退した Loop の排除または明示的エラーの設計があるか
./features/12-cylinder/design-review-3.md:37:- 新しい依存は `[workspace.dependencies]` + `{ workspace = true }` 規約に従っているか
./features/12-cylinder/design-review-3.md:38:- ライブラリエラーには `thiserror` を使用しているか
./features/12-cylinder/design-review-3.md:39:- カーネルにレンダリング依存が混入していないか（`TriangleMesh` 生成のみ可）
./features/12-cylinder/design-review-3.md:43:- Euler-Poincaré 検証テストがあるか
./features/12-cylinder/design-review-3.md:61:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
./features/12-cylinder/design-review-3.md:62:    suggestion: "IdGenerator::next() に置き換えること"
./features/12-cylinder/design-review-3.md:79:ROADMAP Phase 3「円柱・球・押し出しが作れる」の最初の一歩。現状カーネルは `make_cuboid` と平面専用の `tessellate_solid` しか持たない。本 Issue で **解析曲面(円柱)を厳密 B-rep として生成し、それを三角形メッシュへ離散化する土台**を確立し、`mycad export examples/cylinder.mycad -o out.stl` を動作させる。
./features/12-cylinder/design-review-3.md:90:  - 新規 `crates/mycad-kernel/src/primitives/cylinder.rs` — `make_cylinder`
./features/12-cylinder/design-review-3.md:102:### 1. トポロジー（`make_cylinder(radius, height, id_gen) -> Solid`）
./features/12-cylinder/design-review-3.md:123:**Euler–Poincaré**: V−E+F = 2−3+3 = 2 = 2(S−H), S=1,H=0 → 成立。
./features/12-cylinder/design-review-3.md:125:**決定性**: ID 採番順を `make_cuboid` と同型に固定。solid → 頂点(v_bot,v_top) → エッジ(e_bot,e_top,e_seam) → 面ごとに[底,上,側]順で HE群→loop→face → shell。すべて `id_gen.next()` を固定順で呼ぶのみ。`HashMap`/乱数等の非決定要素なし。
./features/12-cylinder/design-review-3.md:127:**退化入力の検証（R02）**: `.mycad` 由来のユーザー入力は境界で弾く。`KernelError` に `InvalidParameter { kind: &'static str }` を追加し、`build_solid_from_features` の `CreateCylinder` 分岐で `radius > 0.0 && height > 0.0` を検証（非正なら `InvalidParameter` を返し `make_cylinder` を呼ばない）。`make_cylinder` 自体は `make_cuboid` と同じ `-> Solid` シグネチャを保ち、正値前提（検証は呼び出し側＝境界に集約）。前提条件はドキュメントコメントに明記し、`debug_assert!(radius > 0.0 && height > 0.0)` を先頭に置いて開発時の契約違反を検出（リリースでは境界検証済み前提で素通り）。解像度の退化は `TessellationOptions` のクランプ（前述）で吸収。
./features/12-cylinder/design-review-3.md:138:  pub fn tessellate_solid(solid) -> TriangleMesh           // = _with(solid, &default())  ※既存呼び出し・既存テスト不変
./features/12-cylinder/design-review-3.md:139:  pub fn tessellate_solid_with(solid, &TessellationOptions) -> TriangleMesh  // 内部でも angular.max(3)/axial.max(1) で防御
./features/12-cylinder/design-review-3.md:148:  1. 外側 loop の各 HE について、`forward` なら `(t0,t1)`、`!forward` なら `(t1,t0)` を `sample_segment` に渡して折れ線化 → 連結して境界点列。**先頭点が必ず `HalfEdge.start_vertex` に一致**するため、逆向き Line を含む cuboid でも従来と同一の角点列＝**平面出力不変**。
./features/12-cylinder/design-review-3.md:155:- `mycad-build/src/lib.rs`: `Feature::CreateCylinder{radius,height}` 分岐で `radius>0 && height>0` を検証し、OK なら `Ok(make_cylinder(*radius, *height, gen))`、NG なら `Err(KernelError::InvalidParameter{..})`（`UnsupportedFeature` スタブ削除）。`Feature` enum・`id()` は既存のまま変更不要。
./features/12-cylinder/design-review-3.md:156:- `mycad-cli`: `export` に `--segments <N>`（任意、未指定なら `TessellationOptions::default`）。指定時は `TessellationOptions::new(N, 1)` でクランプ。export パスは `tessellate_solid_with` を呼ぶ。
./features/12-cylinder/design-review-3.md:171:- `TessellationOptions` に `Debug, Clone, Serialize, Deserialize`（既存 `TriangleMesh` と同様）。`Copy` も可。
./features/12-cylinder/design-review-3.md:173:- `KernelError`（既存 `thiserror`）に `InvalidParameter { kind: &'static str }` を 1 variant 追加（R02 の境界検証用）。
./features/12-cylinder/design-review-3.md:177:### Loop 1
./features/12-cylinder/design-review-3.md:180:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
./features/12-cylinder/design-review-3.md:184:### Loop 2
./features/12-cylinder/design-review-3.md:187:- R04(再掲) 反映: `make_cylinder` に `debug_assert!(radius>0 && height>0)` と前提コメントを追加（境界検証は呼び出し側に集約のまま）。
./features/12-cylinder/design-review-3.md:188:- T03 を「実カウントから Euler を計算」へ修正（定数ハードコード撤廃）。
./features/12-cylinder/design-review-3.md:194:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
./features/12-cylinder/design-review-3.md:196:| T03 | Euler | 実 solid の `vertices.len() − edges.len() + faces.len() == 2 * (shells.len() − genus)`（genus=0）を**実カウントから**計算して検証（定数ハードコードでなく構造体から） | `assert_eq!` |
./features/12-cylinder/design-review-3.md:201:| T08 | 回帰(平面バイト不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index。**positions も indices も従来値と完全一致**（スワップ撤去によりバイト不変、R02）。cuboid は逆向き Line HE を含むため R01 の向き対応も同時に担保 | 既存テスト維持 + positions/indices 一致 |
./features/12-cylinder/design-review-3.md:203:| T10 | ディスパッチ統合 | `examples/cylinder.mycad` を `build_solid_from_features` でビルドし T02 と同じ counts | `feature_dispatcher.rs` |
./features/12-cylinder/design-review-3.md:205:| T12 | 解像度外部化+クランプ | `tessellate_solid_with` で分割数が変わり三角形数が増減。`TessellationOptions::new(2,0)` が `angular=3/axial=1` にクランプされる | 2 解像度で差分 + クランプ検証 (R02) |
./features/12-cylinder/design-review-3.md:207:| T14 | 退化入力エラー | `radius<=0` / `height<=0` の `create_cylinder` を `build_solid_from_features` に渡すと `KernelError::InvalidParameter` | `assert!(matches!(..))` (R02) |
./features/12-cylinder/design-review-3.md:218:- `Sphere`/`Cone`/`Extrude`、任意トリム面の 2D メッシュライブラリ導入、pcurve、ビューアの解析/GPU 適応レンダリング、`HalfEdge` への隣接(twin/next/prev)構造導入（R03、Boolean/フィレットが要求した時点で別途）。
./features/12-cylinder/design-review-3.md:221:設計ドキュメントをコードベースの前提と照らしてレビューします。まず既存の `make_cuboid`、トポロジー型、テッセレーション実装、エラー型を確認して、この提案が MyCad の規約と実装実態に噛み合うかを見ます。
./features/12-cylinder/design-review-3.md:223:/bin/bash -lc "rg -n \"make_cuboid|tessellate_solid|IdGenerator|KernelError|HalfEdge|inner_loops|Surface::|Curve::|TriangleMesh|Feature::CreateCylinder|CreateCylinder|UnsupportedFeature|InvalidParameter|mod.rs|pub use\" crates/mycad-kernel crates/mycad-build crates/mycad-cli -g '"'!target'"'" in /home/bacon/mycad
./features/12-cylinder/design-review-3.md:225:crates/mycad-cli/src/main.rs:4:use mycad_kernel::brep::topology::IdGenerator;
./features/12-cylinder/design-review-3.md:227:crates/mycad-cli/src/main.rs:43:    let mut gen = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:233:crates/mycad-build/src/lib.rs:2:use mycad_kernel::brep::topology::{IdGenerator, Solid};
./features/12-cylinder/design-review-3.md:235:crates/mycad-build/src/lib.rs:4:use mycad_kernel::primitives::make_cuboid;
./features/12-cylinder/design-review-3.md:236:crates/mycad-build/src/lib.rs:8:    gen: &mut IdGenerator,
./features/12-cylinder/design-review-3.md:240:crates/mycad-build/src/lib.rs:26:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
./features/12-cylinder/design-review-3.md:247:crates/mycad-kernel/benches/tessellation.rs:1:use mycad_kernel::brep::topology::IdGenerator;
./features/12-cylinder/design-review-3.md:248:crates/mycad-kernel/benches/tessellation.rs:2:use mycad_kernel::primitives::make_cuboid;
./features/12-cylinder/design-review-3.md:250:crates/mycad-kernel/benches/tessellation.rs:14:        let mut id_gen = IdGenerator::new(i as u64);
./features/12-cylinder/design-review-3.md:251:crates/mycad-kernel/benches/tessellation.rs:15:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
./features/12-cylinder/design-review-3.md:254:crates/mycad-kernel/src/tessellation/mod.rs:10:pub struct TriangleMesh {
./features/12-cylinder/design-review-3.md:255:crates/mycad-kernel/src/tessellation/mod.rs:19:impl TriangleMesh {
./features/12-cylinder/design-review-3.md:256:crates/mycad-kernel/src/tessellation/mod.rs:34:impl Default for TriangleMesh {
./features/12-cylinder/design-review-3.md:257:crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/design-review-3.md:258:crates/mycad-kernel/src/tessellation/mod.rs:50:    let mut mesh = TriangleMesh::new();
./features/12-cylinder/design-review-3.md:259:crates/mycad-kernel/src/tessellation/mod.rs:98:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review-3.md:260:crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
./features/12-cylinder/design-review-3.md:261:crates/mycad-kernel/src/tessellation/mod.rs:103:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:262:crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
./features/12-cylinder/design-review-3.md:264:crates/mycad-kernel/src/tessellation/mod.rs:117:        let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:265:crates/mycad-kernel/src/tessellation/mod.rs:118:        let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:266:crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
./features/12-cylinder/design-review-3.md:267:crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
./features/12-cylinder/design-review-3.md:270:crates/mycad-kernel/CLAUDE.md:14:- すべてのEdgeは正確に2つのHalfEdgeを持つ
./features/12-cylinder/design-review-3.md:271:crates/mycad-kernel/CLAUDE.md:16:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
./features/12-cylinder/design-review-3.md:272:crates/mycad-kernel/CLAUDE.md:17:- Loopは閉じたHalfEdge列（最後のHalfEdgeの終点 = 最初のHalfEdgeの始点）
./features/12-cylinder/design-review-3.md:273:crates/mycad-kernel/CLAUDE.md:18:- EntityIDは `IdGenerator` で決定的に生成される
./features/12-cylinder/design-review-3.md:274:crates/mycad-kernel/CLAUDE.md:23:2. 関数シグネチャ: `pub fn make_xxx(params..., id_gen: &mut IdGenerator) -> Solid`
./features/12-cylinder/design-review-3.md:276:crates/mycad-kernel/CLAUDE.md:25:4. 決定性テストを必ず書く（同じ `IdGenerator` 初期値で2回生成し、結果が同一であることを確認）
./features/12-cylinder/design-review-3.md:277:crates/mycad-kernel/src/tessellation/stl.rs:1:use super::TriangleMesh;
./features/12-cylinder/design-review-3.md:278:crates/mycad-kernel/src/tessellation/stl.rs:4:pub fn to_ascii_stl(mesh: &TriangleMesh, solid_name: &str) -> String {
./features/12-cylinder/design-review-3.md:279:crates/mycad-kernel/src/tessellation/stl.rs:51:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review-3.md:280:crates/mycad-kernel/src/tessellation/stl.rs:52:    use crate::primitives::make_cuboid;
./features/12-cylinder/design-review-3.md:282:crates/mycad-kernel/src/tessellation/stl.rs:57:        let mut g = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:283:crates/mycad-kernel/src/tessellation/stl.rs:58:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
./features/12-cylinder/design-review-3.md:285:crates/mycad-kernel/src/tessellation/stl.rs:74:            let mut g = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:286:crates/mycad-kernel/src/tessellation/stl.rs:75:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
./features/12-cylinder/design-review-3.md:288:crates/mycad-build/tests/feature_dispatcher.rs:3:use mycad_kernel::brep::topology::IdGenerator;
./features/12-cylinder/design-review-3.md:289:crates/mycad-build/tests/feature_dispatcher.rs:15:    let mut g = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:290:crates/mycad-kernel/tests/cuboid_roundtrip.rs:1:use mycad_kernel::brep::topology::IdGenerator;
./features/12-cylinder/design-review-3.md:291:crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
./features/12-cylinder/design-review-3.md:293:crates/mycad-kernel/tests/cuboid_roundtrip.rs:8:    let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:294:crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
./features/12-cylinder/design-review-3.md:296:crates/mycad-kernel/tests/cuboid_roundtrip.rs:30:        let mut id_gen = IdGenerator::new(42);
./features/12-cylinder/design-review-3.md:297:crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
./features/12-cylinder/design-review-3.md:319:crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
./features/12-cylinder/design-review-3.md:322:crates/mycad-kernel/src/primitives/cuboid.rs:1:use crate::brep::topology::{IdGenerator, Solid};
./features/12-cylinder/design-review-3.md:323:crates/mycad-kernel/src/primitives/cuboid.rs:16:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
./features/12-cylinder/design-review-3.md:331:crates/mycad-kernel/src/primitives/cuboid.rs:223:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review-3.md:332:crates/mycad-kernel/src/primitives/cuboid.rs:227:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:333:crates/mycad-kernel/src/primitives/cuboid.rs:228:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
./features/12-cylinder/design-review-3.md:334:crates/mycad-kernel/src/primitives/cuboid.rs:247:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:335:crates/mycad-kernel/src/primitives/cuboid.rs:248:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
./features/12-cylinder/design-review-3.md:336:crates/mycad-kernel/src/primitives/cuboid.rs:276:        let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:337:crates/mycad-kernel/src/primitives/cuboid.rs:277:        let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:338:crates/mycad-kernel/src/primitives/cuboid.rs:279:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
./features/12-cylinder/design-review-3.md:339:crates/mycad-kernel/src/primitives/cuboid.rs:280:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);
./features/12-cylinder/design-review-3.md:340:crates/mycad-kernel/src/brep/topology.rs:21:pub struct HalfEdge {
./features/12-cylinder/design-review-3.md:342:crates/mycad-kernel/src/brep/topology.rs:83:    pub half_edges: Vec<HalfEdge>,
./features/12-cylinder/design-review-3.md:343:crates/mycad-kernel/src/brep/topology.rs:138:        self.half_edges.push(HalfEdge {
./features/12-cylinder/design-review-3.md:346:crates/mycad-kernel/src/brep/topology.rs:185:pub struct IdGenerator {
./features/12-cylinder/design-review-3.md:347:crates/mycad-kernel/src/brep/topology.rs:189:impl IdGenerator {
./features/12-cylinder/design-review-3.md:348:crates/mycad-kernel/src/brep/topology.rs:208:        let mut gen1 = IdGenerator::new(100);
./features/12-cylinder/design-review-3.md:349:crates/mycad-kernel/src/brep/topology.rs:209:        let mut gen2 = IdGenerator::new(100);
./features/12-cylinder/design-review-3.md:350:crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
./features/12-cylinder/design-review-3.md:375:既存実装では `Surface` に円柱系の評価関数はすでにあり、トポロジーは `HalfEdge` の順序列だけで表現されています。ここからは、提案がその前提で本当に閉じるかと、テスト計画が実構造を十分に検証できるかを詰めます。
./features/12-cylinder/design-review-3.md:399:pub struct HalfEdge {
./features/12-cylinder/design-review-3.md:424:pub struct Loop {
./features/12-cylinder/design-review-3.md:447:pub struct Shell {
./features/12-cylinder/design-review-3.md:461:    pub half_edges: Vec<HalfEdge>,
./features/12-cylinder/design-review-3.md:463:    pub loops: Vec<Loop>,
./features/12-cylinder/design-review-3.md:465:    pub shells: Vec<Shell>,
./features/12-cylinder/design-review-3.md:516:        self.half_edges.push(HalfEdge {
./features/12-cylinder/design-review-3.md:528:        self.loops.push(Loop { id, half_edges });
./features/12-cylinder/design-review-3.md:555:        self.shells.push(Shell { id, faces, closed });
./features/12-cylinder/design-review-3.md:563:pub struct IdGenerator {
./features/12-cylinder/design-review-3.md:567:impl IdGenerator {
./features/12-cylinder/design-review-3.md:586:        let mut gen1 = IdGenerator::new(100);
./features/12-cylinder/design-review-3.md:587:        let mut gen2 = IdGenerator::new(100);
./features/12-cylinder/design-review-3.md:606:use crate::brep::topology::{IdGenerator, Solid};
./features/12-cylinder/design-review-3.md:621:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
./features/12-cylinder/design-review-3.md:828:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review-3.md:832:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:833:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
./features/12-cylinder/design-review-3.md:852:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:853:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
./features/12-cylinder/design-review-3.md:879:pub struct TriangleMesh {
./features/12-cylinder/design-review-3.md:888:impl TriangleMesh {
./features/12-cylinder/design-review-3.md:903:impl Default for TriangleMesh {
./features/12-cylinder/design-review-3.md:918:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/design-review-3.md:919:    let mut mesh = TriangleMesh::new();
./features/12-cylinder/design-review-3.md:967:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review-3.md:968:    use crate::primitives::make_cuboid;
./features/12-cylinder/design-review-3.md:972:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:973:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
./features/12-cylinder/design-review-3.md:986:        let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:987:        let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:989:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
./features/12-cylinder/design-review-3.md:990:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
./features/12-cylinder/design-review-3.md:1292:use thiserror::Error;
./features/12-cylinder/design-review-3.md:1308:use mycad_kernel::brep::topology::{IdGenerator, Solid};
./features/12-cylinder/design-review-3.md:1310:use mycad_kernel::primitives::make_cuboid;
./features/12-cylinder/design-review-3.md:1312:pub fn build_solid_from_features(
./features/12-cylinder/design-review-3.md:1314:    gen: &mut IdGenerator,
./features/12-cylinder/design-review-3.md:1332:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
./features/12-cylinder/design-review-3.md:1348:use mycad_build::build_solid_from_features;
./features/12-cylinder/design-review-3.md:1350:use mycad_kernel::brep::topology::IdGenerator;
./features/12-cylinder/design-review-3.md:1378:            if let Err(msg) = run_export(&input, &output) {
./features/12-cylinder/design-review-3.md:1386:fn run_export(input: &std::path::Path, output: &std::path::Path) -> Result<(), String> {
./features/12-cylinder/design-review-3.md:1387:    let doc = Document::from_path(input)
./features/12-cylinder/design-review-3.md:1389:    let mut gen = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:1390:    let solid = build_solid_from_features(&doc.root_component.features, &mut gen)
./features/12-cylinder/design-review-3.md:1404:use mycad_build::build_solid_from_features;
./features/12-cylinder/design-review-3.md:1406:use mycad_kernel::brep::topology::IdGenerator;
./features/12-cylinder/design-review-3.md:1417:    let doc = Document::from_path(&path).expect("load .mycad");
./features/12-cylinder/design-review-3.md:1418:    let mut g = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:1420:        build_solid_from_features(&doc.root_component.features, &mut g).expect("build solid");
./features/12-cylinder/design-review-3.md:1462:use mycad_kernel::brep::topology::IdGenerator;
./features/12-cylinder/design-review-3.md:1463:use mycad_kernel::primitives::make_cuboid;
./features/12-cylinder/design-review-3.md:1469:    let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:1470:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
./features/12-cylinder/design-review-3.md:1491:        let mut id_gen = IdGenerator::new(42);
./features/12-cylinder/design-review-3.md:1492:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
./features/12-cylinder/design-review-3.md:1544:crates/mycad-format/src/document.rs:96:        let doc = Document::from_path(&path).unwrap();
./features/12-cylinder/design-review-3.md:1547:crates/mycad-format/src/document.rs:104:        let err = Document::from_path(path).unwrap_err();
./features/12-cylinder/design-review-3.md:1549:crates/mycad-format/src/document.rs:111:        let err = Document::from_path(path).unwrap_err();
./features/12-cylinder/design-review-3.md:1550:crates/mycad-format/src/document.rs:124:        let doc = Document::from_path(&path).unwrap();
./features/12-cylinder/design-review-3.md:1556:crates/mycad-cli/src/main.rs:41:    let doc = Document::from_path(input)
./features/12-cylinder/design-review-3.md:1557:crates/mycad-cli/src/main.rs:44:    let solid = build_solid_from_features(&doc.root_component.features, &mut gen)
./features/12-cylinder/design-review-3.md:1561:crates/mycad-build/tests/feature_dispatcher.rs:14:    let doc = Document::from_path(&path).expect("load .mycad");
./features/12-cylinder/design-review-3.md:1562:crates/mycad-build/tests/feature_dispatcher.rs:17:        build_solid_from_features(&doc.root_component.features, &mut g).expect("build solid");
./features/12-cylinder/design-review-3.md:1596:- すべてのEdgeは正確に2つのHalfEdgeを持つ
./features/12-cylinder/design-review-3.md:1597:- 閉じたShellはwatertight（水密）
./features/12-cylinder/design-review-3.md:1598:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
./features/12-cylinder/design-review-3.md:1599:- Loopは閉じたHalfEdge列（最後のHalfEdgeの終点 = 最初のHalfEdgeの始点）
./features/12-cylinder/design-review-3.md:1600:- EntityIDは `IdGenerator` で決定的に生成される
./features/12-cylinder/design-review-3.md:1605:2. 関数シグネチャ: `pub fn make_xxx(params..., id_gen: &mut IdGenerator) -> Solid`
./features/12-cylinder/design-review-3.md:1607:4. 決定性テストを必ず書く（同じ `IdGenerator` 初期値で2回生成し、結果が同一であることを確認）
./features/12-cylinder/design-review-3.md:1625:pub use cuboid::make_cuboid;
./features/12-cylinder/design-review-3.md:1835:            return Err(FormatError::InvalidExtension(ext.map(str::to_string)));
./features/12-cylinder/design-review-3.md:1892:        let doc = Document::from_path(&path).unwrap();
./features/12-cylinder/design-review-3.md:1900:        let err = Document::from_path(path).unwrap_err();
./features/12-cylinder/design-review-3.md:1907:        let err = Document::from_path(path).unwrap_err();
./features/12-cylinder/design-review-3.md:1908:        assert!(matches!(err, FormatError::InvalidExtension(Some(ref e)) if e == "txt"));
./features/12-cylinder/design-review-3.md:1912:    fn test_assembly_roundtrip() {
./features/12-cylinder/design-review-3.md:1920:        let doc = Document::from_path(&path).unwrap();
./features/12-cylinder/design-review-3.md:1962:[workspace.dependencies]
./features/12-cylinder/design-review-3.md:1974:thiserror = "2"
./features/12-cylinder/design-review-3.md:1995:thiserror = { workspace = true }
./features/12-cylinder/design-review-3.md:2042:/bin/bash -lc "rg -n \"approx =|thiserror =|schemars =|serde_yaml =|tempfile =|workspace.dependencies|\\[workspace.dependencies\\]\" Cargo.toml crates/*/Cargo.toml" in /home/bacon/mycad
./features/12-cylinder/design-review-3.md:2046:crates/mycad-format/Cargo.toml:12:thiserror = { workspace = true }
./features/12-cylinder/design-review-3.md:2048:crates/mycad-kernel/Cargo.toml:11:thiserror = { workspace = true }
./features/12-cylinder/design-review-3.md:2050:Cargo.toml:18:[workspace.dependencies]
./features/12-cylinder/design-review-3.md:2055:Cargo.toml:30:thiserror = "2"
./features/12-cylinder/design-review-3.md:2061:use super::TriangleMesh;
./features/12-cylinder/design-review-3.md:2064:pub fn to_ascii_stl(mesh: &TriangleMesh, solid_name: &str) -> String {
./features/12-cylinder/design-review-3.md:2111:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/design-review-3.md:2112:    use crate::primitives::make_cuboid;
./features/12-cylinder/design-review-3.md:2117:        let mut g = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:2118:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
./features/12-cylinder/design-review-3.md:2134:            let mut g = IdGenerator::new(0);
./features/12-cylinder/design-review-3.md:2135:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
./features/12-cylinder/design-review-3.md:2148:    suggestion: "`tessellate_solid_with` は unsupported face をスキップせず失敗を返す設計に変更し、`TessellationError` もしくは `KernelError` 経由で CLI まで明示的に伝播させること"
./features/12-cylinder/design-review-3.md:2153:    suggestion: "UvGrid を『矩形 UV パッチであることを証明できる面』に限定すること。少なくとも #12 では `make_cylinder` の側面のような既知トポロジーだけに適用し、一般 face には使わないか、矩形トリム判定を別途導入すること"
./features/12-cylinder/design-review-3.md:2162:    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
./features/12-cylinder/design-review-3.md:2163:    suggestion: "`make_cylinder` も fallible にするか、少なくとも `is_finite()` と正値チェックをカーネル境界で強制して、ビルド層の検証はその前段の利便層として重ねること"
./features/12-cylinder/design-review-3.md:2178:    suggestion: "`tessellate_solid_with` は unsupported face をスキップせず失敗を返す設計に変更し、`TessellationError` もしくは `KernelError` 経由で CLI まで明示的に伝播させること"
./features/12-cylinder/design-review-3.md:2183:    suggestion: "UvGrid を『矩形 UV パッチであることを証明できる面』に限定すること。少なくとも #12 では `make_cylinder` の側面のような既知トポロジーだけに適用し、一般 face には使わないか、矩形トリム判定を別途導入すること"
./features/12-cylinder/design-review-3.md:2192:    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
./features/12-cylinder/design-review-3.md:2193:    suggestion: "`make_cylinder` も fallible にするか、少なくとも `is_finite()` と正値チェックをカーネル境界で強制して、ビルド層の検証はその前段の利便層として重ねること"
./features/12-cylinder/final-review.md:39:    Adds make_cylinder (B-rep: 2 vertices / 3 edges / 3 faces / closed shell,
./features/12-cylinder/final-review.md:40:    axis +Z) and extends tessellate_solid_with to handle circular-arc loops:
./features/12-cylinder/final-review.md:44:    - make_cylinder(radius, height) -> Result<Solid, KernelError> with is_finite+positive validation
./features/12-cylinder/final-review.md:62: use mycad_kernel::brep::topology::{IdGenerator, Solid};
./features/12-cylinder/final-review.md:64:-use mycad_kernel::primitives::make_cuboid;
./features/12-cylinder/final-review.md:65:+use mycad_kernel::primitives::{make_cuboid, make_cylinder};
./features/12-cylinder/final-review.md:67: pub fn build_solid_from_features(
./features/12-cylinder/final-review.md:69:     gen: &mut IdGenerator,
./features/12-cylinder/final-review.md:87:         } => Ok(make_cuboid(*width, *height, *depth, gen)),
./features/12-cylinder/final-review.md:91:+        Feature::CreateCylinder { radius, height, .. } => make_cylinder(*radius, *height, gen),
./features/12-cylinder/final-review.md:107: use mycad_build::build_solid_from_features;
./features/12-cylinder/final-review.md:109: use mycad_kernel::brep::topology::IdGenerator;
./features/12-cylinder/final-review.md:111:+use mycad_kernel::tessellation::{tessellate_solid_with, to_ascii_stl, TessellationOptions};
./features/12-cylinder/final-review.md:141:-            if let Err(msg) = run_export(&input, &output) {
./features/12-cylinder/final-review.md:147:+            if let Err(msg) = run_export(&input, &output, segments) {
./features/12-cylinder/final-review.md:155:-fn run_export(input: &std::path::Path, output: &std::path::Path) -> Result<(), String> {
./features/12-cylinder/final-review.md:156:+fn run_export(
./features/12-cylinder/final-review.md:161:     let doc = Document::from_path(input)
./features/12-cylinder/final-review.md:163:     let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:164:     let solid = build_solid_from_features(&doc.root_component.features, &mut gen)
./features/12-cylinder/final-review.md:172:+        tessellate_solid_with(&solid, &opts).map_err(|e| format!("failed to tessellate: {e}"))?;
./features/12-cylinder/final-review.md:183: use mycad_kernel::brep::topology::IdGenerator;
./features/12-cylinder/final-review.md:184: use mycad_kernel::primitives::make_cuboid;
./features/12-cylinder/final-review.md:196:         let mut id_gen = IdGenerator::new(i as u64);
./features/12-cylinder/final-review.md:197:         let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
./features/12-cylinder/final-review.md:215: use thiserror::Error;
./features/12-cylinder/final-review.md:563:+use crate::brep::topology::{IdGenerator, Solid};
./features/12-cylinder/final-review.md:573:+pub fn make_cylinder(
./features/12-cylinder/final-review.md:576:+    id_gen: &mut IdGenerator,
./features/12-cylinder/final-review.md:698:+        let mut gen1 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:699:+        let mut gen2 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:700:+        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
./features/12-cylinder/final-review.md:701:+        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
./features/12-cylinder/final-review.md:726:+        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:727:+        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review.md:742:+    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
./features/12-cylinder/final-review.md:745:+        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:746:+        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review.md:757:+            "Euler-Poincaré V-E+F = 2(S-H), S={shells}, H=0"
./features/12-cylinder/final-review.md:765:+        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:766:+        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review.md:783:+        // Loop closure: each loop's consecutive HEs must connect geometrically
./features/12-cylinder/final-review.md:819:+        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:820:+        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review.md:864:+        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:867:+        let err = make_cylinder(0.0, 20.0, &mut gen).unwrap_err();
./features/12-cylinder/final-review.md:874:+        let err = make_cylinder(-5.0, 20.0, &mut gen).unwrap_err();
./features/12-cylinder/final-review.md:881:+        let err = make_cylinder(f64::NAN, 20.0, &mut gen).unwrap_err();
./features/12-cylinder/final-review.md:888:+        let err = make_cylinder(f64::INFINITY, 20.0, &mut gen).unwrap_err();
./features/12-cylinder/final-review.md:895:+        let err = make_cylinder(5.0, 0.0, &mut gen).unwrap_err();
./features/12-cylinder/final-review.md:902:+        let err = make_cylinder(5.0, -10.0, &mut gen).unwrap_err();
./features/12-cylinder/final-review.md:909:+        let err = make_cylinder(5.0, f64::NAN, &mut gen).unwrap_err();
./features/12-cylinder/final-review.md:916:+        let err = make_cylinder(5.0, f64::INFINITY, &mut gen).unwrap_err();
./features/12-cylinder/final-review.md:945: pub use cuboid::make_cuboid;
./features/12-cylinder/final-review.md:946:+pub use cylinder::make_cylinder;
./features/12-cylinder/final-review.md:960:+use thiserror::Error;
./features/12-cylinder/final-review.md:968: pub struct TriangleMesh {
./features/12-cylinder/final-review.md:977: impl TriangleMesh {
./features/12-cylinder/final-review.md:992: impl Default for TriangleMesh {
./features/12-cylinder/final-review.md:1007:-pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/final-review.md:1041:+    NonManifoldLoop,
./features/12-cylinder/final-review.md:1045:+pub fn tessellate_solid(solid: &Solid) -> Result<TriangleMesh, TessellationError> {
./features/12-cylinder/final-review.md:1046:+    tessellate_solid_with(solid, &TessellationOptions::default())
./features/12-cylinder/final-review.md:1050:+pub fn tessellate_solid_with(
./features/12-cylinder/final-review.md:1053:+) -> Result<TriangleMesh, TessellationError> {
./features/12-cylinder/final-review.md:1054:     let mut mesh = TriangleMesh::new();
./features/12-cylinder/final-review.md:1099:+    mesh: &mut TriangleMesh,
./features/12-cylinder/final-review.md:1136:+    lp: &crate::brep::topology::Loop,
./features/12-cylinder/final-review.md:1165:+    mesh: &mut TriangleMesh,
./features/12-cylinder/final-review.md:1238:+fn push_triangle(mesh: &mut TriangleMesh, i0: u32, i1: u32, i2: u32) {
./features/12-cylinder/final-review.md:1274:     use crate::brep::topology::IdGenerator;
./features/12-cylinder/final-review.md:1276:     use crate::primitives::make_cuboid;
./features/12-cylinder/final-review.md:1277:+    use crate::primitives::make_cylinder;
./features/12-cylinder/final-review.md:1282:+        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:1283:+        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review.md:1301:+        let mut gen1 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:1302:+        let mut gen2 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:1304:+        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
./features/12-cylinder/final-review.md:1305:+        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
./features/12-cylinder/final-review.md:1318:+        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:1319:+        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
./features/12-cylinder/final-review.md:1331:+        let mut gen1 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:1332:+        let mut gen2 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:1334:+        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
./features/12-cylinder/final-review.md:1335:+        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
./features/12-cylinder/final-review.md:1348:+        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:1349:+        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review.md:1402:+        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:1403:+        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review.md:1406:+        let mesh_low = tessellate_solid_with(&solid, &TessellationOptions::new(8, 1)).unwrap();
./features/12-cylinder/final-review.md:1407:+        let mesh_hi = tessellate_solid_with(&solid, &TessellationOptions::new(64, 2)).unwrap();
./features/12-cylinder/final-review.md:1460:         let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:1461:         let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
./features/12-cylinder/final-review.md:1475:         let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:1476:         let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:1478:         let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
./features/12-cylinder/final-review.md:1479:         let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
./features/12-cylinder/final-review.md:1496: use super::TriangleMesh;
./features/12-cylinder/final-review.md:1499: pub fn to_ascii_stl(mesh: &TriangleMesh, solid_name: &str) -> String {
./features/12-cylinder/final-review.md:1546:     use crate::brep::topology::IdGenerator;
./features/12-cylinder/final-review.md:1547:     use crate::primitives::make_cuboid;
./features/12-cylinder/final-review.md:1552:         let mut g = IdGenerator::new(0);
./features/12-cylinder/final-review.md:1553:         let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
./features/12-cylinder/final-review.md:1570:             let mut g = IdGenerator::new(0);
./features/12-cylinder/final-review.md:1571:             let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
./features/12-cylinder/final-review.md:1583: use mycad_kernel::brep::topology::IdGenerator;
./features/12-cylinder/final-review.md:1584: use mycad_kernel::primitives::make_cuboid;
./features/12-cylinder/final-review.md:1590:     let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:1591:     let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
./features/12-cylinder/final-review.md:1613:         let mut id_gen = IdGenerator::new(42);
./features/12-cylinder/final-review.md:1614:         let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
./features/12-cylinder/final-review.md:1666:pub struct HalfEdge {
./features/12-cylinder/final-review.md:1691:pub struct Loop {
./features/12-cylinder/final-review.md:1714:pub struct Shell {
./features/12-cylinder/final-review.md:1728:    pub half_edges: Vec<HalfEdge>,
./features/12-cylinder/final-review.md:1730:    pub loops: Vec<Loop>,
./features/12-cylinder/final-review.md:1732:    pub shells: Vec<Shell>,
./features/12-cylinder/final-review.md:1783:        self.half_edges.push(HalfEdge {
./features/12-cylinder/final-review.md:1795:        self.loops.push(Loop { id, half_edges });
./features/12-cylinder/final-review.md:1822:        self.shells.push(Shell { id, faces, closed });
./features/12-cylinder/final-review.md:1830:pub struct IdGenerator {
./features/12-cylinder/final-review.md:1834:impl IdGenerator {
./features/12-cylinder/final-review.md:1853:        let mut gen1 = IdGenerator::new(100);
./features/12-cylinder/final-review.md:1854:        let mut gen2 = IdGenerator::new(100);
./features/12-cylinder/final-review.md:1871:use crate::brep::topology::{IdGenerator, Solid};
./features/12-cylinder/final-review.md:1881:pub fn make_cylinder(
./features/12-cylinder/final-review.md:1884:    id_gen: &mut IdGenerator,
./features/12-cylinder/final-review.md:2006:        let mut gen1 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:2007:        let mut gen2 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:2008:        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
./features/12-cylinder/final-review.md:2009:        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
./features/12-cylinder/final-review.md:2034:        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:2035:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review.md:2050:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
./features/12-cylinder/final-review.md:2053:        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:2054:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review.md:2065:            "Euler-Poincaré V-E+F = 2(S-H), S={shells}, H=0"
./features/12-cylinder/final-review.md:2073:        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:2074:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review.md:2091:        // Loop closure: each loop's consecutive HEs must connect geometrically
./features/12-cylinder/final-review.md:2127:        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:2128:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review.md:2181:use crate::brep::topology::{IdGenerator, Solid};
./features/12-cylinder/final-review.md:2196:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
./features/12-cylinder/final-review.md:2403:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/final-review.md:2407:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:2408:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
./features/12-cylinder/final-review.md:2427:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:2428:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
./features/12-cylinder/final-review.md:2457:pub use cuboid::make_cuboid;
./features/12-cylinder/final-review.md:2458:pub use cylinder::make_cylinder;
./features/12-cylinder/final-review.md:2481:crates/mycad-build/src/lib.rs:27:        Feature::CreateCylinder { radius, height, .. } => make_cylinder(*radius, *height, gen),
./features/12-cylinder/final-review.md:2620:use thiserror::Error;
./features/12-cylinder/final-review.md:2628:pub struct TriangleMesh {
./features/12-cylinder/final-review.md:2637:impl TriangleMesh {
./features/12-cylinder/final-review.md:2652:impl Default for TriangleMesh {
./features/12-cylinder/final-review.md:2691:    NonManifoldLoop,
./features/12-cylinder/final-review.md:2695:pub fn tessellate_solid(solid: &Solid) -> Result<TriangleMesh, TessellationError> {
./features/12-cylinder/final-review.md:2696:    tessellate_solid_with(solid, &TessellationOptions::default())
./features/12-cylinder/final-review.md:2700:pub fn tessellate_solid_with(
./features/12-cylinder/final-review.md:2703:) -> Result<TriangleMesh, TessellationError> {
./features/12-cylinder/final-review.md:2704:    let mut mesh = TriangleMesh::new();
./features/12-cylinder/final-review.md:2731:    mesh: &mut TriangleMesh,
./features/12-cylinder/final-review.md:2766:    lp: &crate::brep::topology::Loop,
./features/12-cylinder/final-review.md:2795:    mesh: &mut TriangleMesh,
./features/12-cylinder/final-review.md:2862:fn push_triangle(mesh: &mut TriangleMesh, i0: u32, i1: u32, i2: u32) {
./features/12-cylinder/final-review.md:2900:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/final-review.md:2902:    use crate::primitives::make_cuboid;
./features/12-cylinder/final-review.md:2903:    use crate::primitives::make_cylinder;
./features/12-cylinder/final-review.md:2908:        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:2909:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review.md:2927:        let mut gen1 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:2928:        let mut gen2 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:2930:        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
./features/12-cylinder/final-review.md:2931:        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
./features/12-cylinder/final-review.md:2944:        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:2945:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
./features/12-cylinder/final-review.md:2957:        let mut gen1 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:2958:        let mut gen2 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:2960:        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
./features/12-cylinder/final-review.md:2961:        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
./features/12-cylinder/final-review.md:2974:        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:2975:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review.md:3028:        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:3029:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review.md:3032:        let mesh_low = tessellate_solid_with(&solid, &TessellationOptions::new(8, 1)).unwrap();
./features/12-cylinder/final-review.md:3033:        let mesh_hi = tessellate_solid_with(&solid, &TessellationOptions::new(64, 2)).unwrap();
./features/12-cylinder/final-review.md:3086:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:3087:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
./features/12-cylinder/final-review.md:3098:        let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:3099:        let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:3101:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
./features/12-cylinder/final-review.md:3102:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
./features/12-cylinder/final-review.md:3158:        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:3161:        let err = make_cylinder(0.0, 20.0, &mut gen).unwrap_err();
./features/12-cylinder/final-review.md:3168:        let err = make_cylinder(-5.0, 20.0, &mut gen).unwrap_err();
./features/12-cylinder/final-review.md:3175:        let err = make_cylinder(f64::NAN, 20.0, &mut gen).unwrap_err();
./features/12-cylinder/final-review.md:3182:        let err = make_cylinder(f64::INFINITY, 20.0, &mut gen).unwrap_err();
./features/12-cylinder/final-review.md:3189:        let err = make_cylinder(5.0, 0.0, &mut gen).unwrap_err();
./features/12-cylinder/final-review.md:3196:        let err = make_cylinder(5.0, -10.0, &mut gen).unwrap_err();
./features/12-cylinder/final-review.md:3203:        let err = make_cylinder(5.0, f64::NAN, &mut gen).unwrap_err();
./features/12-cylinder/final-review.md:3210:        let err = make_cylinder(5.0, f64::INFINITY, &mut gen).unwrap_err();
./features/12-cylinder/final-review.md:3235:./features/12-cylinder/design-review-3.md:138:  pub fn tessellate_solid(solid) -> TriangleMesh           // = _with(solid, &default())  ※既存呼び出し・既存テスト不変
./features/12-cylinder/final-review.md:3239:./features/12-cylinder/design-review-3.md:257:crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/final-review.md:3247:./features/12-cylinder/design-review-3.md:918:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/final-review.md:3259:./features/12-cylinder/final-review.md:1007:-pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/final-review.md:3260:./features/12-cylinder/final-review.md:1045:+pub fn tessellate_solid(solid: &Solid) -> Result<TriangleMesh, TessellationError> {
./features/12-cylinder/final-review.md:3284:./features/12-cylinder/final-review.md:2695:pub fn tessellate_solid(solid: &Solid) -> Result<TriangleMesh, TessellationError> {
./features/12-cylinder/final-review.md:3297:./features/12-cylinder/design-review-2.md:138:  pub fn tessellate_solid(solid) -> TriangleMesh           // = _with(solid, &default())  ※既存呼び出し・既存テスト不変
./features/12-cylinder/final-review.md:3299:./features/12-cylinder/design-review-2.md:607:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/final-review.md:3304:./features/12-cylinder/design-review-2.md:1888:./features/12-cylinder/design-review.md:528:crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/final-review.md:3310:./features/12-cylinder/design-review.md:137:  pub fn tessellate_solid(solid) -> TriangleMesh           // = _with(solid, &default())  ※既存呼び出し・既存テスト不変
./features/12-cylinder/final-review.md:3312:./features/12-cylinder/design-review.md:458:features/12-cylinder/design-review.md:137:  pub fn tessellate_solid(solid) -> TriangleMesh           // = _with(solid, &default())  ※既存呼び出し・既存テスト不変
./features/12-cylinder/final-review.md:3315:./features/12-cylinder/design-review.md:528:crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/final-review.md:3324:./features/12-cylinder/design-review.md:1220:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/final-review.md:3336:./crates/mycad-kernel/src/tessellation/mod.rs:84:pub fn tessellate_solid(solid: &Solid) -> Result<TriangleMesh, TessellationError> {
./features/12-cylinder/final-review.md:3362:pub struct TriangleMesh {
./features/12-cylinder/final-review.md:3371:impl TriangleMesh {
./features/12-cylinder/final-review.md:3386:impl Default for TriangleMesh {
./features/12-cylinder/final-review.md:3401:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/final-review.md:3402:    let mut mesh = TriangleMesh::new();
./features/12-cylinder/final-review.md:3450:    use crate::brep::topology::IdGenerator;
./features/12-cylinder/final-review.md:3451:    use crate::primitives::make_cuboid;
./features/12-cylinder/final-review.md:3455:        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:3456:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
./features/12-cylinder/final-review.md:3469:        let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:3470:        let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:3472:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
./features/12-cylinder/final-review.md:3473:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
./features/12-cylinder/final-review.md:3667:79:ROADMAP Phase 3「円柱・球・押し出しが作れる」の最初の一歩。現状カーネルは `make_cuboid` と平面専用の `tessellate_solid` しか持たない。本 Issue で **解析曲面(円柱)を厳密 B-rep として生成し、それを三角形メッシュへ離散化する土台**を確立し、`mycad export examples/cylinder.mycad -o out.stl` を動作させる。
./features/12-cylinder/final-review.md:3669:127:**退化入力の検証（R02）**: `.mycad` 由来のユーザー入力は境界で弾く。`KernelError` に `InvalidParameter { kind: &'static str }` を追加し、`build_solid_from_features` の `CreateCylinder` 分岐で `radius > 0.0 && height > 0.0` を検証（非正なら `InvalidParameter` を返し `make_cylinder` を呼ばない）。`make_cylinder` 自体は `make_cuboid` と同じ `-> Solid` シグネチャを保ち、正値前提（検証は呼び出し側＝境界に集約）。前提条件はドキュメントコメントに明記し、`debug_assert!(radius > 0.0 && height > 0.0)` を先頭に置いて開発時の契約違反を検出（リリースでは境界検証済み前提で素通り）。解像度の退化は `TessellationOptions` のクランプ（前述）で吸収。
./features/12-cylinder/final-review.md:3671:171:- `TessellationOptions` に `Debug, Clone, Serialize, Deserialize`（既存 `TriangleMesh` と同様）。`Copy` も可。
./features/12-cylinder/final-review.md:3673:187:- R04(再掲) 反映: `make_cylinder` に `debug_assert!(radius>0 && height>0)` と前提コメントを追加（境界検証は呼び出し側に集約のまま）。
./features/12-cylinder/final-review.md:3710:2162:    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
./features/12-cylinder/final-review.md:3711:2163:    suggestion: "`make_cylinder` も fallible にするか、少なくとも `is_finite()` と正値チェックをカーネル境界で強制して、ビルド層の検証はその前段の利便層として重ねること"
./features/12-cylinder/final-review.md:3713:2192:    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
./features/12-cylinder/final-review.md:3714:2193:    suggestion: "`make_cylinder` も fallible にするか、少なくとも `is_finite()` と正値チェックをカーネル境界で強制して、ビルド層の検証はその前段の利便層として重ねること"
./features/12-cylinder/final-review.md:3723:            let mut g = IdGenerator::new(0);
./features/12-cylinder/final-review.md:3724:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
./features/12-cylinder/final-review.md:3737:    suggestion: "`tessellate_solid_with` は unsupported face をスキップせず失敗を返す設計に変更し、`TessellationError` もしくは `KernelError` 経由で CLI まで明示的に伝播させること"
./features/12-cylinder/final-review.md:3742:    suggestion: "UvGrid を『矩形 UV パッチであることを証明できる面』に限定すること。少なくとも #12 では `make_cylinder` の側面のような既知トポロジーだけに適用し、一般 face には使わないか、矩形トリム判定を別途導入すること"
./features/12-cylinder/final-review.md:3751:    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
./features/12-cylinder/final-review.md:3752:    suggestion: "`make_cylinder` も fallible にするか、少なくとも `is_finite()` と正値チェックをカーネル境界で強制して、ビルド層の検証はその前段の利便層として重ねること"
./features/12-cylinder/final-review.md:3767:    suggestion: "`tessellate_solid_with` は unsupported face をスキップせず失敗を返す設計に変更し、`TessellationError` もしくは `KernelError` 経由で CLI まで明示的に伝播させること"
./features/12-cylinder/final-review.md:3772:    suggestion: "UvGrid を『矩形 UV パッチであることを証明できる面』に限定すること。少なくとも #12 では `make_cylinder` の側面のような既知トポロジーだけに適用し、一般 face には使わないか、矩形トリム判定を別途導入すること"
./features/12-cylinder/final-review.md:3781:    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
./features/12-cylinder/final-review.md:3782:    suggestion: "`make_cylinder` も fallible にするか、少なくとも `is_finite()` と正値チェックをカーネル境界で強制して、ビルド層の検証はその前段の利便層として重ねること"
./features/12-cylinder/final-review.md:3808:            let mut g = IdGenerator::new(0);
./features/12-cylinder/final-review.md:3809:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
./features/12-cylinder/final-review.md:3822:    suggestion: "`tessellate_solid_with` は unsupported face をスキップせず失敗を返す設計に変更し、`TessellationError` もしくは `KernelError` 経由で CLI まで明示的に伝播させること"
./features/12-cylinder/final-review.md:3827:    suggestion: "UvGrid を『矩形 UV パッチであることを証明できる面』に限定すること。少なくとも #12 では `make_cylinder` の側面のような既知トポロジーだけに適用し、一般 face には使わないか、矩形トリム判定を別途導入すること"
./features/12-cylinder/final-review.md:3836:    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
./features/12-cylinder/final-review.md:3837:    suggestion: "`make_cylinder` も fallible にするか、少なくとも `is_finite()` と正値チェックをカーネル境界で強制して、ビルド層の検証はその前段の利便層として重ねること"
./features/12-cylinder/final-review.md:3852:    suggestion: "`tessellate_solid_with` は unsupported face をスキップせず失敗を返す設計に変更し、`TessellationError` もしくは `KernelError` 経由で CLI まで明示的に伝播させること"
./features/12-cylinder/final-review.md:3857:    suggestion: "UvGrid を『矩形 UV パッチであることを証明できる面』に限定すること。少なくとも #12 では `make_cylinder` の側面のような既知トポロジーだけに適用し、一般 face には使わないか、矩形トリム判定を別途導入すること"
./features/12-cylinder/final-review.md:3866:    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
./features/12-cylinder/final-review.md:3867:    suggestion: "`make_cylinder` も fallible にするか、少なくとも `is_finite()` と正値チェックをカーネル境界で強制して、ビルド層の検証はその前段の利便層として重ねること"
./features/12-cylinder/final-review.md:3883:     9	use thiserror::Error;
./features/12-cylinder/final-review.md:3891:    17	pub struct TriangleMesh {
./features/12-cylinder/final-review.md:3900:    26	impl TriangleMesh {
./features/12-cylinder/final-review.md:3915:    41	impl Default for TriangleMesh {
./features/12-cylinder/final-review.md:3954:    80	    NonManifoldLoop,
./features/12-cylinder/final-review.md:3958:    84	pub fn tessellate_solid(solid: &Solid) -> Result<TriangleMesh, TessellationError> {
./features/12-cylinder/final-review.md:3959:    85	    tessellate_solid_with(solid, &TessellationOptions::default())
./features/12-cylinder/final-review.md:3963:    89	pub fn tessellate_solid_with(
./features/12-cylinder/final-review.md:3966:    92	) -> Result<TriangleMesh, TessellationError> {
./features/12-cylinder/final-review.md:3967:    93	    let mut mesh = TriangleMesh::new();
./features/12-cylinder/final-review.md:3994:   120	    mesh: &mut TriangleMesh,
./features/12-cylinder/final-review.md:4029:   155	    lp: &crate::brep::topology::Loop,
./features/12-cylinder/final-review.md:4058:   184	    mesh: &mut TriangleMesh,
./features/12-cylinder/final-review.md:4125:   251	fn push_triangle(mesh: &mut TriangleMesh, i0: u32, i1: u32, i2: u32) {
./features/12-cylinder/final-review.md:4166:   287	    use crate::brep::topology::IdGenerator;
./features/12-cylinder/final-review.md:4168:   289	    use crate::primitives::make_cuboid;
./features/12-cylinder/final-review.md:4169:   290	    use crate::primitives::make_cylinder;
./features/12-cylinder/final-review.md:4174:   295	        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:4175:   296	        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review.md:4193:   314	        let mut gen1 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:4194:   315	        let mut gen2 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:4196:   317	        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
./features/12-cylinder/final-review.md:4197:   318	        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
./features/12-cylinder/final-review.md:4210:   331	        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:4211:   332	        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
./features/12-cylinder/final-review.md:4223:   344	        let mut gen1 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:4224:   345	        let mut gen2 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:4226:   347	        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
./features/12-cylinder/final-review.md:4227:   348	        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
./features/12-cylinder/final-review.md:4240:   361	        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:4241:   362	        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review.md:4294:   415	        let mut gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:4295:   416	        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/12-cylinder/final-review.md:4298:   419	        let mesh_low = tessellate_solid_with(&solid, &TessellationOptions::new(8, 1)).unwrap();
./features/12-cylinder/final-review.md:4299:   420	        let mesh_hi = tessellate_solid_with(&solid, &TessellationOptions::new(64, 2)).unwrap();
./features/12-cylinder/final-review.md:4352:   473	        let mut id_gen = IdGenerator::new(0);
./features/12-cylinder/final-review.md:4353:   474	        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
./features/12-cylinder/final-review.md:4364:   485	        let mut id_gen1 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:4365:   486	        let mut id_gen2 = IdGenerator::new(0);
./features/12-cylinder/final-review.md:4367:   488	        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
./features/12-cylinder/final-review.md:4368:   489	        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
./crates/mycad-kernel/Cargo.toml:11:thiserror = { workspace = true }
./crates/mycad-cli/src/main.rs:2:use mycad_build::build_solid_from_features;
./crates/mycad-cli/src/main.rs:4:use mycad_kernel::brep::topology::IdGenerator;
./crates/mycad-cli/src/main.rs:5:use mycad_kernel::tessellation::{tessellate_solid_with, to_ascii_stl, TessellationOptions};
./crates/mycad-cli/src/main.rs:39:            if let Err(msg) = run_export(&input, &output, segments) {
./crates/mycad-cli/src/main.rs:47:fn run_export(
./crates/mycad-cli/src/main.rs:52:    let doc = Document::from_path(input)
./crates/mycad-cli/src/main.rs:54:    let mut gen = IdGenerator::new(0);
./crates/mycad-cli/src/main.rs:55:    let solid = build_solid_from_features(&doc.root_component.features, &mut gen)
./crates/mycad-cli/src/main.rs:62:        tessellate_solid_with(&solid, &opts).map_err(|e| format!("failed to tessellate: {e}"))?;
./crates/mycad-build/src/lib.rs:2:use mycad_kernel::brep::topology::{IdGenerator, Solid};
./crates/mycad-build/src/lib.rs:4:use mycad_kernel::primitives::{make_cuboid, make_cylinder};
./crates/mycad-build/src/lib.rs:6:pub fn build_solid_from_features(
./crates/mycad-build/src/lib.rs:8:    gen: &mut IdGenerator,
./crates/mycad-build/src/lib.rs:26:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
./crates/mycad-build/src/lib.rs:27:        Feature::CreateCylinder { radius, height, .. } => make_cylinder(*radius, *height, gen),
./crates/mycad-build/tests/feature_dispatcher.rs:1:use mycad_build::build_solid_from_features;
./crates/mycad-build/tests/feature_dispatcher.rs:3:use mycad_kernel::brep::topology::IdGenerator;
./crates/mycad-build/tests/feature_dispatcher.rs:14:    let doc = Document::from_path(&path).expect("load .mycad");
./crates/mycad-build/tests/feature_dispatcher.rs:15:    let mut g = IdGenerator::new(0);
./crates/mycad-build/tests/feature_dispatcher.rs:17:        build_solid_from_features(&doc.root_component.features, &mut g).expect("build solid");
./CLAUDE.md:27:│   ├── brep/         トポロジー (Solid, Shell, Face, Loop, Edge, HalfEdge, Vertex)
./CLAUDE.md:29:│   ├── primitives/   プリミティブ生成 (make_cuboid)
./CLAUDE.md:30:│   └── tessellation/ メッシュ化 (tessellate_solid → TriangleMesh)
./CLAUDE.md:53:- **決定性**: すべてのEntityIDは `IdGenerator` で決定的に生成。テストでは決定性を検証すること。
./CLAUDE.md:54:- **依存管理**: 新しい依存は必ず `[workspace.dependencies]` に追加し、各クレートは `{ workspace = true }` で参照。
./CLAUDE.md:56:- **エラーハンドリング**: ライブラリエラーには `thiserror` を使用。
./CLAUDE.md:64:- **Kernel has zero rendering deps**: カーネルはレンダリング非依存。`TriangleMesh` を生成し、ビューアが消費する。
./crates/mycad-kernel/src/error.rs:1:use thiserror::Error;
./crates/mycad-kernel/src/tessellation/mod.rs:10:use thiserror::Error;
./crates/mycad-kernel/src/tessellation/mod.rs:18:pub struct TriangleMesh {
./crates/mycad-kernel/src/tessellation/mod.rs:27:impl TriangleMesh {
./crates/mycad-kernel/src/tessellation/mod.rs:42:impl Default for TriangleMesh {
./crates/mycad-kernel/src/tessellation/mod.rs:81:    NonManifoldLoop,
./crates/mycad-kernel/src/tessellation/mod.rs:85:pub fn tessellate_solid(solid: &Solid) -> Result<TriangleMesh, TessellationError> {
./crates/mycad-kernel/src/tessellation/mod.rs:86:    tessellate_solid_with(solid, &TessellationOptions::default())
./crates/mycad-kernel/src/tessellation/mod.rs:90:pub fn tessellate_solid_with(
./crates/mycad-kernel/src/tessellation/mod.rs:93:) -> Result<TriangleMesh, TessellationError> {
./crates/mycad-kernel/src/tessellation/mod.rs:94:    let mut mesh = TriangleMesh::new();
./crates/mycad-kernel/src/tessellation/mod.rs:121:    mesh: &mut TriangleMesh,
./crates/mycad-kernel/src/tessellation/mod.rs:156:    lp: &crate::brep::topology::Loop,
./crates/mycad-kernel/src/tessellation/mod.rs:185:    mesh: &mut TriangleMesh,
./crates/mycad-kernel/src/tessellation/mod.rs:277:fn push_triangle(mesh: &mut TriangleMesh, i0: u32, i1: u32, i2: u32) {
./crates/mycad-kernel/src/tessellation/mod.rs:313:    use crate::brep::topology::IdGenerator;
./crates/mycad-kernel/src/tessellation/mod.rs:315:    use crate::primitives::make_cuboid;
./crates/mycad-kernel/src/tessellation/mod.rs:316:    use crate::primitives::make_cylinder;
./crates/mycad-kernel/src/tessellation/mod.rs:321:        let mut gen = IdGenerator::new(0);
./crates/mycad-kernel/src/tessellation/mod.rs:322:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:340:        let mut gen1 = IdGenerator::new(0);
./crates/mycad-kernel/src/tessellation/mod.rs:341:        let mut gen2 = IdGenerator::new(0);
./crates/mycad-kernel/src/tessellation/mod.rs:343:        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:344:        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:357:        let mut gen = IdGenerator::new(0);
./crates/mycad-kernel/src/tessellation/mod.rs:358:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
./crates/mycad-kernel/src/tessellation/mod.rs:370:        let mut gen1 = IdGenerator::new(0);
./crates/mycad-kernel/src/tessellation/mod.rs:371:        let mut gen2 = IdGenerator::new(0);
./crates/mycad-kernel/src/tessellation/mod.rs:373:        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
./crates/mycad-kernel/src/tessellation/mod.rs:374:        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
./crates/mycad-kernel/src/tessellation/mod.rs:387:        let mut gen = IdGenerator::new(0);
./crates/mycad-kernel/src/tessellation/mod.rs:388:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:441:        let mut gen = IdGenerator::new(0);
./crates/mycad-kernel/src/tessellation/mod.rs:442:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:445:        let mesh_low = tessellate_solid_with(&solid, &TessellationOptions::new(8, 1)).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:446:        let mesh_hi = tessellate_solid_with(&solid, &TessellationOptions::new(64, 2)).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:499:        let mut id_gen = IdGenerator::new(0);
./crates/mycad-kernel/src/tessellation/mod.rs:500:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
./crates/mycad-kernel/src/tessellation/mod.rs:511:        let mut id_gen1 = IdGenerator::new(0);
./crates/mycad-kernel/src/tessellation/mod.rs:512:        let mut id_gen2 = IdGenerator::new(0);
./crates/mycad-kernel/src/tessellation/mod.rs:514:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
./crates/mycad-kernel/src/tessellation/mod.rs:515:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
./crates/mycad-kernel/src/tessellation/mod.rs:525:    /// (public fields bypass new() clamping, so tessellate_solid_with must guard at point of use).
./crates/mycad-kernel/src/tessellation/mod.rs:528:        let mut gen = IdGenerator::new(0);
./crates/mycad-kernel/src/tessellation/mod.rs:529:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:534:        let mesh = tessellate_solid_with(&solid, &opts).unwrap();
./crates/mycad-kernel/src/tessellation/stl.rs:1:use super::TriangleMesh;
./crates/mycad-kernel/src/tessellation/stl.rs:4:pub fn to_ascii_stl(mesh: &TriangleMesh, solid_name: &str) -> String {
./crates/mycad-kernel/src/tessellation/stl.rs:51:    use crate::brep::topology::IdGenerator;
./crates/mycad-kernel/src/tessellation/stl.rs:52:    use crate::primitives::make_cuboid;
./crates/mycad-kernel/src/tessellation/stl.rs:57:        let mut g = IdGenerator::new(0);
./crates/mycad-kernel/src/tessellation/stl.rs:58:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
./crates/mycad-kernel/src/tessellation/stl.rs:74:            let mut g = IdGenerator::new(0);
./crates/mycad-kernel/src/tessellation/stl.rs:75:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
./features/18-api-skeleton/design-review.md:22:- `IdGenerator` を使った決定的 ID 生成になっているか
./features/18-api-skeleton/design-review.md:27:- Euler-Poincaré の公式 `V - E + F = 2(S - H)` が成立する設計か（S=殻の数, H=貫通穴）
./features/18-api-skeleton/design-review.md:28:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
./features/18-api-skeleton/design-review.md:29:- Loop/Shell/Solid の入れ子構造が一貫しているか
./features/18-api-skeleton/design-review.md:32:- ゼロ長エッジ、面積ゼロの Face、縮退した Loop の排除または明示的エラーの設計があるか
./features/18-api-skeleton/design-review.md:37:- 新しい依存は `[workspace.dependencies]` + `{ workspace = true }` 規約に従っているか
./features/18-api-skeleton/design-review.md:38:- ライブラリエラーには `thiserror` を使用しているか
./features/18-api-skeleton/design-review.md:39:- カーネルにレンダリング依存が混入していないか（`TriangleMesh` 生成のみ可）
./features/18-api-skeleton/design-review.md:43:- Euler-Poincaré 検証テストがあるか
./features/18-api-skeleton/design-review.md:61:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
./features/18-api-skeleton/design-review.md:62:    suggestion: "IdGenerator::next() に置き換えること"
./features/18-api-skeleton/design-review.md:79:Phase 2「Web ベースビューア基盤」の土台として、CLI / GUI / AI 共通の操作面 v0 を Rust HTTP サーバとして立てる。`.mycad` を読み込み、テッセレーションした `TriangleMesh` を JSON で返す最小 API を `crates/mycad-api/` に新規実装する。後続 Issue #20(Three.js フロント)・#21(`mycad view`)がこの API を消費する前提。
./features/18-api-skeleton/design-review.md:81:既存 CLI の `run_export`(`crates/mycad-cli/src/main.rs:47-67`)が同じ Document→Solid→Mesh フローを既に持つため、STL 化を除いて再利用する。
./features/18-api-skeleton/design-review.md:92:| `Cargo.toml`(ルート) | members に `"crates/mycad-api"` 追加。`[workspace.dependencies]` に `axum`/`tokio`/`serde_json`/`tower`(dev) 追加 |
./features/18-api-skeleton/design-review.md:107:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
./features/18-api-skeleton/design-review.md:116:- `Document::from_path(&Path) -> Result<Document, FormatError>`(`crates/mycad-format/src/document.rs:36`)。`.mycad` 拡張子を強制(InvalidExtension)。
./features/18-api-skeleton/design-review.md:117:- `build_solid_from_features(&[Feature], &mut IdGenerator) -> Result<Solid, KernelError>`(`crates/mycad-build/src/lib.rs:6`)。単一フィーチャ・Box/Cylinder のみ対応。
./features/18-api-skeleton/design-review.md:118:- `tessellate_solid_with(&Solid, &TessellationOptions) -> Result<TriangleMesh, TessellationError>`(`crates/mycad-kernel/src/tessellation/mod.rs:90`)。v0 は `TessellationOptions::default()` 固定。
./features/18-api-skeleton/design-review.md:119:- `TriangleMesh`(同 mod.rs:16)は `Serialize` 済み → `Json(mesh)` でそのまま返却。
./features/18-api-skeleton/design-review.md:120:- ハンドラ本体は CLI `run_export`(main.rs:47-67)から STL 化を除き、末尾を `Ok(Json(mesh))` に置換した形。
./features/18-api-skeleton/design-review.md:128:2. **`.mycad` 拡張子限定**(`Document::from_path` が既に強制)→ `/etc/passwd` 等の機密ファイル読取を遮断。
./features/18-api-skeleton/design-review.md:138:- `[workspace.dependencies]` に追加(前例ゼロなので新規登録): `axum = "0.8"`、`tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }`、`serde_json = "1"`、`tower = "0.5"`(dev、`oneshot` 用)。各クレートは `{ workspace = true }` で参照。
./features/18-api-skeleton/design-review.md:139:- derive 規約・`thiserror`・`version.workspace = true` 等の既存慣例を踏襲。
./features/18-api-skeleton/design-review.md:146:| T01 | 正常系 | `GET /api/v0/mesh?file=examples/simple_box.mycad` を oneshot で叩く | 200、body を `TriangleMesh` に deser でき `positions`/`indices` が非空、`indices.len() % 3 == 0` |
./features/18-api-skeleton/design-review.md:149:| T04 | 400 | 非 `.mycad` 拡張子 `file=Cargo.toml` | 400(InvalidExtension) |
./features/18-api-skeleton/design-review.md:161:# → TriangleMesh JSON(positions/normals/indices)が返ること
./features/18-api-skeleton/design-review.md:213:/bin/bash -lc 'rg -n "struct IdGenerator|pub struct IdGenerator|run_export|build_solid_from_features|tessellate_solid_with|struct TriangleMesh|enum FormatError|pub enum FormatError|JsonSchema|thiserror|workspace.dependencies|mod.rs|HalfEdge|Euler|Serialize|Deserialize" -S Cargo.toml crates' in /home/bacon/mycad
./features/18-api-skeleton/design-review.md:215:Cargo.toml:18:[workspace.dependencies]
./features/18-api-skeleton/design-review.md:216:Cargo.toml:30:thiserror = "2"
./features/18-api-skeleton/design-review.md:217:crates/mycad-format/Cargo.toml:12:thiserror = { workspace = true }
./features/18-api-skeleton/design-review.md:224:crates/mycad-format/src/component.rs:12:    /// Rotation as Euler angles in degrees [rx, ry, rz].
./features/18-api-skeleton/design-review.md:232:crates/mycad-format/src/error.rs:1:use thiserror::Error;
./features/18-api-skeleton/design-review.md:239:crates/mycad-kernel/CLAUDE.md:14:- すべてのEdgeは正確に2つのHalfEdgeを持つ
./features/18-api-skeleton/design-review.md:240:crates/mycad-kernel/CLAUDE.md:16:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
./features/18-api-skeleton/design-review.md:241:crates/mycad-kernel/CLAUDE.md:17:- Loopは閉じたHalfEdge列（最後のHalfEdgeの終点 = 最初のHalfEdgeの始点）
./features/18-api-skeleton/design-review.md:243:crates/mycad-kernel/Cargo.toml:11:thiserror = { workspace = true }
./features/18-api-skeleton/design-review.md:244:crates/mycad-build/src/lib.rs:6:pub fn build_solid_from_features(
./features/18-api-skeleton/design-review.md:245:crates/mycad-cli/src/main.rs:2:use mycad_build::build_solid_from_features;
./features/18-api-skeleton/design-review.md:246:crates/mycad-cli/src/main.rs:5:use mycad_kernel::tessellation::{tessellate_solid_with, to_ascii_stl, TessellationOptions};
./features/18-api-skeleton/design-review.md:247:crates/mycad-cli/src/main.rs:39:            if let Err(msg) = run_export(&input, &output, segments) {
./features/18-api-skeleton/design-review.md:248:crates/mycad-cli/src/main.rs:47:fn run_export(
./features/18-api-skeleton/design-review.md:249:crates/mycad-cli/src/main.rs:55:    let solid = build_solid_from_features(&doc.root_component.features, &mut gen)
./features/18-api-skeleton/design-review.md:250:crates/mycad-cli/src/main.rs:62:        tessellate_solid_with(&solid, &opts).map_err(|e| format!("failed to tessellate: {e}"))?;
./features/18-api-skeleton/design-review.md:251:crates/mycad-kernel/src/error.rs:1:use thiserror::Error;
./features/18-api-skeleton/design-review.md:253:crates/mycad-kernel/src/tessellation/mod.rs:10:use thiserror::Error;
./features/18-api-skeleton/design-review.md:255:crates/mycad-kernel/src/tessellation/mod.rs:18:pub struct TriangleMesh {
./features/18-api-skeleton/design-review.md:257:crates/mycad-kernel/src/tessellation/mod.rs:86:    tessellate_solid_with(solid, &TessellationOptions::default())
./features/18-api-skeleton/design-review.md:258:crates/mycad-kernel/src/tessellation/mod.rs:90:pub fn tessellate_solid_with(
./features/18-api-skeleton/design-review.md:259:crates/mycad-kernel/src/tessellation/mod.rs:445:        let mesh_low = tessellate_solid_with(&solid, &TessellationOptions::new(8, 1)).unwrap();
./features/18-api-skeleton/design-review.md:260:crates/mycad-kernel/src/tessellation/mod.rs:446:        let mesh_hi = tessellate_solid_with(&solid, &TessellationOptions::new(64, 2)).unwrap();
./features/18-api-skeleton/design-review.md:261:crates/mycad-kernel/src/tessellation/mod.rs:525:    /// (public fields bypass new() clamping, so tessellate_solid_with must guard at point of use).
./features/18-api-skeleton/design-review.md:262:crates/mycad-kernel/src/tessellation/mod.rs:534:        let mesh = tessellate_solid_with(&solid, &opts).unwrap();
./features/18-api-skeleton/design-review.md:266:crates/mycad-kernel/src/brep/topology.rs:21:pub struct HalfEdge {
./features/18-api-skeleton/design-review.md:272:crates/mycad-kernel/src/brep/topology.rs:83:    pub half_edges: Vec<HalfEdge>,
./features/18-api-skeleton/design-review.md:273:crates/mycad-kernel/src/brep/topology.rs:138:        self.half_edges.push(HalfEdge {
./features/18-api-skeleton/design-review.md:274:crates/mycad-kernel/src/brep/topology.rs:185:pub struct IdGenerator {
./features/18-api-skeleton/design-review.md:275:crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
./features/18-api-skeleton/design-review.md:276:crates/mycad-kernel/src/primitives/cylinder.rs:180:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
./features/18-api-skeleton/design-review.md:277:crates/mycad-kernel/src/primitives/cylinder.rs:195:            "Euler-Poincaré V-E+F = 2(S-H), S={shells}, H=0"
./features/18-api-skeleton/design-review.md:278:crates/mycad-build/tests/feature_dispatcher.rs:1:use mycad_build::build_solid_from_features;
./features/18-api-skeleton/design-review.md:279:crates/mycad-build/tests/feature_dispatcher.rs:17:        build_solid_from_features(&doc.root_component.features, &mut g).expect("build solid");
./features/18-api-skeleton/design-review.md:312:pub struct HalfEdge {
./features/18-api-skeleton/design-review.md:337:pub struct Loop {
./features/18-api-skeleton/design-review.md:360:pub struct Shell {
./features/18-api-skeleton/design-review.md:374:    pub half_edges: Vec<HalfEdge>,
./features/18-api-skeleton/design-review.md:376:    pub loops: Vec<Loop>,
./features/18-api-skeleton/design-review.md:378:    pub shells: Vec<Shell>,
./features/18-api-skeleton/design-review.md:429:        self.half_edges.push(HalfEdge {
./features/18-api-skeleton/design-review.md:441:        self.loops.push(Loop { id, half_edges });
./features/18-api-skeleton/design-review.md:468:        self.shells.push(Shell { id, faces, closed });
./features/18-api-skeleton/design-review.md:476:pub struct IdGenerator {
./features/18-api-skeleton/design-review.md:480:impl IdGenerator {
./features/18-api-skeleton/design-review.md:499:        let mut gen1 = IdGenerator::new(100);
./features/18-api-skeleton/design-review.md:500:        let mut gen2 = IdGenerator::new(100);
./features/18-api-skeleton/design-review.md:517:use mycad_build::build_solid_from_features;
./features/18-api-skeleton/design-review.md:519:use mycad_kernel::brep::topology::IdGenerator;
./features/18-api-skeleton/design-review.md:520:use mycad_kernel::tessellation::{tessellate_solid_with, to_ascii_stl, TessellationOptions};
./features/18-api-skeleton/design-review.md:554:            if let Err(msg) = run_export(&input, &output, segments) {
./features/18-api-skeleton/design-review.md:562:fn run_export(
./features/18-api-skeleton/design-review.md:567:    let doc = Document::from_path(input)
./features/18-api-skeleton/design-review.md:569:    let mut gen = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:570:    let solid = build_solid_from_features(&doc.root_component.features, &mut gen)
./features/18-api-skeleton/design-review.md:577:        tessellate_solid_with(&solid, &opts).map_err(|e| format!("failed to tessellate: {e}"))?;
./features/18-api-skeleton/design-review.md:588:use mycad_kernel::brep::topology::{IdGenerator, Solid};
./features/18-api-skeleton/design-review.md:590:use mycad_kernel::primitives::{make_cuboid, make_cylinder};
./features/18-api-skeleton/design-review.md:592:pub fn build_solid_from_features(
./features/18-api-skeleton/design-review.md:594:    gen: &mut IdGenerator,
./features/18-api-skeleton/design-review.md:612:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
./features/18-api-skeleton/design-review.md:613:        Feature::CreateCylinder { radius, height, .. } => make_cylinder(*radius, *height, gen),
./features/18-api-skeleton/design-review.md:665:            return Err(FormatError::InvalidExtension(ext.map(str::to_string)));
./features/18-api-skeleton/design-review.md:722:        let doc = Document::from_path(&path).unwrap();
./features/18-api-skeleton/design-review.md:730:        let err = Document::from_path(path).unwrap_err();
./features/18-api-skeleton/design-review.md:737:        let err = Document::from_path(path).unwrap_err();
./features/18-api-skeleton/design-review.md:738:        assert!(matches!(err, FormatError::InvalidExtension(Some(ref e)) if e == "txt"));
./features/18-api-skeleton/design-review.md:742:    fn test_assembly_roundtrip() {
./features/18-api-skeleton/design-review.md:750:        let doc = Document::from_path(&path).unwrap();
./features/18-api-skeleton/design-review.md:773:use thiserror::Error;
./features/18-api-skeleton/design-review.md:784:    InvalidExtension(Option<String>),
./features/18-api-skeleton/design-review.md:802:use thiserror::Error;
./features/18-api-skeleton/design-review.md:810:pub struct TriangleMesh {
./features/18-api-skeleton/design-review.md:819:impl TriangleMesh {
./features/18-api-skeleton/design-review.md:834:impl Default for TriangleMesh {
./features/18-api-skeleton/design-review.md:873:    NonManifoldLoop,
./features/18-api-skeleton/design-review.md:877:pub fn tessellate_solid(solid: &Solid) -> Result<TriangleMesh, TessellationError> {
./features/18-api-skeleton/design-review.md:878:    tessellate_solid_with(solid, &TessellationOptions::default())
./features/18-api-skeleton/design-review.md:882:pub fn tessellate_solid_with(
./features/18-api-skeleton/design-review.md:885:) -> Result<TriangleMesh, TessellationError> {
./features/18-api-skeleton/design-review.md:886:    let mut mesh = TriangleMesh::new();
./features/18-api-skeleton/design-review.md:913:    mesh: &mut TriangleMesh,
./features/18-api-skeleton/design-review.md:948:    lp: &crate::brep::topology::Loop,
./features/18-api-skeleton/design-review.md:977:    mesh: &mut TriangleMesh,
./features/18-api-skeleton/design-review.md:1074:fn push_triangle(mesh: &mut TriangleMesh, i0: u32, i1: u32, i2: u32) {
./features/18-api-skeleton/design-review.md:1110:    use crate::brep::topology::IdGenerator;
./features/18-api-skeleton/design-review.md:1112:    use crate::primitives::make_cuboid;
./features/18-api-skeleton/design-review.md:1113:    use crate::primitives::make_cylinder;
./features/18-api-skeleton/design-review.md:1118:        let mut gen = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:1119:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/18-api-skeleton/design-review.md:1137:        let mut gen1 = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:1138:        let mut gen2 = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:1140:        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
./features/18-api-skeleton/design-review.md:1141:        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
./features/18-api-skeleton/design-review.md:1154:        let mut gen = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:1155:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
./features/18-api-skeleton/design-review.md:1167:        let mut gen1 = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:1168:        let mut gen2 = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:1170:        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
./features/18-api-skeleton/design-review.md:1171:        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
./features/18-api-skeleton/design-review.md:1184:        let mut gen = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:1185:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/18-api-skeleton/design-review.md:1238:        let mut gen = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:1239:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/18-api-skeleton/design-review.md:1242:        let mesh_low = tessellate_solid_with(&solid, &TessellationOptions::new(8, 1)).unwrap();
./features/18-api-skeleton/design-review.md:1243:        let mesh_hi = tessellate_solid_with(&solid, &TessellationOptions::new(64, 2)).unwrap();
./features/18-api-skeleton/design-review.md:1296:        let mut id_gen = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:1297:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
./features/18-api-skeleton/design-review.md:1308:        let mut id_gen1 = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:1309:        let mut id_gen2 = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:1311:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
./features/18-api-skeleton/design-review.md:1312:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
./features/18-api-skeleton/design-review.md:1322:use crate::brep::topology::{IdGenerator, Solid};
./features/18-api-skeleton/design-review.md:1337:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
./features/18-api-skeleton/design-review.md:1544:    use crate::brep::topology::IdGenerator;
./features/18-api-skeleton/design-review.md:1548:        let mut id_gen = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:1549:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
./features/18-api-skeleton/design-review.md:1568:        let mut id_gen = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:1569:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
./features/18-api-skeleton/design-review.md:1586:use crate::brep::topology::{IdGenerator, Solid};
./features/18-api-skeleton/design-review.md:1596:pub fn make_cylinder(
./features/18-api-skeleton/design-review.md:1599:    id_gen: &mut IdGenerator,
./features/18-api-skeleton/design-review.md:1721:        let mut gen1 = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:1722:        let mut gen2 = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:1723:        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
./features/18-api-skeleton/design-review.md:1724:        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
./features/18-api-skeleton/design-review.md:1749:        let mut gen = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:1750:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/18-api-skeleton/design-review.md:1765:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
./features/18-api-skeleton/design-review.md:1768:        let mut gen = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:1769:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/18-api-skeleton/design-review.md:1780:            "Euler-Poincaré V-E+F = 2(S-H), S={shells}, H=0"
./features/18-api-skeleton/design-review.md:1788:        let mut gen = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:1789:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/18-api-skeleton/design-review.md:1806:        // Loop closure: each loop's consecutive HEs must connect geometrically
./features/18-api-skeleton/design-review.md:1842:        let mut gen = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:1843:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/18-api-skeleton/design-review.md:1887:        let mut gen = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:1890:        let err = make_cylinder(0.0, 20.0, &mut gen).unwrap_err();
./features/18-api-skeleton/design-review.md:1897:        let err = make_cylinder(-5.0, 20.0, &mut gen).unwrap_err();
./features/18-api-skeleton/design-review.md:1904:        let err = make_cylinder(f64::NAN, 20.0, &mut gen).unwrap_err();
./features/18-api-skeleton/design-review.md:1927:[workspace.dependencies]
./features/18-api-skeleton/design-review.md:1939:thiserror = "2"
./features/18-api-skeleton/design-review.md:1977:- すべてのEdgeは正確に2つのHalfEdgeを持つ
./features/18-api-skeleton/design-review.md:1978:- 閉じたShellはwatertight（水密）
./features/18-api-skeleton/design-review.md:1979:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
./features/18-api-skeleton/design-review.md:1980:- Loopは閉じたHalfEdge列（最後のHalfEdgeの終点 = 最初のHalfEdgeの始点）
./features/18-api-skeleton/design-review.md:1981:- EntityIDは `IdGenerator` で決定的に生成される
./features/18-api-skeleton/design-review.md:1986:2. 関数シグネチャ: `pub fn make_xxx(params..., id_gen: &mut IdGenerator) -> Solid`
./features/18-api-skeleton/design-review.md:1988:4. 決定性テストを必ず書く（同じ `IdGenerator` 初期値で2回生成し、結果が同一であることを確認）
./features/18-api-skeleton/design-review.md:2185:    /// Rotation as Euler angles in degrees [rx, ry, rz].
./features/18-api-skeleton/design-review.md:2356:use mycad_build::build_solid_from_features;
./features/18-api-skeleton/design-review.md:2358:use mycad_kernel::brep::topology::IdGenerator;
./features/18-api-skeleton/design-review.md:2369:    let doc = Document::from_path(&path).expect("load .mycad");
./features/18-api-skeleton/design-review.md:2370:    let mut g = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:2372:        build_solid_from_features(&doc.root_component.features, &mut g).expect("build solid");
./features/18-api-skeleton/design-review.md:2401:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
./features/18-api-skeleton/design-review.md:2442:./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
./features/18-api-skeleton/design-review.md:2450:./features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
./features/18-api-skeleton/design-review.md:2458:./features/12-cylinder/design-review.md:523:crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
./features/18-api-skeleton/design-review.md:2460:./features/12-cylinder/design-review.md:525:crates/mycad-kernel/src/tessellation/mod.rs:10:pub struct TriangleMesh {
./features/18-api-skeleton/design-review.md:2461:./features/12-cylinder/design-review.md:526:crates/mycad-kernel/src/tessellation/mod.rs:19:impl TriangleMesh {
./features/18-api-skeleton/design-review.md:2462:./features/12-cylinder/design-review.md:527:crates/mycad-kernel/src/tessellation/mod.rs:34:impl Default for TriangleMesh {
./features/18-api-skeleton/design-review.md:2463:./features/12-cylinder/design-review.md:528:crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/18-api-skeleton/design-review.md:2464:./features/12-cylinder/design-review.md:529:crates/mycad-kernel/src/tessellation/mod.rs:50:    let mut mesh = TriangleMesh::new();
./features/18-api-skeleton/design-review.md:2465:./features/12-cylinder/design-review.md:530:crates/mycad-kernel/src/tessellation/mod.rs:98:    use crate::brep::topology::IdGenerator;
./features/18-api-skeleton/design-review.md:2466:./features/12-cylinder/design-review.md:531:crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
./features/18-api-skeleton/design-review.md:2467:./features/12-cylinder/design-review.md:532:crates/mycad-kernel/src/tessellation/mod.rs:103:        let mut id_gen = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:2468:./features/12-cylinder/design-review.md:533:crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
./features/18-api-skeleton/design-review.md:2470:./features/12-cylinder/design-review.md:535:crates/mycad-kernel/src/tessellation/mod.rs:117:        let mut id_gen1 = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:2471:./features/12-cylinder/design-review.md:536:crates/mycad-kernel/src/tessellation/mod.rs:118:        let mut id_gen2 = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:2472:./features/12-cylinder/design-review.md:537:crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
./features/18-api-skeleton/design-review.md:2473:./features/12-cylinder/design-review.md:538:crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
./features/18-api-skeleton/design-review.md:2476:./features/12-cylinder/design-review.md:566:crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
./features/18-api-skeleton/design-review.md:2501:./features/12-cylinder/final-review-2.md:1379:/bin/bash -lc "rg -n \"triangle_count\\(|make_cylinder|tessellate_solid_with\\(&solid, &TessellationOptions|zero_segments|partial_revolution\" crates/mycad-kernel/src/tessellation/mod.rs crates/mycad-kernel/src/primitives -g'*.rs'" in /home/bacon/mycad
./features/18-api-skeleton/design-review.md:2503:./features/12-cylinder/final-review-2.md:1382:crates/mycad-kernel/src/tessellation/mod.rs:306:    use crate::primitives::make_cylinder;
./features/18-api-skeleton/design-review.md:2505:./features/12-cylinder/final-review-2.md:1384:crates/mycad-kernel/src/tessellation/mod.rs:312:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/18-api-skeleton/design-review.md:2508:./features/12-cylinder/final-review-2.md:1387:crates/mycad-kernel/src/tessellation/mod.rs:333:        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
./features/18-api-skeleton/design-review.md:2509:./features/12-cylinder/final-review-2.md:1388:crates/mycad-kernel/src/tessellation/mod.rs:334:        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
./features/18-api-skeleton/design-review.md:2511:./features/12-cylinder/final-review-2.md:1390:crates/mycad-kernel/src/tessellation/mod.rs:378:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/18-api-skeleton/design-review.md:2513:./features/12-cylinder/final-review-2.md:1392:crates/mycad-kernel/src/tessellation/mod.rs:432:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/18-api-skeleton/design-review.md:2514:./features/12-cylinder/final-review-2.md:1393:crates/mycad-kernel/src/tessellation/mod.rs:435:        let mesh_low = tessellate_solid_with(&solid, &TessellationOptions::new(8, 1)).unwrap();
./features/18-api-skeleton/design-review.md:2515:./features/12-cylinder/final-review-2.md:1394:crates/mycad-kernel/src/tessellation/mod.rs:436:        let mesh_hi = tessellate_solid_with(&solid, &TessellationOptions::new(64, 2)).unwrap();
./features/18-api-skeleton/design-review.md:2522:./features/12-cylinder/final-review-2.md:1401:crates/mycad-kernel/src/tessellation/mod.rs:519:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./features/18-api-skeleton/design-review.md:2525:./features/12-cylinder/final-review-2.md:1404:crates/mycad-kernel/src/primitives/mod.rs:5:pub use cylinder::make_cylinder;
./features/18-api-skeleton/design-review.md:2539:./features/12-cylinder/design-review-3.md:223:/bin/bash -lc "rg -n \"make_cuboid|tessellate_solid|IdGenerator|KernelError|HalfEdge|inner_loops|Surface::|Curve::|TriangleMesh|Feature::CreateCylinder|CreateCylinder|UnsupportedFeature|InvalidParameter|mod.rs|pub use\" crates/mycad-kernel crates/mycad-build crates/mycad-cli -g '"'!target'"'" in /home/bacon/mycad
./features/18-api-skeleton/design-review.md:2543:./features/12-cylinder/design-review-3.md:254:crates/mycad-kernel/src/tessellation/mod.rs:10:pub struct TriangleMesh {
./features/18-api-skeleton/design-review.md:2544:./features/12-cylinder/design-review-3.md:255:crates/mycad-kernel/src/tessellation/mod.rs:19:impl TriangleMesh {
./features/18-api-skeleton/design-review.md:2545:./features/12-cylinder/design-review-3.md:256:crates/mycad-kernel/src/tessellation/mod.rs:34:impl Default for TriangleMesh {
./features/18-api-skeleton/design-review.md:2546:./features/12-cylinder/design-review-3.md:257:crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/18-api-skeleton/design-review.md:2547:./features/12-cylinder/design-review-3.md:258:crates/mycad-kernel/src/tessellation/mod.rs:50:    let mut mesh = TriangleMesh::new();
./features/18-api-skeleton/design-review.md:2548:./features/12-cylinder/design-review-3.md:259:crates/mycad-kernel/src/tessellation/mod.rs:98:    use crate::brep::topology::IdGenerator;
./features/18-api-skeleton/design-review.md:2549:./features/12-cylinder/design-review-3.md:260:crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
./features/18-api-skeleton/design-review.md:2550:./features/12-cylinder/design-review-3.md:261:crates/mycad-kernel/src/tessellation/mod.rs:103:        let mut id_gen = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:2551:./features/12-cylinder/design-review-3.md:262:crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
./features/18-api-skeleton/design-review.md:2553:./features/12-cylinder/design-review-3.md:264:crates/mycad-kernel/src/tessellation/mod.rs:117:        let mut id_gen1 = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:2554:./features/12-cylinder/design-review-3.md:265:crates/mycad-kernel/src/tessellation/mod.rs:118:        let mut id_gen2 = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:2555:./features/12-cylinder/design-review-3.md:266:crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
./features/18-api-skeleton/design-review.md:2556:./features/12-cylinder/design-review-3.md:267:crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
./features/18-api-skeleton/design-review.md:2560:./features/12-cylinder/design-review-3.md:319:crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
./features/18-api-skeleton/design-review.md:2561:./features/12-cylinder/design-review-3.md:350:crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
./features/18-api-skeleton/design-review.md:2574:./features/12-cylinder/design-review-3.md:1625:pub use cuboid::make_cuboid;
./features/18-api-skeleton/design-review.md:2583:./features/12-cylinder/design-review-2.md:514:pub use cuboid::make_cuboid;
./features/18-api-skeleton/design-review.md:2587:./features/12-cylinder/design-review-2.md:1041:crates/mycad-kernel/src/tessellation/mod.rs:98:    use crate::brep::topology::IdGenerator;
./features/18-api-skeleton/design-review.md:2588:./features/12-cylinder/design-review-2.md:1042:crates/mycad-kernel/src/tessellation/mod.rs:103:        let mut id_gen = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:2589:./features/12-cylinder/design-review-2.md:1043:crates/mycad-kernel/src/tessellation/mod.rs:117:        let mut id_gen1 = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:2590:./features/12-cylinder/design-review-2.md:1044:crates/mycad-kernel/src/tessellation/mod.rs:118:        let mut id_gen2 = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:2609:./features/12-cylinder/design-review-2.md:1863:./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
./features/18-api-skeleton/design-review.md:2617:./features/12-cylinder/design-review-2.md:1875:./features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
./features/18-api-skeleton/design-review.md:2625:./features/12-cylinder/design-review-2.md:1883:./features/12-cylinder/design-review.md:523:crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
./features/18-api-skeleton/design-review.md:2627:./features/12-cylinder/design-review-2.md:1885:./features/12-cylinder/design-review.md:525:crates/mycad-kernel/src/tessellation/mod.rs:10:pub struct TriangleMesh {
./features/18-api-skeleton/design-review.md:2628:./features/12-cylinder/design-review-2.md:1886:./features/12-cylinder/design-review.md:526:crates/mycad-kernel/src/tessellation/mod.rs:19:impl TriangleMesh {
./features/18-api-skeleton/design-review.md:2629:./features/12-cylinder/design-review-2.md:1887:./features/12-cylinder/design-review.md:527:crates/mycad-kernel/src/tessellation/mod.rs:34:impl Default for TriangleMesh {
./features/18-api-skeleton/design-review.md:2630:./features/12-cylinder/design-review-2.md:1888:./features/12-cylinder/design-review.md:528:crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/18-api-skeleton/design-review.md:2631:./features/12-cylinder/design-review-2.md:1889:./features/12-cylinder/design-review.md:529:crates/mycad-kernel/src/tessellation/mod.rs:50:    let mut mesh = TriangleMesh::new();
./features/18-api-skeleton/design-review.md:2632:./features/12-cylinder/design-review-2.md:1890:./features/12-cylinder/design-review.md:530:crates/mycad-kernel/src/tessellation/mod.rs:98:    use crate::brep::topology::IdGenerator;
./features/18-api-skeleton/design-review.md:2633:./features/12-cylinder/design-review-2.md:1891:./features/12-cylinder/design-review.md:531:crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
./features/18-api-skeleton/design-review.md:2634:./features/12-cylinder/design-review-2.md:1892:./features/12-cylinder/design-review.md:532:crates/mycad-kernel/src/tessellation/mod.rs:103:        let mut id_gen = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:2635:./features/12-cylinder/design-review-2.md:1893:./features/12-cylinder/design-review.md:533:crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
./features/18-api-skeleton/design-review.md:2637:./features/12-cylinder/design-review-2.md:1895:./features/12-cylinder/design-review.md:535:crates/mycad-kernel/src/tessellation/mod.rs:117:        let mut id_gen1 = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:2638:./features/12-cylinder/design-review-2.md:1896:./features/12-cylinder/design-review.md:536:crates/mycad-kernel/src/tessellation/mod.rs:118:        let mut id_gen2 = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:2639:./features/12-cylinder/design-review-2.md:1897:./features/12-cylinder/design-review.md:537:crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
./features/18-api-skeleton/design-review.md:2640:./features/12-cylinder/design-review-2.md:1898:./features/12-cylinder/design-review.md:538:crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
./features/18-api-skeleton/design-review.md:2643:./features/12-cylinder/design-review-2.md:1901:./features/12-cylinder/design-review.md:566:crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
./features/18-api-skeleton/design-review.md:2653:./features/12-cylinder/design-review-2.md:1924:./crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
./features/18-api-skeleton/design-review.md:2656:./features/12-cylinder/design-review-2.md:1928:./crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
./features/18-api-skeleton/design-review.md:2665:./features/12-cylinder/design-review-2.md:1941:./features/12-cylinder/design-review-2.md:514:pub use cuboid::make_cuboid;
./features/18-api-skeleton/design-review.md:2669:./features/12-cylinder/design-review-2.md:1945:./features/12-cylinder/design-review-2.md:1041:crates/mycad-kernel/src/tessellation/mod.rs:98:    use crate::brep::topology::IdGenerator;
./features/18-api-skeleton/design-review.md:2670:./features/12-cylinder/design-review-2.md:1946:./features/12-cylinder/design-review-2.md:1042:crates/mycad-kernel/src/tessellation/mod.rs:103:        let mut id_gen = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:2671:./features/12-cylinder/design-review-2.md:1947:./features/12-cylinder/design-review-2.md:1043:crates/mycad-kernel/src/tessellation/mod.rs:117:        let mut id_gen1 = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:2672:./features/12-cylinder/design-review-2.md:1948:./features/12-cylinder/design-review-2.md:1044:crates/mycad-kernel/src/tessellation/mod.rs:118:        let mut id_gen2 = IdGenerator::new(0);
./features/18-api-skeleton/design-review.md:2687:./features/12-cylinder/final-review.md:945: pub use cuboid::make_cuboid;
./features/18-api-skeleton/design-review.md:2688:./features/12-cylinder/final-review.md:946:+pub use cylinder::make_cylinder;
./features/18-api-skeleton/design-review.md:2696:./features/12-cylinder/final-review.md:2457:pub use cuboid::make_cuboid;
./features/18-api-skeleton/design-review.md:2697:./features/12-cylinder/final-review.md:2458:pub use cylinder::make_cylinder;
./features/18-api-skeleton/design-review.md:2704:./features/12-cylinder/final-review.md:3239:./features/12-cylinder/design-review-3.md:257:crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/18-api-skeleton/design-review.md:2708:./features/12-cylinder/final-review.md:3304:./features/12-cylinder/design-review-2.md:1888:./features/12-cylinder/design-review.md:528:crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/18-api-skeleton/design-review.md:2712:./features/12-cylinder/final-review.md:3315:./features/12-cylinder/design-review.md:528:crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/18-api-skeleton/design-review.md:2716:./features/12-cylinder/final-review.md:3336:./crates/mycad-kernel/src/tessellation/mod.rs:84:pub fn tessellate_solid(solid: &Solid) -> Result<TriangleMesh, TessellationError> {
./features/18-api-skeleton/design-review.md:2746:./features/18-api-skeleton/design-review.md:107:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
./features/18-api-skeleton/design-review.md:2749:./features/18-api-skeleton/design-review.md:118:- `tessellate_solid_with(&Solid, &TessellationOptions) -> Result<TriangleMesh, TessellationError>`(`crates/mycad-kernel/src/tessellation/mod.rs:90`)。v0 は `TessellationOptions::default()` 固定。
./features/18-api-skeleton/design-review.md:2750:./features/18-api-skeleton/design-review.md:119:- `TriangleMesh`(同 mod.rs:16)は `Serialize` 済み → `Json(mesh)` でそのまま返却。
./features/18-api-skeleton/design-review.md:2757:./features/18-api-skeleton/design-review.md:213:/bin/bash -lc 'rg -n "struct IdGenerator|pub struct IdGenerator|run_export|build_solid_from_features|tessellate_solid_with|struct TriangleMesh|enum FormatError|pub enum FormatError|JsonSchema|thiserror|workspace.dependencies|mod.rs|HalfEdge|Euler|Serialize|Deserialize" -S Cargo.toml crates' in /home/bacon/mycad
./features/18-api-skeleton/design-review.md:2760:./features/18-api-skeleton/design-review.md:253:crates/mycad-kernel/src/tessellation/mod.rs:10:use thiserror::Error;
./features/18-api-skeleton/design-review.md:2762:./features/18-api-skeleton/design-review.md:255:crates/mycad-kernel/src/tessellation/mod.rs:18:pub struct TriangleMesh {
./features/18-api-skeleton/design-review.md:2764:./features/18-api-skeleton/design-review.md:257:crates/mycad-kernel/src/tessellation/mod.rs:86:    tessellate_solid_with(solid, &TessellationOptions::default())
./features/18-api-skeleton/design-review.md:2765:./features/18-api-skeleton/design-review.md:258:crates/mycad-kernel/src/tessellation/mod.rs:90:pub fn tessellate_solid_with(
./features/18-api-skeleton/design-review.md:2766:./features/18-api-skeleton/design-review.md:259:crates/mycad-kernel/src/tessellation/mod.rs:445:        let mesh_low = tessellate_solid_with(&solid, &TessellationOptions::new(8, 1)).unwrap();
./features/18-api-skeleton/design-review.md:2767:./features/18-api-skeleton/design-review.md:260:crates/mycad-kernel/src/tessellation/mod.rs:446:        let mesh_hi = tessellate_solid_with(&solid, &TessellationOptions::new(64, 2)).unwrap();
./features/18-api-skeleton/design-review.md:2768:./features/18-api-skeleton/design-review.md:261:crates/mycad-kernel/src/tessellation/mod.rs:525:    /// (public fields bypass new() clamping, so tessellate_solid_with must guard at point of use).
./features/18-api-skeleton/design-review.md:2769:./features/18-api-skeleton/design-review.md:262:crates/mycad-kernel/src/tessellation/mod.rs:534:        let mesh = tessellate_solid_with(&solid, &opts).unwrap();
./features/18-api-skeleton/design-review.md:2770:./features/18-api-skeleton/design-review.md:275:crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
./features/18-api-skeleton/design-review.md:2780:./features/18-api-skeleton/plan.md:33:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
./features/18-api-skeleton/design-review.md:2783:./features/18-api-skeleton/plan.md:44:- `tessellate_solid_with(&Solid, &TessellationOptions) -> Result<TriangleMesh, TessellationError>`(`crates/mycad-kernel/src/tessellation/mod.rs:90`)。v0 は `TessellationOptions::default()` 固定。
./features/18-api-skeleton/design-review.md:2784:./features/18-api-skeleton/plan.md:45:- `TriangleMesh`(同 mod.rs:16)は `Serialize` 済み → `Json(mesh)` でそのまま返却。
./features/18-api-skeleton/design-review.md:2787:./crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
./features/18-api-skeleton/design-review.md:2788:./crates/mycad-kernel/src/primitives/mod.rs:4:pub use cuboid::make_cuboid;
./features/18-api-skeleton/design-review.md:2789:./crates/mycad-kernel/src/primitives/mod.rs:5:pub use cylinder::make_cylinder;
./features/18-api-skeleton/design-review.md:2822:│   ├── brep/         トポロジー (Solid, Shell, Face, Loop, Edge, HalfEdge, Vertex)
./features/18-api-skeleton/design-review.md:2824:│   ├── primitives/   プリミティブ生成 (make_cuboid)
./features/18-api-skeleton/design-review.md:2825:│   └── tessellation/ メッシュ化 (tessellate_solid → TriangleMesh)
./features/18-api-skeleton/design-review.md:2848:- **決定性**: すべてのEntityIDは `IdGenerator` で決定的に生成。テストでは決定性を検証すること。
./features/18-api-skeleton/design-review.md:2849:- **依存管理**: 新しい依存は必ず `[workspace.dependencies]` に追加し、各クレートは `{ workspace = true }` で参照。
./features/18-api-skeleton/design-review.md:2851:- **エラーハンドリング**: ライブラリエラーには `thiserror` を使用。
./features/18-api-skeleton/design-review.md:2859:- **Kernel has zero rendering deps**: カーネルはレンダリング非依存。`TriangleMesh` を生成し、ビューアが消費する。
./features/18-api-skeleton/design-review.md:2868:.claude/skills/3ai/agents/glm-implementer.md:25:- HalfEdge: `twin`/`next`/`prev` インデックスが循環的に正しく設定されているか確認
./features/18-api-skeleton/design-review.md:2869:.claude/skills/3ai/agents/codex-final-reviewer.md:14:- HalfEdge の twin/next/prev インデックスが循環的に正しく設定されているか
./features/18-api-skeleton/design-review.md:2870:.claude/skills/3ai/agents/codex-final-reviewer.md:48:    finding: "HalfEdge の twin インデックスが self を指している（ループ不整合）"
./features/18-api-skeleton/design-review.md:2871:features/12-cylinder/design-review.md:28:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
./features/18-api-skeleton/design-review.md:2874:features/12-cylinder/design-review.md:411:.claude/skills/3ai/agents/glm-implementer.md:25:- HalfEdge: `twin`/`next`/`prev` インデックスが循環的に正しく設定されているか確認
./features/18-api-skeleton/design-review.md:2875:features/12-cylinder/design-review.md:430:.claude/skills/3ai/agents/codex-final-reviewer.md:14:- HalfEdge の twin/next/prev インデックスが循環的に正しく設定されているか
./features/18-api-skeleton/design-review.md:2876:features/12-cylinder/design-review.md:433:.claude/skills/3ai/agents/codex-final-reviewer.md:48:    finding: "HalfEdge の twin インデックスが self を指している（ループ不整合）"
./features/18-api-skeleton/design-review.md:2877:features/12-cylinder/design-review.md:442:features/12-cylinder/design-review.md:28:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
./features/18-api-skeleton/design-review.md:2878:features/12-cylinder/design-review.md:474:.claude/skills/3ai/agents/codex-design-reviewer.md:15:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
./features/18-api-skeleton/design-review.md:2879:features/12-cylinder/design-review.md:590:crates/mycad-kernel/CLAUDE.md:16:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
./features/18-api-skeleton/design-review.md:2883:features/12-cylinder/design-review.md:2088:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
./features/18-api-skeleton/design-review.md:2886:features/12-cylinder/design-review.md:2469:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
./features/18-api-skeleton/design-review.md:2889:features/12-cylinder/design-review.md:2531:    suggestion: "`HalfEdge` に隣接インデックスを追加して決定的に構築・検証するか、少なくとも各 loop で consecutive half-edge の終点/始点一致を全件検証する派生テストを追加すること。"
./features/18-api-skeleton/design-review.md:2890:features/12-cylinder/design-review.md:2561:    suggestion: "`HalfEdge` に隣接インデックスを追加して決定的に構築・検証するか、少なくとも各 loop で consecutive half-edge の終点/始点一致を全件検証する派生テストを追加すること。"
./features/18-api-skeleton/design-review.md:2891:.claude/skills/3ai/agents/codex-design-reviewer.md:15:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
./features/18-api-skeleton/design-review.md:2895:features/12-cylinder/design-review-2.md:28:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
./features/18-api-skeleton/design-review.md:2898:features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
./features/18-api-skeleton/design-review.md:2909:features/12-cylinder/design-review-2.md:1936:./features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
./features/18-api-skeleton/design-review.md:2913:features/12-cylinder/design-review-2.md:2143:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
./features/18-api-skeleton/design-review.md:2916:features/12-cylinder/design-review-3.md:28:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
./features/18-api-skeleton/design-review.md:2920:features/12-cylinder/design-review-3.md:180:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
./features/18-api-skeleton/design-review.md:2922:features/12-cylinder/design-review-3.md:271:crates/mycad-kernel/CLAUDE.md:16:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
./features/18-api-skeleton/design-review.md:2924:features/12-cylinder/design-review-3.md:1598:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
./features/18-api-skeleton/design-review.md:2926:crates/mycad-kernel/CLAUDE.md:16:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
./features/18-api-skeleton/design-review.md:2927:features/18-api-skeleton/design-review.md:28:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
./features/18-api-skeleton/design-review.md:2930:features/18-api-skeleton/design-review.md:240:crates/mycad-kernel/CLAUDE.md:16:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
./features/18-api-skeleton/design-review.md:2932:features/18-api-skeleton/design-review.md:1979:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
./features/18-api-skeleton/design-review.md:2969:- `Solid` が最上位エンティティ、内部に Shell → Face → Loop → HalfEdge → Vertex の階層
./features/18-api-skeleton/design-review.md:3025:pub use cuboid::make_cuboid;
./features/18-api-skeleton/design-review.md:3026:pub use cylinder::make_cylinder;
./features/18-api-skeleton/design-review.md:3031:- カーネルにレンダリング依存が混入していないか（`TriangleMesh` 生成のみ可）
./features/18-api-skeleton/design-review.md:3035:- Euler-Poincaré 検証テストがあるか
./features/18-api-skeleton/design-review.md:3053:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
./features/18-api-skeleton/design-review.md:3054:    suggestion: "IdGenerator::next() に置き換えること"
./features/18-api-skeleton/design-review.md:3071:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
./features/18-api-skeleton/design-review.md:3072:    suggestion: "`sample_segment` を HalfEdge 向き込みの API に変更し、rev では `t1` から降順にサンプルするか、先頭点を必ず `HalfEdge.start_vertex` から取得すること。reversed line edge を含む cuboid 回帰テストも追加すること。"
./features/18-api-skeleton/design-review.md:3077:    suggestion: "primitive 生成時と `TessellationOptions` 構築時に下限チェックを入れ、違反時は `thiserror` ベースの `KernelError` で明示エラーにすること。CLI でも同じ境界値を拒否すること。"
./features/18-api-skeleton/design-review.md:3081:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
./features/18-api-skeleton/design-review.md:3082:    suggestion: "`HalfEdge` に隣接インデックスを追加して決定的に構築・検証するか、少なくとも各 loop で consecutive half-edge の終点/始点一致を全件検証する派生テストを追加すること。"
./features/18-api-skeleton/design-review.md:3092:    suggestion: "`Document::from_path` -> `to_yaml` -> `from_yaml` -> `to_yaml` の equality を `examples/cylinder.mycad` に対して追加し、`create_cylinder` の schema/serialize 回帰を固定すること。"
./features/18-api-skeleton/design-review.md:3101:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
./features/18-api-skeleton/design-review.md:3102:    suggestion: "`sample_segment` を HalfEdge 向き込みの API に変更し、rev では `t1` から降順にサンプルするか、先頭点を必ず `HalfEdge.start_vertex` から取得すること。reversed line edge を含む cuboid 回帰テストも追加すること。"
./features/18-api-skeleton/design-review.md:3107:    suggestion: "primitive 生成時と `TessellationOptions` 構築時に下限チェックを入れ、違反時は `thiserror` ベースの `KernelError` で明示エラーにすること。CLI でも同じ境界値を拒否すること。"
./features/18-api-skeleton/design-review.md:3111:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
./features/18-api-skeleton/design-review.md:3112:    suggestion: "`HalfEdge` に隣接インデックスを追加して決定的に構築・検証するか、少なくとも各 loop で consecutive half-edge の終点/始点一致を全件検証する派生テストを追加すること。"
./features/18-api-skeleton/design-review.md:3122:    suggestion: "`Document::from_path` -> `to_yaml` -> `from_yaml` -> `to_yaml` の equality を `examples/cylinder.mycad` に対して追加し、`create_cylinder` の schema/serialize 回帰を固定すること。"
./features/18-api-skeleton/design-review.md:3158:- `IdGenerator` を使った決定的 ID 生成になっているか
./features/18-api-skeleton/design-review.md:3163:- Euler-Poincaré の公式 `V - E + F = 2(S - H)` が成立する設計か（S=殻の数, H=貫通穴）
./features/18-api-skeleton/design-review.md:3164:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
./features/18-api-skeleton/design-review.md:3165:- Loop/Shell/Solid の入れ子構造が一貫しているか
./features/18-api-skeleton/design-review.md:3168:- ゼロ長エッジ、面積ゼロの Face、縮退した Loop の排除または明示的エラーの設計があるか
./features/18-api-skeleton/design-review.md:3173:- 新しい依存は `[workspace.dependencies]` + `{ workspace = true }` 規約に従っているか
./features/18-api-skeleton/design-review.md:3174:- ライブラリエラーには `thiserror` を使用しているか
./features/18-api-skeleton/design-review.md:3175:- カーネルにレンダリング依存が混入していないか（`TriangleMesh` 生成のみ可）
./features/18-api-skeleton/design-review.md:3179:- Euler-Poincaré 検証テストがあるか
./features/18-api-skeleton/design-review.md:3197:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
./features/18-api-skeleton/design-review.md:3198:    suggestion: "IdGenerator::next() に置き換えること"
./features/18-api-skeleton/design-review.md:3215:Phase 2「Web ベースビューア基盤」の土台として、CLI / GUI / AI 共通の操作面 v0 を Rust HTTP サーバとして立てる。`.mycad` を読み込み、テッセレーションした `TriangleMesh` を JSON で返す最小 API を `crates/mycad-api/` に新規実装する。後続 Issue #20(Three.js フロント)・#21(`mycad view`)がこの API を消費する前提。
./features/18-api-skeleton/design-review.md:3217:既存 CLI の `run_export`(`crates/mycad-cli/src/main.rs:47-67`)が同じ Document→Solid→Mesh フローを既に持つため、STL 化を除いて再利用する。
./features/18-api-skeleton/design-review.md:3228:| `Cargo.toml`(ルート) | members に `"crates/mycad-api"` 追加。`[workspace.dependencies]` に `axum`/`tokio`/`serde_json`/`tower`(dev) 追加 |
./features/18-api-skeleton/design-review.md:3243:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
./features/18-api-skeleton/design-review.md:3252:- `Document::from_path(&Path) -> Result<Document, FormatError>`(`crates/mycad-format/src/document.rs:36`)。`.mycad` 拡張子を強制(InvalidExtension)。
./features/18-api-skeleton/design-review.md:3253:- `build_solid_from_features(&[Feature], &mut IdGenerator) -> Result<Solid, KernelError>`(`crates/mycad-build/src/lib.rs:6`)。単一フィーチャ・Box/Cylinder のみ対応。
./features/18-api-skeleton/design-review.md:3254:- `tessellate_solid_with(&Solid, &TessellationOptions) -> Result<TriangleMesh, TessellationError>`(`crates/mycad-kernel/src/tessellation/mod.rs:90`)。v0 は `TessellationOptions::default()` 固定。
./features/18-api-skeleton/design-review.md:3255:- `TriangleMesh`(同 mod.rs:16)は `Serialize` 済み → `Json(mesh)` でそのまま返却。
./features/18-api-skeleton/design-review.md:3256:- ハンドラ本体は CLI `run_export`(main.rs:47-67)から STL 化を除き、末尾を `Ok(Json(mesh))` に置換した形。
./features/18-api-skeleton/design-review.md:3265:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
./features/18-api-skeleton/design-review.md:3266:    suggestion: "IdGenerator::next() に置き換えること"
./features/18-api-skeleton/design-review.md:3283:Phase 2「Web ベースビューア基盤」の土台として、CLI / GUI / AI 共通の操作面 v0 を Rust HTTP サーバとして立てる。`.mycad` を読み込み、テッセレーションした `TriangleMesh` を JSON で返す最小 API を `crates/mycad-api/` に新規実装する。後続 Issue #20(Three.js フロント)・#21(`mycad view`)がこの API を消費する前提。
./features/18-api-skeleton/design-review.md:3285:既存 CLI の `run_export`(`crates/mycad-cli/src/main.rs:47-67`)が同じ Document→Solid→Mesh フローを既に持つため、STL 化を除いて再利用する。
./features/18-api-skeleton/design-review.md:3299:use thiserror::Error;
./features/18-api-skeleton/design-review.md:3354:    finding: "`build_solid_from_features` をそのまま再利用する方針では、`make_cuboid` 側に寸法バリデーションが無いため `width/height/depth <= 0` の `.mycad` を API が受理しうる。ゼロ長 edge・面積ゼロ face・縮退 loop をどう拒否するか、また数値境界を何で比較するかが計画に無い。"
./features/18-api-skeleton/design-review.md:3359:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
./features/18-api-skeleton/design-review.md:3369:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
./features/18-api-skeleton/design-review.md:3383:    finding: "`build_solid_from_features` をそのまま再利用する方針では、`make_cuboid` 側に寸法バリデーションが無いため `width/height/depth <= 0` の `.mycad` を API が受理しうる。ゼロ長 edge・面積ゼロ face・縮退 loop をどう拒否するか、また数値境界を何で比較するかが計画に無い。"
./features/18-api-skeleton/design-review.md:3388:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
./features/18-api-skeleton/design-review.md:3398:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
./crates/mycad-kernel/benches/tessellation.rs:1:use mycad_kernel::brep::topology::IdGenerator;
./crates/mycad-kernel/benches/tessellation.rs:2:use mycad_kernel::primitives::make_cuboid;
./crates/mycad-kernel/benches/tessellation.rs:14:        let mut id_gen = IdGenerator::new(i as u64);
./crates/mycad-kernel/benches/tessellation.rs:15:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
./features/18-api-skeleton/design-review-2.md:22:- `IdGenerator` を使った決定的 ID 生成になっているか
./features/18-api-skeleton/design-review-2.md:27:- Euler-Poincaré の公式 `V - E + F = 2(S - H)` が成立する設計か（S=殻の数, H=貫通穴）
./features/18-api-skeleton/design-review-2.md:28:- HalfEdge の twin/next/prev ポインタ（インデックス）が正しく結ばれるか
./features/18-api-skeleton/design-review-2.md:29:- Loop/Shell/Solid の入れ子構造が一貫しているか
./features/18-api-skeleton/design-review-2.md:32:- ゼロ長エッジ、面積ゼロの Face、縮退した Loop の排除または明示的エラーの設計があるか
./features/18-api-skeleton/design-review-2.md:37:- 新しい依存は `[workspace.dependencies]` + `{ workspace = true }` 規約に従っているか
./features/18-api-skeleton/design-review-2.md:38:- ライブラリエラーには `thiserror` を使用しているか
./features/18-api-skeleton/design-review-2.md:39:- カーネルにレンダリング依存が混入していないか（`TriangleMesh` 生成のみ可）
./features/18-api-skeleton/design-review-2.md:43:- Euler-Poincaré 検証テストがあるか
./features/18-api-skeleton/design-review-2.md:61:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
./features/18-api-skeleton/design-review-2.md:62:    suggestion: "IdGenerator::next() に置き換えること"
./features/18-api-skeleton/design-review-2.md:79:Phase 2「Web ベースビューア基盤」の土台として、CLI / GUI / AI 共通の操作面 v0 を Rust HTTP サーバとして立てる。`.mycad` を読み込み、テッセレーションした `TriangleMesh` を JSON で返す最小 API を `crates/mycad-api/` に新規実装する。後続 Issue #20(Three.js フロント)・#21(`mycad view`)がこの API を消費する前提。
./features/18-api-skeleton/design-review-2.md:81:既存 CLI の `run_export`(`crates/mycad-cli/src/main.rs:47-67`)が同じ Document→Solid→Mesh フローを既に持つため、STL 化を除いて再利用する。
./features/18-api-skeleton/design-review-2.md:87:- 新規クレート `crates/mycad-api/`(lib + bin 構成)+ `make_cuboid` の入力検証(kernel)
./features/18-api-skeleton/design-review-2.md:98:| `crates/mycad-kernel/src/primitives/cuboid.rs` | **(R02)** `make_cuboid` を `-> Result<Solid, KernelError>` 化。`dx/dy/dz` が非有限 or `<= 0` なら `InvalidParameter { kind }`(`make_cylinder` cylinder.rs:16-22 と対称)。インライン test の呼び出しを `.unwrap()` 化 |
./features/18-api-skeleton/design-review-2.md:99:| `crates/mycad-build/src/lib.rs` | CreateBox 分岐を `Ok(make_cuboid(...))` → `make_cuboid(...)`(`?` 伝播)に変更 |
./features/18-api-skeleton/design-review-2.md:100:| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
./features/18-api-skeleton/design-review-2.md:101:| `Cargo.toml`(ルート) | members に `"crates/mycad-api"` 追加。`[workspace.dependencies]` に `axum`/`tokio`/`serde_json`/`tower`(dev) 追加 |
./features/18-api-skeleton/design-review-2.md:120:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
./features/18-api-skeleton/design-review-2.md:130:- `Document::from_path`(`crates/mycad-format/src/document.rs:36`)。`.mycad` 拡張子を強制(InvalidExtension)。
./features/18-api-skeleton/design-review-2.md:131:- `build_solid_from_features`(`crates/mycad-build/src/lib.rs:6`)。単一フィーチャ・Box/Cylinder のみ。
./features/18-api-skeleton/design-review-2.md:132:- `tessellate_solid_with`(`crates/mycad-kernel/src/tessellation/mod.rs:90`)。v0 は `TessellationOptions::default()` 固定。
./features/18-api-skeleton/design-review-2.md:133:- `TriangleMesh`(mod.rs:16, Serialize 済み)→ `Json(mesh)`。
./features/18-api-skeleton/design-review-2.md:134:- ハンドラ本体は CLI `run_export`(main.rs:47-67)から STL 化を除き末尾を `Ok(Json(mesh))` に置換。
./features/18-api-skeleton/design-review-2.md:147:2. **`.mycad` 拡張子限定**(`Document::from_path` が既に強制)→ 機密ファイル読取を遮断。
./features/18-api-skeleton/design-review-2.md:157:- `[workspace.dependencies]` に新規登録: `axum = "0.8"`、`tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }`、`serde_json = "1"`、`tower = "0.5"`(dev、`oneshot`)。各クレートは `{ workspace = true }`。
./features/18-api-skeleton/design-review-2.md:158:- derive 規約・`thiserror`・`version.workspace = true` を踏襲。clippy `-D warnings` 通過必須。
./features/18-api-skeleton/design-review-2.md:164:| T01 | 正常系(box) | `GET /api/v0/mesh?file=<abs>/examples/simple_box.mycad` を oneshot | 200、`TriangleMesh` deser 可、`positions`/`indices` 非空、`indices.len() % 3 == 0` |
./features/18-api-skeleton/design-review-2.md:168:| T05 | 400 | 非 `.mycad` 拡張子(`Cargo.toml`) | 400(InvalidExtension) |
./features/18-api-skeleton/design-review-2.md:173:| K01 | kernel 単体(R02) | `make_cuboid(0.0, 1.0, 1.0, ..)` 等 | `Err(InvalidParameter { kind: "..." })` |
./features/18-api-skeleton/design-review-2.md:175:> **R05 反論**: Euler-Poincaré 検証は kernel(`cylinder.rs:180-195`)で、golden YAML ラウンドトリップは format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保済み。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。API 層は HTTP 契約(status・JSON 形・決定性・cylinder 経路疎通)に集中する。
./features/18-api-skeleton/design-review-2.md:182:curl -s 'http://127.0.0.1:3000/api/v0/mesh?file=examples/simple_box.mycad' | head -c 300   # TriangleMesh JSON
./features/18-api-skeleton/design-review-2.md:193:設計レビューとして、まず参照されている既存実装と規約が文書の主張と整合しているかを確認します。`run_export`、`make_cuboid`、build/tessellation、workspace 依存規約、既存テスト配置を先に見ます。
./crates/mycad-kernel/tests/cuboid_roundtrip.rs:1:use mycad_kernel::brep::topology::IdGenerator;
./crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
./crates/mycad-kernel/tests/cuboid_roundtrip.rs:8:    let mut id_gen = IdGenerator::new(0);
./crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
./crates/mycad-kernel/tests/cuboid_roundtrip.rs:30:        let mut id_gen = IdGenerator::new(42);
./crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
./features/18-api-skeleton/plan.md:5:Phase 2「Web ベースビューア基盤」の土台として、CLI / GUI / AI 共通の操作面 v0 を Rust HTTP サーバとして立てる。`.mycad` を読み込み、テッセレーションした `TriangleMesh` を JSON で返す最小 API を `crates/mycad-api/` に新規実装する。後続 Issue #20(Three.js フロント)・#21(`mycad view`)がこの API を消費する前提。
./features/18-api-skeleton/plan.md:7:既存 CLI の `run_export`(`crates/mycad-cli/src/main.rs:47-67`)が同じ Document→Solid→Mesh フローを既に持つため、STL 化を除いて再利用する。
./features/18-api-skeleton/plan.md:13:- 新規クレート `crates/mycad-api/`(lib + bin 構成)+ `make_cuboid` の入力検証(kernel)
./features/18-api-skeleton/plan.md:24:| `crates/mycad-kernel/src/primitives/cuboid.rs` | **(R02)** `make_cuboid` を `-> Result<Solid, KernelError>` 化。`dx/dy/dz` が非有限 or `<= 0` なら `InvalidParameter { kind }`(`make_cylinder` cylinder.rs:16-22 と対称)。インライン test の呼び出しを `.unwrap()` 化 |
./features/18-api-skeleton/plan.md:25:| `crates/mycad-build/src/lib.rs` | CreateBox 分岐を `Ok(make_cuboid(...))` → `make_cuboid(...)`(`?` 伝播)に変更 |
./features/18-api-skeleton/plan.md:26:| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
./features/18-api-skeleton/plan.md:27:| `Cargo.toml`(ルート) | members に `"crates/mycad-api"` 追加。`[workspace.dependencies]` に `axum`/`tokio`/`serde_json`/`tower`(dev) 追加 |
./features/18-api-skeleton/plan.md:46:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
./features/18-api-skeleton/plan.md:56:- `Document::from_path`(`crates/mycad-format/src/document.rs:36`)。`.mycad` 拡張子を強制(InvalidExtension)。
./features/18-api-skeleton/plan.md:57:- `build_solid_from_features`(`crates/mycad-build/src/lib.rs:6`)。単一フィーチャ・Box/Cylinder のみ。
./features/18-api-skeleton/plan.md:58:- `tessellate_solid_with`(`crates/mycad-kernel/src/tessellation/mod.rs:90`)。v0 は `TessellationOptions::default()` 固定。
./features/18-api-skeleton/plan.md:59:- `TriangleMesh`(mod.rs:16, Serialize 済み)→ `Json(mesh)`。
./features/18-api-skeleton/plan.md:60:- ハンドラ本体は CLI `run_export`(main.rs:47-67)から STL 化を除き末尾を `Ok(Json(mesh))` に置換。
./features/18-api-skeleton/plan.md:73:2. **`.mycad` 拡張子限定**(`Document::from_path` が既に強制)→ 機密ファイル読取を遮断。
./features/18-api-skeleton/plan.md:83:- `[workspace.dependencies]` に新規登録: `axum = "0.8"`、`tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }`、`serde_json = "1"`、`tower = "0.5"`(dev、`oneshot`)。各クレートは `{ workspace = true }`。
./features/18-api-skeleton/plan.md:84:- derive 規約・`thiserror`・`version.workspace = true` を踏襲。clippy `-D warnings` 通過必須。
./features/18-api-skeleton/plan.md:90:| T01 | 正常系(box) | `GET /api/v0/mesh?file=<abs>/examples/simple_box.mycad` を oneshot | 200、`TriangleMesh` deser 可、`positions`/`indices` 非空、`indices.len() % 3 == 0` |
./features/18-api-skeleton/plan.md:94:| T05 | 400 | 非 `.mycad` 拡張子(`Cargo.toml`) | 400(InvalidExtension) |
./features/18-api-skeleton/plan.md:99:| K01 | kernel 単体(R02) | `make_cuboid(0.0, 1.0, 1.0, ..)` 等 | `Err(InvalidParameter { kind: "..." })` |
./features/18-api-skeleton/plan.md:101:> **R05 反論**: Euler-Poincaré 検証は kernel(`cylinder.rs:180-195`)で、golden YAML ラウンドトリップは format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保済み。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。API 層は HTTP 契約(status・JSON 形・決定性・cylinder 経路疎通)に集中する。
./features/18-api-skeleton/plan.md:108:curl -s 'http://127.0.0.1:3000/api/v0/mesh?file=examples/simple_box.mycad' | head -c 300   # TriangleMesh JSON
./crates/mycad-kernel/CLAUDE.md:14:- すべてのEdgeは正確に2つのHalfEdgeを持つ
./crates/mycad-kernel/CLAUDE.md:15:- 閉じたShellはwatertight（水密）
./crates/mycad-kernel/CLAUDE.md:16:- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
./crates/mycad-kernel/CLAUDE.md:17:- Loopは閉じたHalfEdge列（最後のHalfEdgeの終点 = 最初のHalfEdgeの始点）
./crates/mycad-kernel/CLAUDE.md:18:- EntityIDは `IdGenerator` で決定的に生成される
./crates/mycad-kernel/CLAUDE.md:23:2. 関数シグネチャ: `pub fn make_xxx(params..., id_gen: &mut IdGenerator) -> Solid`
./crates/mycad-kernel/CLAUDE.md:25:4. 決定性テストを必ず書く（同じ `IdGenerator` 初期値で2回生成し、結果が同一であることを確認）
./crates/mycad-kernel/src/brep/mod.rs:3:pub use topology::{Edge, Face, HalfEdge, Loop, Shell, Solid, Vertex};
./crates/mycad-kernel/src/brep/topology.rs:21:pub struct HalfEdge {
./crates/mycad-kernel/src/brep/topology.rs:46:pub struct Loop {
./crates/mycad-kernel/src/brep/topology.rs:69:pub struct Shell {
./crates/mycad-kernel/src/brep/topology.rs:83:    pub half_edges: Vec<HalfEdge>,
./crates/mycad-kernel/src/brep/topology.rs:85:    pub loops: Vec<Loop>,
./crates/mycad-kernel/src/brep/topology.rs:87:    pub shells: Vec<Shell>,
./crates/mycad-kernel/src/brep/topology.rs:138:        self.half_edges.push(HalfEdge {
./crates/mycad-kernel/src/brep/topology.rs:150:        self.loops.push(Loop { id, half_edges });
./crates/mycad-kernel/src/brep/topology.rs:177:        self.shells.push(Shell { id, faces, closed });
./crates/mycad-kernel/src/brep/topology.rs:185:pub struct IdGenerator {
./crates/mycad-kernel/src/brep/topology.rs:189:impl IdGenerator {
./crates/mycad-kernel/src/brep/topology.rs:208:        let mut gen1 = IdGenerator::new(100);
./crates/mycad-kernel/src/brep/topology.rs:209:        let mut gen2 = IdGenerator::new(100);
./crates/mycad-kernel/src/primitives/mod.rs:4:pub use cuboid::make_cuboid;
./crates/mycad-kernel/src/primitives/mod.rs:5:pub use cylinder::make_cylinder;
./crates/mycad-kernel/src/primitives/cuboid.rs:1:use crate::brep::topology::{IdGenerator, Solid};
./crates/mycad-kernel/src/primitives/cuboid.rs:16:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
./crates/mycad-kernel/src/primitives/cuboid.rs:223:    use crate::brep::topology::IdGenerator;
./crates/mycad-kernel/src/primitives/cuboid.rs:227:        let mut id_gen = IdGenerator::new(0);
./crates/mycad-kernel/src/primitives/cuboid.rs:228:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
./crates/mycad-kernel/src/primitives/cuboid.rs:247:        let mut id_gen = IdGenerator::new(0);
./crates/mycad-kernel/src/primitives/cuboid.rs:248:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
./crates/mycad-kernel/src/primitives/cuboid.rs:276:        let mut id_gen1 = IdGenerator::new(0);
./crates/mycad-kernel/src/primitives/cuboid.rs:277:        let mut id_gen2 = IdGenerator::new(0);
./crates/mycad-kernel/src/primitives/cuboid.rs:279:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
./crates/mycad-kernel/src/primitives/cuboid.rs:280:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);
./crates/mycad-kernel/src/primitives/cylinder.rs:1:use crate::brep::topology::{IdGenerator, Solid};
./crates/mycad-kernel/src/primitives/cylinder.rs:11:pub fn make_cylinder(
./crates/mycad-kernel/src/primitives/cylinder.rs:14:    id_gen: &mut IdGenerator,
./crates/mycad-kernel/src/primitives/cylinder.rs:136:        let mut gen1 = IdGenerator::new(0);
./crates/mycad-kernel/src/primitives/cylinder.rs:137:        let mut gen2 = IdGenerator::new(0);
./crates/mycad-kernel/src/primitives/cylinder.rs:138:        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
./crates/mycad-kernel/src/primitives/cylinder.rs:139:        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
./crates/mycad-kernel/src/primitives/cylinder.rs:164:        let mut gen = IdGenerator::new(0);
./crates/mycad-kernel/src/primitives/cylinder.rs:165:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./crates/mycad-kernel/src/primitives/cylinder.rs:180:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
./crates/mycad-kernel/src/primitives/cylinder.rs:183:        let mut gen = IdGenerator::new(0);
./crates/mycad-kernel/src/primitives/cylinder.rs:184:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./crates/mycad-kernel/src/primitives/cylinder.rs:195:            "Euler-Poincaré V-E+F = 2(S-H), S={shells}, H=0"
./crates/mycad-kernel/src/primitives/cylinder.rs:203:        let mut gen = IdGenerator::new(0);
./crates/mycad-kernel/src/primitives/cylinder.rs:204:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./crates/mycad-kernel/src/primitives/cylinder.rs:221:        // Loop closure: each loop's consecutive HEs must connect geometrically
./crates/mycad-kernel/src/primitives/cylinder.rs:257:        let mut gen = IdGenerator::new(0);
./crates/mycad-kernel/src/primitives/cylinder.rs:258:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
./crates/mycad-kernel/src/primitives/cylinder.rs:302:        let mut gen = IdGenerator::new(0);
./crates/mycad-kernel/src/primitives/cylinder.rs:305:        let err = make_cylinder(0.0, 20.0, &mut gen).unwrap_err();
./crates/mycad-kernel/src/primitives/cylinder.rs:312:        let err = make_cylinder(-5.0, 20.0, &mut gen).unwrap_err();
./crates/mycad-kernel/src/primitives/cylinder.rs:319:        let err = make_cylinder(f64::NAN, 20.0, &mut gen).unwrap_err();
./crates/mycad-kernel/src/primitives/cylinder.rs:326:        let err = make_cylinder(f64::INFINITY, 20.0, &mut gen).unwrap_err();
./crates/mycad-kernel/src/primitives/cylinder.rs:333:        let err = make_cylinder(5.0, 0.0, &mut gen).unwrap_err();
./crates/mycad-kernel/src/primitives/cylinder.rs:340:        let err = make_cylinder(5.0, -10.0, &mut gen).unwrap_err();
./crates/mycad-kernel/src/primitives/cylinder.rs:347:        let err = make_cylinder(5.0, f64::NAN, &mut gen).unwrap_err();
./crates/mycad-kernel/src/primitives/cylinder.rs:354:        let err = make_cylinder(5.0, f64::INFINITY, &mut gen).unwrap_err();

codex
既存コードの実際の境界条件とテスト分担を確認しています。特に `make_cuboid` の退化対策、`build_solid_from_features` の assembly 扱い、`Document::from_path` の拡張子・存在判定、既存の決定性/Euler テスト有無を見ます。
exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-cli/src/main.rs" in /home/bacon/mycad
 succeeded in 0ms:
use clap::{Parser, Subcommand};
use mycad_build::build_solid_from_features;
use mycad_format::Document;
use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::tessellation::{tessellate_solid_with, to_ascii_stl, TessellationOptions};
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
        /// Number of angular segments for curved surfaces (default: 32)
        #[arg(long)]
        segments: Option<usize>,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Export {
            input,
            output,
            segments,
        } => {
            if let Err(msg) = run_export(&input, &output, segments) {
                eprintln!("error: {msg}");
                process::exit(1);
            }
        }
    }
}

fn run_export(
    input: &std::path::Path,
    output: &std::path::Path,
    segments: Option<usize>,
) -> Result<(), String> {
    let doc = Document::from_path(input)
        .map_err(|e| format!("failed to read {}: {e}", input.display()))?;
    let mut gen = IdGenerator::new(0);
    let solid = build_solid_from_features(&doc.root_component.features, &mut gen)
        .map_err(|e| format!("failed to build solid: {e}"))?;
    let opts = match segments {
        Some(n) => TessellationOptions::new(n, 1),
        None => TessellationOptions::default(),
    };
    let mesh =
        tessellate_solid_with(&solid, &opts).map_err(|e| format!("failed to tessellate: {e}"))?;
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
use mycad_kernel::primitives::{make_cuboid, make_cylinder};

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
        Feature::CreateCylinder { radius, height, .. } => make_cylinder(*radius, *height, gen),
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
/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/src/error.rs" in /home/bacon/mycad
 succeeded in 0ms:
use thiserror::Error;

#[derive(Debug, Error)]
pub enum KernelError {
    #[error("empty feature list")]
    EmptyFeatureList,

    #[error("multiple features not yet supported (got {count})")]
    MultipleFeatures { count: usize },

    #[error("unsupported feature variant: {kind}")]
    UnsupportedFeature { kind: &'static str },

    #[error("invalid parameter: {kind}")]
    InvalidParameter { kind: &'static str },
}

exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-format/src/document.rs" in /home/bacon/mycad
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
/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/primitives/cuboid.rs" in /home/bacon/mycad
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

exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/primitives/cylinder.rs" in /home/bacon/mycad
 succeeded in 0ms:
use crate::brep::topology::{IdGenerator, Solid};
use crate::error::KernelError;
use crate::geometry::curve::Curve;
use crate::geometry::surface::Surface;
use crate::geometry::{Point, Vec3};

/// Create a cylinder B-rep solid.
///
/// Axis = +Z, bottom center at origin, bottom z=0 / top z=height.
/// Seam vertex at (0, radius, z) — angle u=0 in the orthonormal_basis(+Z) = (+Y, -X) frame.
pub fn make_cylinder(
    radius: f64,
    height: f64,
    id_gen: &mut IdGenerator,
) -> Result<Solid, KernelError> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err(KernelError::InvalidParameter { kind: "radius" });
    }
    if !height.is_finite() || height <= 0.0 {
        return Err(KernelError::InvalidParameter { kind: "height" });
    }

    let mut solid = Solid::new(id_gen.next());

    // 2 vertices: seam at bottom and top
    let v_bot = solid.add_vertex(id_gen.next(), Point::new(0.0, radius, 0.0));
    let v_top = solid.add_vertex(id_gen.next(), Point::new(0.0, radius, height));

    // 3 edges
    let e_bot = solid.add_edge(
        id_gen.next(),
        [v_bot, v_bot],
        Curve::Circle {
            center: Point::origin(),
            normal: Vec3::z(),
            radius,
        },
        [0.0, 2.0 * std::f64::consts::PI],
    );

    let e_top = solid.add_edge(
        id_gen.next(),
        [v_top, v_top],
        Curve::Circle {
            center: Point::new(0.0, 0.0, height),
            normal: Vec3::z(),
            radius,
        },
        [0.0, 2.0 * std::f64::consts::PI],
    );

    let e_seam = solid.add_edge(
        id_gen.next(),
        [v_bot, v_top],
        Curve::Line {
            origin: Point::new(0.0, radius, 0.0),
            direction: Vec3::new(0.0, 0.0, height),
        },
        [0.0, 1.0],
    );

    // 3 faces: bottom cap, top cap, lateral
    let mut face_indices = Vec::with_capacity(3);

    // Bottom cap: Surface::Plane with normal -Z, loop = e_bot reversed (CW from above = outward -Z)
    let he_bot_rev = solid.add_half_edge(id_gen.next(), v_bot, e_bot, false);
    let loop_bot = solid.add_loop(id_gen.next(), vec![he_bot_rev]);
    let f_bot = solid.add_face(
        id_gen.next(),
        Surface::Plane {
            origin: Point::origin(),
            normal: -Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        },
        loop_bot,
        vec![],
        true,
    );
    face_indices.push(f_bot);

    // Top cap: Surface::Plane with normal +Z, loop = e_top forward (CCW from above = outward +Z)
    let he_top_fwd = solid.add_half_edge(id_gen.next(), v_top, e_top, true);
    let loop_top = solid.add_loop(id_gen.next(), vec![he_top_fwd]);
    let f_top = solid.add_face(
        id_gen.next(),
        Surface::Plane {
            origin: Point::new(0.0, 0.0, height),
            normal: Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        },
        loop_top,
        vec![],
        true,
    );
    face_indices.push(f_top);

    // Lateral face: Surface::Cylinder, loop = 4 HE
    //   seam up (fwd, start v_bot) → top circle (rev, start v_top) →
    //   seam down (rev, start v_top) → bot circle (fwd, start v_bot)
    let he_seam_up = solid.add_half_edge(id_gen.next(), v_bot, e_seam, true);
    let he_top_rev = solid.add_half_edge(id_gen.next(), v_top, e_top, false);
    let he_seam_dn = solid.add_half_edge(id_gen.next(), v_top, e_seam, false);
    let he_bot_fwd = solid.add_half_edge(id_gen.next(), v_bot, e_bot, true);
    let loop_lat = solid.add_loop(
        id_gen.next(),
        vec![he_seam_up, he_top_rev, he_seam_dn, he_bot_fwd],
    );
    let f_lat = solid.add_face(
        id_gen.next(),
        Surface::Cylinder {
            origin: Point::origin(),
            axis: Vec3::z(),
            radius,
        },
        loop_lat,
        vec![],
        true,
    );
    face_indices.push(f_lat);

    solid.add_shell(id_gen.next(), face_indices, true);

    Ok(solid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::surface::TessellationStrategy;

    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
    #[test]
    fn test_cylinder_deterministic() {
        let mut gen1 = IdGenerator::new(0);
        let mut gen2 = IdGenerator::new(0);
        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();

        assert_eq!(s1.vertices.len(), s2.vertices.len());
        assert_eq!(s1.edges.len(), s2.edges.len());
        assert_eq!(s1.faces.len(), s2.faces.len());
        assert_eq!(s1.half_edges.len(), s2.half_edges.len());
        assert_eq!(s1.loops.len(), s2.loops.len());
        assert_eq!(s1.shells.len(), s2.shells.len());

        for (v1, v2) in s1.vertices.iter().zip(s2.vertices.iter()) {
            assert_eq!(v1.id, v2.id);
            assert_eq!(v1.point, v2.point);
        }
        for (e1, e2) in s1.edges.iter().zip(s2.edges.iter()) {
            assert_eq!(e1.id, e2.id);
            assert_eq!(e1.vertices, e2.vertices);
        }
        for (f1, f2) in s1.faces.iter().zip(s2.faces.iter()) {
            assert_eq!(f1.id, f2.id);
        }
    }

    /// T02: Topology counts.
    #[test]
    fn test_cylinder_topology() {
        let mut gen = IdGenerator::new(0);
        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();

        assert_eq!(s.vertices.len(), 2, "2 seam vertices");
        assert_eq!(
            s.edges.len(),
            3,
            "3 edges (bot circle, top circle, seam line)"
        );
        assert_eq!(s.faces.len(), 3, "3 faces (bottom cap, top cap, lateral)");
        assert_eq!(s.shells.len(), 1);
        assert!(s.shells[0].closed);
        assert_eq!(s.half_edges.len(), 6, "6 half-edges (3 edges × 2 HE each)");
        assert_eq!(s.loops.len(), 3, "3 loops (one per face)");
    }

    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
    #[test]
    fn test_cylinder_euler() {
        let mut gen = IdGenerator::new(0);
        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();

        let v = s.vertices.len();
        let e = s.edges.len();
        let f = s.faces.len();
        let shells = s.shells.len();
        // genus=0 for simple cylinder
        let euler = (v as i64) - (e as i64) + (f as i64);
        assert_eq!(
            euler,
            2 * (shells as i64),
            "Euler-Poincaré V-E+F = 2(S-H), S={shells}, H=0"
        );
    }

    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
    ///       plus loop closure (consecutive HE endpoint ≈ next HE start point).
    #[test]
    fn test_cylinder_manifold_and_loop_closure() {
        let mut gen = IdGenerator::new(0);
        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
        let eps = 1e-10;

        // Each edge must have exactly 2 HEs with opposite forward flags
        let mut edge_he_count: std::collections::HashMap<usize, Vec<bool>> =
            std::collections::HashMap::new();
        for he in &s.half_edges {
            edge_he_count.entry(he.edge).or_default().push(he.forward);
        }
        for (edge_idx, forwards) in &edge_he_count {
            assert_eq!(forwards.len(), 2, "edge {edge_idx} must have exactly 2 HEs");
            assert_ne!(
                forwards[0], forwards[1],
                "edge {edge_idx} HEs must have opposite orientation"
            );
        }

        // Loop closure: each loop's consecutive HEs must connect geometrically
        for (loop_idx, lp) in s.loops.iter().enumerate() {
            assert!(
                !lp.half_edges.is_empty(),
                "loop {loop_idx} must not be empty"
            );
            for i in 0..lp.half_edges.len() {
                let he_cur = &s.half_edges[lp.half_edges[i]];
                let he_next = &s.half_edges[lp.half_edges[(i + 1) % lp.half_edges.len()]];

                let cur_edge = &s.edges[he_cur.edge];
                let end_v = if he_cur.forward {
                    cur_edge.vertices[1]
                } else {
                    cur_edge.vertices[0]
                };

                let next_start = he_next.start_vertex;

                assert!(
                    (s.vertices[end_v].point - s.vertices[next_start].point).norm() < eps,
                    "loop {loop_idx}: HE {} end vertex {} ({:?}) != HE {} start vertex {} ({:?})",
                    i,
                    end_v,
                    s.vertices[end_v].point,
                    (i + 1) % lp.half_edges.len(),
                    next_start,
                    s.vertices[next_start].point,
                );
            }
        }
    }

    /// T05: Geometry — seam vertices and surface coefficients.
    #[test]
    fn test_cylinder_geometry() {
        let mut gen = IdGenerator::new(0);
        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
        let eps = 1e-10;


exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-format/src/component.rs" in /home/bacon/mycad
 succeeded in 0ms:
use crate::error::FormatError;
use crate::feature::Feature;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// A transform in 3D space.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Transform {
    /// Position offset [x, y, z].
    pub position: [f64; 3],
    /// Rotation as Euler angles in degrees [rx, ry, rz].
    pub rotation: [f64; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
        }
    }
}

/// Reference to an external component, serialized as a URI string.
///
/// - `"stdlib://path/to/part"` → `ComponentRef::StdLib("path/to/part")`
/// - Any other string → `ComponentRef::File(value)`
#[derive(Debug, Clone)]
pub enum ComponentRef {
    /// Standard library reference (e.g., `"stdlib://fasteners/jis_b1176/M5x20"`)
    StdLib(String),
    /// File reference (e.g., `"./motor_v2.mycad"`)
    File(String),
}

impl FromStr for ComponentRef {
    type Err = FormatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(FormatError::InvalidReference {
                value: s.to_string(),
                reason: "reference must not be empty",
            });
        }
        if let Some(path) = s.strip_prefix("stdlib://") {
            if path.is_empty() {
                return Err(FormatError::InvalidReference {
                    value: s.to_string(),
                    reason: "stdlib reference must have a non-empty path after 'stdlib://'",
                });
            }
            Ok(ComponentRef::StdLib(path.to_string()))
        } else {
            Ok(ComponentRef::File(s.to_string()))
        }
    }
}

impl fmt::Display for ComponentRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ComponentRef::StdLib(path) => write!(f, "stdlib://{path}"),
            ComponentRef::File(path) => write!(f, "{path}"),
        }
    }
}

impl Serialize for ComponentRef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ComponentRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for ComponentRef {
    fn schema_name() -> String {
        "ComponentRef".to_string()
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        <String as schemars::JsonSchema>::json_schema(gen)
    }
}

/// A component in the design hierarchy.
/// Can contain features (inline part definition), children (sub-components),
/// or be a reference to an external file/library.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Component {
    /// Human-readable name.
    pub name: String,

    /// Transform relative to the parent component.
    #[serde(default, skip_serializing_if = "is_default_transform")]
    pub transform: Transform,

    /// External reference (if this component is not defined inline).
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "ref")]
    pub reference: Option<ComponentRef>,

    /// Ordered list of features (the operation history).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<Feature>,

    /// Child components.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Component>,
}

fn is_default_transform(t: &Transform) -> bool {
    t.position == [0.0, 0.0, 0.0] && t.rotation == [0.0; 3]
}

impl Component {
    /// Create a new empty component.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            transform: Transform::default(),
            reference: None,
            features: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Create a component referencing an external file or library.
    pub fn from_ref(name: &str, reference: &str) -> Result<Self, FormatError> {
        Ok(Self {
            name: name.to_string(),
            transform: Transform::default(),
            reference: Some(reference.parse()?),
            features: Vec::new(),
            children: Vec::new(),
        })
    }
}

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
/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/src/tessellation/mod.rs" in /home/bacon/mycad
 succeeded in 0ms:
pub mod stl;
pub use stl::to_ascii_stl;

use crate::brep::topology::Solid;
use crate::geometry::curve::Curve;
use crate::geometry::surface::{Surface, TessellationStrategy};
use crate::geometry::Point;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use thiserror::Error;

/// Seam jump detection threshold: |Δu| > π indicates a parameter-space seam crossing.
/// Minimum cross-product norm for a non-degenerate triangle.
const AREA_EPS: f64 = 1e-14;

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

/// Controls the resolution of curved-surface tessellation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TessellationOptions {
    pub angular_segments: usize,
    pub axial_segments: usize,
}

impl Default for TessellationOptions {
    fn default() -> Self {
        Self {
            angular_segments: 32,
            axial_segments: 1,
        }
    }
}

impl TessellationOptions {
    pub fn new(angular_segments: usize, axial_segments: usize) -> Self {
        Self {
            angular_segments: angular_segments.max(3),
            axial_segments: axial_segments.max(1),
        }
    }
}

/// Errors from tessellation.
#[derive(Debug, Error)]
pub enum TessellationError {
    #[error("unsupported surface type: {kind}")]
    UnsupportedSurface { kind: &'static str },
    #[error("trimmed or punctured faces are not yet supported")]
    TrimmedFaceUnsupported,
    #[error("non-manifold loop detected")]
    NonManifoldLoop,
}

/// Tessellate a B-rep solid into a triangle mesh using default options.
pub fn tessellate_solid(solid: &Solid) -> Result<TriangleMesh, TessellationError> {
    tessellate_solid_with(solid, &TessellationOptions::default())
}

/// Tessellate a B-rep solid into a triangle mesh with the given options.
pub fn tessellate_solid_with(
    solid: &Solid,
    opts: &TessellationOptions,
) -> Result<TriangleMesh, TessellationError> {
    let mut mesh = TriangleMesh::new();

    for face in &solid.faces {
        let strategy = face.surface.tessellation_strategy();
        match strategy {
            TessellationStrategy::BoundaryFan => {
                tessellate_face_fan(solid, face, opts, &mut mesh)?;
            }
            TessellationStrategy::UvGridFullPatch => {
                tessellate_face_uv_grid(solid, face, opts, &mut mesh)?;
            }
            TessellationStrategy::Unsupported => {
                return Err(TessellationError::UnsupportedSurface {
                    kind: face.surface.kind_name(),
                });
            }
        }
    }

    Ok(mesh)
}

/// Triangulate a face as a fan from its first boundary vertex (planar convex faces).
fn tessellate_face_fan(
    solid: &Solid,
    face: &crate::brep::topology::Face,
    opts: &TessellationOptions,
    mesh: &mut TriangleMesh,
) -> Result<(), TessellationError> {
    let outer_loop = &solid.loops[face.outer_loop];

    let loop_points = collect_loop_points(solid, outer_loop, opts.angular_segments.max(3))?;

    if loop_points.len() < 3 {
        return Ok(());
    }

    let base_idx = mesh.positions.len() as u32;

    for p in &loop_points {
        let normal = face.surface.normal_at_point(p);
        let n = if face.same_sense {
            [normal.x, normal.y, normal.z]
        } else {
            [-normal.x, -normal.y, -normal.z]
        };
        mesh.positions.push([p.x, p.y, p.z]);
        mesh.normals.push(n);
    }

    for i in 1..(loop_points.len() as u32 - 1) {
        mesh.indices.push(base_idx);
        mesh.indices.push(base_idx + i);
        mesh.indices.push(base_idx + i + 1);
    }

    Ok(())
}

/// Collect the ordered boundary points of a loop by sampling each HE's curve.
fn collect_loop_points(
    solid: &Solid,
    lp: &crate::brep::topology::Loop,
    segments: usize,
) -> Result<Vec<Point>, TessellationError> {
    let mut points = Vec::new();

    for &he_idx in &lp.half_edges {
        let he = &solid.half_edges[he_idx];
        let edge = &solid.edges[he.edge];

        let (t_start, t_end) = if he.forward {
            (edge.t_range[0], edge.t_range[1])
        } else {
            (edge.t_range[1], edge.t_range[0])
        };

        let seg_points = edge.curve.sample_segment(t_start, t_end, segments);
        for p in seg_points {
            points.push(p);
        }
    }

    Ok(points)
}

/// Triangulate a face using UV grid sampling (full untrimmed periodic/rectangular patch).
fn tessellate_face_uv_grid(
    solid: &Solid,
    face: &crate::brep::topology::Face,
    opts: &TessellationOptions,
    mesh: &mut TriangleMesh,
) -> Result<(), TessellationError> {
    if !face.inner_loops.is_empty() {
        return Err(TessellationError::TrimmedFaceUnsupported);
    }

    let outer_loop = &solid.loops[face.outer_loop];

    // Guard: total Circle-edge span must be a positive integer multiple of 2π.
    // Summing spans (rather than testing any single edge) accepts edge-split full cylinders
    // (e.g. two π-arcs per cap) while still rejecting trimmed faces. 1e-9 tolerance is tight
    // enough that floating-point noise never masks a genuine sub-degree trim.
    let total_circle_span: f64 = outer_loop
        .half_edges
        .iter()
        .filter_map(|&he_idx| {
            let he = &solid.half_edges[he_idx];
            let edge = &solid.edges[he.edge];
            if matches!(edge.curve, Curve::Circle { .. }) {
                Some((edge.t_range[1] - edge.t_range[0]).abs())
            } else {
                None
            }
        })
        .sum();
    let full_rev_count = (total_circle_span / (2.0 * PI)).round() as i64;
    if full_rev_count <= 0 || (total_circle_span - full_rev_count as f64 * 2.0 * PI).abs() >= 1e-9 {
        return Err(TessellationError::TrimmedFaceUnsupported);
    }

    // Determine v range from loop corner vertices (start_vertex of each HE).
    // UvGridFullPatch assumes a full 2π revolution in u starting at u=0;
    // we do not sample the circle arcs here — the grid covers [0,2π] directly.
    let corner_uvs: Vec<(f64, f64)> = outer_loop
        .half_edges
        .iter()

exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-format/src/feature.rs" in /home/bacon/mycad
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
/bin/bash -lc "sed -n '1,120p' Cargo.toml" in /home/bacon/mycad
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
/bin/bash -lc "sed -n '1,200p' crates/mycad-kernel/src/lib.rs" in /home/bacon/mycad
 succeeded in 0ms:
pub mod brep;
pub mod error;
pub mod geometry;
pub mod primitives;
pub mod tessellation;

pub use brep::topology;
pub use error::KernelError;

codex
退化幾何の観点を詰めています。`<= 0` だけで十分か、既存コードにある `AREA_EPS` や loop/trim 判定の閾値と整合した epsilon 方針が計画で落ちていないかを確認します。
exec
/bin/bash -lc 'rg -n "AREA_EPS|1e-9|1e-10|epsilon|eps|InvalidParameter|TrimmedFaceUnsupported|NonManifoldLoop|canonicalize|Host|403|127.0.0.1|oneshot|serde_json|JsonSchema|Debug, Clone, Serialize, Deserialize" -S crates docs CLAUDE.md ARCHITECTURE.md' in /home/bacon/mycad
 succeeded in 0ms:
CLAUDE.md:55:- **derive規約**: 公開型には `Debug, Clone, Serialize, Deserialize` を付与。スキーマ生成が必要な型は `JsonSchema` も追加。
CLAUDE.md:64:- **Kernel has zero rendering deps**: カーネルはレンダリング非依存。`TriangleMesh` を生成し、ビューアが消費する。
crates/mycad-format/src/feature.rs:1:use schemars::JsonSchema;
crates/mycad-format/src/feature.rs:5:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
crates/mycad-format/src/feature.rs:14:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
crates/mycad-format/src/component.rs:8:#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
crates/mycad-format/src/component.rs:83:impl schemars::JsonSchema for ComponentRef {
crates/mycad-format/src/component.rs:89:        <String as schemars::JsonSchema>::json_schema(gen)
crates/mycad-format/src/component.rs:96:#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
crates/mycad-kernel/src/tessellation/mod.rs:14:const AREA_EPS: f64 = 1e-14;
crates/mycad-kernel/src/tessellation/mod.rs:17:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/tessellation/mod.rs:79:    TrimmedFaceUnsupported,
crates/mycad-kernel/src/tessellation/mod.rs:81:    NonManifoldLoop,
crates/mycad-kernel/src/tessellation/mod.rs:188:        return Err(TessellationError::TrimmedFaceUnsupported);
crates/mycad-kernel/src/tessellation/mod.rs:195:    // (e.g. two π-arcs per cap) while still rejecting trimmed faces. 1e-9 tolerance is tight
crates/mycad-kernel/src/tessellation/mod.rs:211:    if full_rev_count <= 0 || (total_circle_span - full_rev_count as f64 * 2.0 * PI).abs() >= 1e-9 {
crates/mycad-kernel/src/tessellation/mod.rs:212:        return Err(TessellationError::TrimmedFaceUnsupported);
crates/mycad-kernel/src/tessellation/mod.rs:286:    if cross_norm < AREA_EPS * AREA_EPS {
crates/mycad-kernel/src/tessellation/mod.rs:545:    /// Non-full-revolution cylinder face (arc < 2π) must return TrimmedFaceUnsupported.
crates/mycad-kernel/src/tessellation/mod.rs:578:            Err(TessellationError::TrimmedFaceUnsupported)
crates/mycad-kernel/src/error.rs:15:    InvalidParameter { kind: &'static str },
crates/mycad-format/src/document.rs:3:use schemars::JsonSchema;
crates/mycad-format/src/document.rs:8:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
docs/decisions/003-viewer-and-app-architecture.md:47:- 「型統一」の旨味は **ts-rs / JsonSchema → TS による型自動生成**で代替できる (Rust 型定義を真実とした型安全な TS 型)。
crates/mycad-kernel/src/geometry/surface.rs:17:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/geometry/surface.rs:191:        assert_relative_eq!(u, u2, epsilon = 1e-9);
crates/mycad-kernel/src/geometry/surface.rs:192:        assert_relative_eq!(v, v2, epsilon = 1e-9);
crates/mycad-kernel/src/geometry/surface.rs:262:        assert_relative_eq!(n1.dot(&n2), 1.0, epsilon = 1e-12);
crates/mycad-kernel/src/brep/topology.rs:11:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:20:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:33:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:45:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:54:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:68:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:79:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/geometry/mod.rs:15:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/geometry/math.rs:28:        assert_relative_eq!(u.norm(), 1.0, epsilon = 1e-12);
crates/mycad-kernel/src/geometry/math.rs:29:        assert_relative_eq!(v.norm(), 1.0, epsilon = 1e-12);
crates/mycad-kernel/src/geometry/math.rs:30:        assert_relative_eq!(u.dot(&v), 0.0, epsilon = 1e-12);
crates/mycad-kernel/src/geometry/math.rs:31:        assert_relative_eq!(u.dot(&Vec3::z()), 0.0, epsilon = 1e-12);
crates/mycad-kernel/src/geometry/math.rs:37:        assert_relative_eq!(u.norm(), 1.0, epsilon = 1e-12);
crates/mycad-kernel/src/geometry/math.rs:38:        assert_relative_eq!(v.norm(), 1.0, epsilon = 1e-12);
crates/mycad-kernel/src/geometry/math.rs:39:        assert_relative_eq!(u.dot(&v), 0.0, epsilon = 1e-12);
crates/mycad-kernel/src/geometry/math.rs:40:        assert_relative_eq!(u.dot(&Vec3::x()), 0.0, epsilon = 1e-12);
crates/xtask/src/main.rs:25:    let steps: &[(&str, &[&str])] = &[
crates/xtask/src/main.rs:38:    for (label, cmd) in steps {
crates/mycad-kernel/src/geometry/curve.rs:6:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/primitives/cylinder.rs:17:        return Err(KernelError::InvalidParameter { kind: "radius" });
crates/mycad-kernel/src/primitives/cylinder.rs:20:        return Err(KernelError::InvalidParameter { kind: "height" });
crates/mycad-kernel/src/primitives/cylinder.rs:205:        let eps = 1e-10;
crates/mycad-kernel/src/primitives/cylinder.rs:241:                    (s.vertices[end_v].point - s.vertices[next_start].point).norm() < eps,
crates/mycad-kernel/src/primitives/cylinder.rs:259:        let eps = 1e-10;
crates/mycad-kernel/src/primitives/cylinder.rs:262:        assert!((s.vertices[0].point - Point::new(0.0, 5.0, 0.0)).norm() < eps);
crates/mycad-kernel/src/primitives/cylinder.rs:263:        assert!((s.vertices[1].point - Point::new(0.0, 5.0, 20.0)).norm() < eps);
crates/mycad-kernel/src/primitives/cylinder.rs:269:                    (*normal + Vec3::z()).norm() < eps,
crates/mycad-kernel/src/primitives/cylinder.rs:280:                    (*normal - Vec3::z()).norm() < eps,
crates/mycad-kernel/src/primitives/cylinder.rs:292:                assert!((r - 5.0).abs() < eps);
crates/mycad-kernel/src/primitives/cylinder.rs:293:                assert!((*axis - Vec3::z()).norm() < eps);
crates/mycad-kernel/src/primitives/cylinder.rs:299:    /// T14: Degenerate inputs — radius<=0, height<=0, non-finite → InvalidParameter.
crates/mycad-kernel/src/primitives/cylinder.rs:308:            KernelError::InvalidParameter { kind: "radius" }
crates/mycad-kernel/src/primitives/cylinder.rs:315:            KernelError::InvalidParameter { kind: "radius" }
crates/mycad-kernel/src/primitives/cylinder.rs:322:            KernelError::InvalidParameter { kind: "radius" }
crates/mycad-kernel/src/primitives/cylinder.rs:329:            KernelError::InvalidParameter { kind: "radius" }
crates/mycad-kernel/src/primitives/cylinder.rs:336:            KernelError::InvalidParameter { kind: "height" }
crates/mycad-kernel/src/primitives/cylinder.rs:343:            KernelError::InvalidParameter { kind: "height" }
crates/mycad-kernel/src/primitives/cylinder.rs:350:            KernelError::InvalidParameter { kind: "height" }
crates/mycad-kernel/src/primitives/cylinder.rs:357:            KernelError::InvalidParameter { kind: "height" }

exec
/bin/bash -lc "sed -n '220,520p' crates/mycad-kernel/src/tessellation/mod.rs" in /home/bacon/mycad
 succeeded in 0ms:
        .iter()
        .map(|&he_idx| {
            let he = &solid.half_edges[he_idx];
            face.surface.uv_of(&solid.vertices[he.start_vertex].point)
        })
        .collect();

    let v_min = corner_uvs.iter().map(|(_, v)| *v).fold(f64::MAX, f64::min);
    let v_max = corner_uvs.iter().map(|(_, v)| *v).fold(f64::MIN, f64::max);
    let u_min = 0.0_f64;

    // Clamp at point of use — callers may bypass TessellationOptions::new() via public fields
    // or deserialization, so we defensively enforce minimum viable values here.
    let n_u = opts.angular_segments.max(3);
    let n_v = opts.axial_segments.max(1);

    let base_idx = mesh.positions.len() as u32;

    // Sample UV grid: (n_u+1) × (n_v+1) vertices
    // u goes from u_min to u_min + 2π, v goes from v_min to v_max
    let du = 2.0 * PI / n_u as f64;
    let dv = (v_max - v_min) / n_v as f64;

    for iv in 0..=n_v {
        for iu in 0..=n_u {
            let u = u_min + du * iu as f64;
            let v = v_min + dv * iv as f64;
            let p = face.surface.evaluate(u, v);
            let normal = face.surface.normal_at(u, v);
            let n = if face.same_sense {
                [normal.x, normal.y, normal.z]
            } else {
                [-normal.x, -normal.y, -normal.z]
            };
            mesh.positions.push([p.x, p.y, p.z]);
            mesh.normals.push(n);
        }
    }

    // Generate triangles for each cell
    for iv in 0..n_v {
        for iu in 0..n_u {
            let i00 = base_idx + (iv * (n_u + 1) + iu) as u32;
            let i10 = base_idx + (iv * (n_u + 1) + iu + 1) as u32;
            let i01 = base_idx + ((iv + 1) * (n_u + 1) + iu) as u32;
            let i11 = base_idx + ((iv + 1) * (n_u + 1) + iu + 1) as u32;

            // Two triangles per cell, orientation so ∂u×∂v points outward
            push_triangle(mesh, i00, i10, i01);
            push_triangle(mesh, i10, i11, i01);
        }
    }

    Ok(())
}

/// Push a triangle, but skip degenerate (zero-area) ones.
fn push_triangle(mesh: &mut TriangleMesh, i0: u32, i1: u32, i2: u32) {
    let p0: [f64; 3] = mesh.positions[i0 as usize];
    let p1: [f64; 3] = mesh.positions[i1 as usize];
    let p2: [f64; 3] = mesh.positions[i2 as usize];
    let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
    let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
    let cross_norm = (u[1] * v[2] - u[2] * v[1]).powi(2)
        + (u[2] * v[0] - u[0] * v[2]).powi(2)
        + (u[0] * v[1] - u[1] * v[0]).powi(2).sqrt();
    if cross_norm < AREA_EPS * AREA_EPS {
        return;
    }
    mesh.indices.push(i0);
    mesh.indices.push(i1);
    mesh.indices.push(i2);
}

/// Surface kind name for error messages.
trait SurfaceKind {
    fn kind_name(&self) -> &'static str;
}

impl SurfaceKind for Surface {
    fn kind_name(&self) -> &'static str {
        match self {
            Surface::Plane { .. } => "plane",
            Surface::Cylinder { .. } => "cylinder",
            Surface::Sphere { .. } => "sphere",
            Surface::Cone { .. } => "cone",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::topology::IdGenerator;
    use crate::geometry::Vec3;
    use crate::primitives::make_cuboid;
    use crate::primitives::make_cylinder;

    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
    #[test]
    fn test_tessellate_cylinder_triangle_count() {
        let mut gen = IdGenerator::new(0);
        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
        let mesh = tessellate_solid(&solid).unwrap();

        // Lateral: 32 angular segments × 1 axial × 2 triangles = 64
        // Each cap: 32-gon fan = 30 triangles (32 - 2)
        // Total: 64 + 30 + 30 = 124
        let expected = 32 * 2 + (32 - 2) * 2;
        assert_eq!(
            mesh.triangle_count(),
            expected,
            "expected {expected} triangles, got {}",
            mesh.triangle_count()
        );
    }

    /// T07: Determinism — same input → same output.
    #[test]
    fn test_tessellation_cylinder_deterministic() {
        let mut gen1 = IdGenerator::new(0);
        let mut gen2 = IdGenerator::new(0);

        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();

        let m1 = tessellate_solid(&s1).unwrap();
        let m2 = tessellate_solid(&s2).unwrap();

        assert_eq!(m1.positions, m2.positions);
        assert_eq!(m1.normals, m2.normals);
        assert_eq!(m1.indices, m2.indices);
    }

    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
    #[test]
    fn test_tessellate_cuboid_unchanged() {
        let mut gen = IdGenerator::new(0);
        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
        let mesh = tessellate_solid(&solid).unwrap();

        assert_eq!(mesh.triangle_count(), 12);
        assert_eq!(mesh.positions.len(), 24);
        assert_eq!(mesh.normals.len(), 24);
        assert_eq!(mesh.indices.len(), 36);
    }

    /// T08 extended: exact positions and indices snapshot.
    #[test]
    fn test_cuboid_positions_indices_snapshot() {
        let mut gen1 = IdGenerator::new(0);
        let mut gen2 = IdGenerator::new(0);

        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);

        let m1 = tessellate_solid(&s1).unwrap();
        let m2 = tessellate_solid(&s2).unwrap();

        // Byte-identical across two runs
        assert_eq!(m1.positions, m2.positions);
        assert_eq!(m1.indices, m2.indices);
    }

    /// T09: Outward normals — all facet normals point away from centroid.
    #[test]
    fn test_cylinder_outward_normals() {
        let mut gen = IdGenerator::new(0);
        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
        let mesh = tessellate_solid(&solid).unwrap();

        // Centroid of cylinder: (0, 0, 10)
        let centroid = [0.0f64, 0.0, 10.0];

        for tri in 0..mesh.triangle_count() {
            let i0 = mesh.indices[tri * 3] as usize;
            let i1 = mesh.indices[tri * 3 + 1] as usize;
            let i2 = mesh.indices[tri * 3 + 2] as usize;

            let p0 = mesh.positions[i0];
            let p1 = mesh.positions[i1];
            let p2 = mesh.positions[i2];

            // Facet center
            let center = [
                (p0[0] + p1[0] + p2[0]) / 3.0,
                (p0[1] + p1[1] + p2[1]) / 3.0,
                (p0[2] + p1[2] + p2[2]) / 3.0,
            ];

            // Cross product for facet normal
            let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
            let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
            let nx = u[1] * v[2] - u[2] * v[1];
            let ny = u[2] * v[0] - u[0] * v[2];
            let nz = u[0] * v[1] - u[1] * v[0];

            // Vector from centroid to facet center
            let dx = center[0] - centroid[0];
            let dy = center[1] - centroid[1];
            let dz = center[2] - centroid[2];

            // Dot product should be positive (normal points outward)
            let dot = nx * dx + ny * dy + nz * dz;
            assert!(
                dot > 0.0,
                "facet {tri}: normal should point outward (dot={dot})"
            );
        }
    }

    /// T12: Resolution externalization + clamping.
    #[test]
    fn test_tessellation_options_clamp() {
        let opts = TessellationOptions::new(2, 0);
        assert_eq!(opts.angular_segments, 3);
        assert_eq!(opts.axial_segments, 1);
    }

    #[test]
    fn test_different_resolution_different_count() {
        let mut gen = IdGenerator::new(0);
        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();

        let mesh_default = tessellate_solid(&solid).unwrap();
        let mesh_low = tessellate_solid_with(&solid, &TessellationOptions::new(8, 1)).unwrap();
        let mesh_hi = tessellate_solid_with(&solid, &TessellationOptions::new(64, 2)).unwrap();

        assert_ne!(mesh_default.triangle_count(), mesh_low.triangle_count());
        assert_ne!(mesh_default.triangle_count(), mesh_hi.triangle_count());

        // 8 angular, 1 axial: lateral=8*1*2=16, caps=(8-2)*2=12 → 28
        assert_eq!(mesh_low.triangle_count(), 8 * 2 + (8 - 2) * 2);
        // 64 angular, 2 axial: lateral=64*2*2=256, caps=(64-2)*2=124 → 380
        assert_eq!(mesh_hi.triangle_count(), 64 * 2 * 2 + (64 - 2) * 2);
    }

    /// T15: Unsupported surface type → error.
    #[test]
    fn test_unsupported_surface_error() {
        use crate::brep::topology::Solid as S;
        use crate::geometry::curve::Curve;

        let mut s = S::new(0);
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0));
        let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0));
        let e0 = s.add_edge(
            3,
            [v0, v1],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
        );
        let he0 = s.add_half_edge(4, v0, e0, true);
        let lp = s.add_loop(5, vec![he0]);
        s.add_face(
            6,
            Surface::Sphere {
                center: Point::origin(),
                radius: 1.0,
            },
            lp,
            vec![],
            true,
        );
        s.add_shell(7, vec![0], true);

        let result = tessellate_solid(&s);
        assert!(matches!(
            result,
            Err(TessellationError::UnsupportedSurface { kind: "sphere" })
        ));
    }

    // Legacy tests preserved for backward compatibility
    #[test]
    fn test_tessellate_cuboid() {
        let mut id_gen = IdGenerator::new(0);
        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
        let mesh = tessellate_solid(&solid).unwrap();

        assert_eq!(mesh.triangle_count(), 12);
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

        let mesh1 = tessellate_solid(&solid1).unwrap();
        let mesh2 = tessellate_solid(&solid2).unwrap();

        assert_eq!(mesh1.positions, mesh2.positions);

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
/bin/bash -lc "sed -n '1,220p' CLAUDE.md" in /home/bacon/mycad
 succeeded in 0ms:
# MyCad — Claude Code ガイドライン

## AI作業ルール

- 推測による作業を禁止。必ず検索や調査を行い、事実に基づいて作業すること。
- プランモードではユーザーの許可があるまでプランファイルの作成を禁止。
- プランモードではまず議論や壁打ちを行い、適宜質問や反論を通じて客観的な視点から議論すること。

## セッション開始時のチェックリスト

- `ROADMAP.md` を読み、現在の Phase と完了条件を把握する
- `gh issue list --state open` で open issue を確認する
- ユーザーが特定 Issue を指定しない場合は、現在 Phase の Milestone に紐づく未着手 Issue を提案する
- Issue に着手する際は commit message に `Closes #N` を含めてメインブランチへ直接 push する (branch・PR は不要)
- ただし PR を作る場合は作成直後にセルフマージしてブランチを削除する

## Project Overview

MyCadはRust製のオープンソースB-rep (Boundary Representation) CADカーネル。
設計原則: **決定性** (同じ入力は常に同じ出力)、**パラメトリック履歴** (feature historyが真実の源)、**YAML-based format** (人間可読・diff可能)。

## Project Map

```
crates/
├── mycad-kernel/     B-rep幾何カーネル
│   ├── brep/         トポロジー (Solid, Shell, Face, Loop, Edge, HalfEdge, Vertex)
│   ├── geometry/     幾何型 (Point, Vec3, Plane, Curve, Surface) + 共有数学ユーティリティ
│   ├── primitives/   プリミティブ生成 (make_cuboid)
│   └── tessellation/ メッシュ化 (tessellate_solid → TriangleMesh)
├── mycad-format/     .mycadファイルのYAMLスキーマ・パーサー
│   ├── document.rs   Document (トップレベル)
│   ├── feature.rs    Feature enum (操作履歴の1ステップ)
│   └── component.rs  Component (設計階層)
├── mycad-build/      Featureディスパッチャー (format → kernel の橋渡し)
├── mycad-cli/        CLIバイナリ (mycad)
├── mycad-viewer/     3Dビューア (未実装)
└── xtask/            ビルド自動化タスク
```

## Build & Test Commands

```bash
cargo build --workspace          # 全クレートビルド
cargo test --workspace           # 全テスト実行
cargo clippy --workspace -- -D warnings  # lint
cargo fmt --all -- --check       # フォーマットチェック
cargo xtask ci                   # 上記すべてを順次実行
```

## Conventions

- **決定性**: すべてのEntityIDは `IdGenerator` で決定的に生成。テストでは決定性を検証すること。
- **依存管理**: 新しい依存は必ず `[workspace.dependencies]` に追加し、各クレートは `{ workspace = true }` で参照。
- **derive規約**: 公開型には `Debug, Clone, Serialize, Deserialize` を付与。スキーマ生成が必要な型は `JsonSchema` も追加。
- **エラーハンドリング**: ライブラリエラーには `thiserror` を使用。
- **モジュール構造**: 1概念1ファイル。重要な型は `mod.rs` から `pub use` で再エクスポート。
- **テスト**: 単体テストは `#[cfg(test)] mod tests` でインライン。統合テストは `tests/` ディレクトリ。

## Architecture Principles

- **Index-based topology**: `Solid` 内のトポロジーエンティティはフラット配列に格納し、インデックスで参照（ポインタ不使用）。
- **Feature history = source of truth**: `mycad-format` の Feature 列が設計の真実の源。B-rep は Feature から再生成される。
- **Kernel has zero rendering deps**: カーネルはレンダリング非依存。`TriangleMesh` を生成し、ビューアが消費する。
- **Flat crates layout**: `crates/` 直下にフラットに配置。深いネストは作らない。

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
/bin/bash -lc "sed -n '1,240p' crates/mycad-format/src/error.rs" in /home/bacon/mycad
 succeeded in 0ms:
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("invalid file extension: expected .mycad, got {0:?}")]
    InvalidExtension(Option<String>),

    #[error("invalid reference {value:?}: {reason}")]
    InvalidReference { value: String, reason: &'static str },
}

exec
/bin/bash -lc "sed -n '1,220p' docs/decisions/003-viewer-and-app-architecture.md" in /home/bacon/mycad
 succeeded in 0ms:
# ADR-003: ビューア・アプリ全体アーキテクチャの選定

## Status

Accepted

## Context

Phase 2 (Viewer) 着手にあたり、最終形を見据えたアーキテクチャを決定する必要が生じた。

**最終形のビジョン**:
- Fusion/FreeCAD 的な対話 CAD アプリ (人間が直感的に操作)
- CLI/API で AI・スクリプトも操作可能
- OSS として配布可能。ローカル完結でも self-hosted でも動く

**開発制約**:
- 現在はリモート Linux 環境で開発。ローカルは Ubuntu デスクトップ。将来 Windows / macOS でも動かしたい。
- ローカル完結 (デスクトップアプリ) と self-hosted Onshape 的利用の両取りが理想。

## Decision

**Rust core + Rust API 層 + TS フロント (型は Rust から自動生成) + 二配備 (Tauri / server)**

```
[ Rust Core: kernel / format / build ]  ← 既存。据え置き。Feature 履歴が真実。
          ▲                   ▲
   embed  │            HTTP/WS │ 公開
┌─────────┴──────┐   ┌─────────┴──────────┐
│ Tauri アプリ    │   │  サーバモード        │
│ ローカル完結    │   │  ブラウザ/出先       │
│ 1 クリック起動  │   │  複数人接続可        │
│ native shortcut│   │  self-hosted 可      │
└─────────┬──────┘   └─────────┬──────────┘
          └──── 同じ TS フロント ────┘
                  (型は Rust から自動生成)
```

CLI / AI / スクリプトはすべて同じ **API 層**を経由する。GUI も API の一クライアントに過ぎない。

## Rationale

### なぜ Rust backend を維持するか (TS backend を採用しない理由)

- kernel は既に Rust で稼働しており、B-rep トポロジ・決定的 ID・曲面テッセレーションまで実装済みの主要資産を廃棄することになる。
- CAD kernel は性能要求が高い (テッセレーション、Boolean、大規模アセンブリ)。Rust ネイティブが圧倒的に有利。
- プロジェクト第一原則「決定性」は Rust + nalgebra で制御しやすい。
- 「型統一」の旨味は **ts-rs / JsonSchema → TS による型自動生成**で代替できる (Rust 型定義を真実とした型安全な TS 型)。

### なぜ純ブラウザ (サーバ必須) を採用しないか

- ローカル完結で使うには「サーバ起動 + ブラウザ起動」2 プロセスが必要。
- ブラウザは複雑な CAD ショートカットキーとの相性が悪い。
- ローカルファイルへの直接アクセスが制限される。
- Tauri はこれら 3 点を解消しつつ、同じ TS フロントをそのまま使える。

### なぜ Tauri + server の両取りが可能か

- TS フロントは 1 つ。Tauri は embedded Rust core を直接呼ぶ (追加サーバプロセス不要)。
- server モードは同じ Rust core を axum 等で公開し、同じ TS フロントを配信する。
- Windows / macOS 配布は Tauri のクロスビルドで対応可能。
- server の multi-user 対応: `.mycad` は YAML で Git 管理可能な文字列 Feature 履歴。realtime 協調編集 (OT/CRDT) が不要 (Git で合流) なため、サーバ実装を大幅に簡略化できる。

## 変更モデル: Feature コマンド一本化

GUI / CLI / AI / スクリプトを問わず、**モデルを変える操作はすべて Feature コマンドとして履歴に積む**。

| 種別 | 扱い |
|---|---|
| Feature の追加・編集・削除 (`.mycad` が変わる操作) | 履歴に積む。undo スタック対象。 |
| カメラ位置・ズーム・選択/ホバー | transient 状態。セッション内のみ。`.mycad` にも undo にも含めない。 |
| ドラッグ中プレビュー | transient。ジェスチャー確定時に 1 Feature として coalesce する (毎フレーム積まない)。 |

この一本化により undo/redo・スクリプト記録・AI 操作・決定的リプレイ・増分再生成キャッシュが自然に成立する。

## API 契約 v0 の方針

- **transport**: HTTP (REST) をベースとし、push/preview 用に WebSocket を追加。
- **versioning**: URL prefix `/api/v0/`。破壊的変更時は `/v1/` に移行。
- **操作**: Feature コマンドベース (CRUD on feature list) + tessellation/query エンドポイント。
- **TS 型**: `ts-rs` または JSON Schema → TS で Rust 型から自動生成。フロントは Rust 型定義を真実とする。

## 関連懸念と委譲先

| 懸念 | 委譲先 |
|---|---|
| トポロジカル・ネーミング (生 index 参照禁止、Phase 3 前提) | **ADR-005** |
| `schema_version` / kernel version スタンプ / migration | Issue I-2 |
| 単位系・グローバル公差 | ADR-004 (一部)、Issue I-4 |
| API 契約詳細 | 本 ADR の方針に基づき Phase 2 実装時に確定 |
| 増分再生成 (Feature 純関数保証) | 変更モデルの必然的帰結。将来最適化フェーズで有効化 |
| Tauri 包装・server 認証 | Issue 化は当該 Phase 着手時 (空想 Issue 作らない) |

## Implementation Details

- Phase 2 の最小実装: Rust HTTP サーバ + TS フロント (Three.js) の骨組み。`mycad view` = サーバ起動 + ブラウザ自動起動。
- `TriangleMesh` への face グループ情報付与 (ピッキングの前提) は Phase 3 の `Extrude` 実装前に対応する。
- Tauri 包装は Phase 2 完了後の発展的ステップ。Issue 化は着手時。

exec
/bin/bash -lc "sed -n '1,220p' examples/simple_box.mycad" in /home/bacon/mycad
 succeeded in 0ms:
version: "0.1.0"
root_component:
  name: "Simple Box"
  features:
    - type: create_box
      id: box_1
      width: 10.0
      height: 20.0
      depth: 30.0

exec
/bin/bash -lc "sed -n '1,220p' examples/cylinder.mycad" in /home/bacon/mycad
 succeeded in 0ms:
version: "0.1.0"
root_component:
  name: "Simple Cylinder"
  features:
    - type: create_cylinder
      id: cyl_1
      radius: 5.0
      height: 20.0

exec
/bin/bash -lc "sed -n '1,220p' examples/assembly.mycad" in /home/bacon/mycad
 succeeded in 0ms:
version: "0.1.0"
root_component:
  name: "Simple Assembly"
  children:
    - name: "Base Plate"
      features:
        - type: create_box
          id: plate_1
          width: 100.0
          height: 5.0
          depth: 100.0
    - name: "Pillar"
      transform:
        position: [0.0, 25.0, 0.0]
        rotation: [0.0, 0.0, 0.0]
      features:
        - type: create_box
          id: pillar_1
          width: 10.0
          height: 50.0
          depth: 10.0
    - name: "Bolt"
      transform:
        position: [20.0, 5.0, 20.0]
        rotation: [0.0, 0.0, 0.0]
      ref: "stdlib://fasteners/jis_b1176/M5x20"

codex
既存の round 2 ドキュメント自体が残っているので、前回指摘の未解消点がないかも照合します。ここは実装ではなくレビュー整合性の確認です。
exec
/bin/bash -lc "sed -n '1,160p' features/18-api-skeleton/design-review-2.md" in /home/bacon/mycad
 succeeded in 0ms:
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
session id: 019e5dbc-18e2-7360-a289-2a836eea3bb6
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
# Issue #18: API 層スケルトン (mycad-api / axum)

## Context

Phase 2「Web ベースビューア基盤」の土台として、CLI / GUI / AI 共通の操作面 v0 を Rust HTTP サーバとして立てる。`.mycad` を読み込み、テッセレーションした `TriangleMesh` を JSON で返す最小 API を `crates/mycad-api/` に新規実装する。後続 Issue #20(Three.js フロント)・#21(`mycad view`)がこの API を消費する前提。

既存 CLI の `run_export`(`crates/mycad-cli/src/main.rs:47-67`)が同じ Document→Solid→Mesh フローを既に持つため、STL 化を除いて再利用する。

> Codex 設計レビュー(round 1, verdict=fail)反映済み。R01/R03/R04 受諾、R02 は kernel 検証追加で対応(ユーザー決定)、R05 は部分受諾(下記)。

## 実装対象

- 新規クレート `crates/mycad-api/`(lib + bin 構成)+ `make_cuboid` の入力検証(kernel)
- Issue: #18 / Milestone: Phase 2

### v0 スコープ明確化(R03)
- 対応するのは **`root_component` に単一 part feature を持つ文書のみ**(Box / Cylinder)。
- assembly(`children`/`reference` を持ち root features が空)や複数フィーチャは v0 非対応。明確なエラー + 422 系で返す(クラッシュさせない)。

### 影響ファイル

| ファイル | 変更 |
|----------|------|
| `crates/mycad-kernel/src/primitives/cuboid.rs` | **(R02)** `make_cuboid` を `-> Result<Solid, KernelError>` 化。`dx/dy/dz` が非有限 or `<= 0` なら `InvalidParameter { kind }`(`make_cylinder` cylinder.rs:16-22 と対称)。インライン test の呼び出しを `.unwrap()` 化 |
| `crates/mycad-build/src/lib.rs` | CreateBox 分岐を `Ok(make_cuboid(...))` → `make_cuboid(...)`(`?` 伝播)に変更 |
| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
| `Cargo.toml`(ルート) | members に `"crates/mycad-api"` 追加。`[workspace.dependencies]` に `axum`/`tokio`/`serde_json`/`tower`(dev) 追加 |
| `crates/mycad-api/Cargo.toml` | 新規。`version.workspace = true` 等の既存規約。`[[bin]]` name=`mycad-api` |
| `crates/mycad-api/src/lib.rs` | 新規。**(R04)** `pub use` 中心の薄い再エクスポートのみ |
| `crates/mycad-api/src/router.rs` | 新規。`pub fn app() -> Router`(`/api/v0` を `nest`) |
| `crates/mycad-api/src/handler.rs` | 新規。`get_mesh` ハンドラ + パス解決ロジック |
| `crates/mycad-api/src/error.rs` | 新規。`ApiError` + `IntoResponse` |
| `crates/mycad-api/src/main.rs` | 新規。`#[tokio::main]`、`127.0.0.1:3000` バインド |
| `crates/mycad-api/tests/mesh_api.rs` | 新規。tower `oneshot` 統合テスト + fixtures |
| `crates/mycad-api/tests/fixtures/*.mycad` | 新規。退化/未対応ケース用 fixture(R02/R03 テスト用) |

### シグネチャ

```rust
// router.rs
pub fn app() -> axum::Router;

// handler.rs
#[derive(serde::Deserialize)]
struct MeshQuery { file: String }
async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;

// error.rs
enum ApiError { BadExtension(String), NotFound, Unsupported(String), Build(String), Tessellate(String), Internal(String) }
impl IntoResponse for ApiError { /* status + {"error": "..."} */ }
```

## 設計方針

### 再利用(新規ロジックを書かない)
- `Document::from_path`(`crates/mycad-format/src/document.rs:36`)。`.mycad` 拡張子を強制(InvalidExtension)。
- `build_solid_from_features`(`crates/mycad-build/src/lib.rs:6`)。単一フィーチャ・Box/Cylinder のみ。
- `tessellate_solid_with`(`crates/mycad-kernel/src/tessellation/mod.rs:90`)。v0 は `TessellationOptions::default()` 固定。
- `TriangleMesh`(mod.rs:16, Serialize 済み)→ `Json(mesh)`。
- ハンドラ本体は CLI `run_export`(main.rs:47-67)から STL 化を除き末尾を `Ok(Json(mesh))` に置換。

### パス解決の契約(R01)
- `file` クエリは **絶対パスならそのまま、相対パスならサーバプロセスの CWD(`std::env::current_dir`)基準**で解決する、と明文化。
- bin は起動時に effective base(CWD)をログ出力。
- テストは `env!("CARGO_MANIFEST_DIR")` 起点で絶対パスを組み立て(`crates/mycad-api` から `../../examples/...`)、CWD 非依存にする。
- E2E 例(`examples/...` 相対)はリポジトリルートを CWD として起動する前提と注記。

### エンドポイント / バージョニング
- `GET /api/v0/mesh?file=<path>`。`Router::nest("/api/v0", ...)`。クエリは `file` のみ(v0 スコープ)。解像度は default 固定。

### パス安全性(多層防御 — 任意位置の .mycad 読取とセキュリティを両立)
1. **CWD ジェイルなし** → 任意パスの `.mycad` を読める(要望)。
2. **`.mycad` 拡張子限定**(`Document::from_path` が既に強制)→ 機密ファイル読取を遮断。
3. **`canonicalize` + 実ファイル存在確認** → 無ければ 404、ディレクトリ等は 400。
4. **`127.0.0.1` のみバインド** → ネットワークから到達不可。
5. **Host ヘッダ検証ミドルウェア** → loopback 以外は 403(DNS リバインディング対策)。

### エラーハンドリング
- `FormatError`/`KernelError`/`TessellationError` は `Serialize` 非対応 → `ApiError` に `.to_string()` で集約し `IntoResponse` で status + `{"error": "..."}`。
- status マッピング: ファイル不在 → 404 / 非.mycad 拡張子・不正パラメータ → 400 / `InvalidParameter`(退化寸法)・`UnsupportedFeature`・`MultipleFeatures`・`EmptyFeatureList`・未対応サーフェス → 422 / その他 → 500。

### 依存・規約
- `[workspace.dependencies]` に新規登録: `axum = "0.8"`、`tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }`、`serde_json = "1"`、`tower = "0.5"`(dev、`oneshot`)。各クレートは `{ workspace = true }`。
- derive 規約・`thiserror`・`version.workspace = true` を踏襲。clippy `-D warnings` 通過必須。

## テスト計画

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
/bin/bash -lc 'rg -n "tolerance|公差|epsilon|eps" docs ROADMAP.md ARCHITECTURE.md crates -S' in /home/bacon/mycad
 succeeded in 0ms:
docs/decisions/003-viewer-and-app-architecture.md:88:| 単位系・グローバル公差 | ADR-004 (一部)、Issue I-4 |
docs/decisions/004-freeform-geometry-commitment.md:30:3. **数値モデル (トレラント方式 vs 厳密方式) は本 ADR では保留**し、曲面 Boolean を実装する Phase 4 着手時に決定する。自由曲面の交線は数値的近似になるため、業界カーネル (Parasolid / ACIS) はエンティティ毎に公差を持つトレラントモデルを採る。これを後から全エンティティ・全比較へ導入するのは大改修であり、Phase 4 で意思決定する論点として明示しておく。なお `CLAUDE.md` の **決定性** 原則とは両立可能 (アルゴリズム固定で再現できる) だが、数値ロバスト性の確保が決定性維持の難所になる点に留意する。
crates/mycad-kernel/src/tessellation/mod.rs:14:const AREA_EPS: f64 = 1e-14;
crates/mycad-kernel/src/tessellation/mod.rs:195:    // (e.g. two π-arcs per cap) while still rejecting trimmed faces. 1e-9 tolerance is tight
crates/mycad-kernel/src/tessellation/mod.rs:286:    if cross_norm < AREA_EPS * AREA_EPS {
crates/mycad-kernel/src/geometry/surface.rs:191:        assert_relative_eq!(u, u2, epsilon = 1e-9);
crates/mycad-kernel/src/geometry/surface.rs:192:        assert_relative_eq!(v, v2, epsilon = 1e-9);
crates/mycad-kernel/src/geometry/surface.rs:262:        assert_relative_eq!(n1.dot(&n2), 1.0, epsilon = 1e-12);
crates/mycad-kernel/src/primitives/cylinder.rs:205:        let eps = 1e-10;
crates/mycad-kernel/src/primitives/cylinder.rs:241:                    (s.vertices[end_v].point - s.vertices[next_start].point).norm() < eps,
crates/mycad-kernel/src/primitives/cylinder.rs:259:        let eps = 1e-10;
crates/mycad-kernel/src/primitives/cylinder.rs:262:        assert!((s.vertices[0].point - Point::new(0.0, 5.0, 0.0)).norm() < eps);
crates/mycad-kernel/src/primitives/cylinder.rs:263:        assert!((s.vertices[1].point - Point::new(0.0, 5.0, 20.0)).norm() < eps);
crates/mycad-kernel/src/primitives/cylinder.rs:269:                    (*normal + Vec3::z()).norm() < eps,
crates/mycad-kernel/src/primitives/cylinder.rs:280:                    (*normal - Vec3::z()).norm() < eps,
crates/mycad-kernel/src/primitives/cylinder.rs:292:                assert!((r - 5.0).abs() < eps);
crates/mycad-kernel/src/primitives/cylinder.rs:293:                assert!((*axis - Vec3::z()).norm() < eps);
crates/mycad-kernel/src/geometry/math.rs:28:        assert_relative_eq!(u.norm(), 1.0, epsilon = 1e-12);
crates/mycad-kernel/src/geometry/math.rs:29:        assert_relative_eq!(v.norm(), 1.0, epsilon = 1e-12);
crates/mycad-kernel/src/geometry/math.rs:30:        assert_relative_eq!(u.dot(&v), 0.0, epsilon = 1e-12);
crates/mycad-kernel/src/geometry/math.rs:31:        assert_relative_eq!(u.dot(&Vec3::z()), 0.0, epsilon = 1e-12);
crates/mycad-kernel/src/geometry/math.rs:37:        assert_relative_eq!(u.norm(), 1.0, epsilon = 1e-12);
crates/mycad-kernel/src/geometry/math.rs:38:        assert_relative_eq!(v.norm(), 1.0, epsilon = 1e-12);
crates/mycad-kernel/src/geometry/math.rs:39:        assert_relative_eq!(u.dot(&v), 0.0, epsilon = 1e-12);
crates/mycad-kernel/src/geometry/math.rs:40:        assert_relative_eq!(u.dot(&Vec3::x()), 0.0, epsilon = 1e-12);
crates/xtask/src/main.rs:25:    let steps: &[(&str, &[&str])] = &[
crates/xtask/src/main.rs:38:    for (label, cmd) in steps {

codex
issues:
  - id: R01
    severity: high
    section: "設計方針 > エラーハンドリング"
    finding: "`ApiError` を `Build(String)` / `Tessellate(String)` へ `.to_string()` で畳み込む設計だと、`KernelError::InvalidParameter` と `UnsupportedFeature`/`MultipleFeatures`/`EmptyFeatureList`、`TessellationError::UnsupportedSurface` と 500 系を型で区別できない。文書が約束する 400/404/422/500 の status マッピングが文字列解析頼みになる。"
    suggestion: "`thiserror` で構造化した `ApiError` を定義し、`From<FormatError/KernelError/TessellationError>` で 400/404/422/500 を確定させること。`IntoResponse` では status を型から決め、body だけ error JSON に整形すること。"
  - id: R02
    severity: medium
    section: "実装対象 > v0 スコープ明確化(R03)"
    finding: "`build_solid_from_features(&doc.root_component.features, ...)` の再利用方針と T09 の `EmptyFeatureList` 期待値では、`children` / `ref` を持つ assembly/reference 文書と、本当に feature が空の不正 part 文書を区別できない。これでは文書自身が要求している『assembly は明確なエラー + 422』を満たせない。"
    suggestion: "ハンドラで `root_component.reference` / `children` / `features.len()` を先に検査し、assembly/reference 専用の `ApiError::Unsupported(...)` を返すこと。`EmptyFeatureList` は『part なのに feature が空』に限定すること。"
  - id: R03
    severity: medium
    section: "影響ファイル > crates/mycad-kernel/src/primitives/cuboid.rs"
    finding: "`make_cuboid` の退化入力対策が `!is_finite() || <= 0` だけで、近接ゼロの正値をどう扱うか、既存の `AREA_EPS` や 1e-9 系判定とどう整合させるかが明記されていない。非常に小さい正寸法は B-rep 生成を通っても、後段テッセレーションで実質的に縮退しうる。"
    suggestion: "この Issue で exact-zero 方針を採るなら理由と影響範囲を明記し、少なくとも near-zero ケースを K01/API fixture に追加すること。可能なら共有公差定数を導入し `<= EPS` を `InvalidParameter` に統一すること。"
  - id: R04
    severity: medium
    section: "テスト計画"
    finding: "golden YAML ラウンドトリップが既存 `examples/*` だけに依存しており、新規 `crates/mycad-api/tests/fixtures/*.mycad` の unsupported/退化 fixture については format crate 側の roundtrip 保証が計画されていない。API が依存する入力形状の YAML 安定性が未検証のまま残る。"
    suggestion: "`mycad-format` 側に新規 fixture を読み込む roundtrip/golden テストを追加するか、API fixture を共有して `to_yaml`/`from_yaml` の安定性を format crate の所有テストで担保すること。"

verdict: fail
tokens used
155,341
issues:
  - id: R01
    severity: high
    section: "設計方針 > エラーハンドリング"
    finding: "`ApiError` を `Build(String)` / `Tessellate(String)` へ `.to_string()` で畳み込む設計だと、`KernelError::InvalidParameter` と `UnsupportedFeature`/`MultipleFeatures`/`EmptyFeatureList`、`TessellationError::UnsupportedSurface` と 500 系を型で区別できない。文書が約束する 400/404/422/500 の status マッピングが文字列解析頼みになる。"
    suggestion: "`thiserror` で構造化した `ApiError` を定義し、`From<FormatError/KernelError/TessellationError>` で 400/404/422/500 を確定させること。`IntoResponse` では status を型から決め、body だけ error JSON に整形すること。"
  - id: R02
    severity: medium
    section: "実装対象 > v0 スコープ明確化(R03)"
    finding: "`build_solid_from_features(&doc.root_component.features, ...)` の再利用方針と T09 の `EmptyFeatureList` 期待値では、`children` / `ref` を持つ assembly/reference 文書と、本当に feature が空の不正 part 文書を区別できない。これでは文書自身が要求している『assembly は明確なエラー + 422』を満たせない。"
    suggestion: "ハンドラで `root_component.reference` / `children` / `features.len()` を先に検査し、assembly/reference 専用の `ApiError::Unsupported(...)` を返すこと。`EmptyFeatureList` は『part なのに feature が空』に限定すること。"
  - id: R03
    severity: medium
    section: "影響ファイル > crates/mycad-kernel/src/primitives/cuboid.rs"
    finding: "`make_cuboid` の退化入力対策が `!is_finite() || <= 0` だけで、近接ゼロの正値をどう扱うか、既存の `AREA_EPS` や 1e-9 系判定とどう整合させるかが明記されていない。非常に小さい正寸法は B-rep 生成を通っても、後段テッセレーションで実質的に縮退しうる。"
    suggestion: "この Issue で exact-zero 方針を採るなら理由と影響範囲を明記し、少なくとも near-zero ケースを K01/API fixture に追加すること。可能なら共有公差定数を導入し `<= EPS` を `InvalidParameter` に統一すること。"
  - id: R04
    severity: medium
    section: "テスト計画"
    finding: "golden YAML ラウンドトリップが既存 `examples/*` だけに依存しており、新規 `crates/mycad-api/tests/fixtures/*.mycad` の unsupported/退化 fixture については format crate 側の roundtrip 保証が計画されていない。API が依存する入力形状の YAML 安定性が未検証のまま残る。"
    suggestion: "`mycad-format` 側に新規 fixture を読み込む roundtrip/golden テストを追加するか、API fixture を共有して `to_yaml`/`from_yaml` の安定性を format crate の所有テストで担保すること。"

verdict: fail
