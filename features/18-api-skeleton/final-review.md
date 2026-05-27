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
session id: 019e5dd9-4dfa-7613-afa0-2129328d30f0
--------
user
# Codex 最終コードレビュアー（MyCad CAD カーネル専用）

あなたは Rust 製 B-rep CAD カーネル「MyCad」の実装差分をレビューする専門家です。
`git diff` または `codex review` が提供するコード差分を読み、以下の観点で問題を指摘してください。

## レビュー観点

### 1. 決定性（最重要）
- `IdGenerator` 以外の ID 生成（`Uuid::new_v4()`、`rand`、timestamp 等）が混入していないか
- HashMap/HashSet のイテレーション順に依存した処理がないか
- `#[test]` 内で決定性を検証しているか（同一入力で2回実行→結果一致）

### 2. B-rep トポロジー正確性
- HalfEdge の twin/next/prev インデックスが循環的に正しく設定されているか
- Euler-Poincaré の不変条件が保たれているか
- Face の Loop リスト、Shell の Face リストが整合しているか

### 3. 数値・退化幾何
- ゼロ長エッジ、縮退ポリゴン、coincident vertices の検出・エラー処理があるか
- f64 の直接比較（`==`）を使っていないか（epsilon 比較を使用しているか）

### 4. Rust / MyCad 規約
- `clippy -D warnings` を通過するコードか（unwrap()、expect()、unused 変数等）
- `Debug, Clone, Serialize, Deserialize` が公開型に付いているか
- 新規依存が `[workspace.dependencies]` に追加され `{ workspace = true }` で参照されているか
- エラー型が `thiserror` で定義されているか
- テストは `#[cfg(test)] mod tests` でインラインか、統合テストは `tests/` か

### 5. テスト充足性
- 設計レビュー時に合意したテスト計画（ID T01〜）が全て実装されているか
- エッジケーステストが退化入力・境界数値を含んでいるか
- golden YAML ラウンドトリップが含まれているか

### 6. スコープ逸脱
- プラン外の機能追加・リファクタが含まれていないか
- カーネル(`mycad-kernel`)にレンダリング依存が混入していないか

---

## 出力フォーマット（厳守）

```yaml
issues:
  - id: F01
    severity: critical  # critical | high | medium | low
    file: "crates/mycad-kernel/src/brep/solid.rs"
    line_hint: 42
    finding: "HalfEdge の twin インデックスが self を指している（ループ不整合）"
    suggestion: "make_cuboid の edge 生成ロジックを見直す"
  - id: F02
    severity: high
    ...

verdict: pass  # pass | fail
# fail = Critical または High が1件以上ある
```

`issues` が空の場合は `issues: []` と書く。
**このフォーマット以外の出力は禁止。前置きや後置きの文章は書かない。**
codex
差分ベースで最終レビューを進めます。まず `git diff` と変更ファイル一覧を確認して、影響範囲とテスト追加の有無を把握します。
exec
/bin/bash -lc 'git diff --name-only' in /home/bacon/mycad
 succeeded in 0ms:
.claude/skills/3ai/SKILL.md
.gitignore
Cargo.lock
Cargo.toml
crates/mycad-build/src/lib.rs
crates/mycad-kernel/benches/tessellation.rs
crates/mycad-kernel/src/primitives/cuboid.rs
crates/mycad-kernel/src/tessellation/mod.rs
crates/mycad-kernel/src/tessellation/stl.rs
crates/mycad-kernel/tests/cuboid_roundtrip.rs

exec
/bin/bash -lc 'git status --short' in /home/bacon/mycad
 succeeded in 0ms:
 M .claude/skills/3ai/SKILL.md
 M .gitignore
 M Cargo.lock
 M Cargo.toml
 M crates/mycad-build/src/lib.rs
 M crates/mycad-kernel/benches/tessellation.rs
 M crates/mycad-kernel/src/primitives/cuboid.rs
 M crates/mycad-kernel/src/tessellation/mod.rs
 M crates/mycad-kernel/src/tessellation/stl.rs
 M crates/mycad-kernel/tests/cuboid_roundtrip.rs
?? .claude/settings.json
?? .claude/skills/3ai/agents/
?? .claude/skills/3ai/scripts/guard-crates.sh
?? crates/mycad-api/
?? features/

exec
/bin/bash -lc 'git diff --stat' in /home/bacon/mycad
 succeeded in 0ms:
 .claude/skills/3ai/SKILL.md                   |   6 +
 .gitignore                                    |   4 +
 Cargo.lock                                    | 365 ++++++++++++++++++++++++++
 Cargo.toml                                    |   7 +
 crates/mycad-build/src/lib.rs                 |   2 +-
 crates/mycad-kernel/benches/tessellation.rs   |   2 +-
 crates/mycad-kernel/src/primitives/cuboid.rs  | 115 +++++++-
 crates/mycad-kernel/src/tessellation/mod.rs   |  12 +-
 crates/mycad-kernel/src/tessellation/stl.rs   |   4 +-
 crates/mycad-kernel/tests/cuboid_roundtrip.rs |   4 +-
 10 files changed, 503 insertions(+), 18 deletions(-)

codex
変更は `mycad-kernel` 周辺に加えて `Cargo.toml` にも入っています。次は差分本体と未追跡ファイルを見て、レビュー対象に含めるべき新規コードがあるか確認します。
exec
/bin/bash -lc 'git ls-files --others --exclude-standard' in /home/bacon/mycad
 succeeded in 0ms:
.claude/settings.json
.claude/skills/3ai/agents/codex-design-reviewer.md
.claude/skills/3ai/agents/codex-final-reviewer.md
.claude/skills/3ai/agents/glm-implementer.md
.claude/skills/3ai/scripts/guard-crates.sh
crates/mycad-api/Cargo.toml
crates/mycad-api/src/error.rs
crates/mycad-api/src/handler.rs
crates/mycad-api/src/lib.rs
crates/mycad-api/src/main.rs
crates/mycad-api/src/router.rs
crates/mycad-api/tests/fixtures/assembly.mycad
crates/mycad-api/tests/fixtures/create_sphere.mycad
crates/mycad-api/tests/fixtures/empty_part.mycad
crates/mycad-api/tests/fixtures/zero_box.mycad
crates/mycad-api/tests/mesh_api.rs
features/12-cylinder/design-review-2.md
features/12-cylinder/design-review-3.md
features/12-cylinder/design-review.md
features/12-cylinder/final-review-2.md
features/12-cylinder/final-review.md
features/18-api-skeleton/ci.log
features/18-api-skeleton/design-review-2.md
features/18-api-skeleton/design-review-3.md
features/18-api-skeleton/design-review.md
features/18-api-skeleton/final-review.md
features/18-api-skeleton/glm-result.json
features/18-api-skeleton/plan.md
features/18-api-skeleton/state.json

exec
/bin/bash -lc 'git diff -- Cargo.toml Cargo.lock crates/mycad-build/src/lib.rs crates/mycad-kernel/benches/tessellation.rs crates/mycad-kernel/src/primitives/cuboid.rs crates/mycad-kernel/src/tessellation/mod.rs crates/mycad-kernel/src/tessellation/stl.rs crates/mycad-kernel/tests/cuboid_roundtrip.rs' in /home/bacon/mycad
 succeeded in 0ms:
diff --git a/Cargo.lock b/Cargo.lock
index 9795377..0dbbc3c 100644
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -67,12 +67,70 @@ dependencies = [
  "num-traits",
 ]
 
+[[package]]
+name = "atomic-waker"
+version = "1.1.2"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "1505bd5d3d116872e7271a6d4e16d81d0c8570876c8de68093a09ac269d8aac0"
+
 [[package]]
 name = "autocfg"
 version = "1.5.0"
 source = "registry+https://github.com/rust-lang/crates.io-index"
 checksum = "c08606f8c3cbf4ce6ec8e28fb0014a2c086708fe954eaa885384a6165172e7e8"
 
+[[package]]
+name = "axum"
+version = "0.8.9"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "31b698c5f9a010f6573133b09e0de5408834d0c82f8d7475a89fc1867a71cd90"
+dependencies = [
+ "axum-core",
+ "bytes",
+ "form_urlencoded",
+ "futures-util",
+ "http",
+ "http-body",
+ "http-body-util",
+ "hyper",
+ "hyper-util",
+ "itoa",
+ "matchit",
+ "memchr",
+ "mime",
+ "percent-encoding",
+ "pin-project-lite",
+ "serde_core",
+ "serde_json",
+ "serde_path_to_error",
+ "serde_urlencoded",
+ "sync_wrapper",
+ "tokio",
+ "tower",
+ "tower-layer",
+ "tower-service",
+ "tracing",
+]
+
+[[package]]
+name = "axum-core"
+version = "0.5.6"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "08c78f31d7b1291f7ee735c1c6780ccde7785daae9a9206026862dab7d8792d1"
+dependencies = [
+ "bytes",
+ "futures-core",
+ "http",
+ "http-body",
+ "http-body-util",
+ "mime",
+ "pin-project-lite",
+ "sync_wrapper",
+ "tower-layer",
+ "tower-service",
+ "tracing",
+]
+
 [[package]]
 name = "bitflags"
 version = "2.11.1"
@@ -85,6 +143,12 @@ version = "1.25.0"
 source = "registry+https://github.com/rust-lang/crates.io-index"
 checksum = "c8efb64bd706a16a1bdde310ae86b351e4d21550d98d056f22f8a7f7a2183fec"
 
+[[package]]
+name = "bytes"
+version = "1.11.1"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "1e748733b7cbc798e1434b6ac524f0c1ff2ab456fe201501e6497c8417a4fc33"
+
 [[package]]
 name = "cfg-if"
 version = "1.0.4"
@@ -171,6 +235,48 @@ version = "0.1.5"
 source = "registry+https://github.com/rust-lang/crates.io-index"
 checksum = "d9c4f5dac5e15c24eb999c26181a6ca40b39fe946cbe4c263c7209467bc83af2"
 
+[[package]]
+name = "form_urlencoded"
+version = "1.2.2"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "cb4cb245038516f5f85277875cdaa4f7d2c9a0fa0468de06ed190163b1581fcf"
+dependencies = [
+ "percent-encoding",
+]
+
+[[package]]
+name = "futures-channel"
+version = "0.3.32"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "07bbe89c50d7a535e539b8c17bc0b49bdb77747034daa8087407d655f3f7cc1d"
+dependencies = [
+ "futures-core",
+]
+
+[[package]]
+name = "futures-core"
+version = "0.3.32"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "7e3450815272ef58cec6d564423f6e755e25379b217b0bc688e295ba24df6b1d"
+
+[[package]]
+name = "futures-task"
+version = "0.3.32"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "037711b3d59c33004d3856fbdc83b99d4ff37a24768fa1be9ce3538a1cde4393"
+
+[[package]]
+name = "futures-util"
+version = "0.3.32"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "389ca41296e6190b48053de0321d02a77f32f8a5d2461dd38762c0593805c6d6"
+dependencies = [
+ "futures-core",
+ "futures-task",
+ "pin-project-lite",
+ "slab",
+]
+
 [[package]]
 name = "getrandom"
 version = "0.4.2"
@@ -205,6 +311,86 @@ version = "0.5.0"
 source = "registry+https://github.com/rust-lang/crates.io-index"
 checksum = "2304e00983f87ffb38b55b444b5e3b60a884b5d30c0fca7d82fe33449bbe55ea"
 
+[[package]]
+name = "http"
+version = "1.4.0"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "e3ba2a386d7f85a81f119ad7498ebe444d2e22c2af0b86b069416ace48b3311a"
+dependencies = [
+ "bytes",
+ "itoa",
+]
+
+[[package]]
+name = "http-body"
+version = "1.0.1"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "1efedce1fb8e6913f23e0c92de8e62cd5b772a67e7b3946df930a62566c93184"
+dependencies = [
+ "bytes",
+ "http",
+]
+
+[[package]]
+name = "http-body-util"
+version = "0.1.3"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "b021d93e26becf5dc7e1b75b1bed1fd93124b374ceb73f43d4d4eafec896a64a"
+dependencies = [
+ "bytes",
+ "futures-core",
+ "http",
+ "http-body",
+ "pin-project-lite",
+]
+
+[[package]]
+name = "httparse"
+version = "1.10.1"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "6dbf3de79e51f3d586ab4cb9d5c3e2c14aa28ed23d180cf89b4df0454a69cc87"
+
+[[package]]
+name = "httpdate"
+version = "1.0.3"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "df3b46402a9d5adb4c86a0cf463f42e19994e3ee891101b1841f30a545cb49a9"
+
+[[package]]
+name = "hyper"
+version = "1.9.0"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "6299f016b246a94207e63da54dbe807655bf9e00044f73ded42c3ac5305fbcca"
+dependencies = [
+ "atomic-waker",
+ "bytes",
+ "futures-channel",
+ "futures-core",
+ "http",
+ "http-body",
+ "httparse",
+ "httpdate",
+ "itoa",
+ "pin-project-lite",
+ "smallvec",
+ "tokio",
+]
+
+[[package]]
+name = "hyper-util"
+version = "0.1.20"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "96547c2556ec9d12fb1578c4eaf448b04993e7fb79cbaad930a656880a6bdfa0"
+dependencies = [
+ "bytes",
+ "http",
+ "http-body",
+ "hyper",
+ "pin-project-lite",
+ "tokio",
+ "tower-service",
+]
+
 [[package]]
 name = "id-arena"
 version = "2.3.0"
@@ -259,6 +445,12 @@ version = "0.4.29"
 source = "registry+https://github.com/rust-lang/crates.io-index"
 checksum = "5e5032e24019045c762d3c0f28f5b6b8bbf38563a65908389bf7978758920897"
 
+[[package]]
+name = "matchit"
+version = "0.8.4"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "47e1ffaa40ddd1f3ed91f717a33c8c0ee23fff369e3aa8772b9605cc1d22f4c3"
+
 [[package]]
 name = "matrixmultiply"
 version = "0.3.10"
@@ -275,6 +467,37 @@ version = "2.8.0"
 source = "registry+https://github.com/rust-lang/crates.io-index"
 checksum = "f8ca58f447f06ed17d5fc4043ce1b10dd205e060fb3ce5b979b8ed8e59ff3f79"
 
+[[package]]
+name = "mime"
+version = "0.3.17"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "6877bb514081ee2a7ff5ef9de3281f14a4dd4bceac4c09388074a6b5df8a139a"
+
+[[package]]
+name = "mio"
+version = "1.2.0"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "50b7e5b27aa02a74bac8c3f23f448f8d87ff11f92d3aac1a6ed369ee08cc56c1"
+dependencies = [
+ "libc",
+ "wasi",
+ "windows-sys",
+]
+
+[[package]]
+name = "mycad-api"
+version = "0.1.0"
+dependencies = [
+ "axum",
+ "mycad-build",
+ "mycad-format",
+ "mycad-kernel",
+ "serde",
+ "serde_json",
+ "tokio",
+ "tower",
+]
+
 [[package]]
 name = "mycad-build"
 version = "0.1.0"
@@ -420,6 +643,18 @@ version = "1.0.15"
 source = "registry+https://github.com/rust-lang/crates.io-index"
 checksum = "57c0d7b74b563b49d38dae00a0c37d4d6de9b432382b2892f0574ddcae73fd0a"
 
+[[package]]
+name = "percent-encoding"
+version = "2.3.2"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "9b4f627cb1b25917193a259e49bdad08f671f8d9708acfd5fe0a8c1455d87220"
+
+[[package]]
+name = "pin-project-lite"
+version = "0.2.17"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "a89322df9ebe1c1578d689c92318e070967d1042b512afbe49518723f4e6d5cd"
+
 [[package]]
 name = "prettyplease"
 version = "0.2.37"
@@ -572,6 +807,29 @@ dependencies = [
  "zmij",
 ]
 
+[[package]]
+name = "serde_path_to_error"
+version = "0.1.20"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "10a9ff822e371bb5403e391ecd83e182e0e77ba7f6fe0160b795797109d1b457"
+dependencies = [
+ "itoa",
+ "serde",
+ "serde_core",
+]
+
+[[package]]
+name = "serde_urlencoded"
+version = "0.7.1"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "d3491c14715ca2294c4d6a88f15e84739788c1d030eed8c110436aafdaa2f3fd"
+dependencies = [
+ "form_urlencoded",
+ "itoa",
+ "ryu",
+ "serde",
+]
+
 [[package]]
 name = "serde_yaml"
 version = "0.9.34+deprecated"
@@ -598,6 +856,28 @@ dependencies = [
  "wide",
 ]
 
+[[package]]
+name = "slab"
+version = "0.4.12"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "0c790de23124f9ab44544d7ac05d60440adc586479ce501c1d6d7da3cd8c9cf5"
+
+[[package]]
+name = "smallvec"
+version = "1.15.1"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "67b1b7a3b5fe4f1376887184045fcf45c69e92af734b7aaddc05fb777b6fbd03"
+
+[[package]]
+name = "socket2"
+version = "0.6.3"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "3a766e1110788c36f4fa1c2b71b387a7815aa65f88ce0229841826633d93723e"
+dependencies = [
+ "libc",
+ "windows-sys",
+]
+
 [[package]]
 name = "strsim"
 version = "0.11.1"
@@ -615,6 +895,12 @@ dependencies = [
  "unicode-ident",
 ]
 
+[[package]]
+name = "sync_wrapper"
+version = "1.0.2"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "0bf256ce5efdfa370213c1dabab5935a12e49f2c58d15e9eac2870d3b4f27263"
+
 [[package]]
 name = "tempfile"
 version = "3.27.0"
@@ -648,6 +934,79 @@ dependencies = [
  "syn",
 ]
 
+[[package]]
+name = "tokio"
+version = "1.52.3"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "8fc7f01b389ac15039e4dc9531aa973a135d7a4135281b12d7c1bc79fd57fffe"
+dependencies = [
+ "libc",
+ "mio",
+ "pin-project-lite",
+ "socket2",
+ "tokio-macros",
+ "windows-sys",
+]
+
+[[package]]
+name = "tokio-macros"
+version = "2.7.0"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "385a6cb71ab9ab790c5fe8d67f1645e6c450a7ce006a33de03daa956cf70a496"
+dependencies = [
+ "proc-macro2",
+ "quote",
+ "syn",
+]
+
+[[package]]
+name = "tower"
+version = "0.5.3"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "ebe5ef63511595f1344e2d5cfa636d973292adc0eec1f0ad45fae9f0851ab1d4"
+dependencies = [
+ "futures-core",
+ "futures-util",
+ "pin-project-lite",
+ "sync_wrapper",
+ "tokio",
+ "tower-layer",
+ "tower-service",
+ "tracing",
+]
+
+[[package]]
+name = "tower-layer"
+version = "0.3.3"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "121c2a6cda46980bb0fcd1647ffaf6cd3fc79a013de288782836f6df9c48780e"
+
+[[package]]
+name = "tower-service"
+version = "0.3.3"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "8df9b6e13f2d32c91b9bd719c00d1958837bc7dec474d94952798cc8e69eeec3"
+
+[[package]]
+name = "tracing"
+version = "0.1.44"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "63e71662fa4b2a2c3a26f570f037eb95bb1f85397f3cd8076caed2f026a6d100"
+dependencies = [
+ "log",
+ "pin-project-lite",
+ "tracing-core",
+]
+
+[[package]]
+name = "tracing-core"
+version = "0.1.36"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "db97caf9d906fbde555dd62fa95ddba9eecfd14cb388e4f491a66d74cd5fb79a"
+dependencies = [
+ "once_cell",
+]
+
 [[package]]
 name = "typenum"
 version = "1.19.0"
@@ -678,6 +1037,12 @@ version = "0.2.2"
 source = "registry+https://github.com/rust-lang/crates.io-index"
 checksum = "06abde3611657adf66d383f00b093d7faecc7fa57071cce2578660c9f1010821"
 
+[[package]]
+name = "wasi"
+version = "0.11.1+wasi-snapshot-preview1"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "ccf3ec651a847eb01de73ccad15eb7d99f80485de043efb2f370cd654f4ea44b"
+
 [[package]]
 name = "wasip2"
 version = "1.0.3+wasi-0.2.9"
diff --git a/Cargo.toml b/Cargo.toml
index f187785..13c40d5 100644
--- a/Cargo.toml
+++ b/Cargo.toml
@@ -5,6 +5,7 @@ members = [
     "crates/mycad-build",
     "crates/mycad-viewer",
     "crates/mycad-cli",
+    "crates/mycad-api",
     "crates/xtask",
 ]
 resolver = "2"
@@ -32,7 +33,13 @@ thiserror = "2"
 clap = { version = "4", features = ["derive"] }
 # Ordered collections (deterministic iteration)
 indexmap = { version = "2", features = ["serde"] }
+# HTTP / async
+axum = "0.8"
+tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }
+serde_json = "1"
+tower = "0.5"
 # Internal crates
 mycad-format = { path = "crates/mycad-format" }
 mycad-kernel = { path = "crates/mycad-kernel" }
 mycad-build = { path = "crates/mycad-build" }
+mycad-api = { path = "crates/mycad-api" }
diff --git a/crates/mycad-build/src/lib.rs b/crates/mycad-build/src/lib.rs
index 6e48c33..11c52e4 100644
--- a/crates/mycad-build/src/lib.rs
+++ b/crates/mycad-build/src/lib.rs
@@ -23,7 +23,7 @@ pub fn build_solid_from_features(
             height,
             depth,
             ..
-        } => Ok(make_cuboid(*width, *height, *depth, gen)),
+        } => make_cuboid(*width, *height, *depth, gen),
         Feature::CreateCylinder { radius, height, .. } => make_cylinder(*radius, *height, gen),
         Feature::CreateSphere { .. } => Err(KernelError::UnsupportedFeature {
             kind: "create_sphere",
diff --git a/crates/mycad-kernel/benches/tessellation.rs b/crates/mycad-kernel/benches/tessellation.rs
index c97b61c..d6c7dcf 100644
--- a/crates/mycad-kernel/benches/tessellation.rs
+++ b/crates/mycad-kernel/benches/tessellation.rs
@@ -12,7 +12,7 @@ fn main() {
 
     for i in 0..iterations {
         let mut id_gen = IdGenerator::new(i as u64);
-        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
+        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
         let _mesh = tessellate_solid(&solid).unwrap();
     }
 
diff --git a/crates/mycad-kernel/src/primitives/cuboid.rs b/crates/mycad-kernel/src/primitives/cuboid.rs
index 7eae439..bc8819a 100644
--- a/crates/mycad-kernel/src/primitives/cuboid.rs
+++ b/crates/mycad-kernel/src/primitives/cuboid.rs
@@ -1,4 +1,5 @@
 use crate::brep::topology::{IdGenerator, Solid};
+use crate::error::KernelError;
 use crate::geometry::curve::Curve;
 use crate::geometry::surface::Surface;
 use crate::geometry::Point;
@@ -13,7 +14,21 @@ use crate::geometry::Vec3;
 /// - `id_gen`: deterministic ID generator
 ///
 /// The cuboid spans from (-dx/2, -dy/2, -dz/2) to (dx/2, dy/2, dz/2).
-pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
+pub fn make_cuboid(
+    dx: f64,
+    dy: f64,
+    dz: f64,
+    id_gen: &mut IdGenerator,
+) -> Result<Solid, KernelError> {
+    if !dx.is_finite() || dx <= 0.0 {
+        return Err(KernelError::InvalidParameter { kind: "width" });
+    }
+    if !dy.is_finite() || dy <= 0.0 {
+        return Err(KernelError::InvalidParameter { kind: "height" });
+    }
+    if !dz.is_finite() || dz <= 0.0 {
+        return Err(KernelError::InvalidParameter { kind: "depth" });
+    }
     let mut solid = Solid::new(id_gen.next());
 
     let hx = dx / 2.0;
@@ -214,18 +229,19 @@ pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid
 
     solid.add_shell(id_gen.next(), face_indices, true);
 
-    solid
+    Ok(solid)
 }
 
 #[cfg(test)]
 mod tests {
     use super::*;
     use crate::brep::topology::IdGenerator;
+    use crate::error::KernelError;
 
     #[test]
     fn test_cuboid_topology() {
         let mut id_gen = IdGenerator::new(0);
-        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
+        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
 
         assert_eq!(solid.vertices.len(), 8, "cuboid should have 8 vertices");
         assert_eq!(solid.edges.len(), 12, "cuboid should have 12 edges");
@@ -245,7 +261,7 @@ mod tests {
     #[test]
     fn test_cuboid_vertex_positions() {
         let mut id_gen = IdGenerator::new(0);
-        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
+        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen).unwrap();
 
         // Check that all vertices are at the expected positions
         let expected = [
@@ -276,8 +292,8 @@ mod tests {
         let mut id_gen1 = IdGenerator::new(0);
         let mut id_gen2 = IdGenerator::new(0);
 
-        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
-        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);
+        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1).unwrap();
+        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2).unwrap();
 
         assert_eq!(solid1.vertices.len(), solid2.vertices.len());
         for (v1, v2) in solid1.vertices.iter().zip(solid2.vertices.iter()) {
@@ -296,4 +312,91 @@ mod tests {
             assert_eq!(f1.id, f2.id);
         }
     }
+
+    // K01: make_cuboid rejects zero, negative, NaN, Infinity dimensions
+    #[test]
+    fn test_make_cuboid_invalid_dimensions() {
+        let mut gen = IdGenerator::new(0);
+
+        // Zero
+        assert!(matches!(
+            make_cuboid(0.0, 1.0, 1.0, &mut gen),
+            Err(KernelError::InvalidParameter { kind: "width" })
+        ));
+        assert!(matches!(
+            make_cuboid(1.0, 0.0, 1.0, &mut gen),
+            Err(KernelError::InvalidParameter { kind: "height" })
+        ));
+        assert!(matches!(
+            make_cuboid(1.0, 1.0, 0.0, &mut gen),
+            Err(KernelError::InvalidParameter { kind: "depth" })
+        ));
+
+        // Negative
+        assert!(matches!(
+            make_cuboid(-1.0, 1.0, 1.0, &mut gen),
+            Err(KernelError::InvalidParameter { kind: "width" })
+        ));
+        assert!(matches!(
+            make_cuboid(1.0, -1.0, 1.0, &mut gen),
+            Err(KernelError::InvalidParameter { kind: "height" })
+        ));
+        assert!(matches!(
+            make_cuboid(1.0, 1.0, -1.0, &mut gen),
+            Err(KernelError::InvalidParameter { kind: "depth" })
+        ));
+
+        // NaN
+        assert!(matches!(
+            make_cuboid(f64::NAN, 1.0, 1.0, &mut gen),
+            Err(KernelError::InvalidParameter { kind: "width" })
+        ));
+        assert!(matches!(
+            make_cuboid(1.0, f64::NAN, 1.0, &mut gen),
+            Err(KernelError::InvalidParameter { kind: "height" })
+        ));
+        assert!(matches!(
+            make_cuboid(1.0, 1.0, f64::NAN, &mut gen),
+            Err(KernelError::InvalidParameter { kind: "depth" })
+        ));
+
+        // Infinity
+        assert!(matches!(
+            make_cuboid(f64::INFINITY, 1.0, 1.0, &mut gen),
+            Err(KernelError::InvalidParameter { kind: "width" })
+        ));
+        assert!(matches!(
+            make_cuboid(1.0, f64::INFINITY, 1.0, &mut gen),
+            Err(KernelError::InvalidParameter { kind: "height" })
+        ));
+        assert!(matches!(
+            make_cuboid(1.0, 1.0, f64::INFINITY, &mut gen),
+            Err(KernelError::InvalidParameter { kind: "depth" })
+        ));
+
+        // Negative Infinity
+        assert!(matches!(
+            make_cuboid(f64::NEG_INFINITY, 1.0, 1.0, &mut gen),
+            Err(KernelError::InvalidParameter { kind: "width" })
+        ));
+        assert!(matches!(
+            make_cuboid(1.0, f64::NEG_INFINITY, 1.0, &mut gen),
+            Err(KernelError::InvalidParameter { kind: "height" })
+        ));
+        assert!(matches!(
+            make_cuboid(1.0, 1.0, f64::NEG_INFINITY, &mut gen),
+            Err(KernelError::InvalidParameter { kind: "depth" })
+        ));
+    }
+
+    // K02: near-zero positive value is Ok (exact <= 0.0 policy)
+    #[test]
+    fn test_make_cuboid_near_zero_positive_ok() {
+        let mut gen = IdGenerator::new(0);
+        let result = make_cuboid(1e-9, 1.0, 1.0, &mut gen);
+        assert!(
+            result.is_ok(),
+            "near-zero positive should be Ok per exact <= 0.0 policy"
+        );
+    }
 }
diff --git a/crates/mycad-kernel/src/tessellation/mod.rs b/crates/mycad-kernel/src/tessellation/mod.rs
index e74950a..4960559 100644
--- a/crates/mycad-kernel/src/tessellation/mod.rs
+++ b/crates/mycad-kernel/src/tessellation/mod.rs
@@ -355,7 +355,7 @@ mod tests {
     #[test]
     fn test_tessellate_cuboid_unchanged() {
         let mut gen = IdGenerator::new(0);
-        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
+        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen).unwrap();
         let mesh = tessellate_solid(&solid).unwrap();
 
         assert_eq!(mesh.triangle_count(), 12);
@@ -370,8 +370,8 @@ mod tests {
         let mut gen1 = IdGenerator::new(0);
         let mut gen2 = IdGenerator::new(0);
 
-        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
-        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
+        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1).unwrap();
+        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2).unwrap();
 
         let m1 = tessellate_solid(&s1).unwrap();
         let m2 = tessellate_solid(&s2).unwrap();
@@ -497,7 +497,7 @@ mod tests {
     #[test]
     fn test_tessellate_cuboid() {
         let mut id_gen = IdGenerator::new(0);
-        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
+        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen).unwrap();
         let mesh = tessellate_solid(&solid).unwrap();
 
         assert_eq!(mesh.triangle_count(), 12);
@@ -511,8 +511,8 @@ mod tests {
         let mut id_gen1 = IdGenerator::new(0);
         let mut id_gen2 = IdGenerator::new(0);
 
-        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
-        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
+        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1).unwrap();
+        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2).unwrap();
 
         let mesh1 = tessellate_solid(&solid1).unwrap();
         let mesh2 = tessellate_solid(&solid2).unwrap();
diff --git a/crates/mycad-kernel/src/tessellation/stl.rs b/crates/mycad-kernel/src/tessellation/stl.rs
index 5f201cf..2e40bb7 100644
--- a/crates/mycad-kernel/src/tessellation/stl.rs
+++ b/crates/mycad-kernel/src/tessellation/stl.rs
@@ -55,7 +55,7 @@ mod tests {
     #[test]
     fn ascii_stl_cuboid_structure() {
         let mut g = IdGenerator::new(0);
-        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
+        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g).unwrap();
         let mesh = tessellate_solid(&solid).unwrap();
         let stl = to_ascii_stl(&mesh, "test_cube");
 
@@ -72,7 +72,7 @@ mod tests {
     fn ascii_stl_deterministic() {
         let run = || {
             let mut g = IdGenerator::new(0);
-            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
+            let s = make_cuboid(1.0, 1.0, 1.0, &mut g).unwrap();
             to_ascii_stl(&tessellate_solid(&s).unwrap(), "cube")
         };
         assert_eq!(run(), run());
diff --git a/crates/mycad-kernel/tests/cuboid_roundtrip.rs b/crates/mycad-kernel/tests/cuboid_roundtrip.rs
index 29e77e4..8bb6564 100644
--- a/crates/mycad-kernel/tests/cuboid_roundtrip.rs
+++ b/crates/mycad-kernel/tests/cuboid_roundtrip.rs
@@ -6,7 +6,7 @@ use mycad_kernel::tessellation::tessellate_solid;
 #[test]
 fn cuboid_to_mesh_pipeline() {
     let mut id_gen = IdGenerator::new(0);
-    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
+    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
 
     // Verify topology
     assert_eq!(solid.vertices.len(), 8);
@@ -28,7 +28,7 @@ fn cuboid_to_mesh_pipeline() {
 fn cuboid_pipeline_is_deterministic() {
     let run = || {
         let mut id_gen = IdGenerator::new(42);
-        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
+        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen).unwrap();
         tessellate_solid(&solid).unwrap()
     };
 

codex
未追跡の `mycad-api` クレートが `Cargo.toml` からワークスペースに追加されています。これはスコープ逸脱の可能性が高いので、新規クレート側も最低限レビュー対象に含めます。
exec
/bin/bash -lc 'nl -ba Cargo.toml' in /home/bacon/mycad
exec
/bin/bash -lc 'nl -ba crates/mycad-api/Cargo.toml' in /home/bacon/mycad
 succeeded in 0ms:
     1	[workspace]
     2	members = [
     3	    "crates/mycad-kernel",
     4	    "crates/mycad-format",
     5	    "crates/mycad-build",
     6	    "crates/mycad-viewer",
     7	    "crates/mycad-cli",
     8	    "crates/mycad-api",
     9	    "crates/xtask",
    10	]
    11	resolver = "2"
    12	
    13	[workspace.package]
    14	version = "0.1.0"
    15	edition = "2021"
    16	license = "MIT OR Apache-2.0"
    17	rust-version = "1.75"
    18	
    19	[workspace.dependencies]
    20	# Math
    21	nalgebra = { version = "0.33", features = ["serde-serialize"] }
    22	# Serialization
    23	serde = { version = "1", features = ["derive"] }
    24	serde_yaml = "0.9"
    25	# Schema generation
    26	schemars = "0.8"
    27	# Testing
    28	approx = "0.5"
    29	tempfile = "3"
    30	# Error handling
    31	thiserror = "2"
    32	# CLI
    33	clap = { version = "4", features = ["derive"] }
    34	# Ordered collections (deterministic iteration)
    35	indexmap = { version = "2", features = ["serde"] }
    36	# HTTP / async
    37	axum = "0.8"
    38	tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }
    39	serde_json = "1"
    40	tower = "0.5"
    41	# Internal crates
    42	mycad-format = { path = "crates/mycad-format" }
    43	mycad-kernel = { path = "crates/mycad-kernel" }
    44	mycad-build = { path = "crates/mycad-build" }
    45	mycad-api = { path = "crates/mycad-api" }

 succeeded in 0ms:
     1	[package]
     2	name = "mycad-api"
     3	description = "HTTP API server for MyCad"
     4	version.workspace = true
     5	edition.workspace = true
     6	license.workspace = true
     7	
     8	[dependencies]
     9	axum = { workspace = true }
    10	tokio = { workspace = true }
    11	serde = { workspace = true }
    12	serde_json = { workspace = true }
    13	mycad-format = { workspace = true }
    14	mycad-kernel = { workspace = true }
    15	mycad-build = { workspace = true }
    16	
    17	[dev-dependencies]
    18	tower = { workspace = true }
    19	
    20	[[bin]]
    21	name = "mycad-api"
    22	path = "src/main.rs"

exec
/bin/bash -lc 'rg --files crates/mycad-api/src crates/mycad-api/tests' in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-api/src/lib.rs
crates/mycad-api/src/handler.rs
crates/mycad-api/src/router.rs
crates/mycad-api/src/main.rs
crates/mycad-api/src/error.rs
crates/mycad-api/tests/mesh_api.rs
crates/mycad-api/tests/fixtures/empty_part.mycad
crates/mycad-api/tests/fixtures/assembly.mycad
crates/mycad-api/tests/fixtures/create_sphere.mycad
crates/mycad-api/tests/fixtures/zero_box.mycad

exec
/bin/bash -lc 'nl -ba crates/mycad-api/src/lib.rs' in /home/bacon/mycad
 succeeded in 0ms:
     1	pub mod error;
     2	pub mod handler;
     3	pub mod router;

exec
/bin/bash -lc 'nl -ba crates/mycad-api/src/error.rs' in /home/bacon/mycad
 succeeded in 0ms:
     1	use axum::http::StatusCode;
     2	use axum::response::{IntoResponse, Response};
     3	use mycad_format::error::FormatError;
     4	use mycad_kernel::error::KernelError;
     5	use mycad_kernel::tessellation::TessellationError;
     6	use serde_json::json;
     7	
     8	#[derive(Debug)]
     9	pub enum ApiError {
    10	    BadRequest(String),
    11	    NotFound(String),
    12	    Unprocessable(String),
    13	    Internal(String),
    14	}
    15	
    16	impl std::fmt::Display for ApiError {
    17	    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    18	        match self {
    19	            ApiError::BadRequest(msg) => write!(f, "{msg}"),
    20	            ApiError::NotFound(msg) => write!(f, "{msg}"),
    21	            ApiError::Unprocessable(msg) => write!(f, "{msg}"),
    22	            ApiError::Internal(msg) => write!(f, "{msg}"),
    23	        }
    24	    }
    25	}
    26	
    27	impl From<FormatError> for ApiError {
    28	    fn from(err: FormatError) -> Self {
    29	        match &err {
    30	            FormatError::InvalidExtension(_) | FormatError::Yaml(_) => {
    31	                ApiError::BadRequest(err.to_string())
    32	            }
    33	            FormatError::Io(io_err) => match io_err.kind() {
    34	                std::io::ErrorKind::NotFound => ApiError::NotFound(err.to_string()),
    35	                _ => ApiError::Internal(err.to_string()),
    36	            },
    37	            FormatError::InvalidReference { .. } => ApiError::Unprocessable(err.to_string()),
    38	        }
    39	    }
    40	}
    41	
    42	impl From<KernelError> for ApiError {
    43	    fn from(err: KernelError) -> Self {
    44	        ApiError::Unprocessable(err.to_string())
    45	    }
    46	}
    47	
    48	impl From<TessellationError> for ApiError {
    49	    fn from(err: TessellationError) -> Self {
    50	        ApiError::Unprocessable(err.to_string())
    51	    }
    52	}
    53	
    54	impl IntoResponse for ApiError {
    55	    fn into_response(self) -> Response {
    56	        let (status, msg) = match &self {
    57	            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
    58	            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
    59	            ApiError::Unprocessable(_) => (StatusCode::UNPROCESSABLE_ENTITY, self.to_string()),
    60	            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
    61	        };
    62	        let body = json!({ "error": msg });
    63	        (status, axum::Json(body)).into_response()
    64	    }
    65	}

exec
/bin/bash -lc 'nl -ba crates/mycad-api/src/router.rs' in /home/bacon/mycad
 succeeded in 0ms:
     1	use crate::handler::get_mesh;
     2	use axum::routing::get;
     3	use axum::Router;
     4	
     5	pub fn app() -> Router {
     6	    Router::new().nest("/api/v0", Router::new().route("/mesh", get(get_mesh)))
     7	}

exec
/bin/bash -lc 'nl -ba crates/mycad-api/src/handler.rs' in /home/bacon/mycad
 succeeded in 0ms:
     1	use crate::error::ApiError;
     2	use axum::extract::Query;
     3	use axum::Json;
     4	use mycad_build::build_solid_from_features;
     5	use mycad_format::Document;
     6	use mycad_kernel::brep::topology::IdGenerator;
     7	use mycad_kernel::tessellation::{tessellate_solid_with, TessellationOptions, TriangleMesh};
     8	use serde::Deserialize;
     9	use std::path::Path;
    10	
    11	const V0_TESSELLATION: TessellationOptions = TessellationOptions {
    12	    angular_segments: 32,
    13	    axial_segments: 1,
    14	};
    15	
    16	#[derive(Deserialize)]
    17	pub struct MeshQuery {
    18	    pub file: String,
    19	}
    20	
    21	pub async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError> {
    22	    let path = Path::new(&q.file);
    23	
    24	    if path.is_relative() {
    25	        return Err(ApiError::BadRequest(
    26	            "relative path not allowed: file must be an absolute path".to_string(),
    27	        ));
    28	    }
    29	
    30	    let canonical = std::fs::canonicalize(path).map_err(|e| match e.kind() {
    31	        std::io::ErrorKind::NotFound => {
    32	            ApiError::NotFound(format!("file not found: {}", path.display()))
    33	        }
    34	        _ => ApiError::BadRequest(format!("invalid path: {}", path.display())),
    35	    })?;
    36	
    37	    let doc = Document::from_path(&canonical)?;
    38	
    39	    let root = &doc.root_component;
    40	    if root.reference.is_some() || !root.children.is_empty() {
    41	        return Err(ApiError::Unprocessable(
    42	            "assembly/reference documents are not supported in v0".to_string(),
    43	        ));
    44	    }
    45	    if root.features.is_empty() {
    46	        return Err(ApiError::Unprocessable(
    47	            "empty part: document has no features".to_string(),
    48	        ));
    49	    }
    50	
    51	    let mut gen = IdGenerator::new(0);
    52	    let solid = build_solid_from_features(&root.features, &mut gen)?;
    53	    let mesh = tessellate_solid_with(&solid, &V0_TESSELLATION)?;
    54	    Ok(Json(mesh))
    55	}

exec
/bin/bash -lc 'nl -ba crates/mycad-api/src/main.rs' in /home/bacon/mycad
 succeeded in 0ms:
     1	use mycad_api::router::app;
     2	
     3	#[tokio::main]
     4	async fn main() {
     5	    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
     6	        .await
     7	        .expect("failed to bind to 127.0.0.1:3000");
     8	    axum::serve(listener, app()).await.expect("server error");
     9	}

exec
/bin/bash -lc 'nl -ba crates/mycad-api/tests/mesh_api.rs' in /home/bacon/mycad
 succeeded in 0ms:
     1	use axum::body::Body;
     2	use axum::http::{Request, StatusCode};
     3	use mycad_api::router::app;
     4	use serde::Deserialize;
     5	use tower::ServiceExt;
     6	
     7	#[derive(Deserialize)]
     8	struct ErrorResponse {
     9	    error: String,
    10	}
    11	
    12	fn fixture_path(name: &str) -> String {
    13	    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    14	    let path = std::path::Path::new(&dir)
    15	        .join("tests")
    16	        .join("fixtures")
    17	        .join(name);
    18	    std::fs::canonicalize(&path)
    19	        .unwrap_or_else(|_| panic!("fixture not found: {:?}", path))
    20	        .to_str()
    21	        .unwrap()
    22	        .to_string()
    23	}
    24	
    25	fn example_path(name: &str) -> String {
    26	    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    27	    let path = std::path::Path::new(&dir)
    28	        .join("..")
    29	        .join("..")
    30	        .join("examples")
    31	        .join(name);
    32	    std::fs::canonicalize(&path)
    33	        .unwrap_or_else(|_| panic!("example not found: {:?}", path))
    34	        .to_str()
    35	        .unwrap()
    36	        .to_string()
    37	}
    38	
    39	async fn send_mesh_request(uri: &str) -> (StatusCode, String) {
    40	    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    41	    let app = app();
    42	    let resp = app.oneshot(req).await.unwrap();
    43	    let status = resp.status();
    44	    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
    45	        .await
    46	        .unwrap();
    47	    (status, String::from_utf8(body.to_vec()).unwrap())
    48	}
    49	
    50	// T01: normal box
    51	#[tokio::test]
    52	async fn t01_normal_box() {
    53	    let file = example_path("simple_box.mycad");
    54	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
    55	    let (status, body) = send_mesh_request(&uri).await;
    56	    assert_eq!(status, StatusCode::OK, "body: {body}");
    57	    let mesh: serde_json::Value = serde_json::from_str(&body).unwrap();
    58	    assert!(mesh["positions"].as_array().unwrap().len() > 0);
    59	    assert!(mesh["indices"].as_array().unwrap().len() % 3 == 0);
    60	}
    61	
    62	// T02: normal cylinder
    63	#[tokio::test]
    64	async fn t02_normal_cylinder() {
    65	    let file = example_path("cylinder.mycad");
    66	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
    67	    let (status, body) = send_mesh_request(&uri).await;
    68	    assert_eq!(status, StatusCode::OK, "body: {body}");
    69	    let mesh: serde_json::Value = serde_json::from_str(&body).unwrap();
    70	    assert!(mesh["positions"].as_array().unwrap().len() > 0);
    71	}
    72	
    73	// T03: determinism
    74	#[tokio::test]
    75	async fn t03_determinism() {
    76	    let file = example_path("simple_box.mycad");
    77	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
    78	
    79	    let (_, body1) = send_mesh_request(&uri).await;
    80	    let (_, body2) = send_mesh_request(&uri).await;
    81	    assert_eq!(body1, body2, "mesh responses must be deterministic");
    82	
    83	    let file2 = example_path("cylinder.mycad");
    84	    let uri2 = format!("/api/v0/mesh?file={}", urlencoding(&file2));
    85	    let (_, body3) = send_mesh_request(&uri2).await;
    86	    let (_, body4) = send_mesh_request(&uri2).await;
    87	    assert_eq!(
    88	        body3, body4,
    89	        "cylinder mesh responses must be deterministic"
    90	    );
    91	}
    92	
    93	// T04: 404 not found
    94	#[tokio::test]
    95	async fn t04_not_found() {
    96	    let uri = "/api/v0/mesh?file=/tmp/absolutely_nonexistent_file.mycad";
    97	    let (status, body) = send_mesh_request(uri).await;
    98	    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
    99	    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
   100	    assert!(!err.error.is_empty());
   101	}
   102	
   103	// T05: 400 non-.mycad extension
   104	#[tokio::test]
   105	async fn t05_invalid_extension() {
   106	    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
   107	    let path = std::path::Path::new(&dir)
   108	        .join("..")
   109	        .join("..")
   110	        .join("Cargo.toml");
   111	    let abs = std::fs::canonicalize(&path)
   112	        .unwrap()
   113	        .to_str()
   114	        .unwrap()
   115	        .to_string();
   116	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&abs));
   117	    let (status, body) = send_mesh_request(&uri).await;
   118	    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
   119	}
   120	
   121	// T06: 403 Host header (DNS rebinding protection)
   122	#[tokio::test]
   123	async fn t06_host_header_rebinding() {
   124	    let file = example_path("simple_box.mycad");
   125	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
   126	    let req = Request::builder()
   127	        .uri(&uri)
   128	        .header("Host", "evil.com")
   129	        .body(Body::empty())
   130	        .unwrap();
   131	    let app = app();
   132	    let resp = app.oneshot(req).await.unwrap();
   133	    // Without explicit host validation middleware, this will pass through.
   134	    // The plan says to add Host header validation middleware.
   135	    // For v0, the server binds to 127.0.0.1 only so network-level protection exists.
   136	    // The middleware is a defense-in-depth layer.
   137	    assert_eq!(resp.status(), StatusCode::OK);
   138	}
   139	
   140	// T07: 422 unsupported feature (sphere)
   141	#[tokio::test]
   142	async fn t07_unsupported_feature() {
   143	    let file = fixture_path("create_sphere.mycad");
   144	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
   145	    let (status, body) = send_mesh_request(&uri).await;
   146	    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
   147	}
   148	
   149	// T08: 422 degenerate dimension (zero-width box)
   150	#[tokio::test]
   151	async fn t08_degenerate_dimension() {
   152	    let file = fixture_path("zero_box.mycad");
   153	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
   154	    let (status, body) = send_mesh_request(&uri).await;
   155	    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
   156	}
   157	
   158	// T09: 422 assembly unsupported
   159	#[tokio::test]
   160	async fn t09_assembly_unsupported() {
   161	    let file = fixture_path("assembly.mycad");
   162	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
   163	    let (status, body) = send_mesh_request(&uri).await;
   164	    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
   165	    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
   166	    assert!(
   167	        err.error.contains("assembly") || err.error.contains("reference"),
   168	        "expected assembly/reference message, got: {}",
   169	        err.error
   170	    );
   171	}
   172	
   173	// T10: 422 empty part
   174	#[tokio::test]
   175	async fn t10_empty_part() {
   176	    let file = fixture_path("empty_part.mycad");
   177	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
   178	    let (status, body) = send_mesh_request(&uri).await;
   179	    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
   180	    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
   181	    assert!(
   182	        err.error.contains("empty part") || err.error.contains("no features"),
   183	        "expected empty part message, got: {}",
   184	        err.error
   185	    );
   186	}
   187	
   188	// T11: 400 relative path rejected
   189	#[tokio::test]
   190	async fn t11_relative_path_rejected() {
   191	    let uri = "/api/v0/mesh?file=examples/simple_box.mycad";
   192	    let (status, body) = send_mesh_request(uri).await;
   193	    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
   194	    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
   195	    assert!(
   196	        err.error.contains("relative"),
   197	        "expected relative path error, got: {}",
   198	        err.error
   199	    );
   200	}
   201	
   202	fn urlencoding(s: &str) -> String {
   203	    s.replace('/', "%2F")
   204	}

codex
次はコンパイル観点と既存型定義を確認します。`make_cuboid` のシグネチャ変更が波及しているので、依存側の型制約やエラー型の実装も見ます。
exec
/bin/bash -lc "rg -n \"pub struct TriangleMesh|derive\\(|thiserror|InvalidParameter|tessellate_solid_with|pub enum KernelError|pub enum TessellationError|make_cuboid\\(\" crates/mycad-kernel crates/mycad-build crates/mycad-format -g '*.rs'" in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-format/src/feature.rs:5:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
crates/mycad-format/src/feature.rs:14:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
crates/mycad-format/src/error.rs:1:use thiserror::Error;
crates/mycad-format/src/error.rs:3:#[derive(Debug, Error)]
crates/mycad-build/src/lib.rs:26:        } => make_cuboid(*width, *height, *depth, gen),
crates/mycad-format/src/document.rs:8:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
crates/mycad-format/src/component.rs:8:#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
crates/mycad-format/src/component.rs:29:#[derive(Debug, Clone)]
crates/mycad-format/src/component.rs:96:#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
crates/mycad-kernel/src/tessellation/mod.rs:10:use thiserror::Error;
crates/mycad-kernel/src/tessellation/mod.rs:17:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/tessellation/mod.rs:18:pub struct TriangleMesh {
crates/mycad-kernel/src/tessellation/mod.rs:49:#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
crates/mycad-kernel/src/tessellation/mod.rs:74:#[derive(Debug, Error)]
crates/mycad-kernel/src/tessellation/mod.rs:75:pub enum TessellationError {
crates/mycad-kernel/src/tessellation/mod.rs:86:    tessellate_solid_with(solid, &TessellationOptions::default())
crates/mycad-kernel/src/tessellation/mod.rs:90:pub fn tessellate_solid_with(
crates/mycad-kernel/src/tessellation/mod.rs:358:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:373:        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:374:        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:445:        let mesh_low = tessellate_solid_with(&solid, &TessellationOptions::new(8, 1)).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:446:        let mesh_hi = tessellate_solid_with(&solid, &TessellationOptions::new(64, 2)).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:500:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:514:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:515:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:525:    /// (public fields bypass new() clamping, so tessellate_solid_with must guard at point of use).
crates/mycad-kernel/src/tessellation/mod.rs:534:        let mesh = tessellate_solid_with(&solid, &opts).unwrap();
crates/mycad-kernel/benches/tessellation.rs:15:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
crates/mycad-kernel/src/geometry/surface.rs:6:#[derive(Debug, Clone, Copy, PartialEq, Eq)]
crates/mycad-kernel/src/geometry/surface.rs:17:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/tessellation/stl.rs:58:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g).unwrap();
crates/mycad-kernel/src/tessellation/stl.rs:75:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g).unwrap();
crates/mycad-kernel/src/error.rs:1:use thiserror::Error;
crates/mycad-kernel/src/error.rs:3:#[derive(Debug, Error)]
crates/mycad-kernel/src/error.rs:4:pub enum KernelError {
crates/mycad-kernel/src/error.rs:15:    InvalidParameter { kind: &'static str },
crates/mycad-kernel/src/geometry/curve.rs:6:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/geometry/mod.rs:15:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:11:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:20:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:33:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:45:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:54:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:68:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:79:#[derive(Debug, Clone, Serialize, Deserialize)]
crates/mycad-kernel/src/brep/topology.rs:184:#[derive(Debug, Clone)]
crates/mycad-kernel/src/primitives/cylinder.rs:17:        return Err(KernelError::InvalidParameter { kind: "radius" });
crates/mycad-kernel/src/primitives/cylinder.rs:20:        return Err(KernelError::InvalidParameter { kind: "height" });
crates/mycad-kernel/src/primitives/cylinder.rs:299:    /// T14: Degenerate inputs — radius<=0, height<=0, non-finite → InvalidParameter.
crates/mycad-kernel/src/primitives/cylinder.rs:308:            KernelError::InvalidParameter { kind: "radius" }
crates/mycad-kernel/src/primitives/cylinder.rs:315:            KernelError::InvalidParameter { kind: "radius" }
crates/mycad-kernel/src/primitives/cylinder.rs:322:            KernelError::InvalidParameter { kind: "radius" }
crates/mycad-kernel/src/primitives/cylinder.rs:329:            KernelError::InvalidParameter { kind: "radius" }
crates/mycad-kernel/src/primitives/cylinder.rs:336:            KernelError::InvalidParameter { kind: "height" }
crates/mycad-kernel/src/primitives/cylinder.rs:343:            KernelError::InvalidParameter { kind: "height" }
crates/mycad-kernel/src/primitives/cylinder.rs:350:            KernelError::InvalidParameter { kind: "height" }
crates/mycad-kernel/src/primitives/cylinder.rs:357:            KernelError::InvalidParameter { kind: "height" }
crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen).unwrap();
crates/mycad-kernel/src/primitives/cuboid.rs:17:pub fn make_cuboid(
crates/mycad-kernel/src/primitives/cuboid.rs:24:        return Err(KernelError::InvalidParameter { kind: "width" });
crates/mycad-kernel/src/primitives/cuboid.rs:27:        return Err(KernelError::InvalidParameter { kind: "height" });
crates/mycad-kernel/src/primitives/cuboid.rs:30:        return Err(KernelError::InvalidParameter { kind: "depth" });
crates/mycad-kernel/src/primitives/cuboid.rs:244:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
crates/mycad-kernel/src/primitives/cuboid.rs:264:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen).unwrap();
crates/mycad-kernel/src/primitives/cuboid.rs:295:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1).unwrap();
crates/mycad-kernel/src/primitives/cuboid.rs:296:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2).unwrap();
crates/mycad-kernel/src/primitives/cuboid.rs:323:            make_cuboid(0.0, 1.0, 1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:324:            Err(KernelError::InvalidParameter { kind: "width" })
crates/mycad-kernel/src/primitives/cuboid.rs:327:            make_cuboid(1.0, 0.0, 1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:328:            Err(KernelError::InvalidParameter { kind: "height" })
crates/mycad-kernel/src/primitives/cuboid.rs:331:            make_cuboid(1.0, 1.0, 0.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:332:            Err(KernelError::InvalidParameter { kind: "depth" })
crates/mycad-kernel/src/primitives/cuboid.rs:337:            make_cuboid(-1.0, 1.0, 1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:338:            Err(KernelError::InvalidParameter { kind: "width" })
crates/mycad-kernel/src/primitives/cuboid.rs:341:            make_cuboid(1.0, -1.0, 1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:342:            Err(KernelError::InvalidParameter { kind: "height" })
crates/mycad-kernel/src/primitives/cuboid.rs:345:            make_cuboid(1.0, 1.0, -1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:346:            Err(KernelError::InvalidParameter { kind: "depth" })
crates/mycad-kernel/src/primitives/cuboid.rs:351:            make_cuboid(f64::NAN, 1.0, 1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:352:            Err(KernelError::InvalidParameter { kind: "width" })
crates/mycad-kernel/src/primitives/cuboid.rs:355:            make_cuboid(1.0, f64::NAN, 1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:356:            Err(KernelError::InvalidParameter { kind: "height" })
crates/mycad-kernel/src/primitives/cuboid.rs:359:            make_cuboid(1.0, 1.0, f64::NAN, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:360:            Err(KernelError::InvalidParameter { kind: "depth" })
crates/mycad-kernel/src/primitives/cuboid.rs:365:            make_cuboid(f64::INFINITY, 1.0, 1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:366:            Err(KernelError::InvalidParameter { kind: "width" })
crates/mycad-kernel/src/primitives/cuboid.rs:369:            make_cuboid(1.0, f64::INFINITY, 1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:370:            Err(KernelError::InvalidParameter { kind: "height" })
crates/mycad-kernel/src/primitives/cuboid.rs:373:            make_cuboid(1.0, 1.0, f64::INFINITY, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:374:            Err(KernelError::InvalidParameter { kind: "depth" })
crates/mycad-kernel/src/primitives/cuboid.rs:379:            make_cuboid(f64::NEG_INFINITY, 1.0, 1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:380:            Err(KernelError::InvalidParameter { kind: "width" })
crates/mycad-kernel/src/primitives/cuboid.rs:383:            make_cuboid(1.0, f64::NEG_INFINITY, 1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:384:            Err(KernelError::InvalidParameter { kind: "height" })
crates/mycad-kernel/src/primitives/cuboid.rs:387:            make_cuboid(1.0, 1.0, f64::NEG_INFINITY, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:388:            Err(KernelError::InvalidParameter { kind: "depth" })
crates/mycad-kernel/src/primitives/cuboid.rs:396:        let result = make_cuboid(1e-9, 1.0, 1.0, &mut gen);

exec
/bin/bash -lc 'CARGO_TARGET_DIR=/tmp/mycad-target cargo test -p mycad-kernel --no-run' in /home/bacon/mycad
2026-05-25T06:37:17.744507Z ERROR codex_core::tools::router: error=exec_command failed for `/bin/bash -lc 'CARGO_TARGET_DIR=/tmp/mycad-target cargo test -p mycad-kernel --no-run'`: SandboxDenied { message: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", output: ExecToolCallOutput { exit_code: 101, stdout: StreamOutput { text: "", truncated_after_lines: None }, stderr: StreamOutput { text: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", truncated_after_lines: None }, aggregated_output: StreamOutput { text: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", truncated_after_lines: None }, duration: 0ns, timed_out: false } }
exec
/bin/bash -lc 'CARGO_TARGET_DIR=/tmp/mycad-target cargo test -p mycad-api --no-run' in /home/bacon/mycad
 exited 101 in 160ms:
error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)
--- stderr
sccache: error: Operation not permitted (os error 1)


2026-05-25T06:37:17.862952Z ERROR codex_core::tools::router: error=exec_command failed for `/bin/bash -lc 'CARGO_TARGET_DIR=/tmp/mycad-target cargo test -p mycad-api --no-run'`: SandboxDenied { message: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", output: ExecToolCallOutput { exit_code: 101, stdout: StreamOutput { text: "", truncated_after_lines: None }, stderr: StreamOutput { text: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", truncated_after_lines: None }, aggregated_output: StreamOutput { text: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", truncated_after_lines: None }, duration: 0ns, timed_out: false } }
 exited 101 in 154ms:
error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)
--- stderr
sccache: error: Operation not permitted (os error 1)


codex
コンパイル確認は `sccache` 制約で止まったので、ラッパーを外して再試行します。あわせて、差分内コメントの `T01` 系テストが設計レビュー合意と一致しているかも確認します。
exec
/bin/bash -lc 'RUSTC_WRAPPER= CARGO_TARGET_DIR=/tmp/mycad-target cargo test -p mycad-kernel --no-run' in /home/bacon/mycad
exec
/bin/bash -lc 'RUSTC_WRAPPER= CARGO_TARGET_DIR=/tmp/mycad-target cargo test -p mycad-api --no-run' in /home/bacon/mycad
exec
/bin/bash -lc "rg -n \"T0[1-9]|T10|T11|T12|T13|T14|Host header|golden|roundtrip|plan\" features crates/mycad-kernel crates/mycad-api -g '*.md' -g '*.rs'" in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-kernel/src/tessellation/mod.rs:116:/// Triangulate a face as a fan from its first boundary vertex (planar convex faces).
crates/mycad-kernel/src/tessellation/mod.rs:302:            Surface::Plane { .. } => "plane",
crates/mycad-kernel/src/tessellation/mod.rs:318:    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
crates/mycad-kernel/src/tessellation/mod.rs:337:    /// T07: Determinism — same input → same output.
crates/mycad-kernel/src/tessellation/mod.rs:354:    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
crates/mycad-kernel/src/tessellation/mod.rs:367:    /// T08 extended: exact positions and indices snapshot.
crates/mycad-kernel/src/tessellation/mod.rs:384:    /// T09: Outward normals — all facet normals point away from centroid.
crates/mycad-kernel/src/tessellation/mod.rs:431:    /// T12: Resolution externalization + clamping.
features/12-cylinder/design-review.md:45:- golden YAML ラウンドトリップテストがあるか
features/12-cylinder/design-review.md:176:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/12-cylinder/design-review.md:177:| T02 | トポロジー | 頂点2・エッジ3・面3・シェル1(closed)・HE6・loop3 | 各 `len()` 一致 |
features/12-cylinder/design-review.md:178:| T03 | Euler | V−E+F == 2 | `assert_eq!(2-3+3, 2)` 相当の検証 |
features/12-cylinder/design-review.md:179:| T04 | マニフォルド | 各エッジがちょうど 2 HE に参照され向きが逆 | 集計検証 |
features/12-cylinder/design-review.md:180:| T05 | 幾何 | シーム頂点が (0,r,0)/(0,r,h)、各 Surface/Curve の係数 | `approx` で一致 |
features/12-cylinder/design-review.md:181:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
features/12-cylinder/design-review.md:182:| T07 | 決定性(tessellate) | 同一入力2回で positions/normals/indices 一致 | `assert_eq!` |
features/12-cylinder/design-review.md:183:| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index | 既存テスト維持 |
features/12-cylinder/design-review.md:184:| T09 | 外向き法線 | 全 facet の幾何法線が重心からの外向きと同符号（凸形状なので判定可） | 検証 |
features/12-cylinder/design-review.md:185:| T10 | ディスパッチ統合 | `examples/cylinder.mycad` を `build_solid_from_features` でビルドし T02 と同じ counts | `feature_dispatcher.rs` |
features/12-cylinder/design-review.md:186:| T11 | export 統合 | `mycad export examples/cylinder.mycad -o tmp.stl` が成功し、facet 数が T06 と整合、ASCII STL として妥当 | `export.rs`（`CARGO_BIN_EXE_mycad`） |
features/12-cylinder/design-review.md:187:| T12 | 解像度外部化 | `--segments` / `tessellate_solid_with` で分割数が変わり三角形数が増減 | 2 解像度で差分確認 |
features/12-cylinder/design-review.md:191:1. `cargo test --workspace` で T01–T12 green。
features/12-cylinder/design-review.md:269:crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/12-cylinder/design-review.md:463:features/12-cylinder/design-review.md:176:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/12-cylinder/design-review.md:464:features/12-cylinder/design-review.md:181:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
features/12-cylinder/design-review.md:465:features/12-cylinder/design-review.md:183:| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index | 既存テスト維持 |
features/12-cylinder/design-review.md:466:features/12-cylinder/design-review.md:187:| T12 | 解像度外部化 | `--segments` / `tessellate_solid_with` で分割数が変わり三角形数が増減 | 2 解像度で差分確認 |
features/12-cylinder/design-review.md:580:crates/mycad-kernel/tests/cuboid_roundtrip.rs:1:use mycad_kernel::brep::topology::IdGenerator;
features/12-cylinder/design-review.md:581:crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
features/12-cylinder/design-review.md:582:crates/mycad-kernel/tests/cuboid_roundtrip.rs:3:use mycad_kernel::tessellation::tessellate_solid;
features/12-cylinder/design-review.md:583:crates/mycad-kernel/tests/cuboid_roundtrip.rs:8:    let mut id_gen = IdGenerator::new(0);
features/12-cylinder/design-review.md:584:crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/12-cylinder/design-review.md:585:crates/mycad-kernel/tests/cuboid_roundtrip.rs:17:    let mesh = tessellate_solid(&solid);
features/12-cylinder/design-review.md:586:crates/mycad-kernel/tests/cuboid_roundtrip.rs:30:        let mut id_gen = IdGenerator::new(42);
features/12-cylinder/design-review.md:587:crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/12-cylinder/design-review.md:588:crates/mycad-kernel/tests/cuboid_roundtrip.rs:32:        tessellate_solid(&solid)
features/12-cylinder/design-review.md:1242:        // are shaded correctly (planar faces yield a constant normal).
features/12-cylinder/design-review.md:1313:    /// An infinite plane.
features/12-cylinder/design-review.md:1415:    /// surface is planar / has a constant normal.
features/12-cylinder/design-review.md:1461:    /// never have to special-case planar vs curved surfaces.
features/12-cylinder/design-review.md:1473:    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
features/12-cylinder/design-review.md:1481:    fn test_uv_of_roundtrip_plane() {
features/12-cylinder/design-review.md:1488:        assert_uv_roundtrip(&surface, 0.7, -1.3);
features/12-cylinder/design-review.md:1492:    fn test_uv_of_roundtrip_cylinder() {
features/12-cylinder/design-review.md:1498:        assert_uv_roundtrip(&surface, 0.9, 1.7);
features/12-cylinder/design-review.md:1502:    fn test_uv_of_roundtrip_sphere() {
features/12-cylinder/design-review.md:1507:        assert_uv_roundtrip(&surface, 1.1, 0.6);
features/12-cylinder/design-review.md:1511:    fn test_uv_of_roundtrip_cone() {
features/12-cylinder/design-review.md:1517:        assert_uv_roundtrip(&surface, 0.8, 2.0);
features/12-cylinder/design-review.md:1538:    fn test_plane_normal_constant_per_point() {
features/12-cylinder/design-review.md:2032:/// A plane defined by an origin point and a normal vector.
features/12-cylinder/design-review.md:2169:    fn test_roundtrip_yaml() {
features/12-cylinder/design-review.md:2230:    fn test_assembly_roundtrip() {
features/12-cylinder/design-review.md:2259:/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/tests/cuboid_roundtrip.rs" in /home/bacon/mycad
features/12-cylinder/design-review.md:2486:- golden YAML ラウンドトリップテストがあるか
features/12-cylinder/design-review.md:2520:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/12-cylinder/design-review.md:2529:    section: "設計方針 > 1. トポロジー / テスト計画 > T04"
features/12-cylinder/design-review.md:2530:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/12-cylinder/design-review.md:2540:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
features/12-cylinder/design-review.md:2550:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/12-cylinder/design-review.md:2559:    section: "設計方針 > 1. トポロジー / テスト計画 > T04"
features/12-cylinder/design-review.md:2560:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/12-cylinder/design-review.md:2570:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
crates/mycad-api/tests/mesh_api.rs:50:// T01: normal box
crates/mycad-api/tests/mesh_api.rs:62:// T02: normal cylinder
crates/mycad-api/tests/mesh_api.rs:73:// T03: determinism
crates/mycad-api/tests/mesh_api.rs:93:// T04: 404 not found
crates/mycad-api/tests/mesh_api.rs:103:// T05: 400 non-.mycad extension
crates/mycad-api/tests/mesh_api.rs:121:// T06: 403 Host header (DNS rebinding protection)
crates/mycad-api/tests/mesh_api.rs:134:    // The plan says to add Host header validation middleware.
crates/mycad-api/tests/mesh_api.rs:140:// T07: 422 unsupported feature (sphere)
crates/mycad-api/tests/mesh_api.rs:149:// T08: 422 degenerate dimension (zero-width box)
crates/mycad-api/tests/mesh_api.rs:158:// T09: 422 assembly unsupported
crates/mycad-api/tests/mesh_api.rs:173:// T10: 422 empty part
crates/mycad-api/tests/mesh_api.rs:188:// T11: 400 relative path rejected
crates/mycad-kernel/src/primitives/cylinder.rs:133:    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
crates/mycad-kernel/src/primitives/cylinder.rs:161:    /// T02: Topology counts.
crates/mycad-kernel/src/primitives/cylinder.rs:180:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
crates/mycad-kernel/src/primitives/cylinder.rs:199:    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
crates/mycad-kernel/src/primitives/cylinder.rs:254:    /// T05: Geometry — seam vertices and surface coefficients.
crates/mycad-kernel/src/primitives/cylinder.rs:299:    /// T14: Degenerate inputs — radius<=0, height<=0, non-finite → InvalidParameter.
crates/mycad-kernel/src/geometry/mod.rs:14:/// A plane defined by an origin point and a normal vector.
features/12-cylinder/design-review-2.md:45:- golden YAML ラウンドトリップテストがあるか
features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/12-cylinder/design-review-2.md:181:- R05(medium) 反映: golden YAML ラウンドトリップテスト T13 追加。
features/12-cylinder/design-review-2.md:187:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/12-cylinder/design-review-2.md:188:| T02 | トポロジー | 頂点2・エッジ3・面3・シェル1(closed)・HE6・loop3 | 各 `len()` 一致 |
features/12-cylinder/design-review-2.md:189:| T03 | Euler | V−E+F == 2 | `assert_eq!(2-3+3, 2)` 相当の検証 |
features/12-cylinder/design-review-2.md:190:| T04 | マニフォルド+ループ閉包 | 各エッジがちょうど 2 HE に参照され向きが逆。**かつ**各 loop で連続 HE の終点 ≈ 次 HE の始点、最後の HE 終点 ≈ 先頭 HE 始点（`approx`）。seam 接続も検証 | 集計 + 幾何連続性検証 (R03) |
features/12-cylinder/design-review-2.md:191:| T05 | 幾何 | シーム頂点が (0,r,0)/(0,r,h)、各 Surface/Curve の係数 | `approx` で一致 |
features/12-cylinder/design-review-2.md:192:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
features/12-cylinder/design-review-2.md:193:| T07 | 決定性(tessellate) | 同一入力2回で positions/normals/indices 一致 | `assert_eq!` |
features/12-cylinder/design-review-2.md:194:| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index。cuboid は逆向き Line HE を含むため R01 の向き対応も同時に担保 | 既存テスト維持 + positions が従来値と一致 |
features/12-cylinder/design-review-2.md:195:| T09 | 外向き法線 | 全 facet の幾何法線が重心からの外向きと同符号（凸形状なので判定可） | 検証 |
features/12-cylinder/design-review-2.md:196:| T10 | ディスパッチ統合 | `examples/cylinder.mycad` を `build_solid_from_features` でビルドし T02 と同じ counts | `feature_dispatcher.rs` |
features/12-cylinder/design-review-2.md:197:| T11 | export 統合 | `mycad export examples/cylinder.mycad -o tmp.stl` が成功し、facet 数が T06 と整合、ASCII STL として妥当 | `export.rs`（`CARGO_BIN_EXE_mycad`） |
features/12-cylinder/design-review-2.md:198:| T12 | 解像度外部化+クランプ | `tessellate_solid_with` で分割数が変わり三角形数が増減。`TessellationOptions::new(2,0)` が `angular=3/axial=1` にクランプされる | 2 解像度で差分 + クランプ検証 (R02) |
features/12-cylinder/design-review-2.md:199:| T13 | golden YAML ラウンドトリップ | `examples/cylinder.mycad` を `from_path`→`to_yaml`→`from_yaml`→`to_yaml` で equality。`create_cylinder` の serialize/schema 回帰を固定 | `assert_eq!` (R05) |
features/12-cylinder/design-review-2.md:200:| T14 | 退化入力エラー | `radius<=0` / `height<=0` の `create_cylinder` を `build_solid_from_features` に渡すと `KernelError::InvalidParameter` | `assert!(matches!(..))` (R02) |
features/12-cylinder/design-review-2.md:204:1. `cargo test --workspace` で T01–T12 green。
features/12-cylinder/design-review-2.md:629:        // are shaded correctly (planar faces yield a constant normal).
features/12-cylinder/design-review-2.md:700:    /// An infinite plane.
features/12-cylinder/design-review-2.md:802:    /// surface is planar / has a constant normal.
features/12-cylinder/design-review-2.md:848:    /// never have to special-case planar vs curved surfaces.
features/12-cylinder/design-review-2.md:860:    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
features/12-cylinder/design-review-2.md:868:    fn test_uv_of_roundtrip_plane() {
features/12-cylinder/design-review-2.md:875:        assert_uv_roundtrip(&surface, 0.7, -1.3);
features/12-cylinder/design-review-2.md:879:    fn test_uv_of_roundtrip_cylinder() {
features/12-cylinder/design-review-2.md:885:        assert_uv_roundtrip(&surface, 0.9, 1.7);
features/12-cylinder/design-review-2.md:889:    fn test_uv_of_roundtrip_sphere() {
features/12-cylinder/design-review-2.md:894:        assert_uv_roundtrip(&surface, 1.1, 0.6);
features/12-cylinder/design-review-2.md:898:    fn test_uv_of_roundtrip_cone() {
features/12-cylinder/design-review-2.md:904:        assert_uv_roundtrip(&surface, 0.8, 2.0);
features/12-cylinder/design-review-2.md:925:    fn test_plane_normal_constant_per_point() {
features/12-cylinder/design-review-2.md:1671:    fn test_roundtrip_yaml() {
features/12-cylinder/design-review-2.md:1732:    fn test_assembly_roundtrip() {
features/12-cylinder/design-review-2.md:1789:/bin/bash -lc 'rg -n "golden|roundtrip|tessellate_solid_with|TessellationOptions|export" crates/mycad-cli tests crates/mycad-kernel crates/mycad-build' in /home/bacon/mycad
features/12-cylinder/design-review-2.md:1798:crates/mycad-kernel/src/geometry/surface.rs:168:    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
features/12-cylinder/design-review-2.md:1799:crates/mycad-kernel/src/geometry/surface.rs:176:    fn test_uv_of_roundtrip_plane() {
features/12-cylinder/design-review-2.md:1800:crates/mycad-kernel/src/geometry/surface.rs:183:        assert_uv_roundtrip(&surface, 0.7, -1.3);
features/12-cylinder/design-review-2.md:1801:crates/mycad-kernel/src/geometry/surface.rs:187:    fn test_uv_of_roundtrip_cylinder() {
features/12-cylinder/design-review-2.md:1802:crates/mycad-kernel/src/geometry/surface.rs:193:        assert_uv_roundtrip(&surface, 0.9, 1.7);
features/12-cylinder/design-review-2.md:1803:crates/mycad-kernel/src/geometry/surface.rs:197:    fn test_uv_of_roundtrip_sphere() {
features/12-cylinder/design-review-2.md:1804:crates/mycad-kernel/src/geometry/surface.rs:202:        assert_uv_roundtrip(&surface, 1.1, 0.6);
features/12-cylinder/design-review-2.md:1805:crates/mycad-kernel/src/geometry/surface.rs:206:    fn test_uv_of_roundtrip_cone() {
features/12-cylinder/design-review-2.md:1806:crates/mycad-kernel/src/geometry/surface.rs:212:        assert_uv_roundtrip(&surface, 0.8, 2.0);
features/12-cylinder/design-review-2.md:1936:./features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/12-cylinder/design-review-2.md:2308:    suggestion: "この変更を意図的な winding 修正として明記し、`T08` を『counts は維持するが indices / STL facet normal は変わり得る』前提に更新するか、先に cuboid 側の loop orientation を正規化して本当に不変にすること。"
features/12-cylinder/design-review-2.md:2311:    section: "テスト計画 > T03"
features/12-cylinder/design-review-2.md:2312:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/12-cylinder/design-review-2.md:2333:    suggestion: "この変更を意図的な winding 修正として明記し、`T08` を『counts は維持するが indices / STL facet normal は変わり得る』前提に更新するか、先に cuboid 側の loop orientation を正規化して本当に不変にすること。"
features/12-cylinder/design-review-2.md:2336:    section: "テスト計画 > T03"
features/12-cylinder/design-review-2.md:2337:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/12-cylinder/final-review-2.md:162: /// Triangulate a face as a fan from its first boundary vertex (planar convex faces).
features/12-cylinder/final-review-2.md:342:             Surface::Plane { .. } => "plane",
features/12-cylinder/final-review-2.md:532:    /// Fan from first boundary vertex (convex planar face).
features/12-cylinder/final-review-2.md:543:    /// An infinite plane.
features/12-cylinder/final-review-2.md:645:    /// surface is planar / has a constant normal.
features/12-cylinder/final-review-2.md:691:    /// never have to special-case planar vs curved surfaces.
features/12-cylinder/final-review-2.md:712:    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
features/12-cylinder/final-review-2.md:720:    fn test_uv_of_roundtrip_plane() {
features/12-cylinder/final-review-2.md:727:        assert_uv_roundtrip(&surface, 0.7, -1.3);
features/12-cylinder/final-review-2.md:731:    fn test_uv_of_roundtrip_cylinder() {
features/12-cylinder/final-review-2.md:737:        assert_uv_roundtrip(&surface, 0.9, 1.7);
features/12-cylinder/final-review-2.md:741:    fn test_uv_of_roundtrip_sphere() {
features/12-cylinder/final-review-2.md:746:        assert_uv_roundtrip(&surface, 1.1, 0.6);
features/12-cylinder/final-review-2.md:750:    fn test_uv_of_roundtrip_cone() {
features/12-cylinder/final-review-2.md:756:        assert_uv_roundtrip(&surface, 0.8, 2.0);
features/12-cylinder/final-review-2.md:901:    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
features/12-cylinder/final-review-2.md:929:    /// T02: Topology counts.
features/12-cylinder/final-review-2.md:948:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/12-cylinder/final-review-2.md:967:    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
features/18-api-skeleton/plan.md:9:> Codex 設計レビュー round 1-3 反映済み(3回上限)。round3 残存指摘は全て受諾し最終修正済み: R01=絶対パスのみ受理(決定的)、R02=v0 専用テッセレーション定数で Default 依存排除、R03=InvalidReference→422、R04=T09/T10 で assembly と空 part を分離、R05=K01 に INFINITY 分岐追加。round3 の verdict は fail のままだが指摘内容は本改訂で解消済み(ユーザー判断で再レビューせず確定)。
features/18-api-skeleton/plan.md:26:| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
features/18-api-skeleton/plan.md:74:- 相対パス拒否の専用テスト(T11)を追加。
features/18-api-skeleton/plan.md:107:| T01 | 正常系(box) | `GET /api/v0/mesh?file=<abs>/examples/simple_box.mycad` を oneshot | 200、`TriangleMesh` deser 可、`positions`/`indices` 非空、`indices.len() % 3 == 0` |
features/18-api-skeleton/plan.md:108:| T02 | 正常系(cylinder)(R05) | 同上 `examples/cylinder.mycad`(周期面経路) | 200、mesh 非空 |
features/18-api-skeleton/plan.md:109:| T03 | 決定性 | T01/T02 を 2 回実行 | 返る JSON バイト列が完全一致 |
features/18-api-skeleton/plan.md:110:| T04 | 404 | 存在しない `file` | 404、body `{"error": ...}` |
features/18-api-skeleton/plan.md:111:| T05 | 400 | 非 `.mycad` 拡張子(`Cargo.toml`) | 400(InvalidExtension) |
features/18-api-skeleton/plan.md:112:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/plan.md:113:| T07 | 422 未対応 feature(R03) | fixture `create_sphere` 単一 feature | 422(UnsupportedFeature) |
features/18-api-skeleton/plan.md:114:| T08 | 422 退化寸法(R02) | fixture `create_box` width=0(または負値) | 422(InvalidParameter) |
features/18-api-skeleton/plan.md:115:| T09 | 422 assembly 非対応(R04) | fixture: root features 空 + children あり | 422、body に assembly/reference 専用メッセージ(空 part とは別経路) |
features/18-api-skeleton/plan.md:116:| T10 | 422 空 part(R04) | fixture: 単一 part だが features が空(children/ref なし) | 422(EmptyFeatureList、assembly とは別メッセージ) |
features/18-api-skeleton/plan.md:117:| T11 | 400 相対パス(R01) | `file=examples/simple_box.mycad`(相対) | 400(相対パス拒否) |
features/18-api-skeleton/plan.md:121:> **R05 反論**(round 1): Euler-Poincaré は kernel(`cylinder.rs:180-195`)、golden YAML roundtrip は format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。
features/18-api-skeleton/plan.md:125:> **R04 反論**(round 2): 新規 `crates/mycad-api/tests/fixtures/*.mycad` は T07-T09 で API 経由で必ずパースされる(パースできなければ期待する 422 が出ず test fail)ため、入力形状の妥当性は API テストで担保済み。`Document` の YAML roundtrip 安定性は `mycad-format` の既存 roundtrip テストが所有。クロスクレートの roundtrip 追加は「テストは所有クレートに置く」規約に反するため行わない。fixture は最小に保つ。
features/18-api-skeleton/plan.md:141:- **STEP 3(Codex 設計レビュー)はプランモードを抜けずに実行する**(Bash dispatch は plan mode でも通る。deny は `crates/**` の Edit/Write のみ)。本改訂後に再レビュー(round 2/3)し、Critical/High 解消後に STEP 4 `ExitPlanMode` で最終承認。
features/12-cylinder/final-review.md:29: crates/mycad-kernel/tests/cuboid_roundtrip.rs  |   4 +-
features/12-cylinder/final-review.md:41:    BoundaryFan for plane caps, UvGridFullPatch for the cylinder side surface.
features/12-cylinder/final-review.md:49:    - examples/cylinder.mycad + T01-T15 test suite (32 tests green)
features/12-cylinder/final-review.md:304:+    /// Fan from first boundary vertex (convex planar face).
features/12-cylinder/final-review.md:315:     /// An infinite plane.
features/12-cylinder/final-review.md:417:     /// surface is planar / has a constant normal.
features/12-cylinder/final-review.md:463:     /// never have to special-case planar vs curved surfaces.
features/12-cylinder/final-review.md:484:     fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
features/12-cylinder/final-review.md:492:     fn test_uv_of_roundtrip_plane() {
features/12-cylinder/final-review.md:499:         assert_uv_roundtrip(&surface, 0.7, -1.3);
features/12-cylinder/final-review.md:503:     fn test_uv_of_roundtrip_cylinder() {
features/12-cylinder/final-review.md:509:         assert_uv_roundtrip(&surface, 0.9, 1.7);
features/12-cylinder/final-review.md:513:     fn test_uv_of_roundtrip_sphere() {
features/12-cylinder/final-review.md:518:         assert_uv_roundtrip(&surface, 1.1, 0.6);
features/12-cylinder/final-review.md:522:     fn test_uv_of_roundtrip_cone() {
features/12-cylinder/final-review.md:528:         assert_uv_roundtrip(&surface, 0.8, 2.0);
features/12-cylinder/final-review.md:549:     fn test_plane_normal_constant_per_point() {
features/12-cylinder/final-review.md:695:+    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
features/12-cylinder/final-review.md:723:+    /// T02: Topology counts.
features/12-cylinder/final-review.md:742:+    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/12-cylinder/final-review.md:761:+    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
features/12-cylinder/final-review.md:816:+    /// T05: Geometry — seam vertices and surface coefficients.
features/12-cylinder/final-review.md:861:+    /// T14: Degenerate inputs — radius<=0, height<=0, non-finite → InvalidParameter.
features/12-cylinder/final-review.md:1092:-        // are shaded correctly (planar faces yield a constant normal).
features/12-cylinder/final-review.md:1094:+/// Triangulate a face as a fan from its first boundary vertex (planar convex faces).
features/12-cylinder/final-review.md:1263:+            Surface::Plane { .. } => "plane",
features/12-cylinder/final-review.md:1279:+    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
features/12-cylinder/final-review.md:1298:+    /// T07: Determinism — same input → same output.
features/12-cylinder/final-review.md:1315:+    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
features/12-cylinder/final-review.md:1328:+    /// T08 extended: exact positions and indices snapshot.
features/12-cylinder/final-review.md:1345:+    /// T09: Outward normals — all facet normals point away from centroid.
features/12-cylinder/final-review.md:1392:+    /// T12: Resolution externalization + clamping.
features/12-cylinder/final-review.md:1578:diff --git a/crates/mycad-kernel/tests/cuboid_roundtrip.rs b/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/12-cylinder/final-review.md:1580:--- a/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/12-cylinder/final-review.md:1581:+++ b/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/12-cylinder/final-review.md:2003:    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
features/12-cylinder/final-review.md:2031:    /// T02: Topology counts.
features/12-cylinder/final-review.md:2050:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/12-cylinder/final-review.md:2069:    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
features/12-cylinder/final-review.md:2124:    /// T05: Geometry — seam vertices and surface coefficients.
features/12-cylinder/final-review.md:2726:/// Triangulate a face as a fan from its first boundary vertex (planar convex faces).
features/12-cylinder/final-review.md:2889:            Surface::Plane { .. } => "plane",
features/12-cylinder/final-review.md:2905:    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
features/12-cylinder/final-review.md:2924:    /// T07: Determinism — same input → same output.
features/12-cylinder/final-review.md:2941:    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
features/12-cylinder/final-review.md:2954:    /// T08 extended: exact positions and indices snapshot.
features/12-cylinder/final-review.md:2971:    /// T09: Outward normals — all facet normals point away from centroid.
features/12-cylinder/final-review.md:3018:    /// T12: Resolution externalization + clamping.
features/12-cylinder/final-review.md:3155:    /// T14: Degenerate inputs — radius<=0, height<=0, non-finite → InvalidParameter.
features/12-cylinder/final-review.md:3236:./features/12-cylinder/design-review-3.md:199:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
features/12-cylinder/final-review.md:3245:./features/12-cylinder/design-review-3.md:295:crates/mycad-kernel/tests/cuboid_roundtrip.rs:17:    let mesh = tessellate_solid(&solid);
features/12-cylinder/final-review.md:3246:./features/12-cylinder/design-review-3.md:298:crates/mycad-kernel/tests/cuboid_roundtrip.rs:32:        tessellate_solid(&solid)
features/12-cylinder/final-review.md:3298:./features/12-cylinder/design-review-2.md:192:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
features/12-cylinder/final-review.md:3311:./features/12-cylinder/design-review.md:181:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
features/12-cylinder/final-review.md:3313:./features/12-cylinder/design-review.md:464:features/12-cylinder/design-review.md:181:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
features/12-cylinder/final-review.md:3322:./features/12-cylinder/design-review.md:585:crates/mycad-kernel/tests/cuboid_roundtrip.rs:17:    let mesh = tessellate_solid(&solid);
features/12-cylinder/final-review.md:3323:./features/12-cylinder/design-review.md:588:crates/mycad-kernel/tests/cuboid_roundtrip.rs:32:        tessellate_solid(&solid)
features/12-cylinder/final-review.md:3331:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:17:    let mesh = tessellate_solid(&solid).unwrap();
features/12-cylinder/final-review.md:3332:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:32:        tessellate_solid(&solid).unwrap()
features/12-cylinder/final-review.md:3423:        // are shaded correctly (planar faces yield a constant normal).
features/12-cylinder/final-review.md:3551:1392:+    /// T12: Resolution externalization + clamping.
features/12-cylinder/final-review.md:3606:3018:    /// T12: Resolution externalization + clamping.
features/12-cylinder/final-review.md:3672:185:- R01(high) 反映: `is_planar() -> bool` の二分岐は将来 variant を既定経路へ静かに流す危険がある → `tessellation_strategy() -> TessellationStrategy`（enum・網羅 match）へ変更。さらに UvGrid 経路にトリム面ガード（`inner_loops` 非空はスキップ＋`debug_assert`）を追加し、穴あき面の誤メッシュ化を防止（§2）。
features/12-cylinder/final-review.md:3685:1011:    /// An infinite plane.
features/12-cylinder/final-review.md:3755:    section: "テスト計画 > T01"
features/12-cylinder/final-review.md:3756:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/12-cylinder/final-review.md:3785:    section: "テスト計画 > T01"
features/12-cylinder/final-review.md:3786:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/12-cylinder/final-review.md:3840:    section: "テスト計画 > T01"
features/12-cylinder/final-review.md:3841:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/12-cylinder/final-review.md:3870:    section: "テスト計画 > T01"
features/12-cylinder/final-review.md:3989:   115	/// Triangulate a face as a fan from its first boundary vertex (planar convex faces).
features/12-cylinder/final-review.md:4155:   276	            Surface::Plane { .. } => "plane",
features/12-cylinder/final-review.md:4171:   292	    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
features/12-cylinder/final-review.md:4190:   311	    /// T07: Determinism — same input → same output.
features/12-cylinder/final-review.md:4207:   328	    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
features/12-cylinder/final-review.md:4220:   341	    /// T08 extended: exact positions and indices snapshot.
features/12-cylinder/final-review.md:4237:   358	    /// T09: Outward normals — all facet normals point away from centroid.
features/12-cylinder/final-review.md:4284:   405	    /// T12: Resolution externalization + clamping.
features/12-cylinder/final-review.md:4393:861:+    /// T14: Degenerate inputs — radius<=0, height<=0, non-finite → InvalidParameter.
features/12-cylinder/final-review.md:4406:3155:    /// T14: Degenerate inputs — radius<=0, height<=0, non-finite → InvalidParameter.
features/12-cylinder/final-review.md:4424:  `TessellationOptions::new()` clamps invalid values, but callers can bypass that via the public fields or `Deserialize`. If `angular_segments == 0` or `axial_segments == 0`, these divisions produce `inf`, and the subsequent `0 * inf` in the UV loop yields `NaN` vertices while the planar caps are skipped for having too few sampled points. In other words, a perfectly valid cylinder can export an invalid mesh just by passing a deserialized or literal `TessellationOptions` value.
features/12-cylinder/final-review.md:4433:  `TessellationOptions::new()` clamps invalid values, but callers can bypass that via the public fields or `Deserialize`. If `angular_segments == 0` or `axial_segments == 0`, these divisions produce `inf`, and the subsequent `0 * inf` in the UV loop yields `NaN` vertices while the planar caps are skipped for having too few sampled points. In other words, a perfectly valid cylinder can export an invalid mesh just by passing a deserialized or literal `TessellationOptions` value.
crates/mycad-kernel/src/geometry/surface.rs:8:    /// Fan from first boundary vertex (convex planar face).
crates/mycad-kernel/src/geometry/surface.rs:19:    /// An infinite plane.
crates/mycad-kernel/src/geometry/surface.rs:121:    /// surface is planar / has a constant normal.
crates/mycad-kernel/src/geometry/surface.rs:167:    /// never have to special-case planar vs curved surfaces.
crates/mycad-kernel/src/geometry/surface.rs:188:    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
crates/mycad-kernel/src/geometry/surface.rs:196:    fn test_uv_of_roundtrip_plane() {
crates/mycad-kernel/src/geometry/surface.rs:203:        assert_uv_roundtrip(&surface, 0.7, -1.3);
crates/mycad-kernel/src/geometry/surface.rs:207:    fn test_uv_of_roundtrip_cylinder() {
crates/mycad-kernel/src/geometry/surface.rs:213:        assert_uv_roundtrip(&surface, 0.9, 1.7);
crates/mycad-kernel/src/geometry/surface.rs:217:    fn test_uv_of_roundtrip_sphere() {
crates/mycad-kernel/src/geometry/surface.rs:222:        assert_uv_roundtrip(&surface, 1.1, 0.6);
crates/mycad-kernel/src/geometry/surface.rs:226:    fn test_uv_of_roundtrip_cone() {
crates/mycad-kernel/src/geometry/surface.rs:232:        assert_uv_roundtrip(&surface, 0.8, 2.0);
crates/mycad-kernel/src/geometry/surface.rs:253:    fn test_plane_normal_constant_per_point() {
features/18-api-skeleton/final-review.md:43:- 設計レビュー時に合意したテスト計画（ID T01〜）が全て実装されているか
features/18-api-skeleton/final-review.md:45:- golden YAML ラウンドトリップが含まれているか
features/18-api-skeleton/final-review.md:87:crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:101: M crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:120: crates/mycad-kernel/tests/cuboid_roundtrip.rs |   4 +-
features/18-api-skeleton/final-review.md:155:features/18-api-skeleton/plan.md
features/18-api-skeleton/final-review.md:159:/bin/bash -lc 'git diff -- Cargo.toml Cargo.lock crates/mycad-build/src/lib.rs crates/mycad-kernel/benches/tessellation.rs crates/mycad-kernel/src/primitives/cuboid.rs crates/mycad-kernel/src/tessellation/mod.rs crates/mycad-kernel/src/tessellation/stl.rs crates/mycad-kernel/tests/cuboid_roundtrip.rs' in /home/bacon/mycad
features/18-api-skeleton/final-review.md:905:diff --git a/crates/mycad-kernel/tests/cuboid_roundtrip.rs b/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:907:--- a/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:908:+++ b/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:1230:    50	// T01: normal box
features/18-api-skeleton/final-review.md:1242:    62	// T02: normal cylinder
features/18-api-skeleton/final-review.md:1253:    73	// T03: determinism
features/18-api-skeleton/final-review.md:1273:    93	// T04: 404 not found
features/18-api-skeleton/final-review.md:1283:   103	// T05: 400 non-.mycad extension
features/18-api-skeleton/final-review.md:1301:   121	// T06: 403 Host header (DNS rebinding protection)
features/18-api-skeleton/final-review.md:1314:   134	    // The plan says to add Host header validation middleware.
features/18-api-skeleton/final-review.md:1320:   140	// T07: 422 unsupported feature (sphere)
features/18-api-skeleton/final-review.md:1329:   149	// T08: 422 degenerate dimension (zero-width box)
features/18-api-skeleton/final-review.md:1338:   158	// T09: 422 assembly unsupported
features/18-api-skeleton/final-review.md:1353:   173	// T10: 422 empty part
features/18-api-skeleton/final-review.md:1368:   188	// T11: 400 relative path rejected
features/18-api-skeleton/final-review.md:1439:crates/mycad-kernel/src/primitives/cylinder.rs:299:    /// T14: Degenerate inputs — radius<=0, height<=0, non-finite → InvalidParameter.
features/18-api-skeleton/final-review.md:1448:crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review.md:1449:crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review.md:1492:2026-05-25T06:37:17.744507Z ERROR codex_core::tools::router: error=exec_command failed for `/bin/bash -lc 'CARGO_TARGET_DIR=/tmp/mycad-target cargo test -p mycad-kernel --no-run'`: SandboxDenied { message: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", output: ExecToolCallOutput { exit_code: 101, stdout: StreamOutput { text: "", truncated_after_lines: None }, stderr: StreamOutput { text: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", truncated_after_lines: None }, aggregated_output: StreamOutput { text: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", truncated_after_lines: None }, duration: 0ns, timed_out: false } }
features/18-api-skeleton/final-review.md:1501:2026-05-25T06:37:17.862952Z ERROR codex_core::tools::router: error=exec_command failed for `/bin/bash -lc 'CARGO_TARGET_DIR=/tmp/mycad-target cargo test -p mycad-api --no-run'`: SandboxDenied { message: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", output: ExecToolCallOutput { exit_code: 101, stdout: StreamOutput { text: "", truncated_after_lines: None }, stderr: StreamOutput { text: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", truncated_after_lines: None }, aggregated_output: StreamOutput { text: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", truncated_after_lines: None }, duration: 0ns, timed_out: false } }
features/18-api-skeleton/final-review.md:1509:コンパイル確認は `sccache` 制約で止まったので、ラッパーを外して再試行します。あわせて、差分内コメントの `T01` 系テストが設計レビュー合意と一致しているかも確認します。
features/18-api-skeleton/design-review-3.md:45:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/design-review-3.md:83:> Codex 設計レビュー round 1 + round 2 反映済み。round1: R01/R03/R04 受諾、R02 kernel 検証追加(ユーザー決定)、R05 部分受諾。round2: R01(構造化エラー)受諾、R02(assembly 判別)受諾、R03(公差)は #17 委譲で反論、R04(fixture roundtrip)反論。
features/18-api-skeleton/design-review-3.md:100:| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
features/18-api-skeleton/design-review-3.md:181:| T01 | 正常系(box) | `GET /api/v0/mesh?file=<abs>/examples/simple_box.mycad` を oneshot | 200、`TriangleMesh` deser 可、`positions`/`indices` 非空、`indices.len() % 3 == 0` |
features/18-api-skeleton/design-review-3.md:182:| T02 | 正常系(cylinder)(R05) | 同上 `examples/cylinder.mycad`(周期面経路) | 200、mesh 非空 |
features/18-api-skeleton/design-review-3.md:183:| T03 | 決定性 | T01/T02 を 2 回実行 | 返る JSON バイト列が完全一致 |
features/18-api-skeleton/design-review-3.md:184:| T04 | 404 | 存在しない `file` | 404、body `{"error": ...}` |
features/18-api-skeleton/design-review-3.md:185:| T05 | 400 | 非 `.mycad` 拡張子(`Cargo.toml`) | 400(InvalidExtension) |
features/18-api-skeleton/design-review-3.md:186:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/design-review-3.md:187:| T07 | 422 未対応 feature(R03) | fixture `create_sphere` 単一 feature | 422(UnsupportedFeature) |
features/18-api-skeleton/design-review-3.md:188:| T08 | 422 退化寸法(R02) | fixture `create_box` width=0(または負値) | 422(InvalidParameter) |
features/18-api-skeleton/design-review-3.md:189:| T09 | 422 assembly 非対応(R03) | fixture: root features 空 + children あり | 422(EmptyFeatureList、クラッシュしない) |
features/18-api-skeleton/design-review-3.md:193:> **R05 反論**(round 1): Euler-Poincaré は kernel(`cylinder.rs:180-195`)、golden YAML roundtrip は format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。
features/18-api-skeleton/design-review-3.md:197:> **R04 反論**(round 2): 新規 `crates/mycad-api/tests/fixtures/*.mycad` は T07-T09 で API 経由で必ずパースされる(パースできなければ期待する 422 が出ず test fail)ため、入力形状の妥当性は API テストで担保済み。`Document` の YAML roundtrip 安定性は `mycad-format` の既存 roundtrip テストが所有。クロスクレートの roundtrip 追加は「テストは所有クレートに置く」規約に反するため行わない。fixture は最小に保つ。
features/18-api-skeleton/design-review-3.md:211:- **STEP 3(Codex 設計レビュー)はプランモードを抜けずに実行する**(Bash dispatch は plan mode でも通る。deny は `crates/**` の Edit/Write のみ)。本改訂後に再レビュー(round 2/3)し、Critical/High 解消後に STEP 4 `ExitPlanMode` で最終承認。
features/18-api-skeleton/design-review-3.md:239:crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-3.md:240:crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-3.md:241:crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/18-api-skeleton/design-review-3.md:371:    fn test_roundtrip_yaml() {
features/18-api-skeleton/design-review-3.md:432:    fn test_assembly_roundtrip() {
features/18-api-skeleton/design-review-3.md:598:/// Triangulate a face as a fan from its first boundary vertex (planar convex faces).
features/18-api-skeleton/design-review-3.md:1210:crates/mycad-kernel/tests/cuboid_roundtrip.rs:26:/// Integration test: deterministic pipeline produces identical results.
features/18-api-skeleton/design-review-3.md:1211:crates/mycad-kernel/tests/cuboid_roundtrip.rs:28:fn cuboid_pipeline_is_deterministic() {
features/18-api-skeleton/design-review-3.md:1215:crates/mycad-kernel/src/primitives/cylinder.rs:180:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review-3.md:1244:    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
features/18-api-skeleton/design-review-3.md:1272:    /// T02: Topology counts.
features/18-api-skeleton/design-review-3.md:1291:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review-3.md:1310:    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
features/18-api-skeleton/design-review-3.md:1363:    /// T07: Determinism — same input → same output.
features/18-api-skeleton/design-review-3.md:1380:    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
features/18-api-skeleton/design-review-3.md:1393:    /// T08 extended: exact positions and indices snapshot.
features/18-api-skeleton/design-review-3.md:1410:    /// T09: Outward normals — all facet normals point away from centroid.
features/18-api-skeleton/design-review-3.md:1457:    /// T12: Resolution externalization + clamping.
features/18-api-skeleton/design-review-3.md:1816:/bin/bash -lc "sed -n '1,140p' crates/mycad-kernel/tests/cuboid_roundtrip.rs" in /home/bacon/mycad
features/18-api-skeleton/design-review-3.md:1882:    section: "テスト計画 > T09"
features/18-api-skeleton/design-review-3.md:1883:    finding: "設計本文では assembly/reference を build 前に検出して専用の 422 にするとしている一方、T09 の期待値は `422(EmptyFeatureList)` のままで矛盾している。これでは assembly と空 part を区別しない旧挙動でもテストが通る。"
features/18-api-skeleton/design-review-3.md:1884:    suggestion: "T09 は assembly/reference 専用メッセージを明示的に assert し、`features.is_empty()` の空 part は別テストに分離すること。"
features/18-api-skeleton/design-review-3.md:1916:    section: "テスト計画 > T09"
features/18-api-skeleton/design-review-3.md:1917:    finding: "設計本文では assembly/reference を build 前に検出して専用の 422 にするとしている一方、T09 の期待値は `422(EmptyFeatureList)` のままで矛盾している。これでは assembly と空 part を区別しない旧挙動でもテストが通る。"
features/18-api-skeleton/design-review-3.md:1918:    suggestion: "T09 は assembly/reference 専用メッセージを明示的に assert し、`features.is_empty()` の空 part は別テストに分離すること。"
features/12-cylinder/design-review-3.md:45:- golden YAML ラウンドトリップテストがあるか
features/12-cylinder/design-review-3.md:149:  2. `BoundaryFan`（平面） → 境界点列を **頂点0 から fan**（凸前提。cuboid 4 角=従来 2 三角形で不変、円盤キャップ=N角形 fan）。巻き方向は §1 で構築済み（底キャップ rev / 上キャップ fwd）なので fan の幾何法線が外向きになる。**スワップなし → cuboid のインデックス列もバイト不変**（T08 で固定）。
features/12-cylinder/design-review-3.md:180:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/12-cylinder/design-review-3.md:182:- R05(medium) 反映: golden YAML ラウンドトリップテスト T13 追加。
features/12-cylinder/design-review-3.md:185:- R01(high) 反映: `is_planar() -> bool` の二分岐は将来 variant を既定経路へ静かに流す危険がある → `tessellation_strategy() -> TessellationStrategy`（enum・網羅 match）へ変更。さらに UvGrid 経路にトリム面ガード（`inner_loops` 非空はスキップ＋`debug_assert`）を追加し、穴あき面の誤メッシュ化を防止（§2）。
features/12-cylinder/design-review-3.md:186:- R02(high) **検証して反映**: cuboid.rs / stl.rs を読み確認 → 「実行時の向き整合スワップは cuboid 出力不変」という前回主張は **誤り**（現行 cuboid のインデックス巻きが右手系外向き規約と一致する保証はなく、スワップ導入で index 列が変わり T08 が回帰しうる）。→ **テッセレータからスワップを撤去**し、外向きは §1 の**構築時巻き方向**（底キャップ=e_bot rev / 上キャップ=e_top fwd / 側面グリッド=∂u×∂v 外向き）で保証。平面 fan 経路は現行コードとバイト同一 → cuboid 完全不変（T08 で固定）。
features/12-cylinder/design-review-3.md:188:- T03 を「実カウントから Euler を計算」へ修正（定数ハードコード撤廃）。
features/12-cylinder/design-review-3.md:194:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/12-cylinder/design-review-3.md:195:| T02 | トポロジー | 頂点2・エッジ3・面3・シェル1(closed)・HE6・loop3 | 各 `len()` 一致 |
features/12-cylinder/design-review-3.md:196:| T03 | Euler | 実 solid の `vertices.len() − edges.len() + faces.len() == 2 * (shells.len() − genus)`（genus=0）を**実カウントから**計算して検証（定数ハードコードでなく構造体から） | `assert_eq!` |
features/12-cylinder/design-review-3.md:197:| T04 | マニフォルド+ループ閉包 | 各エッジがちょうど 2 HE に参照され向きが逆。**かつ**各 loop で連続 HE の終点 ≈ 次 HE の始点、最後の HE 終点 ≈ 先頭 HE 始点（`approx`）。seam 接続も検証 | 集計 + 幾何連続性検証 (R03) |
features/12-cylinder/design-review-3.md:198:| T05 | 幾何 | シーム頂点が (0,r,0)/(0,r,h)、各 Surface/Curve の係数 | `approx` で一致 |
features/12-cylinder/design-review-3.md:199:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
features/12-cylinder/design-review-3.md:200:| T07 | 決定性(tessellate) | 同一入力2回で positions/normals/indices 一致 | `assert_eq!` |
features/12-cylinder/design-review-3.md:201:| T08 | 回帰(平面バイト不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index。**positions も indices も従来値と完全一致**（スワップ撤去によりバイト不変、R02）。cuboid は逆向き Line HE を含むため R01 の向き対応も同時に担保 | 既存テスト維持 + positions/indices 一致 |
features/12-cylinder/design-review-3.md:202:| T09 | 外向き法線 | 全 facet の幾何法線が重心からの外向きと同符号（凸形状なので判定可） | 検証 |
features/12-cylinder/design-review-3.md:203:| T10 | ディスパッチ統合 | `examples/cylinder.mycad` を `build_solid_from_features` でビルドし T02 と同じ counts | `feature_dispatcher.rs` |
features/12-cylinder/design-review-3.md:204:| T11 | export 統合 | `mycad export examples/cylinder.mycad -o tmp.stl` が成功し、facet 数が T06 と整合、ASCII STL として妥当 | `export.rs`（`CARGO_BIN_EXE_mycad`） |
features/12-cylinder/design-review-3.md:205:| T12 | 解像度外部化+クランプ | `tessellate_solid_with` で分割数が変わり三角形数が増減。`TessellationOptions::new(2,0)` が `angular=3/axial=1` にクランプされる | 2 解像度で差分 + クランプ検証 (R02) |
features/12-cylinder/design-review-3.md:206:| T13 | golden YAML ラウンドトリップ | `examples/cylinder.mycad` を `from_path`→`to_yaml`→`from_yaml`→`to_yaml` で equality。`create_cylinder` の serialize/schema 回帰を固定 | `assert_eq!` (R05) |
features/12-cylinder/design-review-3.md:207:| T14 | 退化入力エラー | `radius<=0` / `height<=0` の `create_cylinder` を `build_solid_from_features` に渡すと `KernelError::InvalidParameter` | `assert!(matches!(..))` (R02) |
features/12-cylinder/design-review-3.md:211:1. `cargo test --workspace` で T01–T12 green。
features/12-cylinder/design-review-3.md:290:crates/mycad-kernel/tests/cuboid_roundtrip.rs:1:use mycad_kernel::brep::topology::IdGenerator;
features/12-cylinder/design-review-3.md:291:crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
features/12-cylinder/design-review-3.md:292:crates/mycad-kernel/tests/cuboid_roundtrip.rs:3:use mycad_kernel::tessellation::tessellate_solid;
features/12-cylinder/design-review-3.md:293:crates/mycad-kernel/tests/cuboid_roundtrip.rs:8:    let mut id_gen = IdGenerator::new(0);
features/12-cylinder/design-review-3.md:294:crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/12-cylinder/design-review-3.md:295:crates/mycad-kernel/tests/cuboid_roundtrip.rs:17:    let mesh = tessellate_solid(&solid);
features/12-cylinder/design-review-3.md:296:crates/mycad-kernel/tests/cuboid_roundtrip.rs:30:        let mut id_gen = IdGenerator::new(42);
features/12-cylinder/design-review-3.md:297:crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/12-cylinder/design-review-3.md:298:crates/mycad-kernel/tests/cuboid_roundtrip.rs:32:        tessellate_solid(&solid)
features/12-cylinder/design-review-3.md:940:        // are shaded correctly (planar faces yield a constant normal).
features/12-cylinder/design-review-3.md:1011:    /// An infinite plane.
features/12-cylinder/design-review-3.md:1113:    /// surface is planar / has a constant normal.
features/12-cylinder/design-review-3.md:1159:    /// never have to special-case planar vs curved surfaces.
features/12-cylinder/design-review-3.md:1171:    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
features/12-cylinder/design-review-3.md:1179:    fn test_uv_of_roundtrip_plane() {
features/12-cylinder/design-review-3.md:1186:        assert_uv_roundtrip(&surface, 0.7, -1.3);
features/12-cylinder/design-review-3.md:1190:    fn test_uv_of_roundtrip_cylinder() {
features/12-cylinder/design-review-3.md:1196:        assert_uv_roundtrip(&surface, 0.9, 1.7);
features/12-cylinder/design-review-3.md:1200:    fn test_uv_of_roundtrip_sphere() {
features/12-cylinder/design-review-3.md:1205:        assert_uv_roundtrip(&surface, 1.1, 0.6);
features/12-cylinder/design-review-3.md:1209:    fn test_uv_of_roundtrip_cone() {
features/12-cylinder/design-review-3.md:1215:        assert_uv_roundtrip(&surface, 0.8, 2.0);
features/12-cylinder/design-review-3.md:1236:    fn test_plane_normal_constant_per_point() {
features/12-cylinder/design-review-3.md:1402:/bin/bash -lc "sed -n '1,260p' crates/mycad-build/tests/feature_dispatcher.rs && printf '\\n---\\n' && sed -n '1,260p' crates/mycad-cli/tests/export.rs && printf '\\n---\\n' && sed -n '1,220p' crates/mycad-kernel/tests/cuboid_roundtrip.rs" in /home/bacon/mycad
features/12-cylinder/design-review-3.md:1851:    fn test_roundtrip_yaml() {
features/12-cylinder/design-review-3.md:1912:    fn test_assembly_roundtrip() {
features/12-cylinder/design-review-3.md:2166:    section: "テスト計画 > T01"
features/12-cylinder/design-review-3.md:2167:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/12-cylinder/design-review-3.md:2196:    section: "テスト計画 > T01"
features/12-cylinder/design-review-3.md:2197:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/18-api-skeleton/design-review.md:45:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/design-review.md:146:| T01 | 正常系 | `GET /api/v0/mesh?file=examples/simple_box.mycad` を oneshot で叩く | 200、body を `TriangleMesh` に deser でき `positions`/`indices` が非空、`indices.len() % 3 == 0` |
features/18-api-skeleton/design-review.md:147:| T02 | 決定性 | 同一リクエストを 2 回実行 | 返る JSON バイト列が完全一致 |
features/18-api-skeleton/design-review.md:148:| T03 | 404 | 存在しない `file=examples/nope.mycad` | 404、body `{"error": ...}` |
features/18-api-skeleton/design-review.md:149:| T04 | 400 | 非 `.mycad` 拡張子 `file=Cargo.toml` | 400(InvalidExtension) |
features/18-api-skeleton/design-review.md:150:| T05 | 403 | `Host: evil.com` ヘッダ付与 | 403(Host 検証) |
features/18-api-skeleton/design-review.md:151:| T06 | 422 | 複数/未対応フィーチャ `file=examples/assembly.mycad` | 422 系エラー(build/tessellate 失敗を JSON で返す) |
features/18-api-skeleton/design-review.md:169:  2. このプラン内容を `features/18-api-skeleton/plan.md` に転記
features/18-api-skeleton/design-review.md:204:/home/bacon/mycad/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/design-review.md:276:crates/mycad-kernel/src/primitives/cylinder.rs:180:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review.md:681:    fn test_roundtrip_yaml() {
features/18-api-skeleton/design-review.md:742:    fn test_assembly_roundtrip() {
features/18-api-skeleton/design-review.md:908:/// Triangulate a face as a fan from its first boundary vertex (planar convex faces).
features/18-api-skeleton/design-review.md:1099:            Surface::Plane { .. } => "plane",
features/18-api-skeleton/design-review.md:1115:    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
features/18-api-skeleton/design-review.md:1134:    /// T07: Determinism — same input → same output.
features/18-api-skeleton/design-review.md:1151:    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
features/18-api-skeleton/design-review.md:1164:    /// T08 extended: exact positions and indices snapshot.
features/18-api-skeleton/design-review.md:1181:    /// T09: Outward normals — all facet normals point away from centroid.
features/18-api-skeleton/design-review.md:1228:    /// T12: Resolution externalization + clamping.
features/18-api-skeleton/design-review.md:1718:    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
features/18-api-skeleton/design-review.md:1746:    /// T02: Topology counts.
features/18-api-skeleton/design-review.md:1765:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review.md:1784:    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
features/18-api-skeleton/design-review.md:1839:    /// T05: Geometry — seam vertices and surface coefficients.
features/18-api-skeleton/design-review.md:1884:    /// T14: Degenerate inputs — radius<=0, height<=0, non-finite → InvalidParameter.
features/18-api-skeleton/design-review.md:2779:./features/18-api-skeleton/plan.md:20:| `crates/mycad-api/src/lib.rs` | 新規。Router 構築関数 `pub fn app() -> Router`、ハンドラ、`ApiError` |
features/18-api-skeleton/design-review.md:2780:./features/18-api-skeleton/plan.md:33:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
features/18-api-skeleton/design-review.md:2781:./features/18-api-skeleton/plan.md:35:enum ApiError { NotFound, BadExtension(String), Format(String), Build(String), Tessellate(String) }
features/18-api-skeleton/design-review.md:2782:./features/18-api-skeleton/plan.md:36:impl IntoResponse for ApiError { /* status + {"error": "..."} */ }
features/18-api-skeleton/design-review.md:2783:./features/18-api-skeleton/plan.md:44:- `tessellate_solid_with(&Solid, &TessellationOptions) -> Result<TriangleMesh, TessellationError>`(`crates/mycad-kernel/src/tessellation/mod.rs:90`)。v0 は `TessellationOptions::default()` 固定。
features/18-api-skeleton/design-review.md:2784:./features/18-api-skeleton/plan.md:45:- `TriangleMesh`(同 mod.rs:16)は `Serialize` 済み → `Json(mesh)` でそのまま返却。
features/18-api-skeleton/design-review.md:2785:./features/18-api-skeleton/plan.md:60:- `FormatError`/`KernelError`/`TessellationError` は `Serialize` 非対応 → `ApiError` enum に `.to_string()` で集約し、`IntoResponse` で status + `{"error": "..."}` を返す。
features/18-api-skeleton/design-review.md:2898:features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/design-review.md:2909:features/12-cylinder/design-review-2.md:1936:./features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/design-review.md:2919:features/12-cylinder/design-review-3.md:149:  2. `BoundaryFan`（平面） → 境界点列を **頂点0 から fan**（凸前提。cuboid 4 角=従来 2 三角形で不変、円盤キャップ=N角形 fan）。巻き方向は §1 で構築済み（底キャップ rev / 上キャップ fwd）なので fan の幾何法線が外向きになる。**スワップなし → cuboid のインデックス列もバイト不変**（T08 で固定）。
features/18-api-skeleton/design-review.md:2920:features/12-cylinder/design-review-3.md:180:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/design-review.md:2921:features/12-cylinder/design-review-3.md:186:- R02(high) **検証して反映**: cuboid.rs / stl.rs を読み確認 → 「実行時の向き整合スワップは cuboid 出力不変」という前回主張は **誤り**（現行 cuboid のインデックス巻きが右手系外向き規約と一致する保証はなく、スワップ導入で index 列が変わり T08 が回帰しうる）。→ **テッセレータからスワップを撤去**し、外向きは §1 の**構築時巻き方向**（底キャップ=e_bot rev / 上キャップ=e_top fwd / 側面グリッド=∂u×∂v 外向き）で保証。平面 fan 経路は現行コードとバイト同一 → cuboid 完全不変（T08 で固定）。
features/18-api-skeleton/design-review.md:3037:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/design-review.md:3071:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/design-review.md:3080:    section: "設計方針 > 1. トポロジー / テスト計画 > T04"
features/18-api-skeleton/design-review.md:3081:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/design-review.md:3091:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
features/18-api-skeleton/design-review.md:3101:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/design-review.md:3110:    section: "設計方針 > 1. トポロジー / テスト計画 > T04"
features/18-api-skeleton/design-review.md:3111:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/design-review.md:3121:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
features/18-api-skeleton/design-review.md:3127:/bin/bash -lc 'rg -n "''^issues:|''^verdict:|id: R" -n features/18-api-skeleton/design-review.md features/18-api-skeleton/plan.md' in /home/bacon/mycad
features/18-api-skeleton/design-review.md:3181:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/design-review.md:3350:    suggestion: "絶対パスのみ受け付けるか、`app(state)` に明示的な base directory を持たせて相対パス解決を固定し、その規則を T02 と E2E で検証すること。"
features/18-api-skeleton/design-review.md:3359:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/design-review.md:3360:    suggestion: "v0 を『root_component に単一 part feature を持つ文書のみ対応』と明記して assembly 非対応を別エラーに分けるか、Document/Component 階層を解釈する document-level build API を設計すること。T06 は専用 fixture に差し替えること。"
features/18-api-skeleton/design-review.md:3369:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/design-review.md:3379:    suggestion: "絶対パスのみ受け付けるか、`app(state)` に明示的な base directory を持たせて相対パス解決を固定し、その規則を T02 と E2E で検証すること。"
features/18-api-skeleton/design-review.md:3388:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/design-review.md:3389:    suggestion: "v0 を『root_component に単一 part feature を持つ文書のみ対応』と明記して assembly 非対応を別エラーに分けるか、Document/Component 階層を解釈する document-level build API を設計すること。T06 は専用 fixture に差し替えること。"
features/18-api-skeleton/design-review.md:3398:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/design-review-2.md:45:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/design-review-2.md:100:| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
features/18-api-skeleton/design-review-2.md:164:| T01 | 正常系(box) | `GET /api/v0/mesh?file=<abs>/examples/simple_box.mycad` を oneshot | 200、`TriangleMesh` deser 可、`positions`/`indices` 非空、`indices.len() % 3 == 0` |
features/18-api-skeleton/design-review-2.md:165:| T02 | 正常系(cylinder)(R05) | 同上 `examples/cylinder.mycad`(周期面経路) | 200、mesh 非空 |
features/18-api-skeleton/design-review-2.md:166:| T03 | 決定性 | T01/T02 を 2 回実行 | 返る JSON バイト列が完全一致 |
features/18-api-skeleton/design-review-2.md:167:| T04 | 404 | 存在しない `file` | 404、body `{"error": ...}` |
features/18-api-skeleton/design-review-2.md:168:| T05 | 400 | 非 `.mycad` 拡張子(`Cargo.toml`) | 400(InvalidExtension) |
features/18-api-skeleton/design-review-2.md:169:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/design-review-2.md:170:| T07 | 422 未対応 feature(R03) | fixture `create_sphere` 単一 feature | 422(UnsupportedFeature) |
features/18-api-skeleton/design-review-2.md:171:| T08 | 422 退化寸法(R02) | fixture `create_box` width=0(または負値) | 422(InvalidParameter) |
features/18-api-skeleton/design-review-2.md:172:| T09 | 422 assembly 非対応(R03) | fixture: root features 空 + children あり | 422(EmptyFeatureList、クラッシュしない) |
features/18-api-skeleton/design-review-2.md:175:> **R05 反論**: Euler-Poincaré 検証は kernel(`cylinder.rs:180-195`)で、golden YAML ラウンドトリップは format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保済み。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。API 層は HTTP 契約(status・JSON 形・決定性・cylinder 経路疎通)に集中する。
features/18-api-skeleton/design-review-2.md:189:- **STEP 3(Codex 設計レビュー)はプランモードを抜けずに実行する**(Bash dispatch は plan mode でも通る。deny は `crates/**` の Edit/Write のみ)。本改訂後に再レビュー(round 2/3)し、Critical/High 解消後に STEP 4 `ExitPlanMode` で最終承認。
features/18-api-skeleton/design-review-2.md:283:./features/18-api-skeleton/plan.md
features/18-api-skeleton/design-review-2.md:324:./crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/design-review-2.md:476:/bin/bash -lc 'rg -n "run_export|make_cuboid|tessellate_solid_with|test_assembly_roundtrip|workspace.dependencies|thiserror|TriangleMesh|build_solid_from_features|Document::from_path|InvalidExtension|make_cylinder|Euler|HalfEdge|Shell|Loop|IdGenerator" -S .' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:546:./features/12-cylinder/design-review.md:176:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/18-api-skeleton/design-review-2.md:547:./features/12-cylinder/design-review.md:178:| T03 | Euler | V−E+F == 2 | `assert_eq!(2-3+3, 2)` 相当の検証 |
features/18-api-skeleton/design-review-2.md:548:./features/12-cylinder/design-review.md:183:| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index | 既存テスト維持 |
features/18-api-skeleton/design-review-2.md:549:./features/12-cylinder/design-review.md:185:| T10 | ディスパッチ統合 | `examples/cylinder.mycad` を `build_solid_from_features` でビルドし T02 と同じ counts | `feature_dispatcher.rs` |
features/18-api-skeleton/design-review-2.md:550:./features/12-cylinder/design-review.md:187:| T12 | 解像度外部化 | `--segments` / `tessellate_solid_with` で分割数が変わり三角形数が増減 | 2 解像度で差分確認 |
features/18-api-skeleton/design-review-2.md:597:./features/12-cylinder/design-review.md:463:features/12-cylinder/design-review.md:176:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/18-api-skeleton/design-review-2.md:598:./features/12-cylinder/design-review.md:465:features/12-cylinder/design-review.md:183:| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index | 既存テスト維持 |
features/18-api-skeleton/design-review-2.md:599:./features/12-cylinder/design-review.md:466:features/12-cylinder/design-review.md:187:| T12 | 解像度外部化 | `--segments` / `tessellate_solid_with` で分割数が変わり三角形数が増減 | 2 解像度で差分確認 |
features/18-api-skeleton/design-review-2.md:674:./features/12-cylinder/design-review.md:580:crates/mycad-kernel/tests/cuboid_roundtrip.rs:1:use mycad_kernel::brep::topology::IdGenerator;
features/18-api-skeleton/design-review-2.md:675:./features/12-cylinder/design-review.md:581:crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:676:./features/12-cylinder/design-review.md:583:crates/mycad-kernel/tests/cuboid_roundtrip.rs:8:    let mut id_gen = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:677:./features/12-cylinder/design-review.md:584:crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:678:./features/12-cylinder/design-review.md:586:crates/mycad-kernel/tests/cuboid_roundtrip.rs:30:        let mut id_gen = IdGenerator::new(42);
features/18-api-skeleton/design-review-2.md:679:./features/12-cylinder/design-review.md:587:crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:756:./features/12-cylinder/design-review.md:2230:    fn test_assembly_roundtrip() {
features/18-api-skeleton/design-review-2.md:777:./features/12-cylinder/design-review.md:2520:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/design-review-2.md:780:./features/12-cylinder/design-review.md:2530:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/design-review-2.md:783:./features/12-cylinder/design-review.md:2550:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/design-review-2.md:786:./features/12-cylinder/design-review.md:2560:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/design-review-2.md:818:./features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/design-review-2.md:819:./features/12-cylinder/design-review-2.md:187:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/18-api-skeleton/design-review-2.md:820:./features/12-cylinder/design-review-2.md:189:| T03 | Euler | V−E+F == 2 | `assert_eq!(2-3+3, 2)` 相当の検証 |
features/18-api-skeleton/design-review-2.md:821:./features/12-cylinder/design-review-2.md:194:| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index。cuboid は逆向き Line HE を含むため R01 の向き対応も同時に担保 | 既存テスト維持 + positions が従来値と一致 |
features/18-api-skeleton/design-review-2.md:822:./features/12-cylinder/design-review-2.md:196:| T10 | ディスパッチ統合 | `examples/cylinder.mycad` を `build_solid_from_features` でビルドし T02 と同じ counts | `feature_dispatcher.rs` |
features/18-api-skeleton/design-review-2.md:823:./features/12-cylinder/design-review-2.md:198:| T12 | 解像度外部化+クランプ | `tessellate_solid_with` で分割数が変わり三角形数が増減。`TessellationOptions::new(2,0)` が `angular=3/axial=1` にクランプされる | 2 解像度で差分 + クランプ検証 (R02) |
features/18-api-skeleton/design-review-2.md:824:./features/12-cylinder/design-review-2.md:200:| T14 | 退化入力エラー | `radius<=0` / `height<=0` の `create_cylinder` を `build_solid_from_features` に渡すと `KernelError::InvalidParameter` | `assert!(matches!(..))` (R02) |
features/18-api-skeleton/design-review-2.md:912:./features/12-cylinder/design-review-2.md:1732:    fn test_assembly_roundtrip() {
features/18-api-skeleton/design-review-2.md:919:./features/12-cylinder/design-review-2.md:1789:/bin/bash -lc 'rg -n "golden|roundtrip|tessellate_solid_with|TessellationOptions|export" crates/mycad-cli tests crates/mycad-kernel crates/mycad-build' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:941:./features/12-cylinder/design-review-2.md:1936:./features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/design-review-2.md:974:./features/12-cylinder/design-review-2.md:2312:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/18-api-skeleton/design-review-2.md:978:./features/12-cylinder/design-review-2.md:2337:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/18-api-skeleton/design-review-2.md:986:./crates/mycad-format/src/document.rs:116:    fn test_assembly_roundtrip() {
features/18-api-skeleton/design-review-2.md:1025:./features/12-cylinder/final-review-2.md:948:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review-2.md:1098:./features/12-cylinder/design-review-3.md:180:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/design-review-2.md:1101:./features/12-cylinder/design-review-3.md:188:- T03 を「実カウントから Euler を計算」へ修正（定数ハードコード撤廃）。
features/18-api-skeleton/design-review-2.md:1102:./features/12-cylinder/design-review-3.md:194:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/18-api-skeleton/design-review-2.md:1103:./features/12-cylinder/design-review-3.md:196:| T03 | Euler | 実 solid の `vertices.len() − edges.len() + faces.len() == 2 * (shells.len() − genus)`（genus=0）を**実カウントから**計算して検証（定数ハードコードでなく構造体から） | `assert_eq!` |
features/18-api-skeleton/design-review-2.md:1104:./features/12-cylinder/design-review-3.md:201:| T08 | 回帰(平面バイト不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index。**positions も indices も従来値と完全一致**（スワップ撤去によりバイト不変、R02）。cuboid は逆向き Line HE を含むため R01 の向き対応も同時に担保 | 既存テスト維持 + positions/indices 一致 |
features/18-api-skeleton/design-review-2.md:1105:./features/12-cylinder/design-review-3.md:203:| T10 | ディスパッチ統合 | `examples/cylinder.mycad` を `build_solid_from_features` でビルドし T02 と同じ counts | `feature_dispatcher.rs` |
features/18-api-skeleton/design-review-2.md:1106:./features/12-cylinder/design-review-3.md:205:| T12 | 解像度外部化+クランプ | `tessellate_solid_with` で分割数が変わり三角形数が増減。`TessellationOptions::new(2,0)` が `angular=3/axial=1` にクランプされる | 2 解像度で差分 + クランプ検証 (R02) |
features/18-api-skeleton/design-review-2.md:1107:./features/12-cylinder/design-review-3.md:207:| T14 | 退化入力エラー | `radius<=0` / `height<=0` の `create_cylinder` を `build_solid_from_features` に渡すと `KernelError::InvalidParameter` | `assert!(matches!(..))` (R02) |
features/18-api-skeleton/design-review-2.md:1150:./features/12-cylinder/design-review-3.md:290:crates/mycad-kernel/tests/cuboid_roundtrip.rs:1:use mycad_kernel::brep::topology::IdGenerator;
features/18-api-skeleton/design-review-2.md:1151:./features/12-cylinder/design-review-3.md:291:crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1152:./features/12-cylinder/design-review-3.md:293:crates/mycad-kernel/tests/cuboid_roundtrip.rs:8:    let mut id_gen = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:1153:./features/12-cylinder/design-review-3.md:294:crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1154:./features/12-cylinder/design-review-3.md:296:crates/mycad-kernel/tests/cuboid_roundtrip.rs:30:        let mut id_gen = IdGenerator::new(42);
features/18-api-skeleton/design-review-2.md:1155:./features/12-cylinder/design-review-3.md:297:crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1255:./features/12-cylinder/design-review-3.md:1912:    fn test_assembly_roundtrip() {
features/18-api-skeleton/design-review-2.md:1316:./features/12-cylinder/final-review.md:742:+    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review-2.md:1414:./features/12-cylinder/final-review.md:2050:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review-2.md:1684:./features/18-api-skeleton/design-review.md:146:| T01 | 正常系 | `GET /api/v0/mesh?file=examples/simple_box.mycad` を oneshot で叩く | 200、body を `TriangleMesh` に deser でき `positions`/`indices` が非空、`indices.len() % 3 == 0` |
features/18-api-skeleton/design-review-2.md:1685:./features/18-api-skeleton/design-review.md:149:| T04 | 400 | 非 `.mycad` 拡張子 `file=Cargo.toml` | 400(InvalidExtension) |
features/18-api-skeleton/design-review-2.md:1718:./features/18-api-skeleton/design-review.md:276:crates/mycad-kernel/src/primitives/cylinder.rs:180:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review-2.md:1755:./features/18-api-skeleton/design-review.md:742:    fn test_assembly_roundtrip() {
features/18-api-skeleton/design-review-2.md:1816:./features/18-api-skeleton/design-review.md:1765:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review-2.md:1940:./features/18-api-skeleton/design-review.md:2780:./features/18-api-skeleton/plan.md:33:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
features/18-api-skeleton/design-review-2.md:1941:./features/18-api-skeleton/design-review.md:2783:./features/18-api-skeleton/plan.md:44:- `tessellate_solid_with(&Solid, &TessellationOptions) -> Result<TriangleMesh, TessellationError>`(`crates/mycad-kernel/src/tessellation/mod.rs:90`)。v0 は `TessellationOptions::default()` 固定。
features/18-api-skeleton/design-review-2.md:1942:./features/18-api-skeleton/design-review.md:2784:./features/18-api-skeleton/plan.md:45:- `TriangleMesh`(同 mod.rs:16)は `Serialize` 済み → `Json(mesh)` でそのまま返却。
features/18-api-skeleton/design-review-2.md:1969:./features/18-api-skeleton/design-review.md:2898:features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/design-review-2.md:1970:./features/18-api-skeleton/design-review.md:2909:features/12-cylinder/design-review-2.md:1936:./features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/design-review-2.md:1973:./features/18-api-skeleton/design-review.md:2920:features/12-cylinder/design-review-3.md:180:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/design-review-2.md:1987:./features/18-api-skeleton/design-review.md:3071:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/design-review-2.md:1990:./features/18-api-skeleton/design-review.md:3081:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/design-review-2.md:1993:./features/18-api-skeleton/design-review.md:3101:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/design-review-2.md:1996:./features/18-api-skeleton/design-review.md:3111:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/design-review-2.md:2025:./features/18-api-skeleton/design-review.md:3359:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/design-review-2.md:2026:./features/18-api-skeleton/design-review.md:3369:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/design-review-2.md:2028:./features/18-api-skeleton/design-review.md:3388:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/design-review-2.md:2029:./features/18-api-skeleton/design-review.md:3398:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/design-review-2.md:2050:./features/18-api-skeleton/design-review-2.md:100:| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
features/18-api-skeleton/design-review-2.md:2061:./features/18-api-skeleton/design-review-2.md:164:| T01 | 正常系(box) | `GET /api/v0/mesh?file=<abs>/examples/simple_box.mycad` を oneshot | 200、`TriangleMesh` deser 可、`positions`/`indices` 非空、`indices.len() % 3 == 0` |
features/18-api-skeleton/design-review-2.md:2062:./features/18-api-skeleton/design-review-2.md:168:| T05 | 400 | 非 `.mycad` 拡張子(`Cargo.toml`) | 400(InvalidExtension) |
features/18-api-skeleton/design-review-2.md:2064:./features/18-api-skeleton/design-review-2.md:175:> **R05 反論**: Euler-Poincaré 検証は kernel(`cylinder.rs:180-195`)で、golden YAML ラウンドトリップは format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保済み。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。API 層は HTTP 契約(status・JSON 形・決定性・cylinder 経路疎通)に集中する。
features/18-api-skeleton/design-review-2.md:2067:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:1:use mycad_kernel::brep::topology::IdGenerator;
features/18-api-skeleton/design-review-2.md:2068:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:2069:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:8:    let mut id_gen = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:2070:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:2071:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:30:        let mut id_gen = IdGenerator::new(42);
features/18-api-skeleton/design-review-2.md:2072:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:2073:./features/18-api-skeleton/plan.md:5:Phase 2「Web ベースビューア基盤」の土台として、CLI / GUI / AI 共通の操作面 v0 を Rust HTTP サーバとして立てる。`.mycad` を読み込み、テッセレーションした `TriangleMesh` を JSON で返す最小 API を `crates/mycad-api/` に新規実装する。後続 Issue #20(Three.js フロント)・#21(`mycad view`)がこの API を消費する前提。
features/18-api-skeleton/design-review-2.md:2074:./features/18-api-skeleton/plan.md:7:既存 CLI の `run_export`(`crates/mycad-cli/src/main.rs:47-67`)が同じ Document→Solid→Mesh フローを既に持つため、STL 化を除いて再利用する。
features/18-api-skeleton/design-review-2.md:2075:./features/18-api-skeleton/plan.md:13:- 新規クレート `crates/mycad-api/`(lib + bin 構成)+ `make_cuboid` の入力検証(kernel)
features/18-api-skeleton/design-review-2.md:2076:./features/18-api-skeleton/plan.md:24:| `crates/mycad-kernel/src/primitives/cuboid.rs` | **(R02)** `make_cuboid` を `-> Result<Solid, KernelError>` 化。`dx/dy/dz` が非有限 or `<= 0` なら `InvalidParameter { kind }`(`make_cylinder` cylinder.rs:16-22 と対称)。インライン test の呼び出しを `.unwrap()` 化 |
features/18-api-skeleton/design-review-2.md:2077:./features/18-api-skeleton/plan.md:25:| `crates/mycad-build/src/lib.rs` | CreateBox 分岐を `Ok(make_cuboid(...))` → `make_cuboid(...)`(`?` 伝播)に変更 |
features/18-api-skeleton/design-review-2.md:2078:./features/18-api-skeleton/plan.md:26:| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
features/18-api-skeleton/design-review-2.md:2079:./features/18-api-skeleton/plan.md:27:| `Cargo.toml`(ルート) | members に `"crates/mycad-api"` 追加。`[workspace.dependencies]` に `axum`/`tokio`/`serde_json`/`tower`(dev) 追加 |
features/18-api-skeleton/design-review-2.md:2080:./features/18-api-skeleton/plan.md:46:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
features/18-api-skeleton/design-review-2.md:2081:./features/18-api-skeleton/plan.md:56:- `Document::from_path`(`crates/mycad-format/src/document.rs:36`)。`.mycad` 拡張子を強制(InvalidExtension)。
features/18-api-skeleton/design-review-2.md:2082:./features/18-api-skeleton/plan.md:57:- `build_solid_from_features`(`crates/mycad-build/src/lib.rs:6`)。単一フィーチャ・Box/Cylinder のみ。
features/18-api-skeleton/design-review-2.md:2083:./features/18-api-skeleton/plan.md:58:- `tessellate_solid_with`(`crates/mycad-kernel/src/tessellation/mod.rs:90`)。v0 は `TessellationOptions::default()` 固定。
features/18-api-skeleton/design-review-2.md:2084:./features/18-api-skeleton/plan.md:59:- `TriangleMesh`(mod.rs:16, Serialize 済み)→ `Json(mesh)`。
features/18-api-skeleton/design-review-2.md:2085:./features/18-api-skeleton/plan.md:60:- ハンドラ本体は CLI `run_export`(main.rs:47-67)から STL 化を除き末尾を `Ok(Json(mesh))` に置換。
features/18-api-skeleton/design-review-2.md:2086:./features/18-api-skeleton/plan.md:73:2. **`.mycad` 拡張子限定**(`Document::from_path` が既に強制)→ 機密ファイル読取を遮断。
features/18-api-skeleton/design-review-2.md:2087:./features/18-api-skeleton/plan.md:83:- `[workspace.dependencies]` に新規登録: `axum = "0.8"`、`tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }`、`serde_json = "1"`、`tower = "0.5"`(dev、`oneshot`)。各クレートは `{ workspace = true }`。
features/18-api-skeleton/design-review-2.md:2088:./features/18-api-skeleton/plan.md:84:- derive 規約・`thiserror`・`version.workspace = true` を踏襲。clippy `-D warnings` 通過必須。
features/18-api-skeleton/design-review-2.md:2089:./features/18-api-skeleton/plan.md:90:| T01 | 正常系(box) | `GET /api/v0/mesh?file=<abs>/examples/simple_box.mycad` を oneshot | 200、`TriangleMesh` deser 可、`positions`/`indices` 非空、`indices.len() % 3 == 0` |
features/18-api-skeleton/design-review-2.md:2090:./features/18-api-skeleton/plan.md:94:| T05 | 400 | 非 `.mycad` 拡張子(`Cargo.toml`) | 400(InvalidExtension) |
features/18-api-skeleton/design-review-2.md:2091:./features/18-api-skeleton/plan.md:99:| K01 | kernel 単体(R02) | `make_cuboid(0.0, 1.0, 1.0, ..)` 等 | `Err(InvalidParameter { kind: "..." })` |
features/18-api-skeleton/design-review-2.md:2092:./features/18-api-skeleton/plan.md:101:> **R05 反論**: Euler-Poincaré 検証は kernel(`cylinder.rs:180-195`)で、golden YAML ラウンドトリップは format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保済み。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。API 層は HTTP 契約(status・JSON 形・決定性・cylinder 経路疎通)に集中する。
features/18-api-skeleton/design-review-2.md:2093:./features/18-api-skeleton/plan.md:108:curl -s 'http://127.0.0.1:3000/api/v0/mesh?file=examples/simple_box.mycad' | head -c 300   # TriangleMesh JSON
features/18-api-skeleton/design-review-2.md:2137:./crates/mycad-kernel/src/primitives/cylinder.rs:180:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review-2.md:2346:    fn test_roundtrip_yaml() {
features/18-api-skeleton/design-review-2.md:2407:    fn test_assembly_roundtrip() {
features/18-api-skeleton/design-review-2.md:2834:    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
features/18-api-skeleton/design-review-2.md:2862:    /// T02: Topology counts.
features/18-api-skeleton/design-review-2.md:2881:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review-2.md:2900:    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
features/18-api-skeleton/design-review-2.md:2955:    /// T05: Geometry — seam vertices and surface coefficients.
features/18-api-skeleton/design-review-2.md:3456:/// Triangulate a face as a fan from its first boundary vertex (planar convex faces).
features/18-api-skeleton/design-review-2.md:3802:crates/mycad-kernel/src/primitives/cylinder.rs:299:    /// T14: Degenerate inputs — radius<=0, height<=0, non-finite → InvalidParameter.
features/18-api-skeleton/design-review-2.md:3897:            Surface::Plane { .. } => "plane",
features/18-api-skeleton/design-review-2.md:3913:    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
features/18-api-skeleton/design-review-2.md:3932:    /// T07: Determinism — same input → same output.
features/18-api-skeleton/design-review-2.md:3949:    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
features/18-api-skeleton/design-review-2.md:3962:    /// T08 extended: exact positions and indices snapshot.
features/18-api-skeleton/design-review-2.md:3979:    /// T09: Outward normals — all facet normals point away from centroid.
features/18-api-skeleton/design-review-2.md:4026:    /// T12: Resolution externalization + clamping.
features/18-api-skeleton/design-review-2.md:4515:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/design-review-2.md:4570:| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
features/18-api-skeleton/design-review-2.md:4720:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` の再利用方針と T09 の `EmptyFeatureList` 期待値では、`children` / `ref` を持つ assembly/reference 文書と、本当に feature が空の不正 part 文書を区別できない。これでは文書自身が要求している『assembly は明確なエラー + 422』を満たせない。"
features/18-api-skeleton/design-review-2.md:4730:    finding: "golden YAML ラウンドトリップが既存 `examples/*` だけに依存しており、新規 `crates/mycad-api/tests/fixtures/*.mycad` の unsupported/退化 fixture については format crate 側の roundtrip 保証が計画されていない。API が依存する入力形状の YAML 安定性が未検証のまま残る。"
features/18-api-skeleton/design-review-2.md:4731:    suggestion: "`mycad-format` 側に新規 fixture を読み込む roundtrip/golden テストを追加するか、API fixture を共有して `to_yaml`/`from_yaml` の安定性を format crate の所有テストで担保すること。"
features/18-api-skeleton/design-review-2.md:4745:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` の再利用方針と T09 の `EmptyFeatureList` 期待値では、`children` / `ref` を持つ assembly/reference 文書と、本当に feature が空の不正 part 文書を区別できない。これでは文書自身が要求している『assembly は明確なエラー + 422』を満たせない。"
features/18-api-skeleton/design-review-2.md:4755:    finding: "golden YAML ラウンドトリップが既存 `examples/*` だけに依存しており、新規 `crates/mycad-api/tests/fixtures/*.mycad` の unsupported/退化 fixture については format crate 側の roundtrip 保証が計画されていない。API が依存する入力形状の YAML 安定性が未検証のまま残る。"
features/18-api-skeleton/design-review-2.md:4756:    suggestion: "`mycad-format` 側に新規 fixture を読み込む roundtrip/golden テストを追加するか、API fixture を共有して `to_yaml`/`from_yaml` の安定性を format crate の所有テストで担保すること。"

2026-05-25T06:37:27.352616Z ERROR codex_core::tools::router: error=exec_command failed for `/bin/bash -lc 'RUSTC_WRAPPER= CARGO_TARGET_DIR=/tmp/mycad-target cargo test -p mycad-kernel --no-run'`: SandboxDenied { message: "error: Read-only file system (os error 30) at path \"/tmp/mycad-targetWKXIcx\"\n", output: ExecToolCallOutput { exit_code: 101, stdout: StreamOutput { text: "", truncated_after_lines: None }, stderr: StreamOutput { text: "error: Read-only file system (os error 30) at path \"/tmp/mycad-targetWKXIcx\"\n", truncated_after_lines: None }, aggregated_output: StreamOutput { text: "error: Read-only file system (os error 30) at path \"/tmp/mycad-targetWKXIcx\"\n", truncated_after_lines: None }, duration: 0ns, timed_out: false } }
2026-05-25T06:37:27.390183Z ERROR codex_core::tools::router: error=exec_command failed for `/bin/bash -lc 'RUSTC_WRAPPER= CARGO_TARGET_DIR=/tmp/mycad-target cargo test -p mycad-api --no-run'`: SandboxDenied { message: "error: Read-only file system (os error 30) at path \"/tmp/mycad-targetWmMi9p\"\n", output: ExecToolCallOutput { exit_code: 101, stdout: StreamOutput { text: "", truncated_after_lines: None }, stderr: StreamOutput { text: "error: Read-only file system (os error 30) at path \"/tmp/mycad-targetWmMi9p\"\n", truncated_after_lines: None }, aggregated_output: StreamOutput { text: "error: Read-only file system (os error 30) at path \"/tmp/mycad-targetWmMi9p\"\n", truncated_after_lines: None }, duration: 0ns, timed_out: false } }
 exited 101 in 268ms:
error: Read-only file system (os error 30) at path "/tmp/mycad-targetWKXIcx"

 exited 101 in 299ms:
error: Read-only file system (os error 30) at path "/tmp/mycad-targetWmMi9p"

exec
/bin/bash -lc "sed -n '1,220p' features/18-api-skeleton/plan.md" in /home/bacon/mycad
 succeeded in 0ms:
# Issue #18: API 層スケルトン (mycad-api / axum)

## Context

Phase 2「Web ベースビューア基盤」の土台として、CLI / GUI / AI 共通の操作面 v0 を Rust HTTP サーバとして立てる。`.mycad` を読み込み、テッセレーションした `TriangleMesh` を JSON で返す最小 API を `crates/mycad-api/` に新規実装する。後続 Issue #20(Three.js フロント)・#21(`mycad view`)がこの API を消費する前提。

既存 CLI の `run_export`(`crates/mycad-cli/src/main.rs:47-67`)が同じ Document→Solid→Mesh フローを既に持つため、STL 化を除いて再利用する。

> Codex 設計レビュー round 1-3 反映済み(3回上限)。round3 残存指摘は全て受諾し最終修正済み: R01=絶対パスのみ受理(決定的)、R02=v0 専用テッセレーション定数で Default 依存排除、R03=InvalidReference→422、R04=T09/T10 で assembly と空 part を分離、R05=K01 に INFINITY 分岐追加。round3 の verdict は fail のままだが指摘内容は本改訂で解消済み(ユーザー判断で再レビューせず確定)。

## 実装対象

- 新規クレート `crates/mycad-api/`(lib + bin 構成)+ `make_cuboid` の入力検証(kernel)
- Issue: #18 / Milestone: Phase 2

### v0 スコープ明確化(R03)
- 対応するのは **`root_component` に単一 part feature を持つ文書のみ**(Box / Cylinder)。
- assembly(`children`/`reference` を持ち root features が空)や複数フィーチャは v0 非対応。明確なエラー + 422 系で返す(クラッシュさせない)。

### 影響ファイル

| ファイル | 変更 |
|----------|------|
| `crates/mycad-kernel/src/primitives/cuboid.rs` | `make_cuboid` を `-> Result<Solid, KernelError>` 化。`dx/dy/dz` が非有限 or `<= 0.0` なら `InvalidParameter { kind }`。**(R03)** 閾値は `make_cylinder`(cylinder.rs:16-22)と同じ exact `<= 0.0` で対称化。near-zero 正値の公差判定は導入しない(下記 R03 注記)。インライン test を `.unwrap()` 化 |
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

// error.rs — (R01) status は型から確定。文字列解析しない
enum ApiError {
    BadRequest(String),     // 400
    NotFound(String),       // 404
    Unprocessable(String),  // 422
    Internal(String),       // 500
}
impl From<FormatError>      for ApiError { /* variant 別に 400/404/500 */ }
impl From<KernelError>      for ApiError { /* 全 variant → 422 */ }
impl From<TessellationError> for ApiError { /* 全 variant → 422 */ }
impl IntoResponse for ApiError { /* status は variant から、body = {"error": msg} */ }
```

## 設計方針

### 再利用(新規ロジックを書かない)
- `Document::from_path`(`crates/mycad-format/src/document.rs:36`)。`.mycad` 拡張子を強制(InvalidExtension)。
- `build_solid_from_features`(`crates/mycad-build/src/lib.rs:6`)。単一フィーチャ・Box/Cylinder のみ。
- `tessellate_solid_with`(`crates/mycad-kernel/src/tessellation/mod.rs:90`)。**(R02)** API v0 専用の固定定数 `const V0_TESSELLATION: TessellationOptions = TessellationOptions { angular_segments: 32, axial_segments: 1 }` を api crate に定義して渡す。kernel の `Default` には依存しない(Default 変更でメッシュ仕様が静かにドリフトするのを防ぐ)。
- `TriangleMesh`(mod.rs:16, Serialize 済み)→ `Json(mesh)`。
- ハンドラ本体は CLI `run_export`(main.rs:47-67)から STL 化を除き末尾を `Ok(Json(mesh))` に置換。

### パス解決の契約(R01: 絶対パスのみ — 決定的)
- `file` クエリは **絶対パスのみ受理**する。相対パスは `400 BadRequest`(理由: CWD 依存だと同一 HTTP 入力でもサーバ起動 CWD で結果が変わり「同一入力→同一出力」に反する)。
- これにより「任意の位置の `.mycad` を読める」要望は満たしつつ、CWD 非依存で決定的になる。
- テストは `env!("CARGO_MANIFEST_DIR")` 起点で絶対パスを組み立てる(`crates/mycad-api` から `../../examples/...` を `canonicalize`)。
- 相対パス拒否の専用テスト(T11)を追加。

### エンドポイント / バージョニング
- `GET /api/v0/mesh?file=<path>`。`Router::nest("/api/v0", ...)`。クエリは `file` のみ(v0 スコープ)。解像度は default 固定。

### パス安全性(多層防御 — 任意位置の .mycad 読取とセキュリティを両立)
1. **CWD ジェイルなし** → 任意パスの `.mycad` を読める(要望)。
2. **`.mycad` 拡張子限定**(`Document::from_path` が既に強制)→ 機密ファイル読取を遮断。
3. **`canonicalize` + 実ファイル存在確認** → 無ければ 404、ディレクトリ等は 400。
4. **`127.0.0.1` のみバインド** → ネットワークから到達不可。
5. **Host ヘッダ検証ミドルウェア** → loopback 以外は 403(DNS リバインディング対策)。

### エラーハンドリング(R01: 型から status を確定。文字列解析禁止)
- `ApiError` は 4 variant(`BadRequest`/`NotFound`/`Unprocessable`/`Internal`)。各下層エラーから `From` で変換し、`IntoResponse` は **variant から status を決め**、body は `{"error": msg}` に整形するだけ。
- `From<FormatError>`: `InvalidExtension`/`Yaml` → 400(`BadRequest`)、`Io(ErrorKind::NotFound)` → 404、`InvalidReference` → 422(`Unprocessable`、ユーザー入力起因の文書不正)(R03)、その他 `Io` → 500(`Internal`)。
- `From<KernelError>`: `InvalidParameter`(退化寸法)・`UnsupportedFeature`・`MultipleFeatures`・`EmptyFeatureList` → 422(`Unprocessable`)。
- `From<TessellationError>`: `UnsupportedSurface`・`TrimmedFaceUnsupported`・`NonManifoldLoop` → 422。
- handler が assembly/reference 文書を検出した場合(下記 R02)も 422(`Unprocessable`)。

### v0 文書判別(R02: handler で build 前に検査)
- `build_solid_from_features` を呼ぶ前に `root_component` を検査する:
  - `reference.is_some()` または `!children.is_empty()` → assembly/reference 文書 → `ApiError::Unprocessable("assembly/reference は v0 非対応")`。
  - `features.is_empty()`(かつ assembly でない)→ 「part なのに feature が空」として 422。
- これにより `EmptyFeatureList` の意味を「不正な空 part」に限定し、assembly を別メッセージで区別する。

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
| T09 | 422 assembly 非対応(R04) | fixture: root features 空 + children あり | 422、body に assembly/reference 専用メッセージ(空 part とは別経路) |
| T10 | 422 空 part(R04) | fixture: 単一 part だが features が空(children/ref なし) | 422(EmptyFeatureList、assembly とは別メッセージ) |
| T11 | 400 相対パス(R01) | `file=examples/simple_box.mycad`(相対) | 400(相対パス拒否) |
| K01 | kernel 単体 | `make_cuboid` の `0.0` / 負値 / `NaN` / `INFINITY` / `-INFINITY`(dx/dy/dz 各々)(R05) | 全分岐で `Err(InvalidParameter { kind: "..." })` |
| K02 | kernel 単体(R03 境界可視化) | `make_cuboid(1e-9, 1.0, 1.0, ..)`(near-zero 正値) | 現状は `Ok`(exact `<= 0.0` 方針)であることを assert し境界を明文化 |

> **R05 反論**(round 1): Euler-Poincaré は kernel(`cylinder.rs:180-195`)、golden YAML roundtrip は format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。
>
> **R03 判断**(round 2): 退化寸法の閾値は `make_cylinder` と同じ exact `<= 0.0` で対称化する。**グローバル公差(near-zero の扱い)は ROADMAP の Issue #17「単位系・グローバル公差の早期固定(ADR-004 範囲)」が所有する未決事項**であり、#18 で独自 EPS を導入すると #17 の設計判断を先取りしてしまう。よって #18 は exact-zero 方針に留め、near-zero 正値が現状 `Ok` となる境界を K02 で明文化する(将来 #17 で公差確定後に統一)。
>
> **R04 反論**(round 2): 新規 `crates/mycad-api/tests/fixtures/*.mycad` は T07-T09 で API 経由で必ずパースされる(パースできなければ期待する 422 が出ず test fail)ため、入力形状の妥当性は API テストで担保済み。`Document` の YAML roundtrip 安定性は `mycad-format` の既存 roundtrip テストが所有。クロスクレートの roundtrip 追加は「テストは所有クレートに置く」規約に反するため行わない。fixture は最小に保つ。

## 検証(end-to-end)

```bash
cargo xtask ci                      # fmt → clippy(-D warnings) → test --workspace → build
cargo run -p mycad-api --bin mycad-api &   # 127.0.0.1:3000
ROOT=$(pwd)   # 絶対パスのみ受理(R01)
curl -s "http://127.0.0.1:3000/api/v0/mesh?file=$ROOT/examples/simple_box.mycad" | head -c 300   # TriangleMesh JSON
curl -s "http://127.0.0.1:3000/api/v0/mesh?file=$ROOT/examples/cylinder.mycad"    | head -c 300   # 同上(曲面)
curl -s -o /dev/null -w '%{http_code}\n' "http://127.0.0.1:3000/api/v0/mesh?file=$ROOT/examples/nope.mycad"  # 404
curl -s -o /dev/null -w '%{http_code}\n' 'http://127.0.0.1:3000/api/v0/mesh?file=examples/simple_box.mycad'  # 400(相対)
```

## 3ai フロー上の注記

- **STEP 3(Codex 設計レビュー)はプランモードを抜けずに実行する**(Bash dispatch は plan mode でも通る。deny は `crates/**` の Edit/Write のみ)。本改訂後に再レビュー(round 2/3)し、Critical/High 解消後に STEP 4 `ExitPlanMode` で最終承認。
- 承認後: STEP 5(ブランチ `cad/18-api-skeleton`)→ STEP 6(GLM 実装、`crates/**` は Claude 編集禁止)→ STEP 7(Codex 最終レビュー)→ STEP 8(squash マージ, `Closes #18`)。

exec
/bin/bash -lc "sed -n '1,260p' features/18-api-skeleton/final-review.md" in /home/bacon/mycad
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
session id: 019e5dd9-4dfa-7613-afa0-2129328d30f0
--------
user
# Codex 最終コードレビュアー（MyCad CAD カーネル専用）

あなたは Rust 製 B-rep CAD カーネル「MyCad」の実装差分をレビューする専門家です。
`git diff` または `codex review` が提供するコード差分を読み、以下の観点で問題を指摘してください。

## レビュー観点

### 1. 決定性（最重要）
- `IdGenerator` 以外の ID 生成（`Uuid::new_v4()`、`rand`、timestamp 等）が混入していないか
- HashMap/HashSet のイテレーション順に依存した処理がないか
- `#[test]` 内で決定性を検証しているか（同一入力で2回実行→結果一致）

### 2. B-rep トポロジー正確性
- HalfEdge の twin/next/prev インデックスが循環的に正しく設定されているか
- Euler-Poincaré の不変条件が保たれているか
- Face の Loop リスト、Shell の Face リストが整合しているか

### 3. 数値・退化幾何
- ゼロ長エッジ、縮退ポリゴン、coincident vertices の検出・エラー処理があるか
- f64 の直接比較（`==`）を使っていないか（epsilon 比較を使用しているか）

### 4. Rust / MyCad 規約
- `clippy -D warnings` を通過するコードか（unwrap()、expect()、unused 変数等）
- `Debug, Clone, Serialize, Deserialize` が公開型に付いているか
- 新規依存が `[workspace.dependencies]` に追加され `{ workspace = true }` で参照されているか
- エラー型が `thiserror` で定義されているか
- テストは `#[cfg(test)] mod tests` でインラインか、統合テストは `tests/` か

### 5. テスト充足性
- 設計レビュー時に合意したテスト計画（ID T01〜）が全て実装されているか
- エッジケーステストが退化入力・境界数値を含んでいるか
- golden YAML ラウンドトリップが含まれているか

### 6. スコープ逸脱
- プラン外の機能追加・リファクタが含まれていないか
- カーネル(`mycad-kernel`)にレンダリング依存が混入していないか

---

## 出力フォーマット（厳守）

```yaml
issues:
  - id: F01
    severity: critical  # critical | high | medium | low
    file: "crates/mycad-kernel/src/brep/solid.rs"
    line_hint: 42
    finding: "HalfEdge の twin インデックスが self を指している（ループ不整合）"
    suggestion: "make_cuboid の edge 生成ロジックを見直す"
  - id: F02
    severity: high
    ...

verdict: pass  # pass | fail
# fail = Critical または High が1件以上ある
```

`issues` が空の場合は `issues: []` と書く。
**このフォーマット以外の出力は禁止。前置きや後置きの文章は書かない。**
codex
差分ベースで最終レビューを進めます。まず `git diff` と変更ファイル一覧を確認して、影響範囲とテスト追加の有無を把握します。
exec
/bin/bash -lc 'git diff --name-only' in /home/bacon/mycad
 succeeded in 0ms:
.claude/skills/3ai/SKILL.md
.gitignore
Cargo.lock
Cargo.toml
crates/mycad-build/src/lib.rs
crates/mycad-kernel/benches/tessellation.rs
crates/mycad-kernel/src/primitives/cuboid.rs
crates/mycad-kernel/src/tessellation/mod.rs
crates/mycad-kernel/src/tessellation/stl.rs
crates/mycad-kernel/tests/cuboid_roundtrip.rs

exec
/bin/bash -lc 'git status --short' in /home/bacon/mycad
 succeeded in 0ms:
 M .claude/skills/3ai/SKILL.md
 M .gitignore
 M Cargo.lock
 M Cargo.toml
 M crates/mycad-build/src/lib.rs
 M crates/mycad-kernel/benches/tessellation.rs
 M crates/mycad-kernel/src/primitives/cuboid.rs
 M crates/mycad-kernel/src/tessellation/mod.rs
 M crates/mycad-kernel/src/tessellation/stl.rs
 M crates/mycad-kernel/tests/cuboid_roundtrip.rs
?? .claude/settings.json
?? .claude/skills/3ai/agents/
?? .claude/skills/3ai/scripts/guard-crates.sh
?? crates/mycad-api/
?? features/

exec
/bin/bash -lc 'git diff --stat' in /home/bacon/mycad
 succeeded in 0ms:
 .claude/skills/3ai/SKILL.md                   |   6 +
 .gitignore                                    |   4 +
 Cargo.lock                                    | 365 ++++++++++++++++++++++++++
 Cargo.toml                                    |   7 +
 crates/mycad-build/src/lib.rs                 |   2 +-
 crates/mycad-kernel/benches/tessellation.rs   |   2 +-
 crates/mycad-kernel/src/primitives/cuboid.rs  | 115 +++++++-
 crates/mycad-kernel/src/tessellation/mod.rs   |  12 +-
 crates/mycad-kernel/src/tessellation/stl.rs   |   4 +-
 crates/mycad-kernel/tests/cuboid_roundtrip.rs |   4 +-
 10 files changed, 503 insertions(+), 18 deletions(-)

codex
変更は `mycad-kernel` 周辺に加えて `Cargo.toml` にも入っています。次は差分本体と未追跡ファイルを見て、レビュー対象に含めるべき新規コードがあるか確認します。
exec
/bin/bash -lc 'git ls-files --others --exclude-standard' in /home/bacon/mycad
 succeeded in 0ms:
.claude/settings.json
.claude/skills/3ai/agents/codex-design-reviewer.md
.claude/skills/3ai/agents/codex-final-reviewer.md
.claude/skills/3ai/agents/glm-implementer.md
.claude/skills/3ai/scripts/guard-crates.sh
crates/mycad-api/Cargo.toml
crates/mycad-api/src/error.rs
crates/mycad-api/src/handler.rs
crates/mycad-api/src/lib.rs
crates/mycad-api/src/main.rs
crates/mycad-api/src/router.rs
crates/mycad-api/tests/fixtures/assembly.mycad
crates/mycad-api/tests/fixtures/create_sphere.mycad
crates/mycad-api/tests/fixtures/empty_part.mycad
crates/mycad-api/tests/fixtures/zero_box.mycad
crates/mycad-api/tests/mesh_api.rs
features/12-cylinder/design-review-2.md
features/12-cylinder/design-review-3.md
features/12-cylinder/design-review.md
features/12-cylinder/final-review-2.md
features/12-cylinder/final-review.md
features/18-api-skeleton/ci.log
features/18-api-skeleton/design-review-2.md
features/18-api-skeleton/design-review-3.md
features/18-api-skeleton/design-review.md
features/18-api-skeleton/final-review.md
features/18-api-skeleton/glm-result.json
features/18-api-skeleton/plan.md
features/18-api-skeleton/state.json

exec
/bin/bash -lc 'git diff -- Cargo.toml Cargo.lock crates/mycad-build/src/lib.rs crates/mycad-kernel/benches/tessellation.rs crates/mycad-kernel/src/primitives/cuboid.rs crates/mycad-kernel/src/tessellation/mod.rs crates/mycad-kernel/src/tessellation/stl.rs crates/mycad-kernel/tests/cuboid_roundtrip.rs' in /home/bacon/mycad
 succeeded in 0ms:
diff --git a/Cargo.lock b/Cargo.lock
index 9795377..0dbbc3c 100644
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -67,12 +67,70 @@ dependencies = [
  "num-traits",
 ]
 
+[[package]]
+name = "atomic-waker"
+version = "1.1.2"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "1505bd5d3d116872e7271a6d4e16d81d0c8570876c8de68093a09ac269d8aac0"
+
 [[package]]
 name = "autocfg"
 version = "1.5.0"
 source = "registry+https://github.com/rust-lang/crates.io-index"
 checksum = "c08606f8c3cbf4ce6ec8e28fb0014a2c086708fe954eaa885384a6165172e7e8"
 
+[[package]]
+name = "axum"
+version = "0.8.9"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "31b698c5f9a010f6573133b09e0de5408834d0c82f8d7475a89fc1867a71cd90"
+dependencies = [
+ "axum-core",
+ "bytes",
+ "form_urlencoded",
+ "futures-util",
+ "http",
+ "http-body",
+ "http-body-util",
+ "hyper",
+ "hyper-util",
+ "itoa",
+ "matchit",
+ "memchr",
+ "mime",
+ "percent-encoding",
+ "pin-project-lite",
+ "serde_core",
+ "serde_json",
+ "serde_path_to_error",
+ "serde_urlencoded",
+ "sync_wrapper",
+ "tokio",
+ "tower",
+ "tower-layer",
+ "tower-service",
+ "tracing",
+]
+
+[[package]]
+name = "axum-core"
+version = "0.5.6"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "08c78f31d7b1291f7ee735c1c6780ccde7785daae9a9206026862dab7d8792d1"
+dependencies = [
+ "bytes",
+ "futures-core",
+ "http",
+ "http-body",
+ "http-body-util",
+ "mime",
+ "pin-project-lite",
+ "sync_wrapper",
+ "tower-layer",
+ "tower-service",
+ "tracing",
+]
+
 [[package]]
 name = "bitflags"
 version = "2.11.1"
@@ -85,6 +143,12 @@ version = "1.25.0"
 source = "registry+https://github.com/rust-lang/crates.io-index"
 checksum = "c8efb64bd706a16a1bdde310ae86b351e4d21550d98d056f22f8a7f7a2183fec"
 
+[[package]]
+name = "bytes"
+version = "1.11.1"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "1e748733b7cbc798e1434b6ac524f0c1ff2ab456fe201501e6497c8417a4fc33"
+
 [[package]]
 name = "cfg-if"
 version = "1.0.4"
@@ -171,6 +235,48 @@ version = "0.1.5"
 source = "registry+https://github.com/rust-lang/crates.io-index"
 checksum = "d9c4f5dac5e15c24eb999c26181a6ca40b39fe946cbe4c263c7209467bc83af2"
 
+[[package]]
+name = "form_urlencoded"
+version = "1.2.2"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "cb4cb245038516f5f85277875cdaa4f7d2c9a0fa0468de06ed190163b1581fcf"
+dependencies = [
+ "percent-encoding",
+]

exec
/bin/bash -lc "sed -n '1,260p' features/18-api-skeleton/design-review-3.md" in /home/bacon/mycad
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
session id: 019e5dc1-9272-7a70-b9a3-fa9d29c4365a
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

> Codex 設計レビュー round 1 + round 2 反映済み。round1: R01/R03/R04 受諾、R02 kernel 検証追加(ユーザー決定)、R05 部分受諾。round2: R01(構造化エラー)受諾、R02(assembly 判別)受諾、R03(公差)は #17 委譲で反論、R04(fixture roundtrip)反論。

## 実装対象

- 新規クレート `crates/mycad-api/`(lib + bin 構成)+ `make_cuboid` の入力検証(kernel)
- Issue: #18 / Milestone: Phase 2

### v0 スコープ明確化(R03)
- 対応するのは **`root_component` に単一 part feature を持つ文書のみ**(Box / Cylinder)。
- assembly(`children`/`reference` を持ち root features が空)や複数フィーチャは v0 非対応。明確なエラー + 422 系で返す(クラッシュさせない)。

### 影響ファイル

| ファイル | 変更 |
|----------|------|
| `crates/mycad-kernel/src/primitives/cuboid.rs` | `make_cuboid` を `-> Result<Solid, KernelError>` 化。`dx/dy/dz` が非有限 or `<= 0.0` なら `InvalidParameter { kind }`。**(R03)** 閾値は `make_cylinder`(cylinder.rs:16-22)と同じ exact `<= 0.0` で対称化。near-zero 正値の公差判定は導入しない(下記 R03 注記)。インライン test を `.unwrap()` 化 |
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

// error.rs — (R01) status は型から確定。文字列解析しない
enum ApiError {
    BadRequest(String),     // 400
    NotFound(String),       // 404
    Unprocessable(String),  // 422
    Internal(String),       // 500
}
impl From<FormatError>      for ApiError { /* variant 別に 400/404/500 */ }
impl From<KernelError>      for ApiError { /* 全 variant → 422 */ }
impl From<TessellationError> for ApiError { /* 全 variant → 422 */ }
impl IntoResponse for ApiError { /* status は variant から、body = {"error": msg} */ }
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

### エラーハンドリング(R01: 型から status を確定。文字列解析禁止)
- `ApiError` は 4 variant(`BadRequest`/`NotFound`/`Unprocessable`/`Internal`)。各下層エラーから `From` で変換し、`IntoResponse` は **variant から status を決め**、body は `{"error": msg}` に整形するだけ。
- `From<FormatError>`: `InvalidExtension`/`Yaml` → 400(`BadRequest`)、`Io(ErrorKind::NotFound)` → 404、その他 `Io`/`InvalidReference` → 500(`Internal`)。
- `From<KernelError>`: `InvalidParameter`(退化寸法)・`UnsupportedFeature`・`MultipleFeatures`・`EmptyFeatureList` → 422(`Unprocessable`)。
- `From<TessellationError>`: `UnsupportedSurface`・`TrimmedFaceUnsupported`・`NonManifoldLoop` → 422。
- handler が assembly/reference 文書を検出した場合(下記 R02)も 422(`Unprocessable`)。

### v0 文書判別(R02: handler で build 前に検査)
- `build_solid_from_features` を呼ぶ前に `root_component` を検査する:
  - `reference.is_some()` または `!children.is_empty()` → assembly/reference 文書 → `ApiError::Unprocessable("assembly/reference は v0 非対応")`。
  - `features.is_empty()`(かつ assembly でない)→ 「part なのに feature が空」として 422。
- これにより `EmptyFeatureList` の意味を「不正な空 part」に限定し、assembly を別メッセージで区別する。

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
| K01 | kernel 単体 | `make_cuboid(0.0, 1.0, 1.0, ..)` / 負値 / NaN | `Err(InvalidParameter { kind: "..." })` |
| K02 | kernel 単体(R03 境界可視化) | `make_cuboid(1e-9, 1.0, 1.0, ..)`(near-zero 正値) | 現状は `Ok`(exact `<= 0.0` 方針)であることを assert し境界を明文化 |

> **R05 反論**(round 1): Euler-Poincaré は kernel(`cylinder.rs:180-195`)、golden YAML roundtrip は format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。
>
> **R03 判断**(round 2): 退化寸法の閾値は `make_cylinder` と同じ exact `<= 0.0` で対称化する。**グローバル公差(near-zero の扱い)は ROADMAP の Issue #17「単位系・グローバル公差の早期固定(ADR-004 範囲)」が所有する未決事項**であり、#18 で独自 EPS を導入すると #17 の設計判断を先取りしてしまう。よって #18 は exact-zero 方針に留め、near-zero 正値が現状 `Ok` となる境界を K02 で明文化する(将来 #17 で公差確定後に統一)。
>
> **R04 反論**(round 2): 新規 `crates/mycad-api/tests/fixtures/*.mycad` は T07-T09 で API 経由で必ずパースされる(パースできなければ期待する 422 が出ず test fail)ため、入力形状の妥当性は API テストで担保済み。`Document` の YAML roundtrip 安定性は `mycad-format` の既存 roundtrip テストが所有。クロスクレートの roundtrip 追加は「テストは所有クレートに置く」規約に反するため行わない。fixture は最小に保つ。

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
設計ドキュメントの整合性を確認するため、関連する既存型とエラー定義だけ読みます。API 契約と既存クレートの意味づけが噛み合っているかを先に見ます。
exec
/bin/bash -lc "rg -n \"enum FormatError|struct TriangleMesh|enum KernelError|enum TessellationError|fn from_path|build_solid_from_features|make_cuboid|make_cylinder|EmptyFeatureList|InvalidReference\" crates -g '*.rs'" in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-format/src/component.rs:42:            return Err(FormatError::InvalidReference {
crates/mycad-format/src/component.rs:49:                return Err(FormatError::InvalidReference {
crates/mycad-format/src/error.rs:4:pub enum FormatError {
crates/mycad-format/src/error.rs:15:    InvalidReference { value: String, reason: &'static str },
crates/mycad-format/src/document.rs:36:    pub fn from_path(path: &Path) -> Result<Self, FormatError> {
crates/mycad-cli/src/main.rs:2:use mycad_build::build_solid_from_features;
crates/mycad-cli/src/main.rs:55:    let solid = build_solid_from_features(&doc.root_component.features, &mut gen)
crates/mycad-build/src/lib.rs:4:use mycad_kernel::primitives::{make_cuboid, make_cylinder};
crates/mycad-build/src/lib.rs:6:pub fn build_solid_from_features(
crates/mycad-build/src/lib.rs:11:        return Err(KernelError::EmptyFeatureList);
crates/mycad-build/src/lib.rs:26:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
crates/mycad-build/src/lib.rs:27:        Feature::CreateCylinder { radius, height, .. } => make_cylinder(*radius, *height, gen),
crates/mycad-build/tests/feature_dispatcher.rs:1:use mycad_build::build_solid_from_features;
crates/mycad-build/tests/feature_dispatcher.rs:17:        build_solid_from_features(&doc.root_component.features, &mut g).expect("build solid");
crates/mycad-kernel/src/error.rs:4:pub enum KernelError {
crates/mycad-kernel/src/error.rs:6:    EmptyFeatureList,
crates/mycad-kernel/benches/tessellation.rs:2:use mycad_kernel::primitives::make_cuboid;
crates/mycad-kernel/benches/tessellation.rs:15:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
crates/mycad-kernel/src/primitives/mod.rs:4:pub use cuboid::make_cuboid;
crates/mycad-kernel/src/primitives/mod.rs:5:pub use cylinder::make_cylinder;
crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
crates/mycad-kernel/src/tessellation/mod.rs:18:pub struct TriangleMesh {
crates/mycad-kernel/src/tessellation/mod.rs:75:pub enum TessellationError {
crates/mycad-kernel/src/tessellation/mod.rs:315:    use crate::primitives::make_cuboid;
crates/mycad-kernel/src/tessellation/mod.rs:316:    use crate::primitives::make_cylinder;
crates/mycad-kernel/src/tessellation/mod.rs:322:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:343:        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:344:        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:358:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
crates/mycad-kernel/src/tessellation/mod.rs:373:        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
crates/mycad-kernel/src/tessellation/mod.rs:374:        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
crates/mycad-kernel/src/tessellation/mod.rs:388:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:442:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:500:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
crates/mycad-kernel/src/tessellation/mod.rs:514:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
crates/mycad-kernel/src/tessellation/mod.rs:515:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
crates/mycad-kernel/src/tessellation/mod.rs:529:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
crates/mycad-kernel/src/primitives/cylinder.rs:11:pub fn make_cylinder(
crates/mycad-kernel/src/primitives/cylinder.rs:138:        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
crates/mycad-kernel/src/primitives/cylinder.rs:139:        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();

exec
/bin/bash -lc 'nl -ba crates/mycad-format/src/error.rs' in /home/bacon/mycad
exec
/bin/bash -lc 'nl -ba crates/mycad-build/src/lib.rs' in /home/bacon/mycad
 succeeded in 0ms:
     1	use thiserror::Error;
     2	
     3	#[derive(Debug, Error)]
     4	pub enum FormatError {
     5	    #[error("I/O error: {0}")]
     6	    Io(#[from] std::io::Error),
     7	
     8	    #[error("YAML parse error: {0}")]
     9	    Yaml(#[from] serde_yaml::Error),
    10	
    11	    #[error("invalid file extension: expected .mycad, got {0:?}")]
    12	    InvalidExtension(Option<String>),
    13	
    14	    #[error("invalid reference {value:?}: {reason}")]
    15	    InvalidReference { value: String, reason: &'static str },
    16	}

 succeeded in 0ms:
     1	use mycad_format::Feature;
     2	use mycad_kernel::brep::topology::{IdGenerator, Solid};
     3	use mycad_kernel::error::KernelError;
     4	use mycad_kernel::primitives::{make_cuboid, make_cylinder};
     5	
     6	pub fn build_solid_from_features(
     7	    features: &[Feature],
     8	    gen: &mut IdGenerator,
     9	) -> Result<Solid, KernelError> {
    10	    if features.is_empty() {
    11	        return Err(KernelError::EmptyFeatureList);
    12	    }
    13	
    14	    if features.len() > 1 {
    15	        return Err(KernelError::MultipleFeatures {
    16	            count: features.len(),
    17	        });
    18	    }
    19	
    20	    match &features[0] {
    21	        Feature::CreateBox {
    22	            width,
    23	            height,
    24	            depth,
    25	            ..
    26	        } => make_cuboid(*width, *height, *depth, gen),
    27	        Feature::CreateCylinder { radius, height, .. } => make_cylinder(*radius, *height, gen),
    28	        Feature::CreateSphere { .. } => Err(KernelError::UnsupportedFeature {
    29	            kind: "create_sphere",
    30	        }),
    31	        Feature::Extrude { .. } => Err(KernelError::UnsupportedFeature { kind: "extrude" }),
    32	        Feature::Cut { .. } => Err(KernelError::UnsupportedFeature { kind: "cut" }),
    33	        Feature::Fuse { .. } => Err(KernelError::UnsupportedFeature { kind: "fuse" }),
    34	        Feature::Intersect { .. } => Err(KernelError::UnsupportedFeature { kind: "intersect" }),
    35	    }
    36	}

exec
/bin/bash -lc "nl -ba crates/mycad-format/src/document.rs | sed -n '1,140p'" in /home/bacon/mycad
 succeeded in 0ms:
     1	use crate::component::Component;
     2	use crate::error::FormatError;
     3	use schemars::JsonSchema;
     4	use serde::{Deserialize, Serialize};
     5	use std::path::Path;
     6	
     7	/// The top-level document representing a MyCad design file.
     8	#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
     9	pub struct Document {
    10	    /// Kernel version that created this document.
    11	    pub version: String,
    12	    /// The root component (assembly or single part).
    13	    pub root_component: Component,
    14	}
    15	
    16	impl Document {
    17	    /// Create a new document with a single empty root component.
    18	    pub fn new(name: &str) -> Self {
    19	        Self {
    20	            version: env!("CARGO_PKG_VERSION").to_string(),
    21	            root_component: Component::new(name),
    22	        }
    23	    }
    24	
    25	    /// Serialize to YAML string.
    26	    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
    27	        serde_yaml::to_string(self)
    28	    }
    29	
    30	    /// Deserialize from YAML string.
    31	    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
    32	        serde_yaml::from_str(yaml)
    33	    }
    34	
    35	    /// Load a Document from a `.mycad` file path.
    36	    pub fn from_path(path: &Path) -> Result<Self, FormatError> {
    37	        let ext = path.extension().and_then(|s| s.to_str());
    38	        if ext != Some("mycad") {
    39	            return Err(FormatError::InvalidExtension(ext.map(str::to_string)));
    40	        }
    41	        let content = std::fs::read_to_string(path)?;
    42	        let doc = Self::from_yaml(&content)?;
    43	        Ok(doc)
    44	    }
    45	}
    46	
    47	#[cfg(test)]
    48	mod tests {
    49	    use super::*;
    50	    use crate::error::FormatError;
    51	    use crate::feature::Feature;
    52	    use std::path::Path;
    53	
    54	    #[test]
    55	    fn test_roundtrip_yaml() {
    56	        let mut doc = Document::new("Test Assembly");
    57	        doc.root_component.features.push(Feature::Extrude {
    58	            id: "box_1".to_string(),
    59	            sketch: "sketch_1".to_string(),
    60	            depth: 10.0,
    61	        });
    62	
    63	        let yaml = doc.to_yaml().unwrap();
    64	        let doc2 = Document::from_yaml(&yaml).unwrap();
    65	
    66	        assert_eq!(doc.version, doc2.version);
    67	        assert_eq!(doc.root_component.name, doc2.root_component.name);
    68	        assert_eq!(
    69	            doc.root_component.features.len(),
    70	            doc2.root_component.features.len()
    71	        );
    72	    }
    73	
    74	    #[test]
    75	    fn test_yaml_is_deterministic() {
    76	        let mut doc = Document::new("Test");
    77	        doc.root_component.features.push(Feature::CreateBox {
    78	            id: "box_1".to_string(),
    79	            width: 10.0,
    80	            height: 20.0,
    81	            depth: 30.0,
    82	        });
    83	
    84	        let yaml1 = doc.to_yaml().unwrap();
    85	        let yaml2 = doc.to_yaml().unwrap();
    86	        assert_eq!(yaml1, yaml2, "YAML serialization must be deterministic");
    87	    }
    88	
    89	    #[test]
    90	    fn test_from_path_loads_simple_box() {
    91	        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
    92	            .join("..")
    93	            .join("..")
    94	            .join("examples")
    95	            .join("simple_box.mycad");
    96	        let doc = Document::from_path(&path).unwrap();
    97	        assert_eq!(doc.version, "0.1.0");
    98	        assert_eq!(doc.root_component.name, "Simple Box");
    99	    }
   100	
   101	    #[test]
   102	    fn test_from_path_missing_file_returns_io_error() {
   103	        let path = Path::new("does_not_exist.mycad");
   104	        let err = Document::from_path(path).unwrap_err();
   105	        assert!(matches!(err, FormatError::Io(_)));
   106	    }
   107	
   108	    #[test]
   109	    fn test_from_path_invalid_extension() {
   110	        let path = Path::new("foo.txt");
   111	        let err = Document::from_path(path).unwrap_err();
   112	        assert!(matches!(err, FormatError::InvalidExtension(Some(ref e)) if e == "txt"));
   113	    }
   114	
   115	    #[test]
   116	    fn test_assembly_roundtrip() {
   117	        use crate::component::ComponentRef;
   118	
   119	        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
   120	            .join("..")
   121	            .join("..")
   122	            .join("examples")
   123	            .join("assembly.mycad");
   124	        let doc = Document::from_path(&path).unwrap();
   125	
   126	        // Bolt child has a stdlib reference
   127	        let bolt = doc
   128	            .root_component
   129	            .children
   130	            .iter()
   131	            .find(|c| c.name == "Bolt")
   132	            .expect("Bolt component not found");
   133	        assert!(
   134	            matches!(&bolt.reference, Some(ComponentRef::StdLib(p)) if p == "fasteners/jis_b1176/M5x20")
   135	        );
   136	
   137	        // Roundtrip: serialize then deserialize yields the same YAML
   138	        let yaml = doc.to_yaml().unwrap();
   139	        let doc2 = Document::from_yaml(&yaml).unwrap();
   140	        assert_eq!(doc.to_yaml().unwrap(), doc2.to_yaml().unwrap());

exec
/bin/bash -lc 'nl -ba crates/mycad-kernel/src/error.rs' in /home/bacon/mycad
 succeeded in 0ms:
     1	use thiserror::Error;
     2	
     3	#[derive(Debug, Error)]
     4	pub enum KernelError {
     5	    #[error("empty feature list")]
     6	    EmptyFeatureList,
     7	
     8	    #[error("multiple features not yet supported (got {count})")]
     9	    MultipleFeatures { count: usize },
    10	
    11	    #[error("unsupported feature variant: {kind}")]
    12	    UnsupportedFeature { kind: &'static str },
    13	
    14	    #[error("invalid parameter: {kind}")]
    15	    InvalidParameter { kind: &'static str },
    16	}

codex
テスト依存にも一つ気になる点があります。`tower::ServiceExt::oneshot` は feature gating のことが多いので、`tower` の feature 定義をローカルキャッシュで確認します。
exec
/bin/bash -lc "rg -n \"ServiceExt|oneshot|\\[features\\]|util\" ~/.cargo/registry/src -g 'tower-0.5.*/Cargo.toml' -g 'tower-0.5.*/src/lib.rs' -g 'tower-0.5.*/src/**/*.rs'" in /home/bacon/mycad
 exited 1 in 0ms:
exec
/bin/bash -lc "nl -ba crates/mycad-kernel/src/primitives/cuboid.rs | sed -n '1,430p'" in /home/bacon/mycad
 succeeded in 0ms:
     1	use crate::brep::topology::{IdGenerator, Solid};
     2	use crate::error::KernelError;
     3	use crate::geometry::curve::Curve;
     4	use crate::geometry::surface::Surface;
     5	use crate::geometry::Point;
     6	use crate::geometry::Vec3;
     7	
     8	/// Create a cuboid (box) B-rep solid centered at the origin.
     9	///
    10	/// Parameters:
    11	/// - `dx`: width (X dimension)
    12	/// - `dy`: height (Y dimension)
    13	/// - `dz`: depth (Z dimension)
    14	/// - `id_gen`: deterministic ID generator
    15	///
    16	/// The cuboid spans from (-dx/2, -dy/2, -dz/2) to (dx/2, dy/2, dz/2).
    17	pub fn make_cuboid(
    18	    dx: f64,
    19	    dy: f64,
    20	    dz: f64,
    21	    id_gen: &mut IdGenerator,
    22	) -> Result<Solid, KernelError> {
    23	    if !dx.is_finite() || dx <= 0.0 {
    24	        return Err(KernelError::InvalidParameter { kind: "width" });
    25	    }
    26	    if !dy.is_finite() || dy <= 0.0 {
    27	        return Err(KernelError::InvalidParameter { kind: "height" });
    28	    }
    29	    if !dz.is_finite() || dz <= 0.0 {
    30	        return Err(KernelError::InvalidParameter { kind: "depth" });
    31	    }
    32	    let mut solid = Solid::new(id_gen.next());
    33	
    34	    let hx = dx / 2.0;
    35	    let hy = dy / 2.0;
    36	    let hz = dz / 2.0;
    37	
    38	    // 8 vertices of the cuboid
    39	    //
    40	    //     6-------7
    41	    //    /|      /|
    42	    //   4-------5 |
    43	    //   | 2-----|-3
    44	    //   |/      |/
    45	    //   0-------1
    46	    //
    47	    // v0 = (-hx, -hy, -hz), v1 = (hx, -hy, -hz), ...
    48	    let v_positions = [
    49	        Point::new(-hx, -hy, -hz), // 0: front-bottom-left
    50	        Point::new(hx, -hy, -hz),  // 1: front-bottom-right
    51	        Point::new(-hx, hy, -hz),  // 2: front-top-left
    52	        Point::new(hx, hy, -hz),   // 3: front-top-right
    53	        Point::new(-hx, -hy, hz),  // 4: back-bottom-left
    54	        Point::new(hx, -hy, hz),   // 5: back-bottom-right
    55	        Point::new(-hx, hy, hz),   // 6: back-top-left
    56	        Point::new(hx, hy, hz),    // 7: back-top-right
    57	    ];
    58	
    59	    let v: Vec<usize> = v_positions
    60	        .iter()
    61	        .map(|p| solid.add_vertex(id_gen.next(), *p))
    62	        .collect();
    63	
    64	    // 12 edges of the cuboid
    65	    // Each edge is defined by [start_vertex_index, end_vertex_index]
    66	    let edge_defs: [(usize, usize); 12] = [
    67	        (0, 1), // e0:  bottom-front
    68	        (1, 3), // e1:  right-front
    69	        (3, 2), // e2:  top-front
    70	        (2, 0), // e3:  left-front
    71	        (4, 5), // e4:  bottom-back
    72	        (5, 7), // e5:  right-back
    73	        (7, 6), // e6:  top-back
    74	        (6, 4), // e7:  left-back
    75	        (0, 4), // e8:  bottom-left
    76	        (1, 5), // e9:  bottom-right
    77	        (3, 7), // e10: top-right
    78	        (2, 6), // e11: top-left
    79	    ];
    80	
    81	    let e: Vec<usize> = edge_defs
    82	        .iter()
    83	        .map(|&(v0, v1)| {
    84	            let p0 = solid.vertices[v[v0]].point;
    85	            let p1 = solid.vertices[v[v1]].point;
    86	            let direction = p1 - p0;
    87	            solid.add_edge(
    88	                id_gen.next(),
    89	                [v[v0], v[v1]],
    90	                Curve::Line {
    91	                    origin: p0,
    92	                    direction,
    93	                },
    94	                [0.0, 1.0],
    95	            )
    96	        })
    97	        .collect();
    98	
    99	    // 6 faces of the cuboid
   100	    // Each face is defined by its 4 edges and their orientation (forward/backward),
   101	    // plus the surface definition.
   102	    struct FaceDef {
   103	        // (edge_index, forward, start_vertex_index)
   104	        half_edges: [(usize, bool, usize); 4],
   105	        surface: Surface,
   106	        same_sense: bool,
   107	    }
   108	
   109	    let face_defs = [
   110	        // Front face (Z = -hz): vertices 0,1,3,2 — normal -Z
   111	        FaceDef {
   112	            half_edges: [
   113	                (0, true, 0), // e0: 0->1
   114	                (1, true, 1), // e1: 1->3
   115	                (2, true, 3), // e2: 3->2
   116	                (3, true, 2), // e3: 2->0
   117	            ],
   118	            surface: Surface::Plane {
   119	                origin: Point::new(0.0, 0.0, -hz),
   120	                normal: -Vec3::z(),
   121	                u_axis: Vec3::x(),
   122	                v_axis: Vec3::y(),
   123	            },
   124	            same_sense: true,
   125	        },
   126	        // Back face (Z = +hz): vertices 5,4,6,7 — normal +Z
   127	        FaceDef {
   128	            half_edges: [
   129	                (4, false, 5), // e4: 5->4
   130	                (7, false, 4), // e7: 4->6
   131	                (6, false, 6), // e6: 6->7
   132	                (5, false, 7), // e5: 7->5
   133	            ],
   134	            surface: Surface::Plane {
   135	                origin: Point::new(0.0, 0.0, hz),
   136	                normal: Vec3::z(),
   137	                u_axis: -Vec3::x(),
   138	                v_axis: Vec3::y(),
   139	            },
   140	            same_sense: true,
   141	        },
   142	        // Bottom face (Y = -hy): vertices 0,4,5,1 — normal -Y
   143	        FaceDef {
   144	            half_edges: [
   145	                (8, true, 0),  // e8: 0->4
   146	                (4, true, 4),  // e4: 4->5
   147	                (9, false, 5), // e9: 5->1
   148	                (0, false, 1), // e0: 1->0
   149	            ],
   150	            surface: Surface::Plane {
   151	                origin: Point::new(0.0, -hy, 0.0),
   152	                normal: -Vec3::y(),
   153	                u_axis: Vec3::x(),
   154	                v_axis: Vec3::z(),
   155	            },
   156	            same_sense: true,
   157	        },
   158	        // Top face (Y = +hy): vertices 2,3,7,6 — normal +Y
   159	        FaceDef {
   160	            half_edges: [
   161	                (2, false, 2),  // e2: 2->3
   162	                (10, true, 3),  // e10: 3->7
   163	                (6, true, 7),   // e6: 7->6
   164	                (11, false, 6), // e11: 6->2
   165	            ],
   166	            surface: Surface::Plane {
   167	                origin: Point::new(0.0, hy, 0.0),
   168	                normal: Vec3::y(),
   169	                u_axis: Vec3::x(),
   170	                v_axis: Vec3::z(),
   171	            },
   172	            same_sense: true,
   173	        },
   174	        // Left face (X = -hx): vertices 0,2,6,4 — normal -X
   175	        FaceDef {
   176	            half_edges: [
   177	                (3, false, 0), // e3: 0->2
   178	                (11, true, 2), // e11: 2->6
   179	                (7, true, 6),  // e7: 6->4
   180	                (8, false, 4), // e8: 4->0
   181	            ],
   182	            surface: Surface::Plane {
   183	                origin: Point::new(-hx, 0.0, 0.0),
   184	                normal: -Vec3::x(),
   185	                u_axis: Vec3::y(),
   186	                v_axis: Vec3::z(),
   187	            },
   188	            same_sense: true,
   189	        },
   190	        // Right face (X = +hx): vertices 1,5,7,3 — normal +X
   191	        FaceDef {
   192	            half_edges: [
   193	                (9, true, 1),   // e9: 1->5
   194	                (5, true, 5),   // e5: 5->7
   195	                (10, false, 7), // e10: 7->3
   196	                (1, false, 3),  // e1: 3->1
   197	            ],
   198	            surface: Surface::Plane {
   199	                origin: Point::new(hx, 0.0, 0.0),
   200	                normal: Vec3::x(),
   201	                u_axis: Vec3::y(),
   202	                v_axis: Vec3::z(),
   203	            },
   204	            same_sense: true,
   205	        },
   206	    ];
   207	
   208	    let mut face_indices = Vec::new();
   209	
   210	    for face_def in &face_defs {
   211	        let he_indices: Vec<usize> = face_def
   212	            .half_edges
   213	            .iter()
   214	            .map(|&(edge_idx, forward, start_v)| {
   215	                solid.add_half_edge(id_gen.next(), v[start_v], e[edge_idx], forward)
   216	            })
   217	            .collect();
   218	
   219	        let loop_idx = solid.add_loop(id_gen.next(), he_indices);
   220	        let face_idx = solid.add_face(
   221	            id_gen.next(),
   222	            face_def.surface.clone(),
   223	            loop_idx,
   224	            vec![],
   225	            face_def.same_sense,
   226	        );
   227	        face_indices.push(face_idx);
   228	    }
   229	
   230	    solid.add_shell(id_gen.next(), face_indices, true);
   231	
   232	    Ok(solid)
   233	}
   234	
   235	#[cfg(test)]
   236	mod tests {
   237	    use super::*;
   238	    use crate::brep::topology::IdGenerator;
   239	    use crate::error::KernelError;
   240	
   241	    #[test]
   242	    fn test_cuboid_topology() {
   243	        let mut id_gen = IdGenerator::new(0);
   244	        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
   245	
   246	        assert_eq!(solid.vertices.len(), 8, "cuboid should have 8 vertices");
   247	        assert_eq!(solid.edges.len(), 12, "cuboid should have 12 edges");
   248	        assert_eq!(solid.faces.len(), 6, "cuboid should have 6 faces");
   249	        assert_eq!(solid.shells.len(), 1, "cuboid should have 1 shell");
   250	        assert!(solid.shells[0].closed, "cuboid shell should be closed");
   251	        // 6 faces * 4 half-edges = 24 half-edges
   252	        assert_eq!(
   253	            solid.half_edges.len(),
   254	            24,
   255	            "cuboid should have 24 half-edges"
   256	        );
   257	        // 6 loops (one per face)
   258	        assert_eq!(solid.loops.len(), 6, "cuboid should have 6 loops");
   259	    }
   260	
   261	    #[test]
   262	    fn test_cuboid_vertex_positions() {
   263	        let mut id_gen = IdGenerator::new(0);
   264	        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen).unwrap();
   265	
   266	        // Check that all vertices are at the expected positions
   267	        let expected = [
   268	            Point::new(-1.0, -2.0, -3.0),
   269	            Point::new(1.0, -2.0, -3.0),
   270	            Point::new(-1.0, 2.0, -3.0),
   271	            Point::new(1.0, 2.0, -3.0),
   272	            Point::new(-1.0, -2.0, 3.0),
   273	            Point::new(1.0, -2.0, 3.0),
   274	            Point::new(-1.0, 2.0, 3.0),
   275	            Point::new(1.0, 2.0, 3.0),
   276	        ];
   277	
   278	        for (v, exp) in solid.vertices.iter().zip(expected.iter()) {
   279	            assert!(
   280	                (v.point - exp).norm() < 1e-12,
   281	                "vertex {:?} doesn't match expected {:?}",
   282	                v.point,
   283	                exp
   284	            );
   285	        }
   286	    }
   287	
   288	    #[test]
   289	    fn test_cuboid_deterministic() {
   290	        // Building the same cuboid twice with the same ID generator should
   291	        // produce identical results.
   292	        let mut id_gen1 = IdGenerator::new(0);
   293	        let mut id_gen2 = IdGenerator::new(0);
   294	
   295	        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1).unwrap();
   296	        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2).unwrap();
   297	
   298	        assert_eq!(solid1.vertices.len(), solid2.vertices.len());
   299	        for (v1, v2) in solid1.vertices.iter().zip(solid2.vertices.iter()) {
   300	            assert_eq!(v1.id, v2.id);
   301	            assert_eq!(v1.point, v2.point);
   302	        }
   303	
   304	        assert_eq!(solid1.edges.len(), solid2.edges.len());
   305	        for (e1, e2) in solid1.edges.iter().zip(solid2.edges.iter()) {
   306	            assert_eq!(e1.id, e2.id);
   307	            assert_eq!(e1.vertices, e2.vertices);
   308	        }
   309	
   310	        assert_eq!(solid1.faces.len(), solid2.faces.len());
   311	        for (f1, f2) in solid1.faces.iter().zip(solid2.faces.iter()) {
   312	            assert_eq!(f1.id, f2.id);
   313	        }
   314	    }
   315	
   316	    // K01: make_cuboid rejects zero, negative, NaN, Infinity dimensions
   317	    #[test]
   318	    fn test_make_cuboid_invalid_dimensions() {
   319	        let mut gen = IdGenerator::new(0);
   320	
   321	        // Zero
   322	        assert!(matches!(
   323	            make_cuboid(0.0, 1.0, 1.0, &mut gen),
   324	            Err(KernelError::InvalidParameter { kind: "width" })
   325	        ));
   326	        assert!(matches!(
   327	            make_cuboid(1.0, 0.0, 1.0, &mut gen),
   328	            Err(KernelError::InvalidParameter { kind: "height" })
   329	        ));
   330	        assert!(matches!(
   331	            make_cuboid(1.0, 1.0, 0.0, &mut gen),
   332	            Err(KernelError::InvalidParameter { kind: "depth" })
   333	        ));
   334	
   335	        // Negative
   336	        assert!(matches!(
   337	            make_cuboid(-1.0, 1.0, 1.0, &mut gen),
   338	            Err(KernelError::InvalidParameter { kind: "width" })
   339	        ));
   340	        assert!(matches!(
   341	            make_cuboid(1.0, -1.0, 1.0, &mut gen),
   342	            Err(KernelError::InvalidParameter { kind: "height" })
   343	        ));
   344	        assert!(matches!(
   345	            make_cuboid(1.0, 1.0, -1.0, &mut gen),
   346	            Err(KernelError::InvalidParameter { kind: "depth" })
   347	        ));
   348	
   349	        // NaN
   350	        assert!(matches!(
   351	            make_cuboid(f64::NAN, 1.0, 1.0, &mut gen),
   352	            Err(KernelError::InvalidParameter { kind: "width" })
   353	        ));
   354	        assert!(matches!(
   355	            make_cuboid(1.0, f64::NAN, 1.0, &mut gen),
   356	            Err(KernelError::InvalidParameter { kind: "height" })
   357	        ));
   358	        assert!(matches!(
   359	            make_cuboid(1.0, 1.0, f64::NAN, &mut gen),
   360	            Err(KernelError::InvalidParameter { kind: "depth" })
   361	        ));
   362	
   363	        // Infinity
   364	        assert!(matches!(
   365	            make_cuboid(f64::INFINITY, 1.0, 1.0, &mut gen),
   366	            Err(KernelError::InvalidParameter { kind: "width" })
   367	        ));
   368	        assert!(matches!(
   369	            make_cuboid(1.0, f64::INFINITY, 1.0, &mut gen),
   370	            Err(KernelError::InvalidParameter { kind: "height" })
   371	        ));
   372	        assert!(matches!(
   373	            make_cuboid(1.0, 1.0, f64::INFINITY, &mut gen),
   374	            Err(KernelError::InvalidParameter { kind: "depth" })
   375	        ));
   376	
   377	        // Negative Infinity
   378	        assert!(matches!(
   379	            make_cuboid(f64::NEG_INFINITY, 1.0, 1.0, &mut gen),
   380	            Err(KernelError::InvalidParameter { kind: "width" })
   381	        ));
   382	        assert!(matches!(
   383	            make_cuboid(1.0, f64::NEG_INFINITY, 1.0, &mut gen),
   384	            Err(KernelError::InvalidParameter { kind: "height" })
   385	        ));
   386	        assert!(matches!(
   387	            make_cuboid(1.0, 1.0, f64::NEG_INFINITY, &mut gen),
   388	            Err(KernelError::InvalidParameter { kind: "depth" })
   389	        ));
   390	    }
   391	
   392	    // K02: near-zero positive value is Ok (exact <= 0.0 policy)
   393	    #[test]
   394	    fn test_make_cuboid_near_zero_positive_ok() {
   395	        let mut gen = IdGenerator::new(0);
   396	        let result = make_cuboid(1e-9, 1.0, 1.0, &mut gen);
   397	        assert!(
   398	            result.is_ok(),
   399	            "near-zero positive should be Ok per exact <= 0.0 policy"
   400	        );
   401	    }
   402	}

exec
/bin/bash -lc "rg -n \"pub struct HalfEdge|twin|next|prev|loop_indices|face_indices|shells|faces\" crates/mycad-kernel/src/brep -g '*.rs'" in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-kernel/src/brep/topology.rs:18:/// Half-edges are oriented: they go from `start_vertex` to the next
crates/mycad-kernel/src/brep/topology.rs:21:pub struct HalfEdge {
crates/mycad-kernel/src/brep/topology.rs:67:/// A shell — a connected set of faces forming a closed or open surface.
crates/mycad-kernel/src/brep/topology.rs:71:    /// Indices of faces in the parent solid's face list.
crates/mycad-kernel/src/brep/topology.rs:72:    pub faces: Vec<usize>,
crates/mycad-kernel/src/brep/topology.rs:86:    pub faces: Vec<Face>,
crates/mycad-kernel/src/brep/topology.rs:87:    pub shells: Vec<Shell>,
crates/mycad-kernel/src/brep/topology.rs:99:            faces: Vec::new(),
crates/mycad-kernel/src/brep/topology.rs:100:            shells: Vec::new(),
crates/mycad-kernel/src/brep/topology.rs:163:        let idx = self.faces.len();
crates/mycad-kernel/src/brep/topology.rs:164:        self.faces.push(Face {
crates/mycad-kernel/src/brep/topology.rs:175:    pub fn add_shell(&mut self, id: EntityId, faces: Vec<usize>, closed: bool) -> usize {
crates/mycad-kernel/src/brep/topology.rs:176:        let idx = self.shells.len();
crates/mycad-kernel/src/brep/topology.rs:177:        self.shells.push(Shell { id, faces, closed });
crates/mycad-kernel/src/brep/topology.rs:186:    next_id: EntityId,
crates/mycad-kernel/src/brep/topology.rs:191:        Self { next_id: start }
crates/mycad-kernel/src/brep/topology.rs:195:    pub fn next(&mut self) -> EntityId {
crates/mycad-kernel/src/brep/topology.rs:196:        let id = self.next_id;
crates/mycad-kernel/src/brep/topology.rs:197:        self.next_id += 1;
crates/mycad-kernel/src/brep/topology.rs:211:            assert_eq!(gen1.next(), gen2.next());

exec
/bin/bash -lc 'ls ~/.cargo/registry/src' in /home/bacon/mycad
 succeeded in 0ms:
index.crates.io-1949cf8c6b5b557f

exec
/bin/bash -lc "find ~/.cargo/registry/src -maxdepth 2 -type d -name 'tower-0.5*'" in /home/bacon/mycad
 succeeded in 0ms:
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3

exec
/bin/bash -lc "nl -ba crates/mycad-kernel/src/brep/topology.rs | sed -n '1,260p'" in /home/bacon/mycad
 succeeded in 0ms:
     1	use crate::geometry::curve::Curve;
     2	use crate::geometry::surface::Surface;
     3	use crate::geometry::Point;
     4	use serde::{Deserialize, Serialize};
     5	
     6	/// Unique identifier for topological entities.
     7	/// Generated deterministically by creation order within a feature.
     8	pub type EntityId = u64;
     9	
    10	/// A vertex in B-rep — a point in 3D space.
    11	#[derive(Debug, Clone, Serialize, Deserialize)]
    12	pub struct Vertex {
    13	    pub id: EntityId,
    14	    pub point: Point,
    15	}
    16	
    17	/// A half-edge — one side of an edge, used within a loop (wire).
    18	/// Half-edges are oriented: they go from `start_vertex` to the next
    19	/// half-edge's start vertex.
    20	#[derive(Debug, Clone, Serialize, Deserialize)]
    21	pub struct HalfEdge {
    22	    pub id: EntityId,
    23	    /// Index of the start vertex in the parent solid's vertex list.
    24	    pub start_vertex: usize,
    25	    /// Index of the edge this half-edge belongs to.
    26	    pub edge: usize,
    27	    /// Whether this half-edge follows the edge's natural direction.
    28	    pub forward: bool,
    29	}
    30	
    31	/// An edge — a curve segment bounded by two vertices.
    32	/// Each edge has exactly two half-edges (one for each adjacent face).
    33	#[derive(Debug, Clone, Serialize, Deserialize)]
    34	pub struct Edge {
    35	    pub id: EntityId,
    36	    /// Indices of the two bounding vertices in the parent solid's vertex list.
    37	    pub vertices: [usize; 2],
    38	    /// The geometric curve this edge lies on.
    39	    pub curve: Curve,
    40	    /// Parameter range [t_start, t_end] on the curve.
    41	    pub t_range: [f64; 2],
    42	}
    43	
    44	/// A loop (wire) — a closed sequence of half-edges forming a boundary.
    45	#[derive(Debug, Clone, Serialize, Deserialize)]
    46	pub struct Loop {
    47	    pub id: EntityId,
    48	    /// Ordered indices of half-edges in the parent solid's half-edge list.
    49	    pub half_edges: Vec<usize>,
    50	}
    51	
    52	/// A face — a bounded region on a surface.
    53	/// Has one outer loop and zero or more inner loops (holes).
    54	#[derive(Debug, Clone, Serialize, Deserialize)]
    55	pub struct Face {
    56	    pub id: EntityId,
    57	    /// The geometric surface this face lies on.
    58	    pub surface: Surface,
    59	    /// Index of the outer loop in the parent solid's loop list.
    60	    pub outer_loop: usize,
    61	    /// Indices of inner loops (holes) in the parent solid's loop list.
    62	    pub inner_loops: Vec<usize>,
    63	    /// Whether the face normal matches the surface normal.
    64	    pub same_sense: bool,
    65	}
    66	
    67	/// A shell — a connected set of faces forming a closed or open surface.
    68	#[derive(Debug, Clone, Serialize, Deserialize)]
    69	pub struct Shell {
    70	    pub id: EntityId,
    71	    /// Indices of faces in the parent solid's face list.
    72	    pub faces: Vec<usize>,
    73	    /// Whether this shell is closed (watertight).
    74	    pub closed: bool,
    75	}
    76	
    77	/// A solid — the top-level B-rep entity.
    78	/// Contains all topological entities in flat arrays for deterministic ordering.
    79	#[derive(Debug, Clone, Serialize, Deserialize)]
    80	pub struct Solid {
    81	    pub id: EntityId,
    82	    pub vertices: Vec<Vertex>,
    83	    pub half_edges: Vec<HalfEdge>,
    84	    pub edges: Vec<Edge>,
    85	    pub loops: Vec<Loop>,
    86	    pub faces: Vec<Face>,
    87	    pub shells: Vec<Shell>,
    88	}
    89	
    90	impl Solid {
    91	    /// Create an empty solid with the given ID.
    92	    pub fn new(id: EntityId) -> Self {
    93	        Self {
    94	            id,
    95	            vertices: Vec::new(),
    96	            half_edges: Vec::new(),
    97	            edges: Vec::new(),
    98	            loops: Vec::new(),
    99	            faces: Vec::new(),
   100	            shells: Vec::new(),
   101	        }
   102	    }
   103	
   104	    /// Add a vertex and return its index.
   105	    pub fn add_vertex(&mut self, id: EntityId, point: Point) -> usize {
   106	        let idx = self.vertices.len();
   107	        self.vertices.push(Vertex { id, point });
   108	        idx
   109	    }
   110	
   111	    /// Add an edge and return its index.
   112	    pub fn add_edge(
   113	        &mut self,
   114	        id: EntityId,
   115	        vertices: [usize; 2],
   116	        curve: Curve,
   117	        t_range: [f64; 2],
   118	    ) -> usize {
   119	        let idx = self.edges.len();
   120	        self.edges.push(Edge {
   121	            id,
   122	            vertices,
   123	            curve,
   124	            t_range,
   125	        });
   126	        idx
   127	    }
   128	
   129	    /// Add a half-edge and return its index.
   130	    pub fn add_half_edge(
   131	        &mut self,
   132	        id: EntityId,
   133	        start_vertex: usize,
   134	        edge: usize,
   135	        forward: bool,
   136	    ) -> usize {
   137	        let idx = self.half_edges.len();
   138	        self.half_edges.push(HalfEdge {
   139	            id,
   140	            start_vertex,
   141	            edge,
   142	            forward,
   143	        });
   144	        idx
   145	    }
   146	
   147	    /// Add a loop and return its index.
   148	    pub fn add_loop(&mut self, id: EntityId, half_edges: Vec<usize>) -> usize {
   149	        let idx = self.loops.len();
   150	        self.loops.push(Loop { id, half_edges });
   151	        idx
   152	    }
   153	
   154	    /// Add a face and return its index.
   155	    pub fn add_face(
   156	        &mut self,
   157	        id: EntityId,
   158	        surface: Surface,
   159	        outer_loop: usize,
   160	        inner_loops: Vec<usize>,
   161	        same_sense: bool,
   162	    ) -> usize {
   163	        let idx = self.faces.len();
   164	        self.faces.push(Face {
   165	            id,
   166	            surface,
   167	            outer_loop,
   168	            inner_loops,
   169	            same_sense,
   170	        });
   171	        idx
   172	    }
   173	
   174	    /// Add a shell and return its index.
   175	    pub fn add_shell(&mut self, id: EntityId, faces: Vec<usize>, closed: bool) -> usize {
   176	        let idx = self.shells.len();
   177	        self.shells.push(Shell { id, faces, closed });
   178	        idx
   179	    }
   180	}
   181	
   182	/// Counter for generating deterministic entity IDs.
   183	/// Each feature gets its own counter starting from a feature-specific offset.
   184	#[derive(Debug, Clone)]
   185	pub struct IdGenerator {
   186	    next_id: EntityId,
   187	}
   188	
   189	impl IdGenerator {
   190	    pub fn new(start: EntityId) -> Self {
   191	        Self { next_id: start }
   192	    }
   193	
   194	    #[allow(clippy::should_implement_trait)]
   195	    pub fn next(&mut self) -> EntityId {
   196	        let id = self.next_id;
   197	        self.next_id += 1;
   198	        id
   199	    }
   200	}
   201	
   202	#[cfg(test)]
   203	mod tests {
   204	    use super::*;
   205	
   206	    #[test]
   207	    fn test_id_generator_is_deterministic() {
   208	        let mut gen1 = IdGenerator::new(100);
   209	        let mut gen2 = IdGenerator::new(100);
   210	        for _ in 0..10 {
   211	            assert_eq!(gen1.next(), gen2.next());
   212	        }
   213	    }
   214	
   215	    #[test]
   216	    fn test_solid_add_vertex() {
   217	        let mut solid = Solid::new(0);
   218	        let idx = solid.add_vertex(1, Point::new(1.0, 2.0, 3.0));
   219	        assert_eq!(idx, 0);
   220	        assert_eq!(solid.vertices[0].id, 1);
   221	        assert_eq!(solid.vertices[0].point, Point::new(1.0, 2.0, 3.0));
   222	    }
   223	}

exec
/bin/bash -lc "nl -ba /home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/Cargo.toml | sed -n '1,220p'" in /home/bacon/mycad
 succeeded in 0ms:
     1	# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
     2	#
     3	# When uploading crates to the registry Cargo will automatically
     4	# "normalize" Cargo.toml files for maximal compatibility
     5	# with all versions of Cargo and also rewrite `path` dependencies
     6	# to registry (e.g., crates.io) dependencies.
     7	#
     8	# If you are reading this file be aware that the original Cargo.toml
     9	# will likely look very different (and much more reasonable).
    10	# See Cargo.toml.orig for the original contents.
    11	
    12	[package]
    13	edition = "2018"
    14	rust-version = "1.64.0"
    15	name = "tower"
    16	version = "0.5.3"
    17	authors = ["Tower Maintainers <team@tower-rs.com>"]
    18	build = false
    19	autolib = false
    20	autobins = false
    21	autoexamples = false
    22	autotests = false
    23	autobenches = false
    24	description = """
    25	Tower is a library of modular and reusable components for building robust
    26	clients and servers.
    27	"""
    28	homepage = "https://github.com/tower-rs/tower"
    29	readme = "README.md"
    30	keywords = [
    31	    "io",
    32	    "async",
    33	    "non-blocking",
    34	    "futures",
    35	    "service",
    36	]
    37	categories = [
    38	    "asynchronous",
    39	    "network-programming",
    40	]
    41	license = "MIT"
    42	repository = "https://github.com/tower-rs/tower"
    43	
    44	[package.metadata.docs.rs]
    45	all-features = true
    46	rustdoc-args = [
    47	    "--cfg",
    48	    "docsrs",
    49	]
    50	
    51	[package.metadata.playground]
    52	features = ["full"]
    53	
    54	[features]
    55	balance = [
    56	    "discover",
    57	    "load",
    58	    "ready-cache",
    59	    "make",
    60	    "slab",
    61	    "util",
    62	]
    63	buffer = [
    64	    "tokio/sync",
    65	    "tokio/rt",
    66	    "tokio-util",
    67	    "tracing",
    68	    "pin-project-lite",
    69	]
    70	discover = [
    71	    "futures-core",
    72	    "pin-project-lite",
    73	]
    74	filter = [
    75	    "futures-util",
    76	    "pin-project-lite",
    77	]
    78	full = [
    79	    "balance",
    80	    "buffer",
    81	    "discover",
    82	    "filter",
    83	    "hedge",
    84	    "limit",
    85	    "load",
    86	    "load-shed",
    87	    "make",
    88	    "ready-cache",
    89	    "reconnect",
    90	    "retry",
    91	    "spawn-ready",
    92	    "steer",
    93	    "timeout",
    94	    "util",
    95	]
    96	hedge = [
    97	    "util",
    98	    "filter",
    99	    "futures-util",
   100	    "hdrhistogram",
   101	    "tokio/time",
   102	    "tracing",
   103	]
   104	limit = [
   105	    "tokio/time",
   106	    "tokio/sync",
   107	    "tokio-util",
   108	    "tracing",
   109	    "pin-project-lite",
   110	]
   111	load = [
   112	    "tokio/time",
   113	    "tracing",
   114	    "pin-project-lite",
   115	]
   116	load-shed = ["pin-project-lite"]
   117	log = ["tracing/log"]
   118	make = [
   119	    "pin-project-lite",
   120	    "tokio",
   121	]
   122	ready-cache = [
   123	    "futures-core",
   124	    "futures-util",
   125	    "indexmap",
   126	    "tokio/sync",
   127	    "tracing",
   128	    "pin-project-lite",
   129	]
   130	reconnect = [
   131	    "make",
   132	    "tracing",
   133	]
   134	retry = [
   135	    "tokio/time",
   136	    "util",
   137	]
   138	spawn-ready = [
   139	    "futures-util",
   140	    "tokio/sync",
   141	    "tokio/rt",
   142	    "util",
   143	    "tracing",
   144	]
   145	steer = []
   146	timeout = [
   147	    "pin-project-lite",
   148	    "tokio/time",
   149	]
   150	tokio-stream = []
   151	util = [
   152	    "futures-core",
   153	    "futures-util",
   154	    "pin-project-lite",
   155	    "sync_wrapper",
   156	]
   157	
   158	[lib]
   159	name = "tower"
   160	path = "src/lib.rs"
   161	
   162	[[example]]
   163	name = "tower-balance"
   164	path = "examples/tower-balance.rs"
   165	required-features = ["full"]
   166	
   167	[[test]]
   168	name = "balance"
   169	path = "tests/balance/main.rs"
   170	
   171	[[test]]
   172	name = "buffer"
   173	path = "tests/buffer/main.rs"
   174	
   175	[[test]]
   176	name = "builder"
   177	path = "tests/builder.rs"
   178	
   179	[[test]]
   180	name = "hedge"
   181	path = "tests/hedge/main.rs"
   182	
   183	[[test]]
   184	name = "limit"
   185	path = "tests/limit/main.rs"
   186	
   187	[[test]]
   188	name = "load_shed"
   189	path = "tests/load_shed/main.rs"
   190	
   191	[[test]]
   192	name = "ready_cache"
   193	path = "tests/ready_cache/main.rs"
   194	
   195	[[test]]
   196	name = "retry"
   197	path = "tests/retry/main.rs"
   198	
   199	[[test]]
   200	name = "spawn_ready"
   201	path = "tests/spawn_ready/main.rs"
   202	
   203	[[test]]
   204	name = "steer"
   205	path = "tests/steer/main.rs"
   206	
   207	[[test]]
   208	name = "support"
   209	path = "tests/support.rs"
   210	
   211	[[test]]
   212	name = "util"
   213	path = "tests/util/main.rs"
   214	
   215	[dependencies.futures-core]
   216	version = "0.3.22"
   217	optional = true
   218	
   219	[dependencies.futures-util]
   220	version = "0.3.22"

exec
/bin/bash -lc "rg -n \"ServiceExt|oneshot|feature = \\\"util\\\"|cfg.*util\" /home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src -g '*.rs'" in /home/bacon/mycad
 succeeded in 0ms:
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/macros.rs:2:    feature = "util",
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/map_result.rs:9:/// [`map_result`]: crate::util::ServiceExt::map_result
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/mod.rs:14://! # #[cfg(feature = "util")]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/mod.rs:16://! # #[cfg(feature = "util")]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/mod.rs:17://! use tower::{Service, ServiceExt};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/mod.rs:18://! # #[cfg(feature = "util")]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/message.rs:2:use tokio::sync::oneshot;
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/message.rs:13:pub(crate) type Tx<Fut> = oneshot::Sender<Result<Fut, ServiceError>>;
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/message.rs:16:pub(crate) type Rx<Fut> = oneshot::Receiver<Result<Fut, ServiceError>>;
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/steer/mod.rs:16://! # use tower::ServiceExt;
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:154:    #[cfg(feature = "util")]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:315:    /// use tower::ServiceExt;
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:334:    /// usize_svc.oneshot(42).await?;
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:343:    /// use tower::ServiceExt;
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:357:    /// let response = svc.oneshot(1).await?;
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:364:    #[cfg(feature = "util")]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:383:    /// [`map_response` combinator]: crate::util::ServiceExt::map_response
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:384:    #[cfg(feature = "util")]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:400:    /// [`map_err` combinator]: crate::util::ServiceExt::map_err
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:401:    #[cfg(feature = "util")]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:413:    /// [`map_future`]: crate::util::ServiceExt::map_future
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:414:    #[cfg(feature = "util")]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:434:    /// [`then` combinator]: crate::util::ServiceExt::then
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:437:    #[cfg(feature = "util")]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:456:    /// [`and_then` combinator]: crate::util::ServiceExt::and_then
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:458:    #[cfg(feature = "util")]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:472:    /// [`map_result` combinator]: crate::util::ServiceExt::map_result
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:474:    #[cfg(feature = "util")]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:516:    /// use tower::{ServiceBuilder, ServiceExt, BoxError, service_fn};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:529:    /// let response = svc.oneshot("foo").await?;
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:539:    #[cfg(feature = "util")]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:630:    /// use tower::{Service, ServiceExt};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:705:    #[cfg(feature = "util")]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:768:    #[cfg(feature = "util")]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:831:    #[cfg(feature = "util")]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/call_all/ordered.rs:31:    /// use tower::util::ServiceExt;
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/worker.rs:82:            // If the oneshot sender is closed, then the receiver is dropped,
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/worker.rs:114:        // receives an error on the `oneshot` it constructed. Specifically, we want to avoid the
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/call_all/common.rs:12:    /// The [`Future`] returned by the [`ServiceExt::call_all`] combinator.
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/service.rs:11:use tokio::sync::{mpsc, oneshot};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/service.rs:119:        // acquired, so we can freely allocate a oneshot.
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/service.rs:120:        let (tx, rx) = oneshot::channel();
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/load/mod.rs:35://! # #[cfg(feature = "util")]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/load/mod.rs:36://! use tower::util::ServiceExt;
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/load/mod.rs:37://! # #[cfg(feature = "util")]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/load/mod.rs:39://! # #[cfg(feature = "util")]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/ready_cache/cache.rs:70:    /// The cancelation oneshot is preserved (though unused) while the service is
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/ready_cache/cache.rs:415:/// A `tokio::sync::oneshot` is NOT used, as a `Receiver` is not guaranteed to
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/spawn_ready/service.rs:2:use crate::{util::ServiceExt, BoxError};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/spawn_ready/service.rs:68:                        tokio::spawn(svc.ready_oneshot().map_err(Into::into).in_current_span());
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:17:mod oneshot;
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:39:    oneshot::Oneshot,
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:71:pub trait ServiceExt<Request>: tower_service::Service<Request> {
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:81:    fn ready_oneshot(self) -> ReadyOneshot<Self, Request>
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:89:    fn oneshot(self, req: Request) -> Oneshot<Self, Request>
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:125:    /// # use tower::{Service, ServiceExt};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:194:    /// # use tower::{Service, ServiceExt};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:261:    /// # use tower::{Service, ServiceExt};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:351:    /// # use tower::{Service, ServiceExt};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:414:    /// # use tower::{Service, ServiceExt};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:485:    /// # use tower::{Service, ServiceExt};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:531:    /// [`map_response`]: ServiceExt::map_response
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:532:    /// [`map_err`]: ServiceExt::map_err
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:533:    /// [`map_result`]: ServiceExt::map_result
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:556:    /// # use tower::{Service, ServiceExt};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:616:    /// # use tower::{Service, ServiceExt};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:689:    /// # use tower::{Service, ServiceExt};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:778:    /// combinator, [`ServiceExt::then`] can be used to implement asynchronous
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:796:    /// # use tower::{Service, ServiceExt};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:882:    /// # use tower::{Service, ServiceExt, BoxError};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:958:    /// use tower::{Service, ServiceExt, BoxError, service_fn, util::BoxService};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:1004:    /// use tower::{Service, ServiceExt, BoxError, service_fn, util::BoxCloneService};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:1045:impl<T: ?Sized, Request> ServiceExt<Request> for T where T: tower_service::Service<Request> {}
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/map_response.rs:9:/// [`map_response`]: crate::util::ServiceExt::map_response
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/lib.rs:154://! [utilities]: crate::ServiceExt
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/lib.rs:196:#[cfg(feature = "util")]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/lib.rs:202:#[cfg(feature = "util")]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/lib.rs:204:#[cfg_attr(docsrs, doc(cfg(feature = "util")))]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/lib.rs:205:pub use self::util::{service_fn, ServiceExt};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/boxed_clone.rs:1:use super::ServiceExt;
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/balance/mod.rs:19://! # #[cfg(feature = "util")]
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/balance/mod.rs:24://! use tower::{Service, ServiceExt};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/and_then.rs:12:/// [`and_then`]: crate::util::ServiceExt::and_then
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/service_fn.rs:13:/// use tower::{service_fn, Service, ServiceExt, BoxError};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/boxed_clone_sync.rs:1:use super::ServiceExt;
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/map_future.rs:11:/// [`map_future`]: crate::util::ServiceExt::map_future
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/future_service.rs:16:/// use tower::{service_fn, Service, ServiceExt};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/future_service.rs:72:    /// use tower::{service_fn, Service, ServiceExt};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/future_service.rs:177:    use crate::util::{future_service, ServiceExt};
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/map_err.rs:9:/// [`map_err`]: crate::util::ServiceExt::map_err
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/then.rs:12:/// [`then`]: crate::util::ServiceExt::then
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/boxed/sync.rs:1:use crate::ServiceExt;
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/map_request.rs:8:/// [`MapRequest`]: crate::util::ServiceExt::map_request
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/ready.rs:12:/// [`ReadyOneshot`] values are produced by [`ServiceExt::ready_oneshot`].
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/ready.rs:14:/// [`ServiceExt::ready_oneshot`]: crate::util::ServiceExt::ready_oneshot
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/ready.rs:66:/// [`Ready`] values are produced by [`ServiceExt::ready`].
/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/ready.rs:68:/// [`ServiceExt::ready`]: crate::util::ServiceExt::ready

codex
一通り見えたので、既存レビュー記録と照合して見落としがないかだけ確認します。差分に直接関係する未解消事項だけ拾います。
exec
/bin/bash -lc "rg -n \"finding:|severity:|ApiError|Host|tower|ServiceExt|thiserror|MeshQuery\" features/18-api-skeleton/final-review.md features/18-api-skeleton/design-review-3.md features/18-api-skeleton/design-review-2.md -g '*.md'" in /home/bacon/mycad
 succeeded in 0ms:
features/18-api-skeleton/design-review-3.md:38:- ライブラリエラーには `thiserror` を使用しているか
features/18-api-skeleton/design-review-3.md:59:    severity: critical  # critical | high | medium | low
features/18-api-skeleton/design-review-3.md:61:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
features/18-api-skeleton/design-review-3.md:64:    severity: medium
features/18-api-skeleton/design-review-3.md:101:| `Cargo.toml`(ルート) | members に `"crates/mycad-api"` 追加。`[workspace.dependencies]` に `axum`/`tokio`/`serde_json`/`tower`(dev) 追加 |
features/18-api-skeleton/design-review-3.md:106:| `crates/mycad-api/src/error.rs` | 新規。`ApiError` + `IntoResponse` |
features/18-api-skeleton/design-review-3.md:108:| `crates/mycad-api/tests/mesh_api.rs` | 新規。tower `oneshot` 統合テスト + fixtures |
features/18-api-skeleton/design-review-3.md:119:struct MeshQuery { file: String }
features/18-api-skeleton/design-review-3.md:120:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
features/18-api-skeleton/design-review-3.md:123:enum ApiError {
features/18-api-skeleton/design-review-3.md:129:impl From<FormatError>      for ApiError { /* variant 別に 400/404/500 */ }
features/18-api-skeleton/design-review-3.md:130:impl From<KernelError>      for ApiError { /* 全 variant → 422 */ }
features/18-api-skeleton/design-review-3.md:131:impl From<TessellationError> for ApiError { /* 全 variant → 422 */ }
features/18-api-skeleton/design-review-3.md:132:impl IntoResponse for ApiError { /* status は variant から、body = {"error": msg} */ }
features/18-api-skeleton/design-review-3.md:158:5. **Host ヘッダ検証ミドルウェア** → loopback 以外は 403(DNS リバインディング対策)。
features/18-api-skeleton/design-review-3.md:161:- `ApiError` は 4 variant(`BadRequest`/`NotFound`/`Unprocessable`/`Internal`)。各下層エラーから `From` で変換し、`IntoResponse` は **variant から status を決め**、body は `{"error": msg}` に整形するだけ。
features/18-api-skeleton/design-review-3.md:169:  - `reference.is_some()` または `!children.is_empty()` → assembly/reference 文書 → `ApiError::Unprocessable("assembly/reference は v0 非対応")`。
features/18-api-skeleton/design-review-3.md:174:- `[workspace.dependencies]` に新規登録: `axum = "0.8"`、`tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }`、`serde_json = "1"`、`tower = "0.5"`(dev、`oneshot`)。各クレートは `{ workspace = true }`。
features/18-api-skeleton/design-review-3.md:175:- derive 規約・`thiserror`・`version.workspace = true` を踏襲。clippy `-D warnings` 通過必須。
features/18-api-skeleton/design-review-3.md:186:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/design-review-3.md:297:use thiserror::Error;
features/18-api-skeleton/design-review-3.md:463:use thiserror::Error;
features/18-api-skeleton/design-review-3.md:492:use thiserror::Error;
features/18-api-skeleton/design-review-3.md:1863:    severity: high
features/18-api-skeleton/design-review-3.md:1865:    finding: "相対 `file` を `std::env::current_dir()` 基準で解決するため、同じ `GET /api/v0/mesh?file=examples/simple_box.mycad` でもサーバの起動 CWD によって参照先と結果が変わる。これは MyCad の『同一入力→同一出力』に対する隠れた環境依存になる。"
features/18-api-skeleton/design-review-3.md:1869:    severity: high
features/18-api-skeleton/design-review-3.md:1871:    finding: "`tessellate_solid_with(..., &TessellationOptions::default())` を API 契約にしており、kernel 側の `Default` 値変更がそのまま `/api/v0` の三角形数・頂点順序を変えうる。URL で `v0` を切っていてもメッシュ仕様が静かにドリフトする。"
features/18-api-skeleton/design-review-3.md:1875:    severity: medium
features/18-api-skeleton/design-review-3.md:1877:    finding: "`FormatError::InvalidReference` を 500 に分類しているが、これは `ref` 文字列が空/不正なユーザー入力で発生する既知の文書不正であり、内部障害ではない。クライアント起因の失敗を 500 にすると原因切り分けと再試行判定を誤る。"
features/18-api-skeleton/design-review-3.md:1881:    severity: medium
features/18-api-skeleton/design-review-3.md:1883:    finding: "設計本文では assembly/reference を build 前に検出して専用の 422 にするとしている一方、T09 の期待値は `422(EmptyFeatureList)` のままで矛盾している。これでは assembly と空 part を区別しない旧挙動でもテストが通る。"
features/18-api-skeleton/design-review-3.md:1887:    severity: low
features/18-api-skeleton/design-review-3.md:1889:    finding: "`make_cuboid` の新仕様は `non-finite` を拒否すると書いているが、K01 の列挙は `0.0`/負値/`NaN` までで、`INFINITY`/`-INFINITY` の分岐が未検証である。"
features/18-api-skeleton/design-review-3.md:1897:    severity: high
features/18-api-skeleton/design-review-3.md:1899:    finding: "相対 `file` を `std::env::current_dir()` 基準で解決するため、同じ `GET /api/v0/mesh?file=examples/simple_box.mycad` でもサーバの起動 CWD によって参照先と結果が変わる。これは MyCad の『同一入力→同一出力』に対する隠れた環境依存になる。"
features/18-api-skeleton/design-review-3.md:1903:    severity: high
features/18-api-skeleton/design-review-3.md:1905:    finding: "`tessellate_solid_with(..., &TessellationOptions::default())` を API 契約にしており、kernel 側の `Default` 値変更がそのまま `/api/v0` の三角形数・頂点順序を変えうる。URL で `v0` を切っていてもメッシュ仕様が静かにドリフトする。"
features/18-api-skeleton/design-review-3.md:1909:    severity: medium
features/18-api-skeleton/design-review-3.md:1911:    finding: "`FormatError::InvalidReference` を 500 に分類しているが、これは `ref` 文字列が空/不正なユーザー入力で発生する既知の文書不正であり、内部障害ではない。クライアント起因の失敗を 500 にすると原因切り分けと再試行判定を誤る。"
features/18-api-skeleton/design-review-3.md:1915:    severity: medium
features/18-api-skeleton/design-review-3.md:1917:    finding: "設計本文では assembly/reference を build 前に検出して専用の 422 にするとしている一方、T09 の期待値は `422(EmptyFeatureList)` のままで矛盾している。これでは assembly と空 part を区別しない旧挙動でもテストが通る。"
features/18-api-skeleton/design-review-3.md:1921:    severity: low
features/18-api-skeleton/design-review-3.md:1923:    finding: "`make_cuboid` の新仕様は `non-finite` を拒否すると書いているが、K01 の列挙は `0.0`/負値/`NaN` までで、`INFINITY`/`-INFINITY` の分岐が未検証である。"
features/18-api-skeleton/design-review-2.md:38:- ライブラリエラーには `thiserror` を使用しているか
features/18-api-skeleton/design-review-2.md:59:    severity: critical  # critical | high | medium | low
features/18-api-skeleton/design-review-2.md:61:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
features/18-api-skeleton/design-review-2.md:64:    severity: medium
features/18-api-skeleton/design-review-2.md:101:| `Cargo.toml`(ルート) | members に `"crates/mycad-api"` 追加。`[workspace.dependencies]` に `axum`/`tokio`/`serde_json`/`tower`(dev) 追加 |
features/18-api-skeleton/design-review-2.md:106:| `crates/mycad-api/src/error.rs` | 新規。`ApiError` + `IntoResponse` |
features/18-api-skeleton/design-review-2.md:108:| `crates/mycad-api/tests/mesh_api.rs` | 新規。tower `oneshot` 統合テスト + fixtures |
features/18-api-skeleton/design-review-2.md:119:struct MeshQuery { file: String }
features/18-api-skeleton/design-review-2.md:120:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
features/18-api-skeleton/design-review-2.md:123:enum ApiError { BadExtension(String), NotFound, Unsupported(String), Build(String), Tessellate(String), Internal(String) }
features/18-api-skeleton/design-review-2.md:124:impl IntoResponse for ApiError { /* status + {"error": "..."} */ }
features/18-api-skeleton/design-review-2.md:150:5. **Host ヘッダ検証ミドルウェア** → loopback 以外は 403(DNS リバインディング対策)。
features/18-api-skeleton/design-review-2.md:153:- `FormatError`/`KernelError`/`TessellationError` は `Serialize` 非対応 → `ApiError` に `.to_string()` で集約し `IntoResponse` で status + `{"error": "..."}`。
features/18-api-skeleton/design-review-2.md:157:- `[workspace.dependencies]` に新規登録: `axum = "0.8"`、`tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }`、`serde_json = "1"`、`tower = "0.5"`(dev、`oneshot`)。各クレートは `{ workspace = true }`。
features/18-api-skeleton/design-review-2.md:158:- derive 規約・`thiserror`・`version.workspace = true` を踏襲。clippy `-D warnings` 通過必須。
features/18-api-skeleton/design-review-2.md:169:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/design-review-2.md:476:/bin/bash -lc 'rg -n "run_export|make_cuboid|tessellate_solid_with|test_assembly_roundtrip|workspace.dependencies|thiserror|TriangleMesh|build_solid_from_features|Document::from_path|InvalidExtension|make_cylinder|Euler|HalfEdge|Shell|Loop|IdGenerator" -S .' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:479:./Cargo.toml:30:thiserror = "2"
features/18-api-skeleton/design-review-2.md:487:./.claude/skills/3ai/SKILL.md:62:- エラーハンドリング: thiserror の使い方
features/18-api-skeleton/design-review-2.md:494:./.claude/skills/3ai/agents/glm-implementer.md:35:- ライブラリエラーは `thiserror` で定義
features/18-api-skeleton/design-review-2.md:503:./.claude/skills/3ai/agents/codex-final-reviewer.md:26:- エラー型が `thiserror` で定義されているか
features/18-api-skeleton/design-review-2.md:504:./.claude/skills/3ai/agents/codex-final-reviewer.md:48:    finding: "HalfEdge の twin インデックスが self を指している（ループ不整合）"
features/18-api-skeleton/design-review-2.md:506:./crates/mycad-format/Cargo.toml:12:thiserror = { workspace = true }
features/18-api-skeleton/design-review-2.md:513:./.claude/skills/3ai/agents/codex-design-reviewer.md:25:- ライブラリエラーには `thiserror` を使用しているか
features/18-api-skeleton/design-review-2.md:516:./.claude/skills/3ai/agents/codex-design-reviewer.md:48:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
features/18-api-skeleton/design-review-2.md:519:./Cargo.lock:306: "thiserror",
features/18-api-skeleton/design-review-2.md:520:./Cargo.lock:317: "thiserror",
features/18-api-skeleton/design-review-2.md:521:./Cargo.lock:632:name = "thiserror"
features/18-api-skeleton/design-review-2.md:522:./Cargo.lock:637: "thiserror-impl",
features/18-api-skeleton/design-review-2.md:523:./Cargo.lock:641:name = "thiserror-impl"
features/18-api-skeleton/design-review-2.md:530:./features/12-cylinder/design-review.md:38:- ライブラリエラーには `thiserror` を使用しているか
features/18-api-skeleton/design-review-2.md:533:./features/12-cylinder/design-review.md:61:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
features/18-api-skeleton/design-review-2.md:552:./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:554:./features/12-cylinder/design-review.md:405:Cargo.toml:30:thiserror = "2"
features/18-api-skeleton/design-review-2.md:555:./features/12-cylinder/design-review.md:406:crates/mycad-format/Cargo.toml:12:thiserror = { workspace = true }
features/18-api-skeleton/design-review-2.md:562:./features/12-cylinder/design-review.md:414:.claude/skills/3ai/agents/glm-implementer.md:35:- ライブラリエラーは `thiserror` で定義
features/18-api-skeleton/design-review-2.md:565:./features/12-cylinder/design-review.md:418:.claude/skills/3ai/SKILL.md:55:- エラーハンドリング: thiserror の使い方
features/18-api-skeleton/design-review-2.md:572:./features/12-cylinder/design-review.md:426:CLAUDE.md:56:- **エラーハンドリング**: ライブラリエラーには `thiserror` を使用。
features/18-api-skeleton/design-review-2.md:577:./features/12-cylinder/design-review.md:432:.claude/skills/3ai/agents/codex-final-reviewer.md:26:- エラー型が `thiserror` で定義されているか
features/18-api-skeleton/design-review-2.md:578:./features/12-cylinder/design-review.md:433:.claude/skills/3ai/agents/codex-final-reviewer.md:48:    finding: "HalfEdge の twin インデックスが self を指している（ループ不整合）"
features/18-api-skeleton/design-review-2.md:585:./features/12-cylinder/design-review.md:445:features/12-cylinder/design-review.md:38:- ライブラリエラーには `thiserror` を使用しているか
features/18-api-skeleton/design-review-2.md:587:./features/12-cylinder/design-review.md:448:features/12-cylinder/design-review.md:61:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
features/18-api-skeleton/design-review-2.md:601:./features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:606:./features/12-cylinder/design-review.md:477:.claude/skills/3ai/agents/codex-design-reviewer.md:25:- ライブラリエラーには `thiserror` を使用しているか
features/18-api-skeleton/design-review-2.md:608:./features/12-cylinder/design-review.md:480:.claude/skills/3ai/agents/codex-design-reviewer.md:48:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
features/18-api-skeleton/design-review-2.md:610:./features/12-cylinder/design-review.md:482:Cargo.lock:306: "thiserror",
features/18-api-skeleton/design-review-2.md:611:./features/12-cylinder/design-review.md:483:Cargo.lock:317: "thiserror",
features/18-api-skeleton/design-review-2.md:612:./features/12-cylinder/design-review.md:484:Cargo.lock:632:name = "thiserror"
features/18-api-skeleton/design-review-2.md:613:./features/12-cylinder/design-review.md:485:Cargo.lock:637: "thiserror-impl",
features/18-api-skeleton/design-review-2.md:614:./features/12-cylinder/design-review.md:486:Cargo.lock:641:name = "thiserror-impl"
features/18-api-skeleton/design-review-2.md:615:./features/12-cylinder/design-review.md:491:crates/mycad-format/src/error.rs:1:use thiserror::Error;
features/18-api-skeleton/design-review-2.md:617:./features/12-cylinder/design-review.md:498:crates/mycad-kernel/Cargo.toml:11:thiserror = { workspace = true }
features/18-api-skeleton/design-review-2.md:661:./features/12-cylinder/design-review.md:564:crates/mycad-kernel/src/error.rs:1:use thiserror::Error;
features/18-api-skeleton/design-review-2.md:737:./features/12-cylinder/design-review.md:1851:thiserror = "2"
features/18-api-skeleton/design-review-2.md:772:./features/12-cylinder/design-review.md:2479:- ライブラリエラーには `thiserror` を使用しているか
features/18-api-skeleton/design-review-2.md:775:./features/12-cylinder/design-review.md:2502:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
features/18-api-skeleton/design-review-2.md:777:./features/12-cylinder/design-review.md:2520:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/design-review-2.md:779:./features/12-cylinder/design-review.md:2526:    suggestion: "primitive 生成時と `TessellationOptions` 構築時に下限チェックを入れ、違反時は `thiserror` ベースの `KernelError` で明示エラーにすること。CLI でも同じ境界値を拒否すること。"
features/18-api-skeleton/design-review-2.md:780:./features/12-cylinder/design-review.md:2530:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/design-review-2.md:783:./features/12-cylinder/design-review.md:2550:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/design-review-2.md:785:./features/12-cylinder/design-review.md:2556:    suggestion: "primitive 生成時と `TessellationOptions` 構築時に下限チェックを入れ、違反時は `thiserror` ベースの `KernelError` で明示エラーにすること。CLI でも同じ境界値を拒否すること。"
features/18-api-skeleton/design-review-2.md:786:./features/12-cylinder/design-review.md:2560:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/design-review-2.md:790:./crates/mycad-format/src/error.rs:1:use thiserror::Error;
features/18-api-skeleton/design-review-2.md:800:./features/12-cylinder/design-review-2.md:38:- ライブラリエラーには `thiserror` を使用しているか
features/18-api-skeleton/design-review-2.md:803:./features/12-cylinder/design-review-2.md:61:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
features/18-api-skeleton/design-review-2.md:817:./features/12-cylinder/design-review-2.md:173:- `KernelError`（既存 `thiserror`）に `InvalidParameter { kind: &'static str }` を 1 variant 追加（R02 の境界検証用）。
features/18-api-skeleton/design-review-2.md:885:./features/12-cylinder/design-review-2.md:1114:thiserror = "2"
features/18-api-skeleton/design-review-2.md:899:./features/12-cylinder/design-review-2.md:1354:use thiserror::Error;
features/18-api-skeleton/design-review-2.md:900:./features/12-cylinder/design-review-2.md:1381:thiserror = { workspace = true }
features/18-api-skeleton/design-review-2.md:901:./features/12-cylinder/design-review-2.md:1405:thiserror = { workspace = true }
features/18-api-skeleton/design-review-2.md:922:./features/12-cylinder/design-review-2.md:1863:./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:923:./features/12-cylinder/design-review-2.md:1875:./features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:973:./features/12-cylinder/design-review-2.md:2307:    finding: "「cuboid は既に外向きなのでスワップは発生しない／平面出力不変」という前提が既存 `make_cuboid` と整合していない。現状の cuboid ループ順では front face `0→1→3→2` などの幾何法線が outward normal と逆向きで、提案どおりの法線整合スワップを入れると既存 STL/indices は変わる。"
features/18-api-skeleton/design-review-2.md:974:./features/12-cylinder/design-review-2.md:2312:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/18-api-skeleton/design-review-2.md:976:./features/12-cylinder/design-review-2.md:2317:    finding: "退化入力の排除が `build_solid_from_features` 境界にしかなく、公開 API `make_cylinder(radius, height, id_gen) -> Solid` 自体は `radius<=0` / `height<=0` を受けても不正 B-rep を返せてしまう。`mycad-build` を経由しない kernel 利用では不変条件を守れない。"
features/18-api-skeleton/design-review-2.md:977:./features/12-cylinder/design-review-2.md:2332:    finding: "「cuboid は既に外向きなのでスワップは発生しない／平面出力不変」という前提が既存 `make_cuboid` と整合していない。現状の cuboid ループ順では front face `0→1→3→2` などの幾何法線が outward normal と逆向きで、提案どおりの法線整合スワップを入れると既存 STL/indices は変わる。"
features/18-api-skeleton/design-review-2.md:978:./features/12-cylinder/design-review-2.md:2337:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/18-api-skeleton/design-review-2.md:980:./features/12-cylinder/design-review-2.md:2342:    finding: "退化入力の排除が `build_solid_from_features` 境界にしかなく、公開 API `make_cylinder(radius, height, id_gen) -> Solid` 自体は `radius<=0` / `height<=0` を受けても不正 B-rep を返せてしまう。`mycad-build` を経由しない kernel 利用では不変条件を守れない。"
features/18-api-skeleton/design-review-2.md:989:./features/12-cylinder/final-review-2.md:56: use thiserror::Error;
features/18-api-skeleton/design-review-2.md:1079:./features/12-cylinder/design-review-3.md:38:- ライブラリエラーには `thiserror` を使用しているか
features/18-api-skeleton/design-review-2.md:1082:./features/12-cylinder/design-review-3.md:61:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
features/18-api-skeleton/design-review-2.md:1096:./features/12-cylinder/design-review-3.md:173:- `KernelError`（既存 `thiserror`）に `InvalidParameter { kind: &'static str }` を 1 variant 追加（R02 の境界検証用）。
features/18-api-skeleton/design-review-2.md:1210:./features/12-cylinder/design-review-3.md:1292:use thiserror::Error;
features/18-api-skeleton/design-review-2.md:1258:./features/12-cylinder/design-review-3.md:1974:thiserror = "2"
features/18-api-skeleton/design-review-2.md:1259:./features/12-cylinder/design-review-3.md:1995:thiserror = { workspace = true }
features/18-api-skeleton/design-review-2.md:1260:./features/12-cylinder/design-review-3.md:2042:/bin/bash -lc "rg -n \"approx =|thiserror =|schemars =|serde_yaml =|tempfile =|workspace.dependencies|\\[workspace.dependencies\\]\" Cargo.toml crates/*/Cargo.toml" in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:1261:./features/12-cylinder/design-review-3.md:2046:crates/mycad-format/Cargo.toml:12:thiserror = { workspace = true }
features/18-api-skeleton/design-review-2.md:1262:./features/12-cylinder/design-review-3.md:2048:crates/mycad-kernel/Cargo.toml:11:thiserror = { workspace = true }
features/18-api-skeleton/design-review-2.md:1264:./features/12-cylinder/design-review-3.md:2055:Cargo.toml:30:thiserror = "2"
features/18-api-skeleton/design-review-2.md:1275:./features/12-cylinder/design-review-3.md:2162:    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
features/18-api-skeleton/design-review-2.md:1279:./features/12-cylinder/design-review-3.md:2192:    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
features/18-api-skeleton/design-review-2.md:1306:./features/12-cylinder/final-review.md:215: use thiserror::Error;
features/18-api-skeleton/design-review-2.md:1336:./features/12-cylinder/final-review.md:960:+use thiserror::Error;
features/18-api-skeleton/design-review-2.md:1433:./features/12-cylinder/final-review.md:2620:use thiserror::Error;
features/18-api-skeleton/design-review-2.md:1514:./features/12-cylinder/final-review.md:3710:2162:    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
features/18-api-skeleton/design-review-2.md:1516:./features/12-cylinder/final-review.md:3713:2192:    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
features/18-api-skeleton/design-review-2.md:1522:./features/12-cylinder/final-review.md:3751:    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
features/18-api-skeleton/design-review-2.md:1526:./features/12-cylinder/final-review.md:3781:    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
features/18-api-skeleton/design-review-2.md:1532:./features/12-cylinder/final-review.md:3836:    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
features/18-api-skeleton/design-review-2.md:1536:./features/12-cylinder/final-review.md:3866:    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
features/18-api-skeleton/design-review-2.md:1538:./features/12-cylinder/final-review.md:3883:     9	use thiserror::Error;
features/18-api-skeleton/design-review-2.md:1579:./crates/mycad-kernel/Cargo.toml:11:thiserror = { workspace = true }
features/18-api-skeleton/design-review-2.md:1605:./CLAUDE.md:56:- **エラーハンドリング**: ライブラリエラーには `thiserror` を使用。
features/18-api-skeleton/design-review-2.md:1607:./crates/mycad-kernel/src/error.rs:1:use thiserror::Error;
features/18-api-skeleton/design-review-2.md:1608:./crates/mycad-kernel/src/tessellation/mod.rs:10:use thiserror::Error;
features/18-api-skeleton/design-review-2.md:1667:./features/18-api-skeleton/design-review.md:38:- ライブラリエラーには `thiserror` を使用しているか
features/18-api-skeleton/design-review-2.md:1670:./features/18-api-skeleton/design-review.md:61:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
features/18-api-skeleton/design-review-2.md:1674:./features/18-api-skeleton/design-review.md:92:| `Cargo.toml`(ルート) | members に `"crates/mycad-api"` 追加。`[workspace.dependencies]` に `axum`/`tokio`/`serde_json`/`tower`(dev) 追加 |
features/18-api-skeleton/design-review-2.md:1675:./features/18-api-skeleton/design-review.md:107:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
features/18-api-skeleton/design-review-2.md:1682:./features/18-api-skeleton/design-review.md:138:- `[workspace.dependencies]` に追加(前例ゼロなので新規登録): `axum = "0.8"`、`tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }`、`serde_json = "1"`、`tower = "0.5"`(dev、`oneshot` 用)。各クレートは `{ workspace = true }` で参照。
features/18-api-skeleton/design-review-2.md:1683:./features/18-api-skeleton/design-review.md:139:- derive 規約・`thiserror`・`version.workspace = true` 等の既存慣例を踏襲。
features/18-api-skeleton/design-review-2.md:1687:./features/18-api-skeleton/design-review.md:213:/bin/bash -lc 'rg -n "struct IdGenerator|pub struct IdGenerator|run_export|build_solid_from_features|tessellate_solid_with|struct TriangleMesh|enum FormatError|pub enum FormatError|JsonSchema|thiserror|workspace.dependencies|mod.rs|HalfEdge|Euler|Serialize|Deserialize" -S Cargo.toml crates' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:1689:./features/18-api-skeleton/design-review.md:216:Cargo.toml:30:thiserror = "2"
features/18-api-skeleton/design-review-2.md:1690:./features/18-api-skeleton/design-review.md:217:crates/mycad-format/Cargo.toml:12:thiserror = { workspace = true }
features/18-api-skeleton/design-review-2.md:1692:./features/18-api-skeleton/design-review.md:232:crates/mycad-format/src/error.rs:1:use thiserror::Error;
features/18-api-skeleton/design-review-2.md:1696:./features/18-api-skeleton/design-review.md:243:crates/mycad-kernel/Cargo.toml:11:thiserror = { workspace = true }
features/18-api-skeleton/design-review-2.md:1704:./features/18-api-skeleton/design-review.md:251:crates/mycad-kernel/src/error.rs:1:use thiserror::Error;
features/18-api-skeleton/design-review-2.md:1705:./features/18-api-skeleton/design-review.md:253:crates/mycad-kernel/src/tessellation/mod.rs:10:use thiserror::Error;
features/18-api-skeleton/design-review-2.md:1757:./features/18-api-skeleton/design-review.md:773:use thiserror::Error;
features/18-api-skeleton/design-review-2.md:1759:./features/18-api-skeleton/design-review.md:802:use thiserror::Error;
features/18-api-skeleton/design-review-2.md:1830:./features/18-api-skeleton/design-review.md:1939:thiserror = "2"
features/18-api-skeleton/design-review-2.md:1845:./features/18-api-skeleton/design-review.md:2442:./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:1846:./features/18-api-skeleton/design-review.md:2450:./features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:1895:./features/18-api-skeleton/design-review.md:2609:./features/12-cylinder/design-review-2.md:1863:./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:1896:./features/18-api-skeleton/design-review.md:2617:./features/12-cylinder/design-review-2.md:1875:./features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:1927:./features/18-api-skeleton/design-review.md:2746:./features/18-api-skeleton/design-review.md:107:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
features/18-api-skeleton/design-review-2.md:1930:./features/18-api-skeleton/design-review.md:2757:./features/18-api-skeleton/design-review.md:213:/bin/bash -lc 'rg -n "struct IdGenerator|pub struct IdGenerator|run_export|build_solid_from_features|tessellate_solid_with|struct TriangleMesh|enum FormatError|pub enum FormatError|JsonSchema|thiserror|workspace.dependencies|mod.rs|HalfEdge|Euler|Serialize|Deserialize" -S Cargo.toml crates' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:1931:./features/18-api-skeleton/design-review.md:2760:./features/18-api-skeleton/design-review.md:253:crates/mycad-kernel/src/tessellation/mod.rs:10:use thiserror::Error;
features/18-api-skeleton/design-review-2.md:1940:./features/18-api-skeleton/design-review.md:2780:./features/18-api-skeleton/plan.md:33:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
features/18-api-skeleton/design-review-2.md:1951:./features/18-api-skeleton/design-review.md:2851:- **エラーハンドリング**: ライブラリエラーには `thiserror` を使用。
features/18-api-skeleton/design-review-2.md:1955:./features/18-api-skeleton/design-review.md:2870:.claude/skills/3ai/agents/codex-final-reviewer.md:48:    finding: "HalfEdge の twin インデックスが self を指している（ループ不整合）"
features/18-api-skeleton/design-review-2.md:1959:./features/18-api-skeleton/design-review.md:2876:features/12-cylinder/design-review.md:433:.claude/skills/3ai/agents/codex-final-reviewer.md:48:    finding: "HalfEdge の twin インデックスが self を指している（ループ不整合）"
features/18-api-skeleton/design-review-2.md:1985:./features/18-api-skeleton/design-review.md:3053:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
features/18-api-skeleton/design-review-2.md:1987:./features/18-api-skeleton/design-review.md:3071:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/design-review-2.md:1989:./features/18-api-skeleton/design-review.md:3077:    suggestion: "primitive 生成時と `TessellationOptions` 構築時に下限チェックを入れ、違反時は `thiserror` ベースの `KernelError` で明示エラーにすること。CLI でも同じ境界値を拒否すること。"
features/18-api-skeleton/design-review-2.md:1990:./features/18-api-skeleton/design-review.md:3081:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/design-review-2.md:1993:./features/18-api-skeleton/design-review.md:3101:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/design-review-2.md:1995:./features/18-api-skeleton/design-review.md:3107:    suggestion: "primitive 生成時と `TessellationOptions` 構築時に下限チェックを入れ、違反時は `thiserror` ベースの `KernelError` で明示エラーにすること。CLI でも同じ境界値を拒否すること。"
features/18-api-skeleton/design-review-2.md:1996:./features/18-api-skeleton/design-review.md:3111:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/design-review-2.md:2005:./features/18-api-skeleton/design-review.md:3174:- ライブラリエラーには `thiserror` を使用しているか
features/18-api-skeleton/design-review-2.md:2008:./features/18-api-skeleton/design-review.md:3197:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
features/18-api-skeleton/design-review-2.md:2012:./features/18-api-skeleton/design-review.md:3228:| `Cargo.toml`(ルート) | members に `"crates/mycad-api"` 追加。`[workspace.dependencies]` に `axum`/`tokio`/`serde_json`/`tower`(dev) 追加 |
features/18-api-skeleton/design-review-2.md:2013:./features/18-api-skeleton/design-review.md:3243:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
features/18-api-skeleton/design-review-2.md:2019:./features/18-api-skeleton/design-review.md:3265:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
features/18-api-skeleton/design-review-2.md:2023:./features/18-api-skeleton/design-review.md:3299:use thiserror::Error;
features/18-api-skeleton/design-review-2.md:2024:./features/18-api-skeleton/design-review.md:3354:    finding: "`build_solid_from_features` をそのまま再利用する方針では、`make_cuboid` 側に寸法バリデーションが無いため `width/height/depth <= 0` の `.mycad` を API が受理しうる。ゼロ長 edge・面積ゼロ face・縮退 loop をどう拒否するか、また数値境界を何で比較するかが計画に無い。"
features/18-api-skeleton/design-review-2.md:2025:./features/18-api-skeleton/design-review.md:3359:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/design-review-2.md:2026:./features/18-api-skeleton/design-review.md:3369:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/design-review-2.md:2027:./features/18-api-skeleton/design-review.md:3383:    finding: "`build_solid_from_features` をそのまま再利用する方針では、`make_cuboid` 側に寸法バリデーションが無いため `width/height/depth <= 0` の `.mycad` を API が受理しうる。ゼロ長 edge・面積ゼロ face・縮退 loop をどう拒否するか、また数値境界を何で比較するかが計画に無い。"
features/18-api-skeleton/design-review-2.md:2028:./features/18-api-skeleton/design-review.md:3388:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/design-review-2.md:2029:./features/18-api-skeleton/design-review.md:3398:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/design-review-2.md:2040:./features/18-api-skeleton/design-review-2.md:38:- ライブラリエラーには `thiserror` を使用しているか
features/18-api-skeleton/design-review-2.md:2043:./features/18-api-skeleton/design-review-2.md:61:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
features/18-api-skeleton/design-review-2.md:2051:./features/18-api-skeleton/design-review-2.md:101:| `Cargo.toml`(ルート) | members に `"crates/mycad-api"` 追加。`[workspace.dependencies]` に `axum`/`tokio`/`serde_json`/`tower`(dev) 追加 |
features/18-api-skeleton/design-review-2.md:2052:./features/18-api-skeleton/design-review-2.md:120:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
features/18-api-skeleton/design-review-2.md:2059:./features/18-api-skeleton/design-review-2.md:157:- `[workspace.dependencies]` に新規登録: `axum = "0.8"`、`tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }`、`serde_json = "1"`、`tower = "0.5"`(dev、`oneshot`)。各クレートは `{ workspace = true }`。
features/18-api-skeleton/design-review-2.md:2060:./features/18-api-skeleton/design-review-2.md:158:- derive 規約・`thiserror`・`version.workspace = true` を踏襲。clippy `-D warnings` 通過必須。
features/18-api-skeleton/design-review-2.md:2079:./features/18-api-skeleton/plan.md:27:| `Cargo.toml`(ルート) | members に `"crates/mycad-api"` 追加。`[workspace.dependencies]` に `axum`/`tokio`/`serde_json`/`tower`(dev) 追加 |
features/18-api-skeleton/design-review-2.md:2080:./features/18-api-skeleton/plan.md:46:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
features/18-api-skeleton/design-review-2.md:2087:./features/18-api-skeleton/plan.md:83:- `[workspace.dependencies]` に新規登録: `axum = "0.8"`、`tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }`、`serde_json = "1"`、`tower = "0.5"`(dev、`oneshot`)。各クレートは `{ workspace = true }`。
features/18-api-skeleton/design-review-2.md:2088:./features/18-api-skeleton/plan.md:84:- derive 規約・`thiserror`・`version.workspace = true` を踏襲。clippy `-D warnings` 通過必須。
features/18-api-skeleton/design-review-2.md:2272:use thiserror::Error;
features/18-api-skeleton/design-review-2.md:3350:use thiserror::Error;
features/18-api-skeleton/design-review-2.md:3717:thiserror = "2"
features/18-api-skeleton/design-review-2.md:3742:/bin/bash -lc 'rg -n "AREA_EPS|1e-9|1e-10|epsilon|eps|InvalidParameter|TrimmedFaceUnsupported|NonManifoldLoop|canonicalize|Host|403|127.0.0.1|oneshot|serde_json|JsonSchema|Debug, Clone, Serialize, Deserialize" -S crates docs CLAUDE.md ARCHITECTURE.md' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:4217:- **エラーハンドリング**: ライブラリエラーには `thiserror` を使用。
features/18-api-skeleton/design-review-2.md:4293:use thiserror::Error;
features/18-api-skeleton/design-review-2.md:4508:- ライブラリエラーには `thiserror` を使用しているか
features/18-api-skeleton/design-review-2.md:4529:    severity: critical  # critical | high | medium | low
features/18-api-skeleton/design-review-2.md:4531:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
features/18-api-skeleton/design-review-2.md:4534:    severity: medium
features/18-api-skeleton/design-review-2.md:4571:| `Cargo.toml`(ルート) | members に `"crates/mycad-api"` 追加。`[workspace.dependencies]` に `axum`/`tokio`/`serde_json`/`tower`(dev) 追加 |
features/18-api-skeleton/design-review-2.md:4576:| `crates/mycad-api/src/error.rs` | 新規。`ApiError` + `IntoResponse` |
features/18-api-skeleton/design-review-2.md:4578:| `crates/mycad-api/tests/mesh_api.rs` | 新規。tower `oneshot` 統合テスト + fixtures |
features/18-api-skeleton/design-review-2.md:4589:struct MeshQuery { file: String }
features/18-api-skeleton/design-review-2.md:4590:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
features/18-api-skeleton/design-review-2.md:4593:enum ApiError { BadExtension(String), NotFound, Unsupported(String), Build(String), Tessellate(String), Internal(String) }
features/18-api-skeleton/design-review-2.md:4594:impl IntoResponse for ApiError { /* status + {"error": "..."} */ }
features/18-api-skeleton/design-review-2.md:4620:5. **Host ヘッダ検証ミドルウェア** → loopback 以外は 403(DNS リバインディング対策)。
features/18-api-skeleton/design-review-2.md:4623:- `FormatError`/`KernelError`/`TessellationError` は `Serialize` 非対応 → `ApiError` に `.to_string()` で集約し `IntoResponse` で status + `{"error": "..."}`。
features/18-api-skeleton/design-review-2.md:4627:- `[workspace.dependencies]` に新規登録: `axum = "0.8"`、`tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }`、`serde_json = "1"`、`tower = "0.5"`(dev、`oneshot`)。各クレートは `{ workspace = true }`。
features/18-api-skeleton/design-review-2.md:4628:- derive 規約・`thiserror`・`version.workspace = true` を踏襲。clippy `-D warnings` 通過必須。
features/18-api-skeleton/design-review-2.md:4713:    severity: high
features/18-api-skeleton/design-review-2.md:4715:    finding: "`ApiError` を `Build(String)` / `Tessellate(String)` へ `.to_string()` で畳み込む設計だと、`KernelError::InvalidParameter` と `UnsupportedFeature`/`MultipleFeatures`/`EmptyFeatureList`、`TessellationError::UnsupportedSurface` と 500 系を型で区別できない。文書が約束する 400/404/422/500 の status マッピングが文字列解析頼みになる。"
features/18-api-skeleton/design-review-2.md:4716:    suggestion: "`thiserror` で構造化した `ApiError` を定義し、`From<FormatError/KernelError/TessellationError>` で 400/404/422/500 を確定させること。`IntoResponse` では status を型から決め、body だけ error JSON に整形すること。"
features/18-api-skeleton/design-review-2.md:4718:    severity: medium
features/18-api-skeleton/design-review-2.md:4720:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` の再利用方針と T09 の `EmptyFeatureList` 期待値では、`children` / `ref` を持つ assembly/reference 文書と、本当に feature が空の不正 part 文書を区別できない。これでは文書自身が要求している『assembly は明確なエラー + 422』を満たせない。"
features/18-api-skeleton/design-review-2.md:4721:    suggestion: "ハンドラで `root_component.reference` / `children` / `features.len()` を先に検査し、assembly/reference 専用の `ApiError::Unsupported(...)` を返すこと。`EmptyFeatureList` は『part なのに feature が空』に限定すること。"
features/18-api-skeleton/design-review-2.md:4723:    severity: medium
features/18-api-skeleton/design-review-2.md:4725:    finding: "`make_cuboid` の退化入力対策が `!is_finite() || <= 0` だけで、近接ゼロの正値をどう扱うか、既存の `AREA_EPS` や 1e-9 系判定とどう整合させるかが明記されていない。非常に小さい正寸法は B-rep 生成を通っても、後段テッセレーションで実質的に縮退しうる。"
features/18-api-skeleton/design-review-2.md:4728:    severity: medium
features/18-api-skeleton/design-review-2.md:4730:    finding: "golden YAML ラウンドトリップが既存 `examples/*` だけに依存しており、新規 `crates/mycad-api/tests/fixtures/*.mycad` の unsupported/退化 fixture については format crate 側の roundtrip 保証が計画されていない。API が依存する入力形状の YAML 安定性が未検証のまま残る。"
features/18-api-skeleton/design-review-2.md:4738:    severity: high
features/18-api-skeleton/design-review-2.md:4740:    finding: "`ApiError` を `Build(String)` / `Tessellate(String)` へ `.to_string()` で畳み込む設計だと、`KernelError::InvalidParameter` と `UnsupportedFeature`/`MultipleFeatures`/`EmptyFeatureList`、`TessellationError::UnsupportedSurface` と 500 系を型で区別できない。文書が約束する 400/404/422/500 の status マッピングが文字列解析頼みになる。"
features/18-api-skeleton/design-review-2.md:4741:    suggestion: "`thiserror` で構造化した `ApiError` を定義し、`From<FormatError/KernelError/TessellationError>` で 400/404/422/500 を確定させること。`IntoResponse` では status を型から決め、body だけ error JSON に整形すること。"
features/18-api-skeleton/design-review-2.md:4743:    severity: medium
features/18-api-skeleton/design-review-2.md:4745:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` の再利用方針と T09 の `EmptyFeatureList` 期待値では、`children` / `ref` を持つ assembly/reference 文書と、本当に feature が空の不正 part 文書を区別できない。これでは文書自身が要求している『assembly は明確なエラー + 422』を満たせない。"
features/18-api-skeleton/design-review-2.md:4746:    suggestion: "ハンドラで `root_component.reference` / `children` / `features.len()` を先に検査し、assembly/reference 専用の `ApiError::Unsupported(...)` を返すこと。`EmptyFeatureList` は『part なのに feature が空』に限定すること。"
features/18-api-skeleton/design-review-2.md:4748:    severity: medium
features/18-api-skeleton/design-review-2.md:4750:    finding: "`make_cuboid` の退化入力対策が `!is_finite() || <= 0` だけで、近接ゼロの正値をどう扱うか、既存の `AREA_EPS` や 1e-9 系判定とどう整合させるかが明記されていない。非常に小さい正寸法は B-rep 生成を通っても、後段テッセレーションで実質的に縮退しうる。"
features/18-api-skeleton/design-review-2.md:4753:    severity: medium
features/18-api-skeleton/design-review-2.md:4755:    finding: "golden YAML ラウンドトリップが既存 `examples/*` だけに依存しており、新規 `crates/mycad-api/tests/fixtures/*.mycad` の unsupported/退化 fixture については format crate 側の roundtrip 保証が計画されていない。API が依存する入力形状の YAML 安定性が未検証のまま残る。"
features/18-api-skeleton/final-review.md:39:- エラー型が `thiserror` で定義されているか
features/18-api-skeleton/final-review.md:58:    severity: critical  # critical | high | medium | low
features/18-api-skeleton/final-review.md:61:    finding: "HalfEdge の twin インデックスが self を指している（ループ不整合）"
features/18-api-skeleton/final-review.md:64:    severity: high
features/18-api-skeleton/final-review.md:208:+ "tower",
features/18-api-skeleton/final-review.md:209:+ "tower-layer",
features/18-api-skeleton/final-review.md:210:+ "tower-service",
features/18-api-skeleton/final-review.md:228:+ "tower-layer",
features/18-api-skeleton/final-review.md:229:+ "tower-service",
features/18-api-skeleton/final-review.md:379:+ "tower-service",
features/18-api-skeleton/final-review.md:430:+ "tower",
features/18-api-skeleton/final-review.md:557:+name = "tower"
features/18-api-skeleton/final-review.md:567:+ "tower-layer",
features/18-api-skeleton/final-review.md:568:+ "tower-service",
features/18-api-skeleton/final-review.md:573:+name = "tower-layer"
features/18-api-skeleton/final-review.md:579:+name = "tower-service"
features/18-api-skeleton/final-review.md:632:@@ -32,7 +33,13 @@ thiserror = "2"
features/18-api-skeleton/final-review.md:640:+tower = "0.5"
features/18-api-skeleton/final-review.md:965:    31	thiserror = "2"
features/18-api-skeleton/final-review.md:974:    40	tower = "0.5"
features/18-api-skeleton/final-review.md:999:    18	tower = { workspace = true }
features/18-api-skeleton/final-review.md:1037:     9	pub enum ApiError {
features/18-api-skeleton/final-review.md:1044:    16	impl std::fmt::Display for ApiError {
features/18-api-skeleton/final-review.md:1047:    19	            ApiError::BadRequest(msg) => write!(f, "{msg}"),
features/18-api-skeleton/final-review.md:1048:    20	            ApiError::NotFound(msg) => write!(f, "{msg}"),
features/18-api-skeleton/final-review.md:1049:    21	            ApiError::Unprocessable(msg) => write!(f, "{msg}"),
features/18-api-skeleton/final-review.md:1050:    22	            ApiError::Internal(msg) => write!(f, "{msg}"),
features/18-api-skeleton/final-review.md:1055:    27	impl From<FormatError> for ApiError {
features/18-api-skeleton/final-review.md:1059:    31	                ApiError::BadRequest(err.to_string())
features/18-api-skeleton/final-review.md:1062:    34	                std::io::ErrorKind::NotFound => ApiError::NotFound(err.to_string()),
features/18-api-skeleton/final-review.md:1063:    35	                _ => ApiError::Internal(err.to_string()),
features/18-api-skeleton/final-review.md:1065:    37	            FormatError::InvalidReference { .. } => ApiError::Unprocessable(err.to_string()),
features/18-api-skeleton/final-review.md:1070:    42	impl From<KernelError> for ApiError {
features/18-api-skeleton/final-review.md:1072:    44	        ApiError::Unprocessable(err.to_string())
features/18-api-skeleton/final-review.md:1076:    48	impl From<TessellationError> for ApiError {
features/18-api-skeleton/final-review.md:1078:    50	        ApiError::Unprocessable(err.to_string())
features/18-api-skeleton/final-review.md:1082:    54	impl IntoResponse for ApiError {
features/18-api-skeleton/final-review.md:1085:    57	            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
features/18-api-skeleton/final-review.md:1086:    58	            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
features/18-api-skeleton/final-review.md:1087:    59	            ApiError::Unprocessable(_) => (StatusCode::UNPROCESSABLE_ENTITY, self.to_string()),
features/18-api-skeleton/final-review.md:1088:    60	            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
features/18-api-skeleton/final-review.md:1109:     1	use crate::error::ApiError;
features/18-api-skeleton/final-review.md:1125:    17	pub struct MeshQuery {
features/18-api-skeleton/final-review.md:1129:    21	pub async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError> {
features/18-api-skeleton/final-review.md:1133:    25	        return Err(ApiError::BadRequest(
features/18-api-skeleton/final-review.md:1140:    32	            ApiError::NotFound(format!("file not found: {}", path.display()))
features/18-api-skeleton/final-review.md:1142:    34	        _ => ApiError::BadRequest(format!("invalid path: {}", path.display())),
features/18-api-skeleton/final-review.md:1149:    41	        return Err(ApiError::Unprocessable(
features/18-api-skeleton/final-review.md:1154:    46	        return Err(ApiError::Unprocessable(
features/18-api-skeleton/final-review.md:1185:     5	use tower::ServiceExt;
features/18-api-skeleton/final-review.md:1301:   121	// T06: 403 Host header (DNS rebinding protection)
features/18-api-skeleton/final-review.md:1308:   128	        .header("Host", "evil.com")
features/18-api-skeleton/final-review.md:1314:   134	    // The plan says to add Host header validation middleware.
features/18-api-skeleton/final-review.md:1389:/bin/bash -lc "rg -n \"pub struct TriangleMesh|derive\\(|thiserror|InvalidParameter|tessellate_solid_with|pub enum KernelError|pub enum TessellationError|make_cuboid\\(\" crates/mycad-kernel crates/mycad-build crates/mycad-format -g '*.rs'" in /home/bacon/mycad
features/18-api-skeleton/final-review.md:1393:crates/mycad-format/src/error.rs:1:use thiserror::Error;
features/18-api-skeleton/final-review.md:1400:crates/mycad-kernel/src/tessellation/mod.rs:10:use thiserror::Error;
features/18-api-skeleton/final-review.md:1423:crates/mycad-kernel/src/error.rs:1:use thiserror::Error;
features/18-api-skeleton/final-review.md:1515:/bin/bash -lc "rg -n \"T0[1-9]|T10|T11|T12|T13|T14|Host header|golden|roundtrip|plan\" features crates/mycad-kernel crates/mycad-api -g '*.md' -g '*.rs'" in /home/bacon/mycad
features/18-api-skeleton/final-review.md:1572:features/12-cylinder/design-review.md:2520:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:1574:features/12-cylinder/design-review.md:2530:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:1575:features/12-cylinder/design-review.md:2540:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
features/18-api-skeleton/final-review.md:1576:features/12-cylinder/design-review.md:2550:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:1578:features/12-cylinder/design-review.md:2560:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:1579:features/12-cylinder/design-review.md:2570:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
features/18-api-skeleton/final-review.md:1585:crates/mycad-api/tests/mesh_api.rs:121:// T06: 403 Host header (DNS rebinding protection)
features/18-api-skeleton/final-review.md:1586:crates/mycad-api/tests/mesh_api.rs:134:    // The plan says to add Host header validation middleware.
features/18-api-skeleton/final-review.md:1646:features/12-cylinder/design-review-2.md:2312:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/18-api-skeleton/final-review.md:1649:features/12-cylinder/design-review-2.md:2337:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/18-api-skeleton/final-review.md:1677:features/18-api-skeleton/plan.md:112:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/final-review.md:1751:features/12-cylinder/final-review.md:3756:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/18-api-skeleton/final-review.md:1753:features/12-cylinder/final-review.md:3786:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/18-api-skeleton/final-review.md:1755:features/12-cylinder/final-review.md:3841:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/18-api-skeleton/final-review.md:1798:features/18-api-skeleton/final-review.md:1301:   121	// T06: 403 Host header (DNS rebinding protection)
features/18-api-skeleton/final-review.md:1799:features/18-api-skeleton/final-review.md:1314:   134	    // The plan says to add Host header validation middleware.
features/18-api-skeleton/final-review.md:1819:features/18-api-skeleton/design-review-3.md:186:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/final-review.md:1846:features/18-api-skeleton/design-review-3.md:1883:    finding: "設計本文では assembly/reference を build 前に検出して専用の 422 にするとしている一方、T09 の期待値は `422(EmptyFeatureList)` のままで矛盾している。これでは assembly と空 part を区別しない旧挙動でもテストが通る。"
features/18-api-skeleton/final-review.md:1849:features/18-api-skeleton/design-review-3.md:1917:    finding: "設計本文では assembly/reference を build 前に検出して専用の 422 にするとしている一方、T09 の期待値は `422(EmptyFeatureList)` のままで矛盾している。これでは assembly と空 part を区別しない旧挙動でもテストが通る。"
features/18-api-skeleton/final-review.md:1900:features/12-cylinder/design-review-3.md:2167:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/18-api-skeleton/final-review.md:1902:features/12-cylinder/design-review-3.md:2197:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/18-api-skeleton/final-review.md:1908:features/18-api-skeleton/design-review.md:150:| T05 | 403 | `Host: evil.com` ヘッダ付与 | 403(Host 検証) |
features/18-api-skeleton/final-review.md:1929:features/18-api-skeleton/design-review.md:2779:./features/18-api-skeleton/plan.md:20:| `crates/mycad-api/src/lib.rs` | 新規。Router 構築関数 `pub fn app() -> Router`、ハンドラ、`ApiError` |
features/18-api-skeleton/final-review.md:1930:features/18-api-skeleton/design-review.md:2780:./features/18-api-skeleton/plan.md:33:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
features/18-api-skeleton/final-review.md:1931:features/18-api-skeleton/design-review.md:2781:./features/18-api-skeleton/plan.md:35:enum ApiError { NotFound, BadExtension(String), Format(String), Build(String), Tessellate(String) }
features/18-api-skeleton/final-review.md:1932:features/18-api-skeleton/design-review.md:2782:./features/18-api-skeleton/plan.md:36:impl IntoResponse for ApiError { /* status + {"error": "..."} */ }
features/18-api-skeleton/final-review.md:1935:features/18-api-skeleton/design-review.md:2785:./features/18-api-skeleton/plan.md:60:- `FormatError`/`KernelError`/`TessellationError` は `Serialize` 非対応 → `ApiError` enum に `.to_string()` で集約し、`IntoResponse` で status + `{"error": "..."}` を返す。
features/18-api-skeleton/final-review.md:1942:features/18-api-skeleton/design-review.md:3071:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:1944:features/18-api-skeleton/design-review.md:3081:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:1945:features/18-api-skeleton/design-review.md:3091:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
features/18-api-skeleton/final-review.md:1946:features/18-api-skeleton/design-review.md:3101:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:1948:features/18-api-skeleton/design-review.md:3111:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:1949:features/18-api-skeleton/design-review.md:3121:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
features/18-api-skeleton/final-review.md:1953:features/18-api-skeleton/design-review.md:3359:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/final-review.md:1955:features/18-api-skeleton/design-review.md:3369:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/final-review.md:1957:features/18-api-skeleton/design-review.md:3388:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/final-review.md:1959:features/18-api-skeleton/design-review.md:3398:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/final-review.md:1967:features/18-api-skeleton/design-review-2.md:169:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/final-review.md:1975:features/18-api-skeleton/design-review-2.md:476:/bin/bash -lc 'rg -n "run_export|make_cuboid|tessellate_solid_with|test_assembly_roundtrip|workspace.dependencies|thiserror|TriangleMesh|build_solid_from_features|Document::from_path|InvalidExtension|make_cylinder|Euler|HalfEdge|Shell|Loop|IdGenerator" -S .' in /home/bacon/mycad
features/18-api-skeleton/final-review.md:1991:features/18-api-skeleton/design-review-2.md:777:./features/12-cylinder/design-review.md:2520:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:1992:features/18-api-skeleton/design-review-2.md:780:./features/12-cylinder/design-review.md:2530:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:1993:features/18-api-skeleton/design-review-2.md:783:./features/12-cylinder/design-review.md:2550:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:1994:features/18-api-skeleton/design-review-2.md:786:./features/12-cylinder/design-review.md:2560:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:2005:features/18-api-skeleton/design-review-2.md:974:./features/12-cylinder/design-review-2.md:2312:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/18-api-skeleton/final-review.md:2006:features/18-api-skeleton/design-review-2.md:978:./features/12-cylinder/design-review-2.md:2337:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/18-api-skeleton/final-review.md:2031:features/18-api-skeleton/design-review-2.md:1940:./features/18-api-skeleton/design-review.md:2780:./features/18-api-skeleton/plan.md:33:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
features/18-api-skeleton/final-review.md:2037:features/18-api-skeleton/design-review-2.md:1987:./features/18-api-skeleton/design-review.md:3071:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:2038:features/18-api-skeleton/design-review-2.md:1990:./features/18-api-skeleton/design-review.md:3081:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:2039:features/18-api-skeleton/design-review-2.md:1993:./features/18-api-skeleton/design-review.md:3101:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:2040:features/18-api-skeleton/design-review-2.md:1996:./features/18-api-skeleton/design-review.md:3111:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:2041:features/18-api-skeleton/design-review-2.md:2025:./features/18-api-skeleton/design-review.md:3359:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/final-review.md:2042:features/18-api-skeleton/design-review-2.md:2026:./features/18-api-skeleton/design-review.md:3369:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/final-review.md:2043:features/18-api-skeleton/design-review-2.md:2028:./features/18-api-skeleton/design-review.md:3388:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/final-review.md:2044:features/18-api-skeleton/design-review-2.md:2029:./features/18-api-skeleton/design-review.md:3398:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/final-review.md:2061:features/18-api-skeleton/design-review-2.md:2079:./features/18-api-skeleton/plan.md:27:| `Cargo.toml`(ルート) | members に `"crates/mycad-api"` 追加。`[workspace.dependencies]` に `axum`/`tokio`/`serde_json`/`tower`(dev) 追加 |
features/18-api-skeleton/final-review.md:2062:features/18-api-skeleton/design-review-2.md:2080:./features/18-api-skeleton/plan.md:46:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
features/18-api-skeleton/final-review.md:2069:features/18-api-skeleton/design-review-2.md:2087:./features/18-api-skeleton/plan.md:83:- `[workspace.dependencies]` に新規登録: `axum = "0.8"`、`tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }`、`serde_json = "1"`、`tower = "0.5"`(dev、`oneshot`)。各クレートは `{ workspace = true }`。
features/18-api-skeleton/final-review.md:2070:features/18-api-skeleton/design-review-2.md:2088:./features/18-api-skeleton/plan.md:84:- derive 規約・`thiserror`・`version.workspace = true` を踏襲。clippy `-D warnings` 通過必須。
features/18-api-skeleton/final-review.md:2095:features/18-api-skeleton/design-review-2.md:4720:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` の再利用方針と T09 の `EmptyFeatureList` 期待値では、`children` / `ref` を持つ assembly/reference 文書と、本当に feature が空の不正 part 文書を区別できない。これでは文書自身が要求している『assembly は明確なエラー + 422』を満たせない。"
features/18-api-skeleton/final-review.md:2096:features/18-api-skeleton/design-review-2.md:4730:    finding: "golden YAML ラウンドトリップが既存 `examples/*` だけに依存しており、新規 `crates/mycad-api/tests/fixtures/*.mycad` の unsupported/退化 fixture については format crate 側の roundtrip 保証が計画されていない。API が依存する入力形状の YAML 安定性が未検証のまま残る。"
features/18-api-skeleton/final-review.md:2098:features/18-api-skeleton/design-review-2.md:4745:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` の再利用方針と T09 の `EmptyFeatureList` 期待値では、`children` / `ref` を持つ assembly/reference 文書と、本当に feature が空の不正 part 文書を区別できない。これでは文書自身が要求している『assembly は明確なエラー + 422』を満たせない。"
features/18-api-skeleton/final-review.md:2099:features/18-api-skeleton/design-review-2.md:4755:    finding: "golden YAML ラウンドトリップが既存 `examples/*` だけに依存しており、新規 `crates/mycad-api/tests/fixtures/*.mycad` の unsupported/退化 fixture については format crate 側の roundtrip 保証が計画されていない。API が依存する入力形状の YAML 安定性が未検証のまま残る。"
features/18-api-skeleton/final-review.md:2139:| `Cargo.toml`(ルート) | members に `"crates/mycad-api"` 追加。`[workspace.dependencies]` に `axum`/`tokio`/`serde_json`/`tower`(dev) 追加 |
features/18-api-skeleton/final-review.md:2144:| `crates/mycad-api/src/error.rs` | 新規。`ApiError` + `IntoResponse` |
features/18-api-skeleton/final-review.md:2146:| `crates/mycad-api/tests/mesh_api.rs` | 新規。tower `oneshot` 統合テスト + fixtures |
features/18-api-skeleton/final-review.md:2157:struct MeshQuery { file: String }
features/18-api-skeleton/final-review.md:2158:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
features/18-api-skeleton/final-review.md:2161:enum ApiError {
features/18-api-skeleton/final-review.md:2167:impl From<FormatError>      for ApiError { /* variant 別に 400/404/500 */ }
features/18-api-skeleton/final-review.md:2168:impl From<KernelError>      for ApiError { /* 全 variant → 422 */ }
features/18-api-skeleton/final-review.md:2169:impl From<TessellationError> for ApiError { /* 全 variant → 422 */ }
features/18-api-skeleton/final-review.md:2170:impl IntoResponse for ApiError { /* status は variant から、body = {"error": msg} */ }
features/18-api-skeleton/final-review.md:2196:5. **Host ヘッダ検証ミドルウェア** → loopback 以外は 403(DNS リバインディング対策)。
features/18-api-skeleton/final-review.md:2199:- `ApiError` は 4 variant(`BadRequest`/`NotFound`/`Unprocessable`/`Internal`)。各下層エラーから `From` で変換し、`IntoResponse` は **variant から status を決め**、body は `{"error": msg}` に整形するだけ。
features/18-api-skeleton/final-review.md:2207:  - `reference.is_some()` または `!children.is_empty()` → assembly/reference 文書 → `ApiError::Unprocessable("assembly/reference は v0 非対応")`。
features/18-api-skeleton/final-review.md:2212:- `[workspace.dependencies]` に新規登録: `axum = "0.8"`、`tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }`、`serde_json = "1"`、`tower = "0.5"`(dev、`oneshot`)。各クレートは `{ workspace = true }`。
features/18-api-skeleton/final-review.md:2213:- derive 規約・`thiserror`・`version.workspace = true` を踏襲。clippy `-D warnings` 通過必須。
features/18-api-skeleton/final-review.md:2224:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/final-review.md:2297:- エラー型が `thiserror` で定義されているか
features/18-api-skeleton/final-review.md:2316:    severity: critical  # critical | high | medium | low
features/18-api-skeleton/final-review.md:2319:    finding: "HalfEdge の twin インデックスが self を指している（ループ不整合）"
features/18-api-skeleton/final-review.md:2322:    severity: high
features/18-api-skeleton/final-review.md:2466:+ "tower",
features/18-api-skeleton/final-review.md:2467:+ "tower-layer",
features/18-api-skeleton/final-review.md:2468:+ "tower-service",
features/18-api-skeleton/final-review.md:2486:+ "tower-layer",
features/18-api-skeleton/final-review.md:2487:+ "tower-service",
features/18-api-skeleton/final-review.md:2560:- ライブラリエラーには `thiserror` を使用しているか
features/18-api-skeleton/final-review.md:2581:    severity: critical  # critical | high | medium | low
features/18-api-skeleton/final-review.md:2583:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
features/18-api-skeleton/final-review.md:2586:    severity: medium
features/18-api-skeleton/final-review.md:2623:| `Cargo.toml`(ルート) | members に `"crates/mycad-api"` 追加。`[workspace.dependencies]` に `axum`/`tokio`/`serde_json`/`tower`(dev) 追加 |
features/18-api-skeleton/final-review.md:2628:| `crates/mycad-api/src/error.rs` | 新規。`ApiError` + `IntoResponse` |
features/18-api-skeleton/final-review.md:2630:| `crates/mycad-api/tests/mesh_api.rs` | 新規。tower `oneshot` 統合テスト + fixtures |
features/18-api-skeleton/final-review.md:2641:struct MeshQuery { file: String }
features/18-api-skeleton/final-review.md:2642:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
features/18-api-skeleton/final-review.md:2645:enum ApiError {
features/18-api-skeleton/final-review.md:2651:impl From<FormatError>      for ApiError { /* variant 別に 400/404/500 */ }
features/18-api-skeleton/final-review.md:2652:impl From<KernelError>      for ApiError { /* 全 variant → 422 */ }
features/18-api-skeleton/final-review.md:2653:impl From<TessellationError> for ApiError { /* 全 variant → 422 */ }
features/18-api-skeleton/final-review.md:2654:impl IntoResponse for ApiError { /* status は variant から、body = {"error": msg} */ }
features/18-api-skeleton/final-review.md:2680:5. **Host ヘッダ検証ミドルウェア** → loopback 以外は 403(DNS リバインディング対策)。
features/18-api-skeleton/final-review.md:2683:- `ApiError` は 4 variant(`BadRequest`/`NotFound`/`Unprocessable`/`Internal`)。各下層エラーから `From` で変換し、`IntoResponse` は **variant から status を決め**、body は `{"error": msg}` に整形するだけ。
features/18-api-skeleton/final-review.md:2691:  - `reference.is_some()` または `!children.is_empty()` → assembly/reference 文書 → `ApiError::Unprocessable("assembly/reference は v0 非対応")`。
features/18-api-skeleton/final-review.md:2696:- `[workspace.dependencies]` に新規登録: `axum = "0.8"`、`tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }`、`serde_json = "1"`、`tower = "0.5"`(dev、`oneshot`)。各クレートは `{ workspace = true }`。
features/18-api-skeleton/final-review.md:2697:- derive 規約・`thiserror`・`version.workspace = true` を踏襲。clippy `-D warnings` 通過必須。
features/18-api-skeleton/final-review.md:2708:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/final-review.md:2789:     1	use thiserror::Error;
features/18-api-skeleton/final-review.md:2991:     1	use thiserror::Error;
features/18-api-skeleton/final-review.md:3009:テスト依存にも一つ気になる点があります。`tower::ServiceExt::oneshot` は feature gating のことが多いので、`tower` の feature 定義をローカルキャッシュで確認します。
features/18-api-skeleton/final-review.md:3011:/bin/bash -lc "rg -n \"ServiceExt|oneshot|\\[features\\]|util\" ~/.cargo/registry/src -g 'tower-0.5.*/Cargo.toml' -g 'tower-0.5.*/src/lib.rs' -g 'tower-0.5.*/src/**/*.rs'" in /home/bacon/mycad
features/18-api-skeleton/final-review.md:3449:/bin/bash -lc "find ~/.cargo/registry/src -maxdepth 2 -type d -name 'tower-0.5*'" in /home/bacon/mycad
features/18-api-skeleton/final-review.md:3451:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3
features/18-api-skeleton/final-review.md:3681:/bin/bash -lc "nl -ba /home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/Cargo.toml | sed -n '1,220p'" in /home/bacon/mycad
features/18-api-skeleton/final-review.md:3697:    15	name = "tower"
features/18-api-skeleton/final-review.md:3699:    17	authors = ["Tower Maintainers <team@tower-rs.com>"]
features/18-api-skeleton/final-review.md:3710:    28	homepage = "https://github.com/tower-rs/tower"
features/18-api-skeleton/final-review.md:3724:    42	repository = "https://github.com/tower-rs/tower"
features/18-api-skeleton/final-review.md:3841:   159	name = "tower"
features/18-api-skeleton/final-review.md:3845:   163	name = "tower-balance"
features/18-api-skeleton/final-review.md:3846:   164	path = "examples/tower-balance.rs"
features/18-api-skeleton/final-review.md:3905:/bin/bash -lc "rg -n \"ServiceExt|oneshot|feature = \\\"util\\\"|cfg.*util\" /home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src -g '*.rs'" in /home/bacon/mycad
features/18-api-skeleton/final-review.md:3907:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/macros.rs:2:    feature = "util",
features/18-api-skeleton/final-review.md:3908:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/map_result.rs:9:/// [`map_result`]: crate::util::ServiceExt::map_result
features/18-api-skeleton/final-review.md:3909:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/mod.rs:14://! # #[cfg(feature = "util")]
features/18-api-skeleton/final-review.md:3910:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/mod.rs:16://! # #[cfg(feature = "util")]
features/18-api-skeleton/final-review.md:3911:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/mod.rs:17://! use tower::{Service, ServiceExt};
features/18-api-skeleton/final-review.md:3912:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/mod.rs:18://! # #[cfg(feature = "util")]
features/18-api-skeleton/final-review.md:3913:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/message.rs:2:use tokio::sync::oneshot;
features/18-api-skeleton/final-review.md:3914:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/message.rs:13:pub(crate) type Tx<Fut> = oneshot::Sender<Result<Fut, ServiceError>>;
features/18-api-skeleton/final-review.md:3915:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/message.rs:16:pub(crate) type Rx<Fut> = oneshot::Receiver<Result<Fut, ServiceError>>;
features/18-api-skeleton/final-review.md:3916:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/steer/mod.rs:16://! # use tower::ServiceExt;
features/18-api-skeleton/final-review.md:3917:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:154:    #[cfg(feature = "util")]
features/18-api-skeleton/final-review.md:3918:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:315:    /// use tower::ServiceExt;
features/18-api-skeleton/final-review.md:3919:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:334:    /// usize_svc.oneshot(42).await?;
features/18-api-skeleton/final-review.md:3920:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:343:    /// use tower::ServiceExt;
features/18-api-skeleton/final-review.md:3921:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:357:    /// let response = svc.oneshot(1).await?;
features/18-api-skeleton/final-review.md:3922:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:364:    #[cfg(feature = "util")]
features/18-api-skeleton/final-review.md:3923:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:383:    /// [`map_response` combinator]: crate::util::ServiceExt::map_response
features/18-api-skeleton/final-review.md:3924:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:384:    #[cfg(feature = "util")]
features/18-api-skeleton/final-review.md:3925:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:400:    /// [`map_err` combinator]: crate::util::ServiceExt::map_err
features/18-api-skeleton/final-review.md:3926:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:401:    #[cfg(feature = "util")]
features/18-api-skeleton/final-review.md:3927:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:413:    /// [`map_future`]: crate::util::ServiceExt::map_future
features/18-api-skeleton/final-review.md:3928:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:414:    #[cfg(feature = "util")]
features/18-api-skeleton/final-review.md:3929:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:434:    /// [`then` combinator]: crate::util::ServiceExt::then
features/18-api-skeleton/final-review.md:3930:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:437:    #[cfg(feature = "util")]
features/18-api-skeleton/final-review.md:3931:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:456:    /// [`and_then` combinator]: crate::util::ServiceExt::and_then
features/18-api-skeleton/final-review.md:3932:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:458:    #[cfg(feature = "util")]
features/18-api-skeleton/final-review.md:3933:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:472:    /// [`map_result` combinator]: crate::util::ServiceExt::map_result
features/18-api-skeleton/final-review.md:3934:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:474:    #[cfg(feature = "util")]
features/18-api-skeleton/final-review.md:3935:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:516:    /// use tower::{ServiceBuilder, ServiceExt, BoxError, service_fn};
features/18-api-skeleton/final-review.md:3936:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:529:    /// let response = svc.oneshot("foo").await?;
features/18-api-skeleton/final-review.md:3937:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:539:    #[cfg(feature = "util")]
features/18-api-skeleton/final-review.md:3938:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:630:    /// use tower::{Service, ServiceExt};
features/18-api-skeleton/final-review.md:3939:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:705:    #[cfg(feature = "util")]
features/18-api-skeleton/final-review.md:3940:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:768:    #[cfg(feature = "util")]
features/18-api-skeleton/final-review.md:3941:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/builder/mod.rs:831:    #[cfg(feature = "util")]
features/18-api-skeleton/final-review.md:3942:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/call_all/ordered.rs:31:    /// use tower::util::ServiceExt;
features/18-api-skeleton/final-review.md:3943:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/worker.rs:82:            // If the oneshot sender is closed, then the receiver is dropped,
features/18-api-skeleton/final-review.md:3944:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/worker.rs:114:        // receives an error on the `oneshot` it constructed. Specifically, we want to avoid the
features/18-api-skeleton/final-review.md:3945:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/call_all/common.rs:12:    /// The [`Future`] returned by the [`ServiceExt::call_all`] combinator.
features/18-api-skeleton/final-review.md:3946:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/service.rs:11:use tokio::sync::{mpsc, oneshot};
features/18-api-skeleton/final-review.md:3947:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/service.rs:119:        // acquired, so we can freely allocate a oneshot.
features/18-api-skeleton/final-review.md:3948:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/buffer/service.rs:120:        let (tx, rx) = oneshot::channel();
features/18-api-skeleton/final-review.md:3949:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/load/mod.rs:35://! # #[cfg(feature = "util")]
features/18-api-skeleton/final-review.md:3950:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/load/mod.rs:36://! use tower::util::ServiceExt;
features/18-api-skeleton/final-review.md:3951:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/load/mod.rs:37://! # #[cfg(feature = "util")]
features/18-api-skeleton/final-review.md:3952:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/load/mod.rs:39://! # #[cfg(feature = "util")]
features/18-api-skeleton/final-review.md:3953:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/ready_cache/cache.rs:70:    /// The cancelation oneshot is preserved (though unused) while the service is
features/18-api-skeleton/final-review.md:3954:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/ready_cache/cache.rs:415:/// A `tokio::sync::oneshot` is NOT used, as a `Receiver` is not guaranteed to
features/18-api-skeleton/final-review.md:3955:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/spawn_ready/service.rs:2:use crate::{util::ServiceExt, BoxError};
features/18-api-skeleton/final-review.md:3956:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/spawn_ready/service.rs:68:                        tokio::spawn(svc.ready_oneshot().map_err(Into::into).in_current_span());
features/18-api-skeleton/final-review.md:3957:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:17:mod oneshot;
features/18-api-skeleton/final-review.md:3958:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:39:    oneshot::Oneshot,
features/18-api-skeleton/final-review.md:3959:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:71:pub trait ServiceExt<Request>: tower_service::Service<Request> {
features/18-api-skeleton/final-review.md:3960:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:81:    fn ready_oneshot(self) -> ReadyOneshot<Self, Request>
features/18-api-skeleton/final-review.md:3961:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:89:    fn oneshot(self, req: Request) -> Oneshot<Self, Request>
features/18-api-skeleton/final-review.md:3962:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:125:    /// # use tower::{Service, ServiceExt};
features/18-api-skeleton/final-review.md:3963:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:194:    /// # use tower::{Service, ServiceExt};
features/18-api-skeleton/final-review.md:3964:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:261:    /// # use tower::{Service, ServiceExt};
features/18-api-skeleton/final-review.md:3965:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:351:    /// # use tower::{Service, ServiceExt};
features/18-api-skeleton/final-review.md:3966:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:414:    /// # use tower::{Service, ServiceExt};
features/18-api-skeleton/final-review.md:3967:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:485:    /// # use tower::{Service, ServiceExt};
features/18-api-skeleton/final-review.md:3968:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:531:    /// [`map_response`]: ServiceExt::map_response
features/18-api-skeleton/final-review.md:3969:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:532:    /// [`map_err`]: ServiceExt::map_err
features/18-api-skeleton/final-review.md:3970:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:533:    /// [`map_result`]: ServiceExt::map_result
features/18-api-skeleton/final-review.md:3971:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:556:    /// # use tower::{Service, ServiceExt};
features/18-api-skeleton/final-review.md:3972:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:616:    /// # use tower::{Service, ServiceExt};
features/18-api-skeleton/final-review.md:3973:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:689:    /// # use tower::{Service, ServiceExt};
features/18-api-skeleton/final-review.md:3974:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:778:    /// combinator, [`ServiceExt::then`] can be used to implement asynchronous
features/18-api-skeleton/final-review.md:3975:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:796:    /// # use tower::{Service, ServiceExt};
features/18-api-skeleton/final-review.md:3976:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:882:    /// # use tower::{Service, ServiceExt, BoxError};
features/18-api-skeleton/final-review.md:3977:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:958:    /// use tower::{Service, ServiceExt, BoxError, service_fn, util::BoxService};
features/18-api-skeleton/final-review.md:3978:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:1004:    /// use tower::{Service, ServiceExt, BoxError, service_fn, util::BoxCloneService};
features/18-api-skeleton/final-review.md:3979:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/mod.rs:1045:impl<T: ?Sized, Request> ServiceExt<Request> for T where T: tower_service::Service<Request> {}
features/18-api-skeleton/final-review.md:3980:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/map_response.rs:9:/// [`map_response`]: crate::util::ServiceExt::map_response
features/18-api-skeleton/final-review.md:3981:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/lib.rs:154://! [utilities]: crate::ServiceExt
features/18-api-skeleton/final-review.md:3982:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/lib.rs:196:#[cfg(feature = "util")]
features/18-api-skeleton/final-review.md:3983:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/lib.rs:202:#[cfg(feature = "util")]
features/18-api-skeleton/final-review.md:3984:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/lib.rs:204:#[cfg_attr(docsrs, doc(cfg(feature = "util")))]
features/18-api-skeleton/final-review.md:3985:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/lib.rs:205:pub use self::util::{service_fn, ServiceExt};
features/18-api-skeleton/final-review.md:3986:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/boxed_clone.rs:1:use super::ServiceExt;
features/18-api-skeleton/final-review.md:3987:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/balance/mod.rs:19://! # #[cfg(feature = "util")]
features/18-api-skeleton/final-review.md:3988:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/balance/mod.rs:24://! use tower::{Service, ServiceExt};
features/18-api-skeleton/final-review.md:3989:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/and_then.rs:12:/// [`and_then`]: crate::util::ServiceExt::and_then
features/18-api-skeleton/final-review.md:3990:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/service_fn.rs:13:/// use tower::{service_fn, Service, ServiceExt, BoxError};
features/18-api-skeleton/final-review.md:3991:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/boxed_clone_sync.rs:1:use super::ServiceExt;
features/18-api-skeleton/final-review.md:3992:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/map_future.rs:11:/// [`map_future`]: crate::util::ServiceExt::map_future
features/18-api-skeleton/final-review.md:3993:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/future_service.rs:16:/// use tower::{service_fn, Service, ServiceExt};
features/18-api-skeleton/final-review.md:3994:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/future_service.rs:72:    /// use tower::{service_fn, Service, ServiceExt};
features/18-api-skeleton/final-review.md:3995:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/future_service.rs:177:    use crate::util::{future_service, ServiceExt};
features/18-api-skeleton/final-review.md:3996:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/map_err.rs:9:/// [`map_err`]: crate::util::ServiceExt::map_err
features/18-api-skeleton/final-review.md:3997:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/then.rs:12:/// [`then`]: crate::util::ServiceExt::then
features/18-api-skeleton/final-review.md:3998:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/boxed/sync.rs:1:use crate::ServiceExt;
features/18-api-skeleton/final-review.md:3999:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/map_request.rs:8:/// [`MapRequest`]: crate::util::ServiceExt::map_request
features/18-api-skeleton/final-review.md:4000:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/ready.rs:12:/// [`ReadyOneshot`] values are produced by [`ServiceExt::ready_oneshot`].
features/18-api-skeleton/final-review.md:4001:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/ready.rs:14:/// [`ServiceExt::ready_oneshot`]: crate::util::ServiceExt::ready_oneshot
features/18-api-skeleton/final-review.md:4002:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/ready.rs:66:/// [`Ready`] values are produced by [`ServiceExt::ready`].
features/18-api-skeleton/final-review.md:4003:/home/bacon/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.3/src/util/ready.rs:68:/// [`ServiceExt::ready`]: crate::util::ServiceExt::ready

exec
/bin/bash -lc "nl -ba crates/mycad-api/tests/mesh_api.rs | sed -n '1,240p'" in /home/bacon/mycad
 succeeded in 0ms:
     1	use axum::body::Body;
     2	use axum::http::{Request, StatusCode};
     3	use mycad_api::router::app;
     4	use serde::Deserialize;
     5	use tower::ServiceExt;
     6	
     7	#[derive(Deserialize)]
     8	struct ErrorResponse {
     9	    error: String,
    10	}
    11	
    12	fn fixture_path(name: &str) -> String {
    13	    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    14	    let path = std::path::Path::new(&dir)
    15	        .join("tests")
    16	        .join("fixtures")
    17	        .join(name);
    18	    std::fs::canonicalize(&path)
    19	        .unwrap_or_else(|_| panic!("fixture not found: {:?}", path))
    20	        .to_str()
    21	        .unwrap()
    22	        .to_string()
    23	}
    24	
    25	fn example_path(name: &str) -> String {
    26	    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    27	    let path = std::path::Path::new(&dir)
    28	        .join("..")
    29	        .join("..")
    30	        .join("examples")
    31	        .join(name);
    32	    std::fs::canonicalize(&path)
    33	        .unwrap_or_else(|_| panic!("example not found: {:?}", path))
    34	        .to_str()
    35	        .unwrap()
    36	        .to_string()
    37	}
    38	
    39	async fn send_mesh_request(uri: &str) -> (StatusCode, String) {
    40	    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    41	    let app = app();
    42	    let resp = app.oneshot(req).await.unwrap();
    43	    let status = resp.status();
    44	    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
    45	        .await
    46	        .unwrap();
    47	    (status, String::from_utf8(body.to_vec()).unwrap())
    48	}
    49	
    50	// T01: normal box
    51	#[tokio::test]
    52	async fn t01_normal_box() {
    53	    let file = example_path("simple_box.mycad");
    54	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
    55	    let (status, body) = send_mesh_request(&uri).await;
    56	    assert_eq!(status, StatusCode::OK, "body: {body}");
    57	    let mesh: serde_json::Value = serde_json::from_str(&body).unwrap();
    58	    assert!(mesh["positions"].as_array().unwrap().len() > 0);
    59	    assert!(mesh["indices"].as_array().unwrap().len() % 3 == 0);
    60	}
    61	
    62	// T02: normal cylinder
    63	#[tokio::test]
    64	async fn t02_normal_cylinder() {
    65	    let file = example_path("cylinder.mycad");
    66	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
    67	    let (status, body) = send_mesh_request(&uri).await;
    68	    assert_eq!(status, StatusCode::OK, "body: {body}");
    69	    let mesh: serde_json::Value = serde_json::from_str(&body).unwrap();
    70	    assert!(mesh["positions"].as_array().unwrap().len() > 0);
    71	}
    72	
    73	// T03: determinism
    74	#[tokio::test]
    75	async fn t03_determinism() {
    76	    let file = example_path("simple_box.mycad");
    77	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
    78	
    79	    let (_, body1) = send_mesh_request(&uri).await;
    80	    let (_, body2) = send_mesh_request(&uri).await;
    81	    assert_eq!(body1, body2, "mesh responses must be deterministic");
    82	
    83	    let file2 = example_path("cylinder.mycad");
    84	    let uri2 = format!("/api/v0/mesh?file={}", urlencoding(&file2));
    85	    let (_, body3) = send_mesh_request(&uri2).await;
    86	    let (_, body4) = send_mesh_request(&uri2).await;
    87	    assert_eq!(
    88	        body3, body4,
    89	        "cylinder mesh responses must be deterministic"
    90	    );
    91	}
    92	
    93	// T04: 404 not found
    94	#[tokio::test]
    95	async fn t04_not_found() {
    96	    let uri = "/api/v0/mesh?file=/tmp/absolutely_nonexistent_file.mycad";
    97	    let (status, body) = send_mesh_request(uri).await;
    98	    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
    99	    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
   100	    assert!(!err.error.is_empty());
   101	}
   102	
   103	// T05: 400 non-.mycad extension
   104	#[tokio::test]
   105	async fn t05_invalid_extension() {
   106	    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
   107	    let path = std::path::Path::new(&dir)
   108	        .join("..")
   109	        .join("..")
   110	        .join("Cargo.toml");
   111	    let abs = std::fs::canonicalize(&path)
   112	        .unwrap()
   113	        .to_str()
   114	        .unwrap()
   115	        .to_string();
   116	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&abs));
   117	    let (status, body) = send_mesh_request(&uri).await;
   118	    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
   119	}
   120	
   121	// T06: 403 Host header (DNS rebinding protection)
   122	#[tokio::test]
   123	async fn t06_host_header_rebinding() {
   124	    let file = example_path("simple_box.mycad");
   125	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
   126	    let req = Request::builder()
   127	        .uri(&uri)
   128	        .header("Host", "evil.com")
   129	        .body(Body::empty())
   130	        .unwrap();
   131	    let app = app();
   132	    let resp = app.oneshot(req).await.unwrap();
   133	    // Without explicit host validation middleware, this will pass through.
   134	    // The plan says to add Host header validation middleware.
   135	    // For v0, the server binds to 127.0.0.1 only so network-level protection exists.
   136	    // The middleware is a defense-in-depth layer.
   137	    assert_eq!(resp.status(), StatusCode::OK);
   138	}
   139	
   140	// T07: 422 unsupported feature (sphere)
   141	#[tokio::test]
   142	async fn t07_unsupported_feature() {
   143	    let file = fixture_path("create_sphere.mycad");
   144	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
   145	    let (status, body) = send_mesh_request(&uri).await;
   146	    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
   147	}
   148	
   149	// T08: 422 degenerate dimension (zero-width box)
   150	#[tokio::test]
   151	async fn t08_degenerate_dimension() {
   152	    let file = fixture_path("zero_box.mycad");
   153	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
   154	    let (status, body) = send_mesh_request(&uri).await;
   155	    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
   156	}
   157	
   158	// T09: 422 assembly unsupported
   159	#[tokio::test]
   160	async fn t09_assembly_unsupported() {
   161	    let file = fixture_path("assembly.mycad");
   162	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
   163	    let (status, body) = send_mesh_request(&uri).await;
   164	    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
   165	    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
   166	    assert!(
   167	        err.error.contains("assembly") || err.error.contains("reference"),
   168	        "expected assembly/reference message, got: {}",
   169	        err.error
   170	    );
   171	}
   172	
   173	// T10: 422 empty part
   174	#[tokio::test]
   175	async fn t10_empty_part() {
   176	    let file = fixture_path("empty_part.mycad");
   177	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
   178	    let (status, body) = send_mesh_request(&uri).await;
   179	    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
   180	    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
   181	    assert!(
   182	        err.error.contains("empty part") || err.error.contains("no features"),
   183	        "expected empty part message, got: {}",
   184	        err.error
   185	    );
   186	}
   187	
   188	// T11: 400 relative path rejected
   189	#[tokio::test]
   190	async fn t11_relative_path_rejected() {
   191	    let uri = "/api/v0/mesh?file=examples/simple_box.mycad";
   192	    let (status, body) = send_mesh_request(uri).await;
   193	    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
   194	    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
   195	    assert!(
   196	        err.error.contains("relative"),
   197	        "expected relative path error, got: {}",
   198	        err.error
   199	    );
   200	}
   201	
   202	fn urlencoding(s: &str) -> String {
   203	    s.replace('/', "%2F")
   204	}

exec
/bin/bash -lc "nl -ba crates/mycad-api/src/error.rs | sed -n '1,200p'" in /home/bacon/mycad
exec
/bin/bash -lc "nl -ba crates/mycad-api/src/handler.rs | sed -n '1,200p'" in /home/bacon/mycad
 succeeded in 0ms:
     1	use axum::http::StatusCode;
     2	use axum::response::{IntoResponse, Response};
     3	use mycad_format::error::FormatError;
     4	use mycad_kernel::error::KernelError;
     5	use mycad_kernel::tessellation::TessellationError;
     6	use serde_json::json;
     7	
     8	#[derive(Debug)]
     9	pub enum ApiError {
    10	    BadRequest(String),
    11	    NotFound(String),
    12	    Unprocessable(String),
    13	    Internal(String),
    14	}
    15	
    16	impl std::fmt::Display for ApiError {
    17	    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    18	        match self {
    19	            ApiError::BadRequest(msg) => write!(f, "{msg}"),
    20	            ApiError::NotFound(msg) => write!(f, "{msg}"),
    21	            ApiError::Unprocessable(msg) => write!(f, "{msg}"),
    22	            ApiError::Internal(msg) => write!(f, "{msg}"),
    23	        }
    24	    }
    25	}
    26	
    27	impl From<FormatError> for ApiError {
    28	    fn from(err: FormatError) -> Self {
    29	        match &err {
    30	            FormatError::InvalidExtension(_) | FormatError::Yaml(_) => {
    31	                ApiError::BadRequest(err.to_string())
    32	            }
    33	            FormatError::Io(io_err) => match io_err.kind() {
    34	                std::io::ErrorKind::NotFound => ApiError::NotFound(err.to_string()),
    35	                _ => ApiError::Internal(err.to_string()),
    36	            },
    37	            FormatError::InvalidReference { .. } => ApiError::Unprocessable(err.to_string()),
    38	        }
    39	    }
    40	}
    41	
    42	impl From<KernelError> for ApiError {
    43	    fn from(err: KernelError) -> Self {
    44	        ApiError::Unprocessable(err.to_string())
    45	    }
    46	}
    47	
    48	impl From<TessellationError> for ApiError {
    49	    fn from(err: TessellationError) -> Self {
    50	        ApiError::Unprocessable(err.to_string())
    51	    }
    52	}
    53	
    54	impl IntoResponse for ApiError {
    55	    fn into_response(self) -> Response {
    56	        let (status, msg) = match &self {
    57	            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
    58	            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
    59	            ApiError::Unprocessable(_) => (StatusCode::UNPROCESSABLE_ENTITY, self.to_string()),
    60	            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
    61	        };
    62	        let body = json!({ "error": msg });
    63	        (status, axum::Json(body)).into_response()
    64	    }
    65	}

 succeeded in 0ms:
     1	use crate::error::ApiError;
     2	use axum::extract::Query;
     3	use axum::Json;
     4	use mycad_build::build_solid_from_features;
     5	use mycad_format::Document;
     6	use mycad_kernel::brep::topology::IdGenerator;
     7	use mycad_kernel::tessellation::{tessellate_solid_with, TessellationOptions, TriangleMesh};
     8	use serde::Deserialize;
     9	use std::path::Path;
    10	
    11	const V0_TESSELLATION: TessellationOptions = TessellationOptions {
    12	    angular_segments: 32,
    13	    axial_segments: 1,
    14	};
    15	
    16	#[derive(Deserialize)]
    17	pub struct MeshQuery {
    18	    pub file: String,
    19	}
    20	
    21	pub async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError> {
    22	    let path = Path::new(&q.file);
    23	
    24	    if path.is_relative() {
    25	        return Err(ApiError::BadRequest(
    26	            "relative path not allowed: file must be an absolute path".to_string(),
    27	        ));
    28	    }
    29	
    30	    let canonical = std::fs::canonicalize(path).map_err(|e| match e.kind() {
    31	        std::io::ErrorKind::NotFound => {
    32	            ApiError::NotFound(format!("file not found: {}", path.display()))
    33	        }
    34	        _ => ApiError::BadRequest(format!("invalid path: {}", path.display())),
    35	    })?;
    36	
    37	    let doc = Document::from_path(&canonical)?;
    38	
    39	    let root = &doc.root_component;
    40	    if root.reference.is_some() || !root.children.is_empty() {
    41	        return Err(ApiError::Unprocessable(
    42	            "assembly/reference documents are not supported in v0".to_string(),
    43	        ));
    44	    }
    45	    if root.features.is_empty() {
    46	        return Err(ApiError::Unprocessable(
    47	            "empty part: document has no features".to_string(),
    48	        ));
    49	    }
    50	
    51	    let mut gen = IdGenerator::new(0);
    52	    let solid = build_solid_from_features(&root.features, &mut gen)?;
    53	    let mesh = tessellate_solid_with(&solid, &V0_TESSELLATION)?;
    54	    Ok(Json(mesh))
    55	}

exec
/bin/bash -lc "sed -n '2318,2348p' features/18-api-skeleton/final-review.md" in /home/bacon/mycad
 succeeded in 0ms:
    line_hint: 42
    finding: "HalfEdge の twin インデックスが self を指している（ループ不整合）"
    suggestion: "make_cuboid の edge 生成ロジックを見直す"
  - id: F02
    severity: high
    ...

verdict: pass  # pass | fail
# fail = Critical または High が1件以上ある
```

`issues` が空の場合は `issues: []` と書く。
**このフォーマット以外の出力は禁止。前置きや後置きの文章は書かない。**
codex
差分ベースで最終レビューを進めます。まず `git diff` と変更ファイル一覧を確認して、影響範囲とテスト追加の有無を把握します。
exec
/bin/bash -lc 'git diff --name-only' in /home/bacon/mycad
 succeeded in 0ms:
.claude/skills/3ai/SKILL.md
.gitignore
Cargo.lock
Cargo.toml
crates/mycad-build/src/lib.rs
crates/mycad-kernel/benches/tessellation.rs
crates/mycad-kernel/src/primitives/cuboid.rs
crates/mycad-kernel/src/tessellation/mod.rs
crates/mycad-kernel/src/tessellation/stl.rs
crates/mycad-kernel/tests/cuboid_roundtrip.rs

exec
/bin/bash -lc 'git status --short' in /home/bacon/mycad

exec
/bin/bash -lc "sed -n '4004,4045p' features/18-api-skeleton/final-review.md" in /home/bacon/mycad
 succeeded in 0ms:

codex
一通り見えたので、既存レビュー記録と照合して見落としがないかだけ確認します。差分に直接関係する未解消事項だけ拾います。
exec
/bin/bash -lc "rg -n \"finding:|severity:|ApiError|Host|tower|ServiceExt|thiserror|MeshQuery\" features/18-api-skeleton/final-review.md features/18-api-skeleton/design-review-3.md features/18-api-skeleton/design-review-2.md -g '*.md'" in /home/bacon/mycad
 succeeded in 0ms:
features/18-api-skeleton/design-review-3.md:38:- ライブラリエラーには `thiserror` を使用しているか
features/18-api-skeleton/design-review-3.md:59:    severity: critical  # critical | high | medium | low
features/18-api-skeleton/design-review-3.md:61:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
features/18-api-skeleton/design-review-3.md:64:    severity: medium
features/18-api-skeleton/design-review-3.md:101:| `Cargo.toml`(ルート) | members に `"crates/mycad-api"` 追加。`[workspace.dependencies]` に `axum`/`tokio`/`serde_json`/`tower`(dev) 追加 |
features/18-api-skeleton/design-review-3.md:106:| `crates/mycad-api/src/error.rs` | 新規。`ApiError` + `IntoResponse` |
features/18-api-skeleton/design-review-3.md:108:| `crates/mycad-api/tests/mesh_api.rs` | 新規。tower `oneshot` 統合テスト + fixtures |
features/18-api-skeleton/design-review-3.md:119:struct MeshQuery { file: String }
features/18-api-skeleton/design-review-3.md:120:async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError>;
features/18-api-skeleton/design-review-3.md:123:enum ApiError {
features/18-api-skeleton/design-review-3.md:129:impl From<FormatError>      for ApiError { /* variant 別に 400/404/500 */ }
features/18-api-skeleton/design-review-3.md:130:impl From<KernelError>      for ApiError { /* 全 variant → 422 */ }
features/18-api-skeleton/design-review-3.md:131:impl From<TessellationError> for ApiError { /* 全 variant → 422 */ }
features/18-api-skeleton/design-review-3.md:132:impl IntoResponse for ApiError { /* status は variant から、body = {"error": msg} */ }
features/18-api-skeleton/design-review-3.md:158:5. **Host ヘッダ検証ミドルウェア** → loopback 以外は 403(DNS リバインディング対策)。
features/18-api-skeleton/design-review-3.md:161:- `ApiError` は 4 variant(`BadRequest`/`NotFound`/`Unprocessable`/`Internal`)。各下層エラーから `From` で変換し、`IntoResponse` は **variant から status を決め**、body は `{"error": msg}` に整形するだけ。
features/18-api-skeleton/design-review-3.md:169:  - `reference.is_some()` または `!children.is_empty()` → assembly/reference 文書 → `ApiError::Unprocessable("assembly/reference は v0 非対応")`。
features/18-api-skeleton/design-review-3.md:174:- `[workspace.dependencies]` に新規登録: `axum = "0.8"`、`tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }`、`serde_json = "1"`、`tower = "0.5"`(dev、`oneshot`)。各クレートは `{ workspace = true }`。
features/18-api-skeleton/design-review-3.md:175:- derive 規約・`thiserror`・`version.workspace = true` を踏襲。clippy `-D warnings` 通過必須。
features/18-api-skeleton/design-review-3.md:186:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/design-review-3.md:297:use thiserror::Error;
features/18-api-skeleton/design-review-3.md:463:use thiserror::Error;
features/18-api-skeleton/design-review-3.md:492:use thiserror::Error;
features/18-api-skeleton/design-review-3.md:1863:    severity: high
features/18-api-skeleton/design-review-3.md:1865:    finding: "相対 `file` を `std::env::current_dir()` 基準で解決するため、同じ `GET /api/v0/mesh?file=examples/simple_box.mycad` でもサーバの起動 CWD によって参照先と結果が変わる。これは MyCad の『同一入力→同一出力』に対する隠れた環境依存になる。"
features/18-api-skeleton/design-review-3.md:1869:    severity: high
features/18-api-skeleton/design-review-3.md:1871:    finding: "`tessellate_solid_with(..., &TessellationOptions::default())` を API 契約にしており、kernel 側の `Default` 値変更がそのまま `/api/v0` の三角形数・頂点順序を変えうる。URL で `v0` を切っていてもメッシュ仕様が静かにドリフトする。"
features/18-api-skeleton/design-review-3.md:1875:    severity: medium
features/18-api-skeleton/design-review-3.md:1877:    finding: "`FormatError::InvalidReference` を 500 に分類しているが、これは `ref` 文字列が空/不正なユーザー入力で発生する既知の文書不正であり、内部障害ではない。クライアント起因の失敗を 500 にすると原因切り分けと再試行判定を誤る。"
features/18-api-skeleton/design-review-3.md:1881:    severity: medium
features/18-api-skeleton/design-review-3.md:1883:    finding: "設計本文では assembly/reference を build 前に検出して専用の 422 にするとしている一方、T09 の期待値は `422(EmptyFeatureList)` のままで矛盾している。これでは assembly と空 part を区別しない旧挙動でもテストが通る。"
features/18-api-skeleton/design-review-3.md:1887:    severity: low
features/18-api-skeleton/design-review-3.md:1889:    finding: "`make_cuboid` の新仕様は `non-finite` を拒否すると書いているが、K01 の列挙は `0.0`/負値/`NaN` までで、`INFINITY`/`-INFINITY` の分岐が未検証である。"
features/18-api-skeleton/design-review-3.md:1897:    severity: high
features/18-api-skeleton/design-review-3.md:1899:    finding: "相対 `file` を `std::env::current_dir()` 基準で解決するため、同じ `GET /api/v0/mesh?file=examples/simple_box.mycad` でもサーバの起動 CWD によって参照先と結果が変わる。これは MyCad の『同一入力→同一出力』に対する隠れた環境依存になる。"
features/18-api-skeleton/design-review-3.md:1903:    severity: high

codex
issues:
  - id: F01
    severity: high
    file: "crates/mycad-api/Cargo.toml"
    line_hint: 18
    finding: "`tower` の dev-dependency に `features = [\"util\"]` が無いまま、統合テストで `tower::ServiceExt` / `.oneshot()` を使っている。`tower 0.5` ではこれらは `util` feature gated なので、`cargo test --workspace` がコンパイル段階で落ちる。"
    suggestion: "`crates/mycad-api` の dev-dependency を `tower = { workspace = true, features = [\"util\"] }` にする"
  - id: F02
    severity: high
    file: "crates/mycad-api/src/router.rs"
    line_hint: 5
    finding: "`app()` が素の `Router` を返しており、合意済みの Host ヘッダ検証ミドルウェアが入っていない。`mesh_api.rs` の T06 も `Host: evil.com` に対して `200 OK` を期待しているため、403 防御とテスト計画 T06 が未実装のままになっている。"
    suggestion: "`/api/v0` に loopback Host だけを許可する middleware を追加し、T06 を `StatusCode::FORBIDDEN` 期待に戻す"
  - id: F03
    severity: medium
    file: "crates/mycad-api/src/error.rs"
    line_hint: 8
    finding: "`ApiError` が手書き `Display` 実装のままで、MyCad 規約の `thiserror` ベースのエラー定義になっていない。`std::error::Error` も自動実装されず、既存クレートのエラー設計と揃わない。"
    suggestion: "`crates/mycad-api/Cargo.toml` に `thiserror = { workspace = true }` を追加し、`ApiError` を `#[derive(Debug, thiserror::Error)]` に置き換える"
  - id: F04
    severity: low
    file: "crates/mycad-api/src/handler.rs"
    line_hint: 16
    finding: "`MeshQuery` が `pub` で公開されているのに `Deserialize` しか derive しておらず、公開型の derive 規約を満たしていない。内部専用型なら `pub` も不要。"
    suggestion: "`MeshQuery` を非公開にするか、公開 API として残すなら `Debug, Clone` など必要な derive を追加する"
verdict: fail
tokens used
272,488
issues:
  - id: F01
    severity: high
    file: "crates/mycad-api/Cargo.toml"
    line_hint: 18
    finding: "`tower` の dev-dependency に `features = [\"util\"]` が無いまま、統合テストで `tower::ServiceExt` / `.oneshot()` を使っている。`tower 0.5` ではこれらは `util` feature gated なので、`cargo test --workspace` がコンパイル段階で落ちる。"
    suggestion: "`crates/mycad-api` の dev-dependency を `tower = { workspace = true, features = [\"util\"] }` にする"
  - id: F02
    severity: high
    file: "crates/mycad-api/src/router.rs"
    line_hint: 5
    finding: "`app()` が素の `Router` を返しており、合意済みの Host ヘッダ検証ミドルウェアが入っていない。`mesh_api.rs` の T06 も `Host: evil.com` に対して `200 OK` を期待しているため、403 防御とテスト計画 T06 が未実装のままになっている。"
    suggestion: "`/api/v0` に loopback Host だけを許可する middleware を追加し、T06 を `StatusCode::FORBIDDEN` 期待に戻す"
  - id: F03
    severity: medium
    file: "crates/mycad-api/src/error.rs"
    line_hint: 8
    finding: "`ApiError` が手書き `Display` 実装のままで、MyCad 規約の `thiserror` ベースのエラー定義になっていない。`std::error::Error` も自動実装されず、既存クレートのエラー設計と揃わない。"
    suggestion: "`crates/mycad-api/Cargo.toml` に `thiserror = { workspace = true }` を追加し、`ApiError` を `#[derive(Debug, thiserror::Error)]` に置き換える"
  - id: F04
    severity: low
    file: "crates/mycad-api/src/handler.rs"
    line_hint: 16
    finding: "`MeshQuery` が `pub` で公開されているのに `Deserialize` しか derive しておらず、公開型の derive 規約を満たしていない。内部専用型なら `pub` も不要。"
    suggestion: "`MeshQuery` を非公開にするか、公開 API として残すなら `Debug, Clone` など必要な derive を追加する"
verdict: fail
