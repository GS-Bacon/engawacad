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
session id: 019e5de1-febe-7f61-b605-42286469fbe4
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
差分レビューとして、まず `git status` と変更ファイルの diff を確認し、今回の変更範囲と関連テストを特定します。
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
/bin/bash -lc 'git diff --stat' in /home/bacon/mycad
 succeeded in 0ms:
 .claude/skills/3ai/SKILL.md                   |   6 +
 .gitignore                                    |   4 +
 Cargo.lock                                    | 366 ++++++++++++++++++++++++++
 Cargo.toml                                    |   7 +
 crates/mycad-build/src/lib.rs                 |   2 +-
 crates/mycad-kernel/benches/tessellation.rs   |   2 +-
 crates/mycad-kernel/src/primitives/cuboid.rs  | 115 +++++++-
 crates/mycad-kernel/src/tessellation/mod.rs   |  12 +-
 crates/mycad-kernel/src/tessellation/stl.rs   |   4 +-
 crates/mycad-kernel/tests/cuboid_roundtrip.rs |   4 +-
 10 files changed, 504 insertions(+), 18 deletions(-)

codex
変更範囲は主に `mycad-kernel` の cuboid/tessellation 周りと workspace 設定です。次に各差分を読み、決定性・B-rep 整合性・テスト計画の実装有無を確認します。
exec
/bin/bash -lc 'git diff -- Cargo.toml Cargo.lock crates/mycad-build/src/lib.rs' in /home/bacon/mycad
 succeeded in 0ms:
diff --git a/Cargo.lock b/Cargo.lock
index 9795377..9eb94cb 100644
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
@@ -275,6 +467,38 @@ version = "2.8.0"
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
+ "thiserror",
+ "tokio",
+ "tower",
+]
+
 [[package]]
 name = "mycad-build"
 version = "0.1.0"
@@ -420,6 +644,18 @@ version = "1.0.15"
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
@@ -572,6 +808,29 @@ dependencies = [
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
@@ -598,6 +857,28 @@ dependencies = [
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
@@ -615,6 +896,12 @@ dependencies = [
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
@@ -648,6 +935,79 @@ dependencies = [
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
@@ -678,6 +1038,12 @@ version = "0.2.2"
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

exec
/bin/bash -lc 'git diff -- crates/mycad-kernel/src/primitives/cuboid.rs crates/mycad-kernel/src/tessellation/mod.rs crates/mycad-kernel/src/tessellation/stl.rs' in /home/bacon/mycad
 succeeded in 0ms:
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

exec
/bin/bash -lc 'git diff -- crates/mycad-kernel/benches/tessellation.rs crates/mycad-kernel/tests/cuboid_roundtrip.rs' in /home/bacon/mycad
 succeeded in 0ms:
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
 

exec
/bin/bash -lc 'rg -n "T0[0-9]|golden YAML|roundtrip|determin" crates/mycad-kernel crates/mycad-build Cargo.toml' in /home/bacon/mycad
 succeeded in 0ms:
Cargo.toml:34:# Ordered collections (deterministic iteration)
crates/mycad-kernel/src/tessellation/mod.rs:318:    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
crates/mycad-kernel/src/tessellation/mod.rs:337:    /// T07: Determinism — same input → same output.
crates/mycad-kernel/src/tessellation/mod.rs:339:    fn test_tessellation_cylinder_deterministic() {
crates/mycad-kernel/src/tessellation/mod.rs:354:    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
crates/mycad-kernel/src/tessellation/mod.rs:367:    /// T08 extended: exact positions and indices snapshot.
crates/mycad-kernel/src/tessellation/mod.rs:384:    /// T09: Outward normals — all facet normals point away from centroid.
crates/mycad-kernel/src/tessellation/mod.rs:510:    fn test_tessellation_deterministic() {
crates/mycad-kernel/src/tessellation/stl.rs:72:    fn ascii_stl_deterministic() {
crates/mycad-kernel/src/brep/topology.rs:7:/// Generated deterministically by creation order within a feature.
crates/mycad-kernel/src/brep/topology.rs:78:/// Contains all topological entities in flat arrays for deterministic ordering.
crates/mycad-kernel/src/brep/topology.rs:182:/// Counter for generating deterministic entity IDs.
crates/mycad-kernel/src/brep/topology.rs:207:    fn test_id_generator_is_deterministic() {
crates/mycad-kernel/src/geometry/surface.rs:188:    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
crates/mycad-kernel/src/geometry/surface.rs:196:    fn test_uv_of_roundtrip_plane() {
crates/mycad-kernel/src/geometry/surface.rs:203:        assert_uv_roundtrip(&surface, 0.7, -1.3);
crates/mycad-kernel/src/geometry/surface.rs:207:    fn test_uv_of_roundtrip_cylinder() {
crates/mycad-kernel/src/geometry/surface.rs:213:        assert_uv_roundtrip(&surface, 0.9, 1.7);
crates/mycad-kernel/src/geometry/surface.rs:217:    fn test_uv_of_roundtrip_sphere() {
crates/mycad-kernel/src/geometry/surface.rs:222:        assert_uv_roundtrip(&surface, 1.1, 0.6);
crates/mycad-kernel/src/geometry/surface.rs:226:    fn test_uv_of_roundtrip_cone() {
crates/mycad-kernel/src/geometry/surface.rs:232:        assert_uv_roundtrip(&surface, 0.8, 2.0);
crates/mycad-kernel/tests/cuboid_roundtrip.rs:26:/// Integration test: deterministic pipeline produces identical results.
crates/mycad-kernel/tests/cuboid_roundtrip.rs:28:fn cuboid_pipeline_is_deterministic() {
crates/mycad-kernel/src/primitives/cuboid.rs:14:/// - `id_gen`: deterministic ID generator
crates/mycad-kernel/src/primitives/cuboid.rs:289:    fn test_cuboid_deterministic() {
crates/mycad-kernel/src/primitives/cylinder.rs:133:    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
crates/mycad-kernel/src/primitives/cylinder.rs:135:    fn test_cylinder_deterministic() {
crates/mycad-kernel/src/primitives/cylinder.rs:161:    /// T02: Topology counts.
crates/mycad-kernel/src/primitives/cylinder.rs:180:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
crates/mycad-kernel/src/primitives/cylinder.rs:199:    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
crates/mycad-kernel/src/primitives/cylinder.rs:254:    /// T05: Geometry — seam vertices and surface coefficients.

codex
cuboid 側の差分は入力バリデーション追加が中心で、既存トポロジー生成そのものは変えていません。ここからはテスト計画 ID と仕様文書を突き合わせて、追加テストが計画どおりかを見ます。
exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/primitives/cuboid.rs" in /home/bacon/mycad
 succeeded in 0ms:
use crate::brep::topology::{IdGenerator, Solid};
use crate::error::KernelError;
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
pub fn make_cuboid(
    dx: f64,
    dy: f64,
    dz: f64,
    id_gen: &mut IdGenerator,
) -> Result<Solid, KernelError> {
    if !dx.is_finite() || dx <= 0.0 {
        return Err(KernelError::InvalidParameter { kind: "width" });
    }
    if !dy.is_finite() || dy <= 0.0 {
        return Err(KernelError::InvalidParameter { kind: "height" });
    }
    if !dz.is_finite() || dz <= 0.0 {
        return Err(KernelError::InvalidParameter { kind: "depth" });
    }
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

    Ok(solid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::topology::IdGenerator;
    use crate::error::KernelError;

    #[test]
    fn test_cuboid_topology() {
        let mut id_gen = IdGenerator::new(0);
        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();

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


exec
/bin/bash -lc "sed -n '1,280p' crates/mycad-kernel/src/error.rs" in /home/bacon/mycad
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
/bin/bash -lc "rg -n \"T0[0-9]|K0[0-9]|cuboid|validation|degenerate|YAML|roundtrip\" features crates/mycad-kernel/src/primitives crates/mycad-kernel/tests .claude -g '"'!Cargo.lock'"'" in /home/bacon/mycad
exec
/bin/bash -lc "sed -n '1,320p' crates/mycad-kernel/src/brep/topology.rs" in /home/bacon/mycad
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

 succeeded in 0ms:
.claude/skills/3ai/SKILL.md:68:| T01 | 決定性 | 同一入力を2回実行し全 ID・座標が一致 | assert_eq! |
.claude/skills/3ai/SKILL.md:69:| T02 | 正常系 | ... | ... |
.claude/skills/3ai/SKILL.md:71:| TXX | golden YAML | .mycad → rebuild → 再出力が一致 | assert_eq! |
.claude/skills/3ai/agents/glm-implementer.md:9:2. **コアテスト**: テスト計画 (T01〜) を全て実装する
.claude/skills/3ai/agents/glm-implementer.md:59:- ラウンドトリップ: 構築→YAML serialize→deserialize→再構築で全フィールドが一致するか
crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
crates/mycad-kernel/tests/cuboid_roundtrip.rs:5:/// Integration test: create a cuboid, tessellate it, and verify the pipeline.
crates/mycad-kernel/tests/cuboid_roundtrip.rs:7:fn cuboid_to_mesh_pipeline() {
crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
crates/mycad-kernel/tests/cuboid_roundtrip.rs:28:fn cuboid_pipeline_is_deterministic() {
crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen).unwrap();
.claude/skills/3ai/agents/codex-final-reviewer.md:30:- 設計レビュー時に合意したテスト計画（ID T01〜）が全て実装されているか
.claude/skills/3ai/agents/codex-final-reviewer.md:32:- golden YAML ラウンドトリップが含まれているか
.claude/skills/3ai/agents/codex-final-reviewer.md:49:    suggestion: "make_cuboid の edge 生成ロジックを見直す"
crates/mycad-kernel/src/primitives/mod.rs:1:mod cuboid;
crates/mycad-kernel/src/primitives/mod.rs:4:pub use cuboid::make_cuboid;
crates/mycad-kernel/src/primitives/cylinder.rs:133:    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
crates/mycad-kernel/src/primitives/cylinder.rs:161:    /// T02: Topology counts.
crates/mycad-kernel/src/primitives/cylinder.rs:180:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
crates/mycad-kernel/src/primitives/cylinder.rs:199:    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
crates/mycad-kernel/src/primitives/cylinder.rs:254:    /// T05: Geometry — seam vertices and surface coefficients.
crates/mycad-kernel/src/primitives/cylinder.rs:301:    fn test_cylinder_degenerate_inputs() {
crates/mycad-kernel/src/primitives/cuboid.rs:8:/// Create a cuboid (box) B-rep solid centered at the origin.
crates/mycad-kernel/src/primitives/cuboid.rs:16:/// The cuboid spans from (-dx/2, -dy/2, -dz/2) to (dx/2, dy/2, dz/2).
crates/mycad-kernel/src/primitives/cuboid.rs:17:pub fn make_cuboid(
crates/mycad-kernel/src/primitives/cuboid.rs:38:    // 8 vertices of the cuboid
crates/mycad-kernel/src/primitives/cuboid.rs:64:    // 12 edges of the cuboid
crates/mycad-kernel/src/primitives/cuboid.rs:99:    // 6 faces of the cuboid
crates/mycad-kernel/src/primitives/cuboid.rs:242:    fn test_cuboid_topology() {
crates/mycad-kernel/src/primitives/cuboid.rs:244:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
crates/mycad-kernel/src/primitives/cuboid.rs:246:        assert_eq!(solid.vertices.len(), 8, "cuboid should have 8 vertices");
crates/mycad-kernel/src/primitives/cuboid.rs:247:        assert_eq!(solid.edges.len(), 12, "cuboid should have 12 edges");
crates/mycad-kernel/src/primitives/cuboid.rs:248:        assert_eq!(solid.faces.len(), 6, "cuboid should have 6 faces");
crates/mycad-kernel/src/primitives/cuboid.rs:249:        assert_eq!(solid.shells.len(), 1, "cuboid should have 1 shell");
crates/mycad-kernel/src/primitives/cuboid.rs:250:        assert!(solid.shells[0].closed, "cuboid shell should be closed");
crates/mycad-kernel/src/primitives/cuboid.rs:255:            "cuboid should have 24 half-edges"
crates/mycad-kernel/src/primitives/cuboid.rs:258:        assert_eq!(solid.loops.len(), 6, "cuboid should have 6 loops");
crates/mycad-kernel/src/primitives/cuboid.rs:262:    fn test_cuboid_vertex_positions() {
crates/mycad-kernel/src/primitives/cuboid.rs:264:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen).unwrap();
crates/mycad-kernel/src/primitives/cuboid.rs:289:    fn test_cuboid_deterministic() {
crates/mycad-kernel/src/primitives/cuboid.rs:290:        // Building the same cuboid twice with the same ID generator should
crates/mycad-kernel/src/primitives/cuboid.rs:295:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1).unwrap();
crates/mycad-kernel/src/primitives/cuboid.rs:296:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2).unwrap();
crates/mycad-kernel/src/primitives/cuboid.rs:316:    // K01: make_cuboid rejects zero, negative, NaN, Infinity dimensions
crates/mycad-kernel/src/primitives/cuboid.rs:318:    fn test_make_cuboid_invalid_dimensions() {
crates/mycad-kernel/src/primitives/cuboid.rs:323:            make_cuboid(0.0, 1.0, 1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:327:            make_cuboid(1.0, 0.0, 1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:331:            make_cuboid(1.0, 1.0, 0.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:337:            make_cuboid(-1.0, 1.0, 1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:341:            make_cuboid(1.0, -1.0, 1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:345:            make_cuboid(1.0, 1.0, -1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:351:            make_cuboid(f64::NAN, 1.0, 1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:355:            make_cuboid(1.0, f64::NAN, 1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:359:            make_cuboid(1.0, 1.0, f64::NAN, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:365:            make_cuboid(f64::INFINITY, 1.0, 1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:369:            make_cuboid(1.0, f64::INFINITY, 1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:373:            make_cuboid(1.0, 1.0, f64::INFINITY, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:379:            make_cuboid(f64::NEG_INFINITY, 1.0, 1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:383:            make_cuboid(1.0, f64::NEG_INFINITY, 1.0, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:387:            make_cuboid(1.0, 1.0, f64::NEG_INFINITY, &mut gen),
crates/mycad-kernel/src/primitives/cuboid.rs:392:    // K02: near-zero positive value is Ok (exact <= 0.0 policy)
crates/mycad-kernel/src/primitives/cuboid.rs:394:    fn test_make_cuboid_near_zero_positive_ok() {
crates/mycad-kernel/src/primitives/cuboid.rs:396:        let result = make_cuboid(1e-9, 1.0, 1.0, &mut gen);
.claude/skills/3ai/agents/codex-design-reviewer.md:32:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/plan.md:9:> Codex 設計レビュー round 1-3 反映済み(3回上限)。round3 残存指摘は全て受諾し最終修正済み: R01=絶対パスのみ受理(決定的)、R02=v0 専用テッセレーション定数で Default 依存排除、R03=InvalidReference→422、R04=T09/T10 で assembly と空 part を分離、R05=K01 に INFINITY 分岐追加。round3 の verdict は fail のままだが指摘内容は本改訂で解消済み(ユーザー判断で再レビューせず確定)。
features/18-api-skeleton/plan.md:13:- 新規クレート `crates/mycad-api/`(lib + bin 構成)+ `make_cuboid` の入力検証(kernel)
features/18-api-skeleton/plan.md:24:| `crates/mycad-kernel/src/primitives/cuboid.rs` | `make_cuboid` を `-> Result<Solid, KernelError>` 化。`dx/dy/dz` が非有限 or `<= 0.0` なら `InvalidParameter { kind }`。**(R03)** 閾値は `make_cylinder`(cylinder.rs:16-22)と同じ exact `<= 0.0` で対称化。near-zero 正値の公差判定は導入しない(下記 R03 注記)。インライン test を `.unwrap()` 化 |
features/18-api-skeleton/plan.md:25:| `crates/mycad-build/src/lib.rs` | CreateBox 分岐を `Ok(make_cuboid(...))` → `make_cuboid(...)`(`?` 伝播)に変更 |
features/18-api-skeleton/plan.md:26:| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
features/18-api-skeleton/plan.md:107:| T01 | 正常系(box) | `GET /api/v0/mesh?file=<abs>/examples/simple_box.mycad` を oneshot | 200、`TriangleMesh` deser 可、`positions`/`indices` 非空、`indices.len() % 3 == 0` |
features/18-api-skeleton/plan.md:108:| T02 | 正常系(cylinder)(R05) | 同上 `examples/cylinder.mycad`(周期面経路) | 200、mesh 非空 |
features/18-api-skeleton/plan.md:109:| T03 | 決定性 | T01/T02 を 2 回実行 | 返る JSON バイト列が完全一致 |
features/18-api-skeleton/plan.md:110:| T04 | 404 | 存在しない `file` | 404、body `{"error": ...}` |
features/18-api-skeleton/plan.md:111:| T05 | 400 | 非 `.mycad` 拡張子(`Cargo.toml`) | 400(InvalidExtension) |
features/18-api-skeleton/plan.md:112:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/plan.md:113:| T07 | 422 未対応 feature(R03) | fixture `create_sphere` 単一 feature | 422(UnsupportedFeature) |
features/18-api-skeleton/plan.md:114:| T08 | 422 退化寸法(R02) | fixture `create_box` width=0(または負値) | 422(InvalidParameter) |
features/18-api-skeleton/plan.md:115:| T09 | 422 assembly 非対応(R04) | fixture: root features 空 + children あり | 422、body に assembly/reference 専用メッセージ(空 part とは別経路) |
features/18-api-skeleton/plan.md:118:| K01 | kernel 単体 | `make_cuboid` の `0.0` / 負値 / `NaN` / `INFINITY` / `-INFINITY`(dx/dy/dz 各々)(R05) | 全分岐で `Err(InvalidParameter { kind: "..." })` |
features/18-api-skeleton/plan.md:119:| K02 | kernel 単体(R03 境界可視化) | `make_cuboid(1e-9, 1.0, 1.0, ..)`(near-zero 正値) | 現状は `Ok`(exact `<= 0.0` 方針)であることを assert し境界を明文化 |
features/18-api-skeleton/plan.md:121:> **R05 反論**(round 1): Euler-Poincaré は kernel(`cylinder.rs:180-195`)、golden YAML roundtrip は format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。
features/18-api-skeleton/plan.md:123:> **R03 判断**(round 2): 退化寸法の閾値は `make_cylinder` と同じ exact `<= 0.0` で対称化する。**グローバル公差(near-zero の扱い)は ROADMAP の Issue #17「単位系・グローバル公差の早期固定(ADR-004 範囲)」が所有する未決事項**であり、#18 で独自 EPS を導入すると #17 の設計判断を先取りしてしまう。よって #18 は exact-zero 方針に留め、near-zero 正値が現状 `Ok` となる境界を K02 で明文化する(将来 #17 で公差確定後に統一)。
features/18-api-skeleton/plan.md:125:> **R04 反論**(round 2): 新規 `crates/mycad-api/tests/fixtures/*.mycad` は T07-T09 で API 経由で必ずパースされる(パースできなければ期待する 422 が出ず test fail)ため、入力形状の妥当性は API テストで担保済み。`Document` の YAML roundtrip 安定性は `mycad-format` の既存 roundtrip テストが所有。クロスクレートの roundtrip 追加は「テストは所有クレートに置く」規約に反するため行わない。fixture は最小に保つ。
features/18-api-skeleton/fix-plan.md:18:## F02 (最重要): Host ヘッダ検証 middleware + T06 修正
features/18-api-skeleton/fix-plan.md:57:### T06 の期待値を 403 に修正
features/18-api-skeleton/fix-plan.md:62:- コメント「Without explicit host validation middleware...」を削除
features/18-api-skeleton/fix-plan.md:94:- T06 が `StatusCode::FORBIDDEN` を返すことを確認
features/18-api-skeleton/ci.log:31:test t08_degenerate_dimension ... ok
features/18-api-skeleton/ci.log:72:test document::tests::test_roundtrip_yaml ... ok
features/18-api-skeleton/ci.log:76:test document::tests::test_assembly_roundtrip ... ok
features/18-api-skeleton/ci.log:89:test geometry::surface::tests::test_uv_of_roundtrip_cone ... ok
features/18-api-skeleton/ci.log:90:test geometry::surface::tests::test_uv_of_roundtrip_cylinder ... ok
features/18-api-skeleton/ci.log:91:test geometry::surface::tests::test_uv_of_roundtrip_plane ... ok
features/18-api-skeleton/ci.log:92:test geometry::surface::tests::test_uv_of_roundtrip_sphere ... ok
features/18-api-skeleton/ci.log:93:test primitives::cuboid::tests::test_cuboid_deterministic ... ok
features/18-api-skeleton/ci.log:94:test primitives::cuboid::tests::test_cuboid_topology ... ok
features/18-api-skeleton/ci.log:95:test primitives::cuboid::tests::test_make_cuboid_invalid_dimensions ... ok
features/18-api-skeleton/ci.log:96:test primitives::cuboid::tests::test_cuboid_vertex_positions ... ok
features/18-api-skeleton/ci.log:97:test primitives::cuboid::tests::test_make_cuboid_near_zero_positive_ok ... ok
features/18-api-skeleton/ci.log:98:test primitives::cylinder::tests::test_cylinder_degenerate_inputs ... ok
features/18-api-skeleton/ci.log:105:test tessellation::stl::tests::ascii_stl_cuboid_structure ... ok
features/18-api-skeleton/ci.log:107:test tessellation::tests::test_cuboid_positions_indices_snapshot ... ok
features/18-api-skeleton/ci.log:109:test tessellation::tests::test_tessellate_cuboid ... ok
features/18-api-skeleton/ci.log:110:test tessellation::tests::test_tessellate_cuboid_unchanged ... ok
features/18-api-skeleton/ci.log:122:     Running tests/cuboid_roundtrip.rs (/mnt/build/cargo-target/debug/deps/cuboid_roundtrip-88c9797a9eefc77b)
features/18-api-skeleton/ci.log:125:test cuboid_to_mesh_pipeline ... ok
features/18-api-skeleton/ci.log:126:test cuboid_pipeline_is_deterministic ... ok
features/18-api-skeleton/glm-result-fix.json.raw:1:{"type":"result","subtype":"success","is_error":false,"api_error_status":null,"duration_ms":190285,"duration_api_ms":177980,"ttft_ms":8393,"num_turns":22,"result":"完了しました。実装内容のサマリ:\n\n- **F01**: `tower` dev-dep に `features = [\"util\"]` を追加\n- **F02**: `host_guard` middleware を追加（Host ヘッダが loopback 以外なら 403）、T06 を `StatusCode::FORBIDDEN` に修正\n- **F03**: `ApiError` を `#[derive(Debug, thiserror::Error)]` に変更、手書き `Display` impl を削除\n- **F04**: `MeshQuery` と `get_mesh` を `pub(crate)` に変更（clippy `private_interfaces` 対応で `get_mesh` も必要に応じて変更）\n\n`cargo xtask ci` 全チェック通過（build / 59 tests / clippy -D warnings / fmt check）。","stop_reason":"end_turn","session_id":"b78d2783-7796-4ee3-803e-2dc6ceb091de","total_cost_usd":0.6203400000000001,"usage":{"input_tokens":66309,"cache_creation_input_tokens":0,"cache_read_input_tokens":412544,"output_tokens":3055,"server_tool_use":{"web_search_requests":0,"web_fetch_requests":0},"service_tier":"standard","cache_creation":{"ephemeral_1h_input_tokens":0,"ephemeral_5m_input_tokens":0},"inference_geo":"","iterations":[],"speed":"standard"},"modelUsage":{"GLM-5.1":{"inputTokens":67433,"outputTokens":3071,"cacheReadInputTokens":412800,"cacheCreationInputTokens":0,"webSearchRequests":0,"costUSD":0.6203400000000001,"contextWindow":200000,"maxOutputTokens":32000}},"permission_denials":[],"terminal_reason":"completed","fast_mode_state":"off","uuid":"bad7421e-0874-43fa-90d2-8f7d864b8794"}
features/12-cylinder/final-review-2.md:59: /// Minimum cross-product norm for a non-degenerate triangle.
features/12-cylinder/final-review-2.md:316: /// Push a triangle, but skip degenerate (zero-area) ones.
features/12-cylinder/final-review-2.md:410:     fn test_tessellate_cuboid() {
features/12-cylinder/final-review-2.md:412:         let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/12-cylinder/final-review-2.md:426:         let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/12-cylinder/final-review-2.md:427:         let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
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
features/12-cylinder/final-review-2.md:1435:crates/mycad-kernel/src/primitives/cuboid.rs:72:            solid.add_edge(
features/12-cylinder/design-review.md:45:- golden YAML ラウンドトリップテストがあるか
features/12-cylinder/design-review.md:79:ROADMAP Phase 3「円柱・球・押し出しが作れる」の最初の一歩。現状カーネルは `make_cuboid` と平面専用の `tessellate_solid` しか持たない。本 Issue で **解析曲面(円柱)を厳密 B-rep として生成し、それを三角形メッシュへ離散化する土台**を確立し、`mycad export examples/cylinder.mycad -o out.stl` を動作させる。
features/12-cylinder/design-review.md:125:**決定性**: ID 採番順を `make_cuboid` と同型に固定。solid → 頂点(v_bot,v_top) → エッジ(e_bot,e_top,e_seam) → 面ごとに[底,上,側]順で HE群→loop→face → shell。すべて `id_gen.next()` を固定順で呼ぶのみ。`HashMap`/乱数等の非決定要素なし。
features/12-cylinder/design-review.md:127:**退化**: `radius <= 0` または `height <= 0` はゼロ面積/ゼロ長を生むため将来的にエラー化対象だが、現状 `make_cuboid` も正値前提で検証していない。本 Issue は **正値前提**を踏襲し、退化はテストで「正値での妥当性」を担保（負値の明示エラーはスコープ外、必要なら Codex 指摘で追加検討）。
features/12-cylinder/design-review.md:141:  - `Curve::sample_segment(&self, t_range, segments) -> Vec<Point>`: t0 から t1 手前までの点列（終点は次 HE の始点が担うので除外）。`Line` は `vec![evaluate(t0)]`（1 点）、`Circle` は `segments` 等分点。→ 直線エッジは従来どおり始点 1 点のみ＝**cuboid 出力は不変**。円エッジは折れ線化。
features/12-cylinder/design-review.md:145:  2. `surface.requires_interior_sampling()` が **false（平面）** → 境界点列を **頂点0 から fan**（凸前提。cuboid 4 角=従来 2 三角形で不変、円盤キャップ=N角形 fan）。
features/12-cylinder/design-review.md:148:- **STL 外向き保証**: 各三角形の幾何法線（辺の外積）が、その面の外向き法線（`normal_at_point` を `same_sense` で調整）と逆なら 2 頂点をスワップして向きを揃える。これでキャップ/側面とも STL facet が外向き。cuboid は既に外向きのためスワップ発生せず、出力は実質不変（決定性・三角形数も維持）。
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
features/12-cylinder/design-review.md:191:1. `cargo test --workspace` で T01–T12 green。
features/12-cylinder/design-review.md:201:設計ドキュメントを MyCad の実装規約に照らして確認します。まず既存の `make_cuboid`、トポロジー型、テッセレーション、ADR とテスト構成を読み、提案内容と矛盾する点を特定します。
features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/12-cylinder/design-review.md:269:crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/12-cylinder/design-review.md:277:crates/mycad-kernel/src/primitives/cuboid.rs
features/12-cylinder/design-review.md:421:CLAUDE.md:29:│   ├── primitives/   プリミティブ生成 (make_cuboid)
features/12-cylinder/design-review.md:434:.claude/skills/3ai/agents/codex-final-reviewer.md:49:    suggestion: "make_cuboid の edge 生成ロジックを見直す"
features/12-cylinder/design-review.md:438:ROADMAP.md:17:- `make_cuboid` → `tessellate_solid` のパイプラインが動作し、決定性テストが通る
features/12-cylinder/design-review.md:450:features/12-cylinder/design-review.md:79:ROADMAP Phase 3「円柱・球・押し出しが作れる」の最初の一歩。現状カーネルは `make_cuboid` と平面専用の `tessellate_solid` しか持たない。本 Issue で **解析曲面(円柱)を厳密 B-rep として生成し、それを三角形メッシュへ離散化する土台**を確立し、`mycad export examples/cylinder.mycad -o out.stl` を動作させる。
features/12-cylinder/design-review.md:454:features/12-cylinder/design-review.md:125:**決定性**: ID 採番順を `make_cuboid` と同型に固定。solid → 頂点(v_bot,v_top) → エッジ(e_bot,e_top,e_seam) → 面ごとに[底,上,側]順で HE群→loop→face → shell。すべて `id_gen.next()` を固定順で呼ぶのみ。`HashMap`/乱数等の非決定要素なし。
features/12-cylinder/design-review.md:455:features/12-cylinder/design-review.md:127:**退化**: `radius <= 0` または `height <= 0` はゼロ面積/ゼロ長を生むため将来的にエラー化対象だが、現状 `make_cuboid` も正値前提で検証していない。本 Issue は **正値前提**を踏襲し、退化はテストで「正値での妥当性」を担保（負値の明示エラーはスコープ外、必要なら Codex 指摘で追加検討）。
features/12-cylinder/design-review.md:463:features/12-cylinder/design-review.md:176:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/12-cylinder/design-review.md:464:features/12-cylinder/design-review.md:181:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
features/12-cylinder/design-review.md:465:features/12-cylinder/design-review.md:183:| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index | 既存テスト維持 |
features/12-cylinder/design-review.md:467:features/12-cylinder/design-review.md:201:設計ドキュメントを MyCad の実装規約に照らして確認します。まず既存の `make_cuboid`、トポロジー型、テッセレーション、ADR とテスト構成を読み、提案内容と矛盾する点を特定します。
features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/12-cylinder/design-review.md:496:crates/mycad-format/CLAUDE.md:33:| `CreateBox`    | `make_cuboid`  |
features/12-cylinder/design-review.md:509:crates/mycad-build/src/lib.rs:4:use mycad_kernel::primitives::make_cuboid;
features/12-cylinder/design-review.md:511:crates/mycad-build/src/lib.rs:26:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
features/12-cylinder/design-review.md:523:crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
features/12-cylinder/design-review.md:531:crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
features/12-cylinder/design-review.md:533:crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/12-cylinder/design-review.md:537:crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/12-cylinder/design-review.md:538:crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/12-cylinder/design-review.md:545:crates/mycad-kernel/src/tessellation/stl.rs:52:    use crate::primitives::make_cuboid;
features/12-cylinder/design-review.md:548:crates/mycad-kernel/src/tessellation/stl.rs:58:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/12-cylinder/design-review.md:551:crates/mycad-kernel/src/tessellation/stl.rs:75:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/12-cylinder/design-review.md:553:crates/mycad-kernel/src/primitives/cuboid.rs:1:use crate::brep::topology::{IdGenerator, Solid};
features/12-cylinder/design-review.md:554:crates/mycad-kernel/src/primitives/cuboid.rs:16:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/12-cylinder/design-review.md:555:crates/mycad-kernel/src/primitives/cuboid.rs:223:    use crate::brep::topology::IdGenerator;
features/12-cylinder/design-review.md:556:crates/mycad-kernel/src/primitives/cuboid.rs:227:        let mut id_gen = IdGenerator::new(0);
features/12-cylinder/design-review.md:557:crates/mycad-kernel/src/primitives/cuboid.rs:228:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/12-cylinder/design-review.md:558:crates/mycad-kernel/src/primitives/cuboid.rs:247:        let mut id_gen = IdGenerator::new(0);
features/12-cylinder/design-review.md:559:crates/mycad-kernel/src/primitives/cuboid.rs:248:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
features/12-cylinder/design-review.md:560:crates/mycad-kernel/src/primitives/cuboid.rs:276:        let mut id_gen1 = IdGenerator::new(0);
features/12-cylinder/design-review.md:561:crates/mycad-kernel/src/primitives/cuboid.rs:277:        let mut id_gen2 = IdGenerator::new(0);
features/12-cylinder/design-review.md:562:crates/mycad-kernel/src/primitives/cuboid.rs:279:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
features/12-cylinder/design-review.md:563:crates/mycad-kernel/src/primitives/cuboid.rs:280:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);
features/12-cylinder/design-review.md:575:crates/mycad-kernel/benches/tessellation.rs:2:use mycad_kernel::primitives::make_cuboid;
features/12-cylinder/design-review.md:578:crates/mycad-kernel/benches/tessellation.rs:15:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/12-cylinder/design-review.md:580:crates/mycad-kernel/tests/cuboid_roundtrip.rs:1:use mycad_kernel::brep::topology::IdGenerator;
features/12-cylinder/design-review.md:581:crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
features/12-cylinder/design-review.md:582:crates/mycad-kernel/tests/cuboid_roundtrip.rs:3:use mycad_kernel::tessellation::tessellate_solid;
features/12-cylinder/design-review.md:583:crates/mycad-kernel/tests/cuboid_roundtrip.rs:8:    let mut id_gen = IdGenerator::new(0);
features/12-cylinder/design-review.md:584:crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/12-cylinder/design-review.md:585:crates/mycad-kernel/tests/cuboid_roundtrip.rs:17:    let mesh = tessellate_solid(&solid);
features/12-cylinder/design-review.md:586:crates/mycad-kernel/tests/cuboid_roundtrip.rs:30:        let mut id_gen = IdGenerator::new(42);
features/12-cylinder/design-review.md:587:crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/12-cylinder/design-review.md:588:crates/mycad-kernel/tests/cuboid_roundtrip.rs:32:        tessellate_solid(&solid)
features/12-cylinder/design-review.md:827:/bin/bash -lc "sed -n '1,340p' crates/mycad-kernel/src/primitives/cuboid.rs" in /home/bacon/mycad
features/12-cylinder/design-review.md:835:/// Create a cuboid (box) B-rep solid centered at the origin.
features/12-cylinder/design-review.md:843:/// The cuboid spans from (-dx/2, -dy/2, -dz/2) to (dx/2, dy/2, dz/2).
features/12-cylinder/design-review.md:844:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/12-cylinder/design-review.md:851:    // 8 vertices of the cuboid
features/12-cylinder/design-review.md:877:    // 12 edges of the cuboid
features/12-cylinder/design-review.md:912:    // 6 faces of the cuboid
features/12-cylinder/design-review.md:1054:    fn test_cuboid_topology() {
features/12-cylinder/design-review.md:1056:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/12-cylinder/design-review.md:1058:        assert_eq!(solid.vertices.len(), 8, "cuboid should have 8 vertices");
features/12-cylinder/design-review.md:1059:        assert_eq!(solid.edges.len(), 12, "cuboid should have 12 edges");
features/12-cylinder/design-review.md:1060:        assert_eq!(solid.faces.len(), 6, "cuboid should have 6 faces");
features/12-cylinder/design-review.md:1061:        assert_eq!(solid.shells.len(), 1, "cuboid should have 1 shell");
features/12-cylinder/design-review.md:1062:        assert!(solid.shells[0].closed, "cuboid shell should be closed");
features/12-cylinder/design-review.md:1067:            "cuboid should have 24 half-edges"
features/12-cylinder/design-review.md:1070:        assert_eq!(solid.loops.len(), 6, "cuboid should have 6 loops");
features/12-cylinder/design-review.md:1074:    fn test_cuboid_vertex_positions() {
features/12-cylinder/design-review.md:1076:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
features/12-cylinder/design-review.md:1101:    fn test_cuboid_deterministic() {
features/12-cylinder/design-review.md:1102:        // Building the same cuboid twice with the same ID generator should
features/12-cylinder/design-review.md:1107:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
features/12-cylinder/design-review.md:1108:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);
features/12-cylinder/design-review.md:1270:    use crate::primitives::make_cuboid;
features/12-cylinder/design-review.md:1273:    fn test_tessellate_cuboid() {
features/12-cylinder/design-review.md:1275:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/12-cylinder/design-review.md:1278:        // A cuboid has 6 faces, each tessellated as 2 triangles = 12 triangles
features/12-cylinder/design-review.md:1291:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/12-cylinder/design-review.md:1292:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/12-cylinder/design-review.md:1473:    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
features/12-cylinder/design-review.md:1481:    fn test_uv_of_roundtrip_plane() {
features/12-cylinder/design-review.md:1488:        assert_uv_roundtrip(&surface, 0.7, -1.3);
features/12-cylinder/design-review.md:1492:    fn test_uv_of_roundtrip_cylinder() {
features/12-cylinder/design-review.md:1498:        assert_uv_roundtrip(&surface, 0.9, 1.7);
features/12-cylinder/design-review.md:1502:    fn test_uv_of_roundtrip_sphere() {
features/12-cylinder/design-review.md:1507:        assert_uv_roundtrip(&surface, 1.1, 0.6);
features/12-cylinder/design-review.md:1511:    fn test_uv_of_roundtrip_cone() {
features/12-cylinder/design-review.md:1517:        assert_uv_roundtrip(&surface, 0.8, 2.0);
features/12-cylinder/design-review.md:1660:use mycad_kernel::primitives::make_cuboid;
features/12-cylinder/design-review.md:1682:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
features/12-cylinder/design-review.md:1882:    // simple_box.mycad: width=10, height=20, depth=30 → cuboid topology
features/12-cylinder/design-review.md:1919:        "cuboid should produce 12 facets (6 faces * 2 triangles)"
features/12-cylinder/design-review.md:2094:1. `src/primitives/` に新ファイルを作成（`cuboid.rs` をテンプレートとして使用）
features/12-cylinder/design-review.md:2139:    /// Serialize to YAML string.
features/12-cylinder/design-review.md:2144:    /// Deserialize from YAML string.
features/12-cylinder/design-review.md:2169:    fn test_roundtrip_yaml() {
features/12-cylinder/design-review.md:2200:        assert_eq!(yaml1, yaml2, "YAML serialization must be deterministic");
features/12-cylinder/design-review.md:2230:    fn test_assembly_roundtrip() {
features/12-cylinder/design-review.md:2251:        // Roundtrip: serialize then deserialize yields the same YAML
features/12-cylinder/design-review.md:2259:/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/tests/cuboid_roundtrip.rs" in /home/bacon/mycad
features/12-cylinder/design-review.md:2262:use mycad_kernel::primitives::make_cuboid;
features/12-cylinder/design-review.md:2265:/// Integration test: create a cuboid, tessellate it, and verify the pipeline.
features/12-cylinder/design-review.md:2267:fn cuboid_to_mesh_pipeline() {
features/12-cylinder/design-review.md:2269:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/12-cylinder/design-review.md:2288:fn cuboid_pipeline_is_deterministic() {
features/12-cylinder/design-review.md:2291:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/12-cylinder/design-review.md:2310:`.mycad` ファイルは YAML 形式のテキストファイル。
features/12-cylinder/design-review.md:2394:│    mycad-format     │  永続化層（YAML <-> Document/Feature/Component）
features/12-cylinder/design-review.md:2423:### YAML Format
features/12-cylinder/design-review.md:2425:設計ファイルに YAML を採用。
features/12-cylinder/design-review.md:2449:4. 指摘を指定 YAML 形式に落とす
features/12-cylinder/design-review.md:2486:- golden YAML ラウンドトリップテストがあるか
features/12-cylinder/design-review.md:2520:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/12-cylinder/design-review.md:2521:    suggestion: "`sample_segment` を HalfEdge 向き込みの API に変更し、rev では `t1` から降順にサンプルするか、先頭点を必ず `HalfEdge.start_vertex` から取得すること。reversed line edge を含む cuboid 回帰テストも追加すること。"
features/12-cylinder/design-review.md:2529:    section: "設計方針 > 1. トポロジー / テスト計画 > T04"
features/12-cylinder/design-review.md:2530:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/12-cylinder/design-review.md:2540:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
features/12-cylinder/design-review.md:2550:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/12-cylinder/design-review.md:2551:    suggestion: "`sample_segment` を HalfEdge 向き込みの API に変更し、rev では `t1` から降順にサンプルするか、先頭点を必ず `HalfEdge.start_vertex` から取得すること。reversed line edge を含む cuboid 回帰テストも追加すること。"
features/12-cylinder/design-review.md:2559:    section: "設計方針 > 1. トポロジー / テスト計画 > T04"
features/12-cylinder/design-review.md:2560:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/12-cylinder/design-review.md:2570:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
features/12-cylinder/design-review-3.md:45:- golden YAML ラウンドトリップテストがあるか
features/12-cylinder/design-review-3.md:79:ROADMAP Phase 3「円柱・球・押し出しが作れる」の最初の一歩。現状カーネルは `make_cuboid` と平面専用の `tessellate_solid` しか持たない。本 Issue で **解析曲面(円柱)を厳密 B-rep として生成し、それを三角形メッシュへ離散化する土台**を確立し、`mycad export examples/cylinder.mycad -o out.stl` を動作させる。
features/12-cylinder/design-review-3.md:125:**決定性**: ID 採番順を `make_cuboid` と同型に固定。solid → 頂点(v_bot,v_top) → エッジ(e_bot,e_top,e_seam) → 面ごとに[底,上,側]順で HE群→loop→face → shell。すべて `id_gen.next()` を固定順で呼ぶのみ。`HashMap`/乱数等の非決定要素なし。
features/12-cylinder/design-review-3.md:127:**退化入力の検証（R02）**: `.mycad` 由来のユーザー入力は境界で弾く。`KernelError` に `InvalidParameter { kind: &'static str }` を追加し、`build_solid_from_features` の `CreateCylinder` 分岐で `radius > 0.0 && height > 0.0` を検証（非正なら `InvalidParameter` を返し `make_cylinder` を呼ばない）。`make_cylinder` 自体は `make_cuboid` と同じ `-> Solid` シグネチャを保ち、正値前提（検証は呼び出し側＝境界に集約）。前提条件はドキュメントコメントに明記し、`debug_assert!(radius > 0.0 && height > 0.0)` を先頭に置いて開発時の契約違反を検出（リリースでは境界検証済み前提で素通り）。解像度の退化は `TessellationOptions` のクランプ（前述）で吸収。
features/12-cylinder/design-review-3.md:148:  1. 外側 loop の各 HE について、`forward` なら `(t0,t1)`、`!forward` なら `(t1,t0)` を `sample_segment` に渡して折れ線化 → 連結して境界点列。**先頭点が必ず `HalfEdge.start_vertex` に一致**するため、逆向き Line を含む cuboid でも従来と同一の角点列＝**平面出力不変**。
features/12-cylinder/design-review-3.md:149:  2. `BoundaryFan`（平面） → 境界点列を **頂点0 から fan**（凸前提。cuboid 4 角=従来 2 三角形で不変、円盤キャップ=N角形 fan）。巻き方向は §1 で構築済み（底キャップ rev / 上キャップ fwd）なので fan の幾何法線が外向きになる。**スワップなし → cuboid のインデックス列もバイト不変**（T08 で固定）。
features/12-cylinder/design-review-3.md:180:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/12-cylinder/design-review-3.md:182:- R05(medium) 反映: golden YAML ラウンドトリップテスト T13 追加。
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
features/12-cylinder/design-review-3.md:206:| T13 | golden YAML ラウンドトリップ | `examples/cylinder.mycad` を `from_path`→`to_yaml`→`from_yaml`→`to_yaml` で equality。`create_cylinder` の serialize/schema 回帰を固定 | `assert_eq!` (R05) |
features/12-cylinder/design-review-3.md:211:1. `cargo test --workspace` で T01–T12 green。
features/12-cylinder/design-review-3.md:221:設計ドキュメントをコードベースの前提と照らしてレビューします。まず既存の `make_cuboid`、トポロジー型、テッセレーション実装、エラー型を確認して、この提案が MyCad の規約と実装実態に噛み合うかを見ます。
features/12-cylinder/design-review-3.md:223:/bin/bash -lc "rg -n \"make_cuboid|tessellate_solid|IdGenerator|KernelError|HalfEdge|inner_loops|Surface::|Curve::|TriangleMesh|Feature::CreateCylinder|CreateCylinder|UnsupportedFeature|InvalidParameter|mod.rs|pub use\" crates/mycad-kernel crates/mycad-build crates/mycad-cli -g '"'!target'"'" in /home/bacon/mycad
features/12-cylinder/design-review-3.md:235:crates/mycad-build/src/lib.rs:4:use mycad_kernel::primitives::make_cuboid;
features/12-cylinder/design-review-3.md:240:crates/mycad-build/src/lib.rs:26:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
features/12-cylinder/design-review-3.md:248:crates/mycad-kernel/benches/tessellation.rs:2:use mycad_kernel::primitives::make_cuboid;
features/12-cylinder/design-review-3.md:251:crates/mycad-kernel/benches/tessellation.rs:15:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/12-cylinder/design-review-3.md:260:crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
features/12-cylinder/design-review-3.md:262:crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/12-cylinder/design-review-3.md:266:crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/12-cylinder/design-review-3.md:267:crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/12-cylinder/design-review-3.md:280:crates/mycad-kernel/src/tessellation/stl.rs:52:    use crate::primitives::make_cuboid;
features/12-cylinder/design-review-3.md:283:crates/mycad-kernel/src/tessellation/stl.rs:58:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/12-cylinder/design-review-3.md:286:crates/mycad-kernel/src/tessellation/stl.rs:75:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/12-cylinder/design-review-3.md:290:crates/mycad-kernel/tests/cuboid_roundtrip.rs:1:use mycad_kernel::brep::topology::IdGenerator;
features/12-cylinder/design-review-3.md:291:crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
features/12-cylinder/design-review-3.md:292:crates/mycad-kernel/tests/cuboid_roundtrip.rs:3:use mycad_kernel::tessellation::tessellate_solid;
features/12-cylinder/design-review-3.md:293:crates/mycad-kernel/tests/cuboid_roundtrip.rs:8:    let mut id_gen = IdGenerator::new(0);
features/12-cylinder/design-review-3.md:294:crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/12-cylinder/design-review-3.md:295:crates/mycad-kernel/tests/cuboid_roundtrip.rs:17:    let mesh = tessellate_solid(&solid);
features/12-cylinder/design-review-3.md:296:crates/mycad-kernel/tests/cuboid_roundtrip.rs:30:        let mut id_gen = IdGenerator::new(42);
features/12-cylinder/design-review-3.md:297:crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/12-cylinder/design-review-3.md:298:crates/mycad-kernel/tests/cuboid_roundtrip.rs:32:        tessellate_solid(&solid)
features/12-cylinder/design-review-3.md:319:crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
features/12-cylinder/design-review-3.md:322:crates/mycad-kernel/src/primitives/cuboid.rs:1:use crate::brep::topology::{IdGenerator, Solid};
features/12-cylinder/design-review-3.md:323:crates/mycad-kernel/src/primitives/cuboid.rs:16:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/12-cylinder/design-review-3.md:324:crates/mycad-kernel/src/primitives/cuboid.rs:75:                Curve::Line {
features/12-cylinder/design-review-3.md:325:crates/mycad-kernel/src/primitives/cuboid.rs:103:            surface: Surface::Plane {
features/12-cylinder/design-review-3.md:326:crates/mycad-kernel/src/primitives/cuboid.rs:119:            surface: Surface::Plane {
features/12-cylinder/design-review-3.md:327:crates/mycad-kernel/src/primitives/cuboid.rs:135:            surface: Surface::Plane {
features/12-cylinder/design-review-3.md:328:crates/mycad-kernel/src/primitives/cuboid.rs:151:            surface: Surface::Plane {
features/12-cylinder/design-review-3.md:329:crates/mycad-kernel/src/primitives/cuboid.rs:167:            surface: Surface::Plane {
features/12-cylinder/design-review-3.md:330:crates/mycad-kernel/src/primitives/cuboid.rs:183:            surface: Surface::Plane {
features/12-cylinder/design-review-3.md:331:crates/mycad-kernel/src/primitives/cuboid.rs:223:    use crate::brep::topology::IdGenerator;
features/12-cylinder/design-review-3.md:332:crates/mycad-kernel/src/primitives/cuboid.rs:227:        let mut id_gen = IdGenerator::new(0);
features/12-cylinder/design-review-3.md:333:crates/mycad-kernel/src/primitives/cuboid.rs:228:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/12-cylinder/design-review-3.md:334:crates/mycad-kernel/src/primitives/cuboid.rs:247:        let mut id_gen = IdGenerator::new(0);
features/12-cylinder/design-review-3.md:335:crates/mycad-kernel/src/primitives/cuboid.rs:248:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
features/12-cylinder/design-review-3.md:336:crates/mycad-kernel/src/primitives/cuboid.rs:276:        let mut id_gen1 = IdGenerator::new(0);
features/12-cylinder/design-review-3.md:337:crates/mycad-kernel/src/primitives/cuboid.rs:277:        let mut id_gen2 = IdGenerator::new(0);
features/12-cylinder/design-review-3.md:338:crates/mycad-kernel/src/primitives/cuboid.rs:279:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
features/12-cylinder/design-review-3.md:339:crates/mycad-kernel/src/primitives/cuboid.rs:280:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);
features/12-cylinder/design-review-3.md:371:crates/mycad-kernel/src/primitives/cuboid.rs
features/12-cylinder/design-review-3.md:604:/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/primitives/cuboid.rs" in /home/bacon/mycad
features/12-cylinder/design-review-3.md:612:/// Create a cuboid (box) B-rep solid centered at the origin.
features/12-cylinder/design-review-3.md:620:/// The cuboid spans from (-dx/2, -dy/2, -dz/2) to (dx/2, dy/2, dz/2).
features/12-cylinder/design-review-3.md:621:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/12-cylinder/design-review-3.md:628:    // 8 vertices of the cuboid
features/12-cylinder/design-review-3.md:654:    // 12 edges of the cuboid
features/12-cylinder/design-review-3.md:689:    // 6 faces of the cuboid
features/12-cylinder/design-review-3.md:831:    fn test_cuboid_topology() {
features/12-cylinder/design-review-3.md:833:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/12-cylinder/design-review-3.md:835:        assert_eq!(solid.vertices.len(), 8, "cuboid should have 8 vertices");
features/12-cylinder/design-review-3.md:836:        assert_eq!(solid.edges.len(), 12, "cuboid should have 12 edges");
features/12-cylinder/design-review-3.md:837:        assert_eq!(solid.faces.len(), 6, "cuboid should have 6 faces");
features/12-cylinder/design-review-3.md:838:        assert_eq!(solid.shells.len(), 1, "cuboid should have 1 shell");
features/12-cylinder/design-review-3.md:839:        assert!(solid.shells[0].closed, "cuboid shell should be closed");
features/12-cylinder/design-review-3.md:844:            "cuboid should have 24 half-edges"
features/12-cylinder/design-review-3.md:847:        assert_eq!(solid.loops.len(), 6, "cuboid should have 6 loops");
features/12-cylinder/design-review-3.md:851:    fn test_cuboid_vertex_positions() {
features/12-cylinder/design-review-3.md:853:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
features/12-cylinder/design-review-3.md:968:    use crate::primitives::make_cuboid;
features/12-cylinder/design-review-3.md:971:    fn test_tessellate_cuboid() {
features/12-cylinder/design-review-3.md:973:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/12-cylinder/design-review-3.md:976:        // A cuboid has 6 faces, each tessellated as 2 triangles = 12 triangles
features/12-cylinder/design-review-3.md:989:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/12-cylinder/design-review-3.md:990:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/12-cylinder/design-review-3.md:1171:    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
features/12-cylinder/design-review-3.md:1179:    fn test_uv_of_roundtrip_plane() {
features/12-cylinder/design-review-3.md:1186:        assert_uv_roundtrip(&surface, 0.7, -1.3);
features/12-cylinder/design-review-3.md:1190:    fn test_uv_of_roundtrip_cylinder() {
features/12-cylinder/design-review-3.md:1196:        assert_uv_roundtrip(&surface, 0.9, 1.7);
features/12-cylinder/design-review-3.md:1200:    fn test_uv_of_roundtrip_sphere() {
features/12-cylinder/design-review-3.md:1205:        assert_uv_roundtrip(&surface, 1.1, 0.6);
features/12-cylinder/design-review-3.md:1209:    fn test_uv_of_roundtrip_cone() {
features/12-cylinder/design-review-3.md:1215:        assert_uv_roundtrip(&surface, 0.8, 2.0);
features/12-cylinder/design-review-3.md:1310:use mycad_kernel::primitives::make_cuboid;
features/12-cylinder/design-review-3.md:1332:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
features/12-cylinder/design-review-3.md:1402:/bin/bash -lc "sed -n '1,260p' crates/mycad-build/tests/feature_dispatcher.rs && printf '\\n---\\n' && sed -n '1,260p' crates/mycad-cli/tests/export.rs && printf '\\n---\\n' && sed -n '1,220p' crates/mycad-kernel/tests/cuboid_roundtrip.rs" in /home/bacon/mycad
features/12-cylinder/design-review-3.md:1422:    // simple_box.mycad: width=10, height=20, depth=30 → cuboid topology
features/12-cylinder/design-review-3.md:1457:        "cuboid should produce 12 facets (6 faces * 2 triangles)"
features/12-cylinder/design-review-3.md:1463:use mycad_kernel::primitives::make_cuboid;
features/12-cylinder/design-review-3.md:1466:/// Integration test: create a cuboid, tessellate it, and verify the pipeline.
features/12-cylinder/design-review-3.md:1468:fn cuboid_to_mesh_pipeline() {
features/12-cylinder/design-review-3.md:1470:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/12-cylinder/design-review-3.md:1489:fn cuboid_pipeline_is_deterministic() {
features/12-cylinder/design-review-3.md:1492:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/12-cylinder/design-review-3.md:1528:crates/mycad-format/src/document.rs:25:    /// Serialize to YAML string.
features/12-cylinder/design-review-3.md:1530:crates/mycad-format/src/document.rs:30:    /// Deserialize from YAML string.
features/12-cylinder/design-review-3.md:1558:crates/mycad-format/CLAUDE.md:5:`.mycad` ファイルはYAML形式。`Document` がトップレベルで、`root_component` にComponent階層を持つ。各Componentは `features` (操作履歴) と `children` (子コンポーネント) を持てる。
features/12-cylinder/design-review-3.md:1604:1. `src/primitives/` に新ファイルを作成（`cuboid.rs` をテンプレートとして使用）
features/12-cylinder/design-review-3.md:1623:mod cuboid;
features/12-cylinder/design-review-3.md:1625:pub use cuboid::make_cuboid;
features/12-cylinder/design-review-3.md:1821:    /// Serialize to YAML string.
features/12-cylinder/design-review-3.md:1826:    /// Deserialize from YAML string.
features/12-cylinder/design-review-3.md:1851:    fn test_roundtrip_yaml() {
features/12-cylinder/design-review-3.md:1882:        assert_eq!(yaml1, yaml2, "YAML serialization must be deterministic");
features/12-cylinder/design-review-3.md:1912:    fn test_assembly_roundtrip() {
features/12-cylinder/design-review-3.md:1933:        // Roundtrip: serialize then deserialize yields the same YAML
features/12-cylinder/design-review-3.md:2112:    use crate::primitives::make_cuboid;
features/12-cylinder/design-review-3.md:2116:    fn ascii_stl_cuboid_structure() {
features/12-cylinder/design-review-3.md:2118:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/12-cylinder/design-review-3.md:2135:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/12-cylinder/design-review-3.md:2166:    section: "テスト計画 > T01"
features/12-cylinder/design-review-3.md:2167:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/12-cylinder/design-review-3.md:2196:    section: "テスト計画 > T01"
features/12-cylinder/design-review-3.md:2197:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/18-api-skeleton/design-review-3.md:45:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/design-review-3.md:83:> Codex 設計レビュー round 1 + round 2 反映済み。round1: R01/R03/R04 受諾、R02 kernel 検証追加(ユーザー決定)、R05 部分受諾。round2: R01(構造化エラー)受諾、R02(assembly 判別)受諾、R03(公差)は #17 委譲で反論、R04(fixture roundtrip)反論。
features/18-api-skeleton/design-review-3.md:87:- 新規クレート `crates/mycad-api/`(lib + bin 構成)+ `make_cuboid` の入力検証(kernel)
features/18-api-skeleton/design-review-3.md:98:| `crates/mycad-kernel/src/primitives/cuboid.rs` | `make_cuboid` を `-> Result<Solid, KernelError>` 化。`dx/dy/dz` が非有限 or `<= 0.0` なら `InvalidParameter { kind }`。**(R03)** 閾値は `make_cylinder`(cylinder.rs:16-22)と同じ exact `<= 0.0` で対称化。near-zero 正値の公差判定は導入しない(下記 R03 注記)。インライン test を `.unwrap()` 化 |
features/18-api-skeleton/design-review-3.md:99:| `crates/mycad-build/src/lib.rs` | CreateBox 分岐を `Ok(make_cuboid(...))` → `make_cuboid(...)`(`?` 伝播)に変更 |
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
features/18-api-skeleton/design-review-3.md:190:| K01 | kernel 単体 | `make_cuboid(0.0, 1.0, 1.0, ..)` / 負値 / NaN | `Err(InvalidParameter { kind: "..." })` |
features/18-api-skeleton/design-review-3.md:191:| K02 | kernel 単体(R03 境界可視化) | `make_cuboid(1e-9, 1.0, 1.0, ..)`(near-zero 正値) | 現状は `Ok`(exact `<= 0.0` 方針)であることを assert し境界を明文化 |
features/18-api-skeleton/design-review-3.md:193:> **R05 反論**(round 1): Euler-Poincaré は kernel(`cylinder.rs:180-195`)、golden YAML roundtrip は format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。
features/18-api-skeleton/design-review-3.md:195:> **R03 判断**(round 2): 退化寸法の閾値は `make_cylinder` と同じ exact `<= 0.0` で対称化する。**グローバル公差(near-zero の扱い)は ROADMAP の Issue #17「単位系・グローバル公差の早期固定(ADR-004 範囲)」が所有する未決事項**であり、#18 で独自 EPS を導入すると #17 の設計判断を先取りしてしまう。よって #18 は exact-zero 方針に留め、near-zero 正値が現状 `Ok` となる境界を K02 で明文化する(将来 #17 で公差確定後に統一)。
features/18-api-skeleton/design-review-3.md:197:> **R04 反論**(round 2): 新規 `crates/mycad-api/tests/fixtures/*.mycad` は T07-T09 で API 経由で必ずパースされる(パースできなければ期待する 422 が出ず test fail)ため、入力形状の妥当性は API テストで担保済み。`Document` の YAML roundtrip 安定性は `mycad-format` の既存 roundtrip テストが所有。クロスクレートの roundtrip 追加は「テストは所有クレートに置く」規約に反するため行わない。fixture は最小に保つ。
features/18-api-skeleton/design-review-3.md:217:/bin/bash -lc "rg -n \"enum FormatError|struct TriangleMesh|enum KernelError|enum TessellationError|fn from_path|build_solid_from_features|make_cuboid|make_cylinder|EmptyFeatureList|InvalidReference\" crates -g '*.rs'" in /home/bacon/mycad
features/18-api-skeleton/design-review-3.md:226:crates/mycad-build/src/lib.rs:4:use mycad_kernel::primitives::{make_cuboid, make_cylinder};
features/18-api-skeleton/design-review-3.md:229:crates/mycad-build/src/lib.rs:26:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
features/18-api-skeleton/design-review-3.md:235:crates/mycad-kernel/benches/tessellation.rs:2:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-3.md:236:crates/mycad-kernel/benches/tessellation.rs:15:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-3.md:237:crates/mycad-kernel/src/primitives/mod.rs:4:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-3.md:239:crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-3.md:240:crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-3.md:241:crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/18-api-skeleton/design-review-3.md:244:crates/mycad-kernel/src/tessellation/mod.rs:315:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-3.md:249:crates/mycad-kernel/src/tessellation/mod.rs:358:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
features/18-api-skeleton/design-review-3.md:250:crates/mycad-kernel/src/tessellation/mod.rs:373:        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
features/18-api-skeleton/design-review-3.md:251:crates/mycad-kernel/src/tessellation/mod.rs:374:        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
features/18-api-skeleton/design-review-3.md:254:crates/mycad-kernel/src/tessellation/mod.rs:500:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review-3.md:255:crates/mycad-kernel/src/tessellation/mod.rs:514:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review-3.md:256:crates/mycad-kernel/src/tessellation/mod.rs:515:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review-3.md:273:crates/mycad-kernel/src/tessellation/stl.rs:52:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-3.md:274:crates/mycad-kernel/src/tessellation/stl.rs:58:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/18-api-skeleton/design-review-3.md:275:crates/mycad-kernel/src/tessellation/stl.rs:75:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/18-api-skeleton/design-review-3.md:276:crates/mycad-kernel/src/primitives/cuboid.rs:16:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/18-api-skeleton/design-review-3.md:277:crates/mycad-kernel/src/primitives/cuboid.rs:228:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-3.md:278:crates/mycad-kernel/src/primitives/cuboid.rs:248:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
features/18-api-skeleton/design-review-3.md:279:crates/mycad-kernel/src/primitives/cuboid.rs:279:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
features/18-api-skeleton/design-review-3.md:280:crates/mycad-kernel/src/primitives/cuboid.rs:280:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);
features/18-api-skeleton/design-review-3.md:293:既存の `FormatError` と `TriangleMesh`、`make_cuboid`/`make_cylinder` の現状を確認します。レビューの論点になりそうなのは、エラー分類の意味と既存の決定性テストの有無です。
features/18-api-skeleton/design-review-3.md:304:    #[error("YAML parse error: {0}")]
features/18-api-skeleton/design-review-3.md:341:    /// Serialize to YAML string.
features/18-api-skeleton/design-review-3.md:346:    /// Deserialize from YAML string.
features/18-api-skeleton/design-review-3.md:371:    fn test_roundtrip_yaml() {
features/18-api-skeleton/design-review-3.md:402:        assert_eq!(yaml1, yaml2, "YAML serialization must be deterministic");
features/18-api-skeleton/design-review-3.md:432:    fn test_assembly_roundtrip() {
features/18-api-skeleton/design-review-3.md:453:        // Roundtrip: serialize then deserialize yields the same YAML
features/18-api-skeleton/design-review-3.md:495:/// Minimum cross-product norm for a non-degenerate triangle.
features/18-api-skeleton/design-review-3.md:749:/bin/bash -lc "sed -n '1,120p' crates/mycad-kernel/src/primitives/cuboid.rs" in /home/bacon/mycad
features/18-api-skeleton/design-review-3.md:757:/// Create a cuboid (box) B-rep solid centered at the origin.
features/18-api-skeleton/design-review-3.md:765:/// The cuboid spans from (-dx/2, -dy/2, -dz/2) to (dx/2, dy/2, dz/2).
features/18-api-skeleton/design-review-3.md:766:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/18-api-skeleton/design-review-3.md:773:    // 8 vertices of the cuboid
features/18-api-skeleton/design-review-3.md:799:    // 12 edges of the cuboid
features/18-api-skeleton/design-review-3.md:834:    // 6 faces of the cuboid
features/18-api-skeleton/design-review-3.md:878:use mycad_kernel::primitives::{make_cuboid, make_cylinder};
features/18-api-skeleton/design-review-3.md:900:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
features/18-api-skeleton/design-review-3.md:1061:/bin/bash -lc "sed -n '160,380p' crates/mycad-kernel/src/primitives/cuboid.rs" in /home/bacon/mycad
features/18-api-skeleton/design-review-3.md:1129:    fn test_cuboid_topology() {
features/18-api-skeleton/design-review-3.md:1131:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-3.md:1133:        assert_eq!(solid.vertices.len(), 8, "cuboid should have 8 vertices");
features/18-api-skeleton/design-review-3.md:1134:        assert_eq!(solid.edges.len(), 12, "cuboid should have 12 edges");
features/18-api-skeleton/design-review-3.md:1135:        assert_eq!(solid.faces.len(), 6, "cuboid should have 6 faces");
features/18-api-skeleton/design-review-3.md:1136:        assert_eq!(solid.shells.len(), 1, "cuboid should have 1 shell");
features/18-api-skeleton/design-review-3.md:1137:        assert!(solid.shells[0].closed, "cuboid shell should be closed");
features/18-api-skeleton/design-review-3.md:1142:            "cuboid should have 24 half-edges"
features/18-api-skeleton/design-review-3.md:1145:        assert_eq!(solid.loops.len(), 6, "cuboid should have 6 loops");
features/18-api-skeleton/design-review-3.md:1149:    fn test_cuboid_vertex_positions() {
features/18-api-skeleton/design-review-3.md:1151:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
features/18-api-skeleton/design-review-3.md:1176:    fn test_cuboid_deterministic() {
features/18-api-skeleton/design-review-3.md:1177:        // Building the same cuboid twice with the same ID generator should
features/18-api-skeleton/design-review-3.md:1182:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
features/18-api-skeleton/design-review-3.md:1183:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);
features/18-api-skeleton/design-review-3.md:1205:既存の cuboid/cylinder テストにどこまで決定性・Euler 検証があるかを見ます。今回の計画がその既存担保を正しく引用しているかを確認します。
features/18-api-skeleton/design-review-3.md:1210:crates/mycad-kernel/tests/cuboid_roundtrip.rs:26:/// Integration test: deterministic pipeline produces identical results.
features/18-api-skeleton/design-review-3.md:1211:crates/mycad-kernel/tests/cuboid_roundtrip.rs:28:fn cuboid_pipeline_is_deterministic() {
features/18-api-skeleton/design-review-3.md:1213:crates/mycad-format/src/document.rs:86:        assert_eq!(yaml1, yaml2, "YAML serialization must be deterministic");
features/18-api-skeleton/design-review-3.md:1215:crates/mycad-kernel/src/primitives/cylinder.rs:180:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review-3.md:1225:crates/mycad-kernel/src/primitives/cuboid.rs:13:/// - `id_gen`: deterministic ID generator
features/18-api-skeleton/design-review-3.md:1226:crates/mycad-kernel/src/primitives/cuboid.rs:273:    fn test_cuboid_deterministic() {
features/18-api-skeleton/design-review-3.md:1244:    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
features/18-api-skeleton/design-review-3.md:1272:    /// T02: Topology counts.
features/18-api-skeleton/design-review-3.md:1291:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review-3.md:1310:    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
features/18-api-skeleton/design-review-3.md:1363:    /// T07: Determinism — same input → same output.
features/18-api-skeleton/design-review-3.md:1380:    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
features/18-api-skeleton/design-review-3.md:1382:    fn test_tessellate_cuboid_unchanged() {
features/18-api-skeleton/design-review-3.md:1384:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
features/18-api-skeleton/design-review-3.md:1393:    /// T08 extended: exact positions and indices snapshot.
features/18-api-skeleton/design-review-3.md:1395:    fn test_cuboid_positions_indices_snapshot() {
features/18-api-skeleton/design-review-3.md:1399:        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
features/18-api-skeleton/design-review-3.md:1400:        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
features/18-api-skeleton/design-review-3.md:1410:    /// T09: Outward normals — all facet normals point away from centroid.
features/18-api-skeleton/design-review-3.md:1524:    fn test_tessellate_cuboid() {
features/18-api-skeleton/design-review-3.md:1526:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review-3.md:1540:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review-3.md:1541:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review-3.md:1816:/bin/bash -lc "sed -n '1,140p' crates/mycad-kernel/tests/cuboid_roundtrip.rs" in /home/bacon/mycad
features/18-api-skeleton/design-review-3.md:1819:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-3.md:1822:/// Integration test: create a cuboid, tessellate it, and verify the pipeline.
features/18-api-skeleton/design-review-3.md:1824:fn cuboid_to_mesh_pipeline() {
features/18-api-skeleton/design-review-3.md:1826:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-3.md:1845:fn cuboid_pipeline_is_deterministic() {
features/18-api-skeleton/design-review-3.md:1848:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/18-api-skeleton/design-review-3.md:1882:    section: "テスト計画 > T09"
features/18-api-skeleton/design-review-3.md:1883:    finding: "設計本文では assembly/reference を build 前に検出して専用の 422 にするとしている一方、T09 の期待値は `422(EmptyFeatureList)` のままで矛盾している。これでは assembly と空 part を区別しない旧挙動でもテストが通る。"
features/18-api-skeleton/design-review-3.md:1884:    suggestion: "T09 は assembly/reference 専用メッセージを明示的に assert し、`features.is_empty()` の空 part は別テストに分離すること。"
features/18-api-skeleton/design-review-3.md:1888:    section: "テスト計画 > K01"
features/18-api-skeleton/design-review-3.md:1889:    finding: "`make_cuboid` の新仕様は `non-finite` を拒否すると書いているが、K01 の列挙は `0.0`/負値/`NaN` までで、`INFINITY`/`-INFINITY` の分岐が未検証である。"
features/18-api-skeleton/design-review-3.md:1916:    section: "テスト計画 > T09"
features/18-api-skeleton/design-review-3.md:1917:    finding: "設計本文では assembly/reference を build 前に検出して専用の 422 にするとしている一方、T09 の期待値は `422(EmptyFeatureList)` のままで矛盾している。これでは assembly と空 part を区別しない旧挙動でもテストが通る。"
features/18-api-skeleton/design-review-3.md:1918:    suggestion: "T09 は assembly/reference 専用メッセージを明示的に assert し、`features.is_empty()` の空 part は別テストに分離すること。"
features/18-api-skeleton/design-review-3.md:1922:    section: "テスト計画 > K01"
features/18-api-skeleton/design-review-3.md:1923:    finding: "`make_cuboid` の新仕様は `non-finite` を拒否すると書いているが、K01 の列挙は `0.0`/負値/`NaN` までで、`INFINITY`/`-INFINITY` の分岐が未検証である。"
features/18-api-skeleton/design-review.md:45:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/design-review.md:146:| T01 | 正常系 | `GET /api/v0/mesh?file=examples/simple_box.mycad` を oneshot で叩く | 200、body を `TriangleMesh` に deser でき `positions`/`indices` が非空、`indices.len() % 3 == 0` |
features/18-api-skeleton/design-review.md:147:| T02 | 決定性 | 同一リクエストを 2 回実行 | 返る JSON バイト列が完全一致 |
features/18-api-skeleton/design-review.md:148:| T03 | 404 | 存在しない `file=examples/nope.mycad` | 404、body `{"error": ...}` |
features/18-api-skeleton/design-review.md:149:| T04 | 400 | 非 `.mycad` 拡張子 `file=Cargo.toml` | 400(InvalidExtension) |
features/18-api-skeleton/design-review.md:150:| T05 | 403 | `Host: evil.com` ヘッダ付与 | 403(Host 検証) |
features/18-api-skeleton/design-review.md:151:| T06 | 422 | 複数/未対応フィーチャ `file=examples/assembly.mycad` | 422 系エラー(build/tessellate 失敗を JSON で返す) |
features/18-api-skeleton/design-review.md:198:/home/bacon/mycad/crates/mycad-kernel/src/primitives/cuboid.rs
features/18-api-skeleton/design-review.md:204:/home/bacon/mycad/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/design-review.md:237:crates/mycad-format/src/document.rs:25:    /// Serialize to YAML string.
features/18-api-skeleton/design-review.md:238:crates/mycad-format/src/document.rs:30:    /// Deserialize from YAML string.
features/18-api-skeleton/design-review.md:276:crates/mycad-kernel/src/primitives/cylinder.rs:180:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review.md:590:use mycad_kernel::primitives::{make_cuboid, make_cylinder};
features/18-api-skeleton/design-review.md:612:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
features/18-api-skeleton/design-review.md:651:    /// Serialize to YAML string.
features/18-api-skeleton/design-review.md:656:    /// Deserialize from YAML string.
features/18-api-skeleton/design-review.md:681:    fn test_roundtrip_yaml() {
features/18-api-skeleton/design-review.md:712:        assert_eq!(yaml1, yaml2, "YAML serialization must be deterministic");
features/18-api-skeleton/design-review.md:742:    fn test_assembly_roundtrip() {
features/18-api-skeleton/design-review.md:763:        // Roundtrip: serialize then deserialize yields the same YAML
features/18-api-skeleton/design-review.md:780:    #[error("YAML parse error: {0}")]
features/18-api-skeleton/design-review.md:805:/// Minimum cross-product norm for a non-degenerate triangle.
features/18-api-skeleton/design-review.md:1073:/// Push a triangle, but skip degenerate (zero-area) ones.
features/18-api-skeleton/design-review.md:1112:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review.md:1115:    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
features/18-api-skeleton/design-review.md:1134:    /// T07: Determinism — same input → same output.
features/18-api-skeleton/design-review.md:1151:    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
features/18-api-skeleton/design-review.md:1153:    fn test_tessellate_cuboid_unchanged() {
features/18-api-skeleton/design-review.md:1155:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
features/18-api-skeleton/design-review.md:1164:    /// T08 extended: exact positions and indices snapshot.
features/18-api-skeleton/design-review.md:1166:    fn test_cuboid_positions_indices_snapshot() {
features/18-api-skeleton/design-review.md:1170:        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
features/18-api-skeleton/design-review.md:1171:        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
features/18-api-skeleton/design-review.md:1181:    /// T09: Outward normals — all facet normals point away from centroid.
features/18-api-skeleton/design-review.md:1295:    fn test_tessellate_cuboid() {
features/18-api-skeleton/design-review.md:1297:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review.md:1311:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review.md:1312:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review.md:1320:/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/primitives/cuboid.rs" in /home/bacon/mycad
features/18-api-skeleton/design-review.md:1328:/// Create a cuboid (box) B-rep solid centered at the origin.
features/18-api-skeleton/design-review.md:1336:/// The cuboid spans from (-dx/2, -dy/2, -dz/2) to (dx/2, dy/2, dz/2).
features/18-api-skeleton/design-review.md:1337:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/18-api-skeleton/design-review.md:1344:    // 8 vertices of the cuboid
features/18-api-skeleton/design-review.md:1370:    // 12 edges of the cuboid
features/18-api-skeleton/design-review.md:1405:    // 6 faces of the cuboid
features/18-api-skeleton/design-review.md:1547:    fn test_cuboid_topology() {
features/18-api-skeleton/design-review.md:1549:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review.md:1551:        assert_eq!(solid.vertices.len(), 8, "cuboid should have 8 vertices");
features/18-api-skeleton/design-review.md:1552:        assert_eq!(solid.edges.len(), 12, "cuboid should have 12 edges");
features/18-api-skeleton/design-review.md:1553:        assert_eq!(solid.faces.len(), 6, "cuboid should have 6 faces");
features/18-api-skeleton/design-review.md:1554:        assert_eq!(solid.shells.len(), 1, "cuboid should have 1 shell");
features/18-api-skeleton/design-review.md:1555:        assert!(solid.shells[0].closed, "cuboid shell should be closed");
features/18-api-skeleton/design-review.md:1560:            "cuboid should have 24 half-edges"
features/18-api-skeleton/design-review.md:1563:        assert_eq!(solid.loops.len(), 6, "cuboid should have 6 loops");
features/18-api-skeleton/design-review.md:1567:    fn test_cuboid_vertex_positions() {
features/18-api-skeleton/design-review.md:1569:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
features/18-api-skeleton/design-review.md:1718:    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
features/18-api-skeleton/design-review.md:1746:    /// T02: Topology counts.
features/18-api-skeleton/design-review.md:1765:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review.md:1784:    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
features/18-api-skeleton/design-review.md:1839:    /// T05: Geometry — seam vertices and surface coefficients.
features/18-api-skeleton/design-review.md:1886:    fn test_cylinder_degenerate_inputs() {
features/18-api-skeleton/design-review.md:1985:1. `src/primitives/` に新ファイルを作成（`cuboid.rs` をテンプレートとして使用）
features/18-api-skeleton/design-review.md:2349:        "cuboid should produce 12 facets (6 faces * 2 triangles)"
features/18-api-skeleton/design-review.md:2374:    // simple_box.mycad: width=10, height=20, depth=30 → cuboid topology
features/18-api-skeleton/design-review.md:2442:./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/design-review.md:2450:./features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/design-review.md:2458:./features/12-cylinder/design-review.md:523:crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review.md:2466:./features/12-cylinder/design-review.md:531:crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review.md:2468:./features/12-cylinder/design-review.md:533:crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review.md:2472:./features/12-cylinder/design-review.md:537:crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review.md:2473:./features/12-cylinder/design-review.md:538:crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review.md:2539:./features/12-cylinder/design-review-3.md:223:/bin/bash -lc "rg -n \"make_cuboid|tessellate_solid|IdGenerator|KernelError|HalfEdge|inner_loops|Surface::|Curve::|TriangleMesh|Feature::CreateCylinder|CreateCylinder|UnsupportedFeature|InvalidParameter|mod.rs|pub use\" crates/mycad-kernel crates/mycad-build crates/mycad-cli -g '"'!target'"'" in /home/bacon/mycad
features/18-api-skeleton/design-review.md:2549:./features/12-cylinder/design-review-3.md:260:crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review.md:2551:./features/12-cylinder/design-review-3.md:262:crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review.md:2555:./features/12-cylinder/design-review-3.md:266:crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review.md:2556:./features/12-cylinder/design-review-3.md:267:crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review.md:2560:./features/12-cylinder/design-review-3.md:319:crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review.md:2574:./features/12-cylinder/design-review-3.md:1625:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review.md:2583:./features/12-cylinder/design-review-2.md:514:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review.md:2609:./features/12-cylinder/design-review-2.md:1863:./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/design-review.md:2617:./features/12-cylinder/design-review-2.md:1875:./features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/design-review.md:2625:./features/12-cylinder/design-review-2.md:1883:./features/12-cylinder/design-review.md:523:crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review.md:2633:./features/12-cylinder/design-review-2.md:1891:./features/12-cylinder/design-review.md:531:crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review.md:2635:./features/12-cylinder/design-review-2.md:1893:./features/12-cylinder/design-review.md:533:crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review.md:2639:./features/12-cylinder/design-review-2.md:1897:./features/12-cylinder/design-review.md:537:crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review.md:2640:./features/12-cylinder/design-review-2.md:1898:./features/12-cylinder/design-review.md:538:crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review.md:2656:./features/12-cylinder/design-review-2.md:1928:./crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review.md:2665:./features/12-cylinder/design-review-2.md:1941:./features/12-cylinder/design-review-2.md:514:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review.md:2687:./features/12-cylinder/final-review.md:945: pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review.md:2696:./features/12-cylinder/final-review.md:2457:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review.md:2788:./crates/mycad-kernel/src/primitives/mod.rs:4:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review.md:2815:設計原則: **決定性** (同じ入力は常に同じ出力)、**パラメトリック履歴** (feature historyが真実の源)、**YAML-based format** (人間可読・diff可能)。
features/18-api-skeleton/design-review.md:2824:│   ├── primitives/   プリミティブ生成 (make_cuboid)
features/18-api-skeleton/design-review.md:2826:├── mycad-format/     .mycadファイルのYAMLスキーマ・パーサー
features/18-api-skeleton/design-review.md:2898:features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/design-review.md:2909:features/12-cylinder/design-review-2.md:1936:./features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/design-review.md:2919:features/12-cylinder/design-review-3.md:149:  2. `BoundaryFan`（平面） → 境界点列を **頂点0 から fan**（凸前提。cuboid 4 角=従来 2 三角形で不変、円盤キャップ=N角形 fan）。巻き方向は §1 で構築済み（底キャップ rev / 上キャップ fwd）なので fan の幾何法線が外向きになる。**スワップなし → cuboid のインデックス列もバイト不変**（T08 で固定）。
features/18-api-skeleton/design-review.md:2920:features/12-cylinder/design-review-3.md:180:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/design-review.md:2921:features/12-cylinder/design-review-3.md:186:- R02(high) **検証して反映**: cuboid.rs / stl.rs を読み確認 → 「実行時の向き整合スワップは cuboid 出力不変」という前回主張は **誤り**（現行 cuboid のインデックス巻きが右手系外向き規約と一致する保証はなく、スワップ導入で index 列が変わり T08 が回帰しうる）。→ **テッセレータからスワップを撤去**し、外向きは §1 の**構築時巻き方向**（底キャップ=e_bot rev / 上キャップ=e_top fwd / 側面グリッド=∂u×∂v 外向き）で保証。平面 fan 経路は現行コードとバイト同一 → cuboid 完全不変（T08 で固定）。
features/18-api-skeleton/design-review.md:3022:mod cuboid;
features/18-api-skeleton/design-review.md:3025:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review.md:3037:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/design-review.md:3071:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/design-review.md:3072:    suggestion: "`sample_segment` を HalfEdge 向き込みの API に変更し、rev では `t1` から降順にサンプルするか、先頭点を必ず `HalfEdge.start_vertex` から取得すること。reversed line edge を含む cuboid 回帰テストも追加すること。"
features/18-api-skeleton/design-review.md:3080:    section: "設計方針 > 1. トポロジー / テスト計画 > T04"
features/18-api-skeleton/design-review.md:3081:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/design-review.md:3091:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
features/18-api-skeleton/design-review.md:3101:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/design-review.md:3102:    suggestion: "`sample_segment` を HalfEdge 向き込みの API に変更し、rev では `t1` から降順にサンプルするか、先頭点を必ず `HalfEdge.start_vertex` から取得すること。reversed line edge を含む cuboid 回帰テストも追加すること。"
features/18-api-skeleton/design-review.md:3110:    section: "設計方針 > 1. トポロジー / テスト計画 > T04"
features/18-api-skeleton/design-review.md:3111:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/design-review.md:3121:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
features/18-api-skeleton/design-review.md:3181:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/design-review.md:3350:    suggestion: "絶対パスのみ受け付けるか、`app(state)` に明示的な base directory を持たせて相対パス解決を固定し、その規則を T02 と E2E で検証すること。"
features/18-api-skeleton/design-review.md:3354:    finding: "`build_solid_from_features` をそのまま再利用する方針では、`make_cuboid` 側に寸法バリデーションが無いため `width/height/depth <= 0` の `.mycad` を API が受理しうる。ゼロ長 edge・面積ゼロ face・縮退 loop をどう拒否するか、また数値境界を何で比較するかが計画に無い。"
features/18-api-skeleton/design-review.md:3359:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/design-review.md:3360:    suggestion: "v0 を『root_component に単一 part feature を持つ文書のみ対応』と明記して assembly 非対応を別エラーに分けるか、Document/Component 階層を解釈する document-level build API を設計すること。T06 は専用 fixture に差し替えること。"
features/18-api-skeleton/design-review.md:3369:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/design-review.md:3379:    suggestion: "絶対パスのみ受け付けるか、`app(state)` に明示的な base directory を持たせて相対パス解決を固定し、その規則を T02 と E2E で検証すること。"
features/18-api-skeleton/design-review.md:3383:    finding: "`build_solid_from_features` をそのまま再利用する方針では、`make_cuboid` 側に寸法バリデーションが無いため `width/height/depth <= 0` の `.mycad` を API が受理しうる。ゼロ長 edge・面積ゼロ face・縮退 loop をどう拒否するか、また数値境界を何で比較するかが計画に無い。"
features/18-api-skeleton/design-review.md:3388:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/design-review.md:3389:    suggestion: "v0 を『root_component に単一 part feature を持つ文書のみ対応』と明記して assembly 非対応を別エラーに分けるか、Document/Component 階層を解釈する document-level build API を設計すること。T06 は専用 fixture に差し替えること。"
features/18-api-skeleton/design-review.md:3398:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/12-cylinder/design-review-2.md:45:- golden YAML ラウンドトリップテストがあるか
features/12-cylinder/design-review-2.md:79:ROADMAP Phase 3「円柱・球・押し出しが作れる」の最初の一歩。現状カーネルは `make_cuboid` と平面専用の `tessellate_solid` しか持たない。本 Issue で **解析曲面(円柱)を厳密 B-rep として生成し、それを三角形メッシュへ離散化する土台**を確立し、`mycad export examples/cylinder.mycad -o out.stl` を動作させる。
features/12-cylinder/design-review-2.md:125:**決定性**: ID 採番順を `make_cuboid` と同型に固定。solid → 頂点(v_bot,v_top) → エッジ(e_bot,e_top,e_seam) → 面ごとに[底,上,側]順で HE群→loop→face → shell。すべて `id_gen.next()` を固定順で呼ぶのみ。`HashMap`/乱数等の非決定要素なし。
features/12-cylinder/design-review-2.md:127:**退化入力の検証（R02）**: `.mycad` 由来のユーザー入力は境界で弾く。`KernelError` に `InvalidParameter { kind: &'static str }` を追加し、`build_solid_from_features` の `CreateCylinder` 分岐で `radius > 0.0 && height > 0.0` を検証（非正なら `InvalidParameter` を返し `make_cylinder` を呼ばない）。`make_cylinder` 自体は `make_cuboid` と同じ `-> Solid` シグネチャを保ち、正値前提（検証は呼び出し側＝境界に集約）。解像度の退化は `TessellationOptions` のクランプ（前述）で吸収。
features/12-cylinder/design-review-2.md:147:  1. 外側 loop の各 HE について、`forward` なら `(t0,t1)`、`!forward` なら `(t1,t0)` を `sample_segment` に渡して折れ線化 → 連結して境界点列。**先頭点が必ず `HalfEdge.start_vertex` に一致**するため、逆向き Line を含む cuboid でも従来と同一の角点列＝**平面出力不変**。
features/12-cylinder/design-review-2.md:148:  2. `surface.requires_interior_sampling()` が **false（平面）** → 境界点列を **頂点0 から fan**（凸前提。cuboid 4 角=従来 2 三角形で不変、円盤キャップ=N角形 fan）。
features/12-cylinder/design-review-2.md:151:- **STL 外向き保証**: 各三角形の幾何法線（辺の外積）が、その面の外向き法線（`normal_at_point` を `same_sense` で調整）と逆なら 2 頂点をスワップして向きを揃える。これでキャップ/側面とも STL facet が外向き。cuboid は既に外向きのためスワップ発生せず、出力は実質不変（決定性・三角形数も維持）。
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
features/12-cylinder/design-review-2.md:199:| T13 | golden YAML ラウンドトリップ | `examples/cylinder.mycad` を `from_path`→`to_yaml`→`from_yaml`→`to_yaml` で equality。`create_cylinder` の serialize/schema 回帰を固定 | `assert_eq!` (R05) |
features/12-cylinder/design-review-2.md:204:1. `cargo test --workspace` で T01–T12 green。
features/12-cylinder/design-review-2.md:214:設計レビューとして、まず既存カーネルのトポロジー型・`make_cuboid`・テッセレーション実装・エラー/依存規約を確認します。文書内の前提が既存設計と噛み合っているかを見てから、重大度付きで指摘を整理します。
features/12-cylinder/design-review-2.md:232:/home/bacon/mycad/crates/mycad-kernel/src/primitives/cuboid.rs
features/12-cylinder/design-review-2.md:244:既存実装の骨格が見えました。次に `make_cuboid`、B-rep 型、テッセレータ、`build`/CLI の現状を読み、今回の設計がどこで破綻しうるかを詰めます。
features/12-cylinder/design-review-2.md:246:/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/primitives/cuboid.rs" in /home/bacon/mycad
features/12-cylinder/design-review-2.md:254:/// Create a cuboid (box) B-rep solid centered at the origin.
features/12-cylinder/design-review-2.md:262:/// The cuboid spans from (-dx/2, -dy/2, -dz/2) to (dx/2, dy/2, dz/2).
features/12-cylinder/design-review-2.md:263:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/12-cylinder/design-review-2.md:270:    // 8 vertices of the cuboid
features/12-cylinder/design-review-2.md:296:    // 12 edges of the cuboid
features/12-cylinder/design-review-2.md:331:    // 6 faces of the cuboid
features/12-cylinder/design-review-2.md:473:    fn test_cuboid_topology() {
features/12-cylinder/design-review-2.md:475:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/12-cylinder/design-review-2.md:477:        assert_eq!(solid.vertices.len(), 8, "cuboid should have 8 vertices");
features/12-cylinder/design-review-2.md:478:        assert_eq!(solid.edges.len(), 12, "cuboid should have 12 edges");
features/12-cylinder/design-review-2.md:479:        assert_eq!(solid.faces.len(), 6, "cuboid should have 6 faces");
features/12-cylinder/design-review-2.md:480:        assert_eq!(solid.shells.len(), 1, "cuboid should have 1 shell");
features/12-cylinder/design-review-2.md:481:        assert!(solid.shells[0].closed, "cuboid shell should be closed");
features/12-cylinder/design-review-2.md:486:            "cuboid should have 24 half-edges"
features/12-cylinder/design-review-2.md:489:        assert_eq!(solid.loops.len(), 6, "cuboid should have 6 loops");
features/12-cylinder/design-review-2.md:493:    fn test_cuboid_vertex_positions() {
features/12-cylinder/design-review-2.md:495:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
features/12-cylinder/design-review-2.md:512:mod cuboid;
features/12-cylinder/design-review-2.md:514:pub use cuboid::make_cuboid;
features/12-cylinder/design-review-2.md:657:    use crate::primitives::make_cuboid;
features/12-cylinder/design-review-2.md:660:    fn test_tessellate_cuboid() {
features/12-cylinder/design-review-2.md:662:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/12-cylinder/design-review-2.md:665:        // A cuboid has 6 faces, each tessellated as 2 triangles = 12 triangles
features/12-cylinder/design-review-2.md:678:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/12-cylinder/design-review-2.md:679:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/12-cylinder/design-review-2.md:860:    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
features/12-cylinder/design-review-2.md:868:    fn test_uv_of_roundtrip_plane() {
features/12-cylinder/design-review-2.md:875:        assert_uv_roundtrip(&surface, 0.7, -1.3);
features/12-cylinder/design-review-2.md:879:    fn test_uv_of_roundtrip_cylinder() {
features/12-cylinder/design-review-2.md:885:        assert_uv_roundtrip(&surface, 0.9, 1.7);
features/12-cylinder/design-review-2.md:889:    fn test_uv_of_roundtrip_sphere() {
features/12-cylinder/design-review-2.md:894:        assert_uv_roundtrip(&surface, 1.1, 0.6);
features/12-cylinder/design-review-2.md:898:    fn test_uv_of_roundtrip_cone() {
features/12-cylinder/design-review-2.md:904:        assert_uv_roundtrip(&surface, 0.8, 2.0);
features/12-cylinder/design-review-2.md:939:`make_cuboid` は `HalfEdge.start_vertex` の順序だけでループを表現しており、現状テッセレータもそこを前提にしています。次は B-rep 型定義と `build` 側の feature/history の扱いを見ます。
features/12-cylinder/design-review-2.md:946:use mycad_kernel::primitives::make_cuboid;
features/12-cylinder/design-review-2.md:968:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
features/12-cylinder/design-review-2.md:1065:crates/mycad-kernel/src/primitives/cuboid.rs:1:use crate::brep::topology::{IdGenerator, Solid};
features/12-cylinder/design-review-2.md:1066:crates/mycad-kernel/src/primitives/cuboid.rs:16:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/12-cylinder/design-review-2.md:1067:crates/mycad-kernel/src/primitives/cuboid.rs:87:    struct FaceDef {
features/12-cylinder/design-review-2.md:1068:crates/mycad-kernel/src/primitives/cuboid.rs:91:        same_sense: bool,
features/12-cylinder/design-review-2.md:1069:crates/mycad-kernel/src/primitives/cuboid.rs:109:            same_sense: true,
features/12-cylinder/design-review-2.md:1070:crates/mycad-kernel/src/primitives/cuboid.rs:125:            same_sense: true,
features/12-cylinder/design-review-2.md:1071:crates/mycad-kernel/src/primitives/cuboid.rs:141:            same_sense: true,
features/12-cylinder/design-review-2.md:1072:crates/mycad-kernel/src/primitives/cuboid.rs:157:            same_sense: true,
features/12-cylinder/design-review-2.md:1073:crates/mycad-kernel/src/primitives/cuboid.rs:173:            same_sense: true,
features/12-cylinder/design-review-2.md:1074:crates/mycad-kernel/src/primitives/cuboid.rs:189:            same_sense: true,
features/12-cylinder/design-review-2.md:1075:crates/mycad-kernel/src/primitives/cuboid.rs:210:            face_def.same_sense,
features/12-cylinder/design-review-2.md:1076:crates/mycad-kernel/src/primitives/cuboid.rs:223:    use crate::brep::topology::IdGenerator;
features/12-cylinder/design-review-2.md:1077:crates/mycad-kernel/src/primitives/cuboid.rs:227:        let mut id_gen = IdGenerator::new(0);
features/12-cylinder/design-review-2.md:1078:crates/mycad-kernel/src/primitives/cuboid.rs:247:        let mut id_gen = IdGenerator::new(0);
features/12-cylinder/design-review-2.md:1079:crates/mycad-kernel/src/primitives/cuboid.rs:276:        let mut id_gen1 = IdGenerator::new(0);
features/12-cylinder/design-review-2.md:1080:crates/mycad-kernel/src/primitives/cuboid.rs:277:        let mut id_gen2 = IdGenerator::new(0);
features/12-cylinder/design-review-2.md:1396:description = "YAML schema definition and parser for MyCad feature files"
features/12-cylinder/design-review-2.md:1453:crates/mycad-format/src/document.rs:25:    /// Serialize to YAML string.
features/12-cylinder/design-review-2.md:1455:crates/mycad-format/src/document.rs:30:    /// Deserialize from YAML string.
features/12-cylinder/design-review-2.md:1641:    /// Serialize to YAML string.
features/12-cylinder/design-review-2.md:1646:    /// Deserialize from YAML string.
features/12-cylinder/design-review-2.md:1671:    fn test_roundtrip_yaml() {
features/12-cylinder/design-review-2.md:1702:        assert_eq!(yaml1, yaml2, "YAML serialization must be deterministic");
features/12-cylinder/design-review-2.md:1732:    fn test_assembly_roundtrip() {
features/12-cylinder/design-review-2.md:1753:        // Roundtrip: serialize then deserialize yields the same YAML
features/12-cylinder/design-review-2.md:1781:    // simple_box.mycad: width=10, height=20, depth=30 → cuboid topology
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
features/12-cylinder/design-review-2.md:1838:        "cuboid should produce 12 facets (6 faces * 2 triangles)"
features/12-cylinder/design-review-2.md:1863:./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/12-cylinder/design-review-2.md:1875:./features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/12-cylinder/design-review-2.md:1883:./features/12-cylinder/design-review.md:523:crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
features/12-cylinder/design-review-2.md:1891:./features/12-cylinder/design-review.md:531:crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
features/12-cylinder/design-review-2.md:1893:./features/12-cylinder/design-review.md:533:crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/12-cylinder/design-review-2.md:1897:./features/12-cylinder/design-review.md:537:crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/12-cylinder/design-review-2.md:1898:./features/12-cylinder/design-review.md:538:crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/12-cylinder/design-review-2.md:1928:./crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
features/12-cylinder/design-review-2.md:1936:./features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/12-cylinder/design-review-2.md:1941:./features/12-cylinder/design-review-2.md:514:pub use cuboid::make_cuboid;
features/12-cylinder/design-review-2.md:2016:レビュー項目はほぼ固まりました。既存コードとの照合上、重大なのは「汎用曲面テッセレーションの抽象が cylinder 専用の仮定を漏らしている点」と「Euler テストが実体を検証していない点」です。最終的に YAML だけ返すので、最後に指摘を絞ります。
features/12-cylinder/design-review-2.md:2121:CLAUDE.md:20:設計原則: **決定性** (同じ入力は常に同じ出力)、**パラメトリック履歴** (feature historyが真実の源)、**YAML-based format** (人間可読・diff可能)。
features/12-cylinder/design-review-2.md:2149:1. `src/primitives/` に新ファイルを作成（`cuboid.rs` をテンプレートとして使用）
features/12-cylinder/design-review-2.md:2168:既存 cuboid の面順序も確認しました。設計書の「cuboid は既に外向きでスワップ不要」という前提はかなり怪しく、ここは重大な回帰前提のズレとして扱います。
features/12-cylinder/design-review-2.md:2223:    use crate::primitives::make_cuboid;
features/12-cylinder/design-review-2.md:2227:    fn ascii_stl_cuboid_structure() {
features/12-cylinder/design-review-2.md:2229:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/12-cylinder/design-review-2.md:2246:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/12-cylinder/design-review-2.md:2254:/bin/bash -lc "sed -n '260,340p' crates/mycad-kernel/src/primitives/cuboid.rs" in /home/bacon/mycad
features/12-cylinder/design-review-2.md:2269:    fn test_cuboid_deterministic() {
features/12-cylinder/design-review-2.md:2270:        // Building the same cuboid twice with the same ID generator should
features/12-cylinder/design-review-2.md:2275:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
features/12-cylinder/design-review-2.md:2276:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);
features/12-cylinder/design-review-2.md:2307:    finding: "「cuboid は既に外向きなのでスワップは発生しない／平面出力不変」という前提が既存 `make_cuboid` と整合していない。現状の cuboid ループ順では front face `0→1→3→2` などの幾何法線が outward normal と逆向きで、提案どおりの法線整合スワップを入れると既存 STL/indices は変わる。"
features/12-cylinder/design-review-2.md:2308:    suggestion: "この変更を意図的な winding 修正として明記し、`T08` を『counts は維持するが indices / STL facet normal は変わり得る』前提に更新するか、先に cuboid 側の loop orientation を正規化して本当に不変にすること。"
features/12-cylinder/design-review-2.md:2311:    section: "テスト計画 > T03"
features/12-cylinder/design-review-2.md:2312:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/12-cylinder/design-review-2.md:2332:    finding: "「cuboid は既に外向きなのでスワップは発生しない／平面出力不変」という前提が既存 `make_cuboid` と整合していない。現状の cuboid ループ順では front face `0→1→3→2` などの幾何法線が outward normal と逆向きで、提案どおりの法線整合スワップを入れると既存 STL/indices は変わる。"
features/12-cylinder/design-review-2.md:2333:    suggestion: "この変更を意図的な winding 修正として明記し、`T08` を『counts は維持するが indices / STL facet normal は変わり得る』前提に更新するか、先に cuboid 側の loop orientation を正規化して本当に不変にすること。"
features/12-cylinder/design-review-2.md:2336:    section: "テスト計画 > T03"
features/12-cylinder/design-review-2.md:2337:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/18-api-skeleton/final-review-2.md:43:- 設計レビュー時に合意したテスト計画（ID T01〜）が全て実装されているか
features/18-api-skeleton/final-review-2.md:45:- golden YAML ラウンドトリップが含まれているか
features/18-api-skeleton/final-review-2.md:62:    suggestion: "make_cuboid の edge 生成ロジックを見直す"
features/18-api-skeleton/final-review-2.md:84: M crates/mycad-kernel/src/primitives/cuboid.rs
features/18-api-skeleton/final-review-2.md:87: M crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review-2.md:103:crates/mycad-kernel/src/primitives/cuboid.rs
features/18-api-skeleton/final-review-2.md:106:crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review-2.md:117: crates/mycad-kernel/src/primitives/cuboid.rs  | 115 +++++++-
features/18-api-skeleton/final-review-2.md:120: crates/mycad-kernel/tests/cuboid_roundtrip.rs |   4 +-
features/18-api-skeleton/final-review-2.md:124:変更範囲は主に `mycad-kernel` の cuboid/tessellation 周りと workspace 設定です。次に各差分を読み、決定性・B-rep 整合性・テスト計画の実装有無を確認します。
features/18-api-skeleton/final-review-2.md:622:-        } => Ok(make_cuboid(*width, *height, *depth, gen)),
features/18-api-skeleton/final-review-2.md:623:+        } => make_cuboid(*width, *height, *depth, gen),
features/18-api-skeleton/final-review-2.md:629:/bin/bash -lc 'git diff -- crates/mycad-kernel/src/primitives/cuboid.rs crates/mycad-kernel/src/tessellation/mod.rs crates/mycad-kernel/src/tessellation/stl.rs' in /home/bacon/mycad
features/18-api-skeleton/final-review-2.md:631:diff --git a/crates/mycad-kernel/src/primitives/cuboid.rs b/crates/mycad-kernel/src/primitives/cuboid.rs
features/18-api-skeleton/final-review-2.md:633:--- a/crates/mycad-kernel/src/primitives/cuboid.rs
features/18-api-skeleton/final-review-2.md:634:+++ b/crates/mycad-kernel/src/primitives/cuboid.rs
features/18-api-skeleton/final-review-2.md:644: /// The cuboid spans from (-dx/2, -dy/2, -dz/2) to (dx/2, dy/2, dz/2).
features/18-api-skeleton/final-review-2.md:645:-pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/18-api-skeleton/final-review-2.md:646:+pub fn make_cuboid(
features/18-api-skeleton/final-review-2.md:664:@@ -214,18 +229,19 @@ pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid
features/18-api-skeleton/final-review-2.md:679:     fn test_cuboid_topology() {
features/18-api-skeleton/final-review-2.md:681:-        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/final-review-2.md:682:+        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review-2.md:684:         assert_eq!(solid.vertices.len(), 8, "cuboid should have 8 vertices");
features/18-api-skeleton/final-review-2.md:685:         assert_eq!(solid.edges.len(), 12, "cuboid should have 12 edges");
features/18-api-skeleton/final-review-2.md:688:     fn test_cuboid_vertex_positions() {
features/18-api-skeleton/final-review-2.md:690:-        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
features/18-api-skeleton/final-review-2.md:691:+        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review-2.md:699:-        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
features/18-api-skeleton/final-review-2.md:700:-        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);
features/18-api-skeleton/final-review-2.md:701:+        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1).unwrap();
features/18-api-skeleton/final-review-2.md:702:+        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2).unwrap();
features/18-api-skeleton/final-review-2.md:711:+    // K01: make_cuboid rejects zero, negative, NaN, Infinity dimensions
features/18-api-skeleton/final-review-2.md:713:+    fn test_make_cuboid_invalid_dimensions() {
features/18-api-skeleton/final-review-2.md:718:+            make_cuboid(0.0, 1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review-2.md:722:+            make_cuboid(1.0, 0.0, 1.0, &mut gen),
features/18-api-skeleton/final-review-2.md:726:+            make_cuboid(1.0, 1.0, 0.0, &mut gen),
features/18-api-skeleton/final-review-2.md:732:+            make_cuboid(-1.0, 1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review-2.md:736:+            make_cuboid(1.0, -1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review-2.md:740:+            make_cuboid(1.0, 1.0, -1.0, &mut gen),
features/18-api-skeleton/final-review-2.md:746:+            make_cuboid(f64::NAN, 1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review-2.md:750:+            make_cuboid(1.0, f64::NAN, 1.0, &mut gen),
features/18-api-skeleton/final-review-2.md:754:+            make_cuboid(1.0, 1.0, f64::NAN, &mut gen),
features/18-api-skeleton/final-review-2.md:760:+            make_cuboid(f64::INFINITY, 1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review-2.md:764:+            make_cuboid(1.0, f64::INFINITY, 1.0, &mut gen),
features/18-api-skeleton/final-review-2.md:768:+            make_cuboid(1.0, 1.0, f64::INFINITY, &mut gen),
features/18-api-skeleton/final-review-2.md:774:+            make_cuboid(f64::NEG_INFINITY, 1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review-2.md:778:+            make_cuboid(1.0, f64::NEG_INFINITY, 1.0, &mut gen),
features/18-api-skeleton/final-review-2.md:782:+            make_cuboid(1.0, 1.0, f64::NEG_INFINITY, &mut gen),
features/18-api-skeleton/final-review-2.md:787:+    // K02: near-zero positive value is Ok (exact <= 0.0 policy)
features/18-api-skeleton/final-review-2.md:789:+    fn test_make_cuboid_near_zero_positive_ok() {
features/18-api-skeleton/final-review-2.md:791:+        let result = make_cuboid(1e-9, 1.0, 1.0, &mut gen);
features/18-api-skeleton/final-review-2.md:804:     fn test_tessellate_cuboid_unchanged() {
features/18-api-skeleton/final-review-2.md:806:-        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
features/18-api-skeleton/final-review-2.md:807:+        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen).unwrap();
features/18-api-skeleton/final-review-2.md:815:-        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
features/18-api-skeleton/final-review-2.md:816:-        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
features/18-api-skeleton/final-review-2.md:817:+        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1).unwrap();
features/18-api-skeleton/final-review-2.md:818:+        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2).unwrap();
features/18-api-skeleton/final-review-2.md:824:     fn test_tessellate_cuboid() {
features/18-api-skeleton/final-review-2.md:826:-        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/final-review-2.md:827:+        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review-2.md:835:-        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/final-review-2.md:836:-        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/final-review-2.md:837:+        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1).unwrap();
features/18-api-skeleton/final-review-2.md:838:+        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2).unwrap();
features/18-api-skeleton/final-review-2.md:848:     fn ascii_stl_cuboid_structure() {
features/18-api-skeleton/final-review-2.md:850:-        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/18-api-skeleton/final-review-2.md:851:+        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g).unwrap();
features/18-api-skeleton/final-review-2.md:859:-            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/18-api-skeleton/final-review-2.md:860:+            let s = make_cuboid(1.0, 1.0, 1.0, &mut g).unwrap();
features/18-api-skeleton/final-review-2.md:866:/bin/bash -lc 'git diff -- crates/mycad-kernel/benches/tessellation.rs crates/mycad-kernel/tests/cuboid_roundtrip.rs' in /home/bacon/mycad
features/18-api-skeleton/final-review-2.md:876:-        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/final-review-2.md:877:+        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review-2.md:881:diff --git a/crates/mycad-kernel/tests/cuboid_roundtrip.rs b/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review-2.md:883:--- a/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review-2.md:884:+++ b/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review-2.md:887: fn cuboid_to_mesh_pipeline() {
features/18-api-skeleton/final-review-2.md:889:-    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/final-review-2.md:890:+    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review-2.md:894:@@ -28,7 +28,7 @@ fn cuboid_to_mesh_pipeline() {
features/18-api-skeleton/final-review-2.md:895: fn cuboid_pipeline_is_deterministic() {
features/18-api-skeleton/final-review-2.md:898:-        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/18-api-skeleton/final-review-2.md:899:+        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review-2.md:905:/bin/bash -lc 'rg -n "T0[0-9]|golden YAML|roundtrip|determin" crates/mycad-kernel crates/mycad-build Cargo.toml' in /home/bacon/mycad
features/18-api-skeleton/final-review-2.md:908:crates/mycad-kernel/src/tessellation/mod.rs:318:    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
features/18-api-skeleton/final-review-2.md:909:crates/mycad-kernel/src/tessellation/mod.rs:337:    /// T07: Determinism — same input → same output.
features/18-api-skeleton/final-review-2.md:911:crates/mycad-kernel/src/tessellation/mod.rs:354:    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
features/18-api-skeleton/final-review-2.md:912:crates/mycad-kernel/src/tessellation/mod.rs:367:    /// T08 extended: exact positions and indices snapshot.
features/18-api-skeleton/final-review-2.md:913:crates/mycad-kernel/src/tessellation/mod.rs:384:    /// T09: Outward normals — all facet normals point away from centroid.
features/18-api-skeleton/final-review-2.md:920:crates/mycad-kernel/src/geometry/surface.rs:188:    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
features/18-api-skeleton/final-review-2.md:921:crates/mycad-kernel/src/geometry/surface.rs:196:    fn test_uv_of_roundtrip_plane() {
features/18-api-skeleton/final-review-2.md:922:crates/mycad-kernel/src/geometry/surface.rs:203:        assert_uv_roundtrip(&surface, 0.7, -1.3);
features/18-api-skeleton/final-review-2.md:923:crates/mycad-kernel/src/geometry/surface.rs:207:    fn test_uv_of_roundtrip_cylinder() {
features/18-api-skeleton/final-review-2.md:924:crates/mycad-kernel/src/geometry/surface.rs:213:        assert_uv_roundtrip(&surface, 0.9, 1.7);
features/18-api-skeleton/final-review-2.md:925:crates/mycad-kernel/src/geometry/surface.rs:217:    fn test_uv_of_roundtrip_sphere() {
features/18-api-skeleton/final-review-2.md:926:crates/mycad-kernel/src/geometry/surface.rs:222:        assert_uv_roundtrip(&surface, 1.1, 0.6);
features/18-api-skeleton/final-review-2.md:927:crates/mycad-kernel/src/geometry/surface.rs:226:    fn test_uv_of_roundtrip_cone() {
features/18-api-skeleton/final-review-2.md:928:crates/mycad-kernel/src/geometry/surface.rs:232:        assert_uv_roundtrip(&surface, 0.8, 2.0);
features/18-api-skeleton/final-review-2.md:929:crates/mycad-kernel/tests/cuboid_roundtrip.rs:26:/// Integration test: deterministic pipeline produces identical results.
features/18-api-skeleton/final-review-2.md:930:crates/mycad-kernel/tests/cuboid_roundtrip.rs:28:fn cuboid_pipeline_is_deterministic() {
features/18-api-skeleton/final-review-2.md:931:crates/mycad-kernel/src/primitives/cuboid.rs:14:/// - `id_gen`: deterministic ID generator
features/18-api-skeleton/final-review-2.md:932:crates/mycad-kernel/src/primitives/cuboid.rs:289:    fn test_cuboid_deterministic() {
features/18-api-skeleton/final-review-2.md:933:crates/mycad-kernel/src/primitives/cylinder.rs:133:    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
features/18-api-skeleton/final-review-2.md:935:crates/mycad-kernel/src/primitives/cylinder.rs:161:    /// T02: Topology counts.
features/18-api-skeleton/final-review-2.md:936:crates/mycad-kernel/src/primitives/cylinder.rs:180:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/final-review-2.md:937:crates/mycad-kernel/src/primitives/cylinder.rs:199:    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
features/18-api-skeleton/final-review-2.md:938:crates/mycad-kernel/src/primitives/cylinder.rs:254:    /// T05: Geometry — seam vertices and surface coefficients.
features/18-api-skeleton/final-review-2.md:941:cuboid 側の差分は入力バリデーション追加が中心で、既存トポロジー生成そのものは変えていません。ここからはテスト計画 ID と仕様文書を突き合わせて、追加テストが計画どおりかを見ます。
features/12-cylinder/final-review.md:29: crates/mycad-kernel/tests/cuboid_roundtrip.rs  |   4 +-
features/12-cylinder/final-review.md:44:    - make_cylinder(radius, height) -> Result<Solid, KernelError> with is_finite+positive validation
features/12-cylinder/final-review.md:49:    - examples/cylinder.mycad + T01-T15 test suite (32 tests green)
features/12-cylinder/final-review.md:64:-use mycad_kernel::primitives::make_cuboid;
features/12-cylinder/final-review.md:65:+use mycad_kernel::primitives::{make_cuboid, make_cylinder};
features/12-cylinder/final-review.md:87:         } => Ok(make_cuboid(*width, *height, *depth, gen)),
features/12-cylinder/final-review.md:184: use mycad_kernel::primitives::make_cuboid;
features/12-cylinder/final-review.md:197:         let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/12-cylinder/final-review.md:204:         "Tessellated {} cuboids in {:?} ({:.2} us/iter)",
features/12-cylinder/final-review.md:484:     fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
features/12-cylinder/final-review.md:492:     fn test_uv_of_roundtrip_plane() {
features/12-cylinder/final-review.md:499:         assert_uv_roundtrip(&surface, 0.7, -1.3);
features/12-cylinder/final-review.md:503:     fn test_uv_of_roundtrip_cylinder() {
features/12-cylinder/final-review.md:509:         assert_uv_roundtrip(&surface, 0.9, 1.7);
features/12-cylinder/final-review.md:513:     fn test_uv_of_roundtrip_sphere() {
features/12-cylinder/final-review.md:518:         assert_uv_roundtrip(&surface, 1.1, 0.6);
features/12-cylinder/final-review.md:522:     fn test_uv_of_roundtrip_cone() {
features/12-cylinder/final-review.md:528:         assert_uv_roundtrip(&surface, 0.8, 2.0);
features/12-cylinder/final-review.md:695:+    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
features/12-cylinder/final-review.md:723:+    /// T02: Topology counts.
features/12-cylinder/final-review.md:742:+    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/12-cylinder/final-review.md:761:+    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
features/12-cylinder/final-review.md:816:+    /// T05: Geometry — seam vertices and surface coefficients.
features/12-cylinder/final-review.md:863:+    fn test_cylinder_degenerate_inputs() {
features/12-cylinder/final-review.md:942: mod cuboid;
features/12-cylinder/final-review.md:945: pub use cuboid::make_cuboid;
features/12-cylinder/final-review.md:963:+/// Minimum cross-product norm for a non-degenerate triangle.
features/12-cylinder/final-review.md:1237:+/// Push a triangle, but skip degenerate (zero-area) ones.
features/12-cylinder/final-review.md:1276:     use crate::primitives::make_cuboid;
features/12-cylinder/final-review.md:1279:+    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
features/12-cylinder/final-review.md:1298:+    /// T07: Determinism — same input → same output.
features/12-cylinder/final-review.md:1315:+    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
features/12-cylinder/final-review.md:1317:+    fn test_tessellate_cuboid_unchanged() {
features/12-cylinder/final-review.md:1319:+        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
features/12-cylinder/final-review.md:1328:+    /// T08 extended: exact positions and indices snapshot.
features/12-cylinder/final-review.md:1330:+    fn test_cuboid_positions_indices_snapshot() {
features/12-cylinder/final-review.md:1334:+        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
features/12-cylinder/final-review.md:1335:+        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
features/12-cylinder/final-review.md:1345:+    /// T09: Outward normals — all facet normals point away from centroid.
features/12-cylinder/final-review.md:1459:     fn test_tessellate_cuboid() {
features/12-cylinder/final-review.md:1461:         let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/12-cylinder/final-review.md:1465:-        // A cuboid has 6 faces, each tessellated as 2 triangles = 12 triangles
features/12-cylinder/final-review.md:1478:         let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/12-cylinder/final-review.md:1479:         let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/12-cylinder/final-review.md:1547:     use crate::primitives::make_cuboid;
features/12-cylinder/final-review.md:1551:     fn ascii_stl_cuboid_structure() {
features/12-cylinder/final-review.md:1553:         let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/12-cylinder/final-review.md:1571:             let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/12-cylinder/final-review.md:1578:diff --git a/crates/mycad-kernel/tests/cuboid_roundtrip.rs b/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/12-cylinder/final-review.md:1580:--- a/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/12-cylinder/final-review.md:1581:+++ b/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/12-cylinder/final-review.md:1584: use mycad_kernel::primitives::make_cuboid;
features/12-cylinder/final-review.md:1587: /// Integration test: create a cuboid, tessellate it, and verify the pipeline.
features/12-cylinder/final-review.md:1589: fn cuboid_to_mesh_pipeline() {
features/12-cylinder/final-review.md:1591:     let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/12-cylinder/final-review.md:1611: fn cuboid_pipeline_is_deterministic() {
features/12-cylinder/final-review.md:1614:         let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/12-cylinder/final-review.md:2003:    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
features/12-cylinder/final-review.md:2031:    /// T02: Topology counts.
features/12-cylinder/final-review.md:2050:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/12-cylinder/final-review.md:2069:    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
features/12-cylinder/final-review.md:2124:    /// T05: Geometry — seam vertices and surface coefficients.
features/12-cylinder/final-review.md:2179:/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/primitives/cuboid.rs" in /home/bacon/mycad
features/12-cylinder/final-review.md:2187:/// Create a cuboid (box) B-rep solid centered at the origin.
features/12-cylinder/final-review.md:2195:/// The cuboid spans from (-dx/2, -dy/2, -dz/2) to (dx/2, dy/2, dz/2).
features/12-cylinder/final-review.md:2196:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/12-cylinder/final-review.md:2203:    // 8 vertices of the cuboid
features/12-cylinder/final-review.md:2229:    // 12 edges of the cuboid
features/12-cylinder/final-review.md:2264:    // 6 faces of the cuboid
features/12-cylinder/final-review.md:2406:    fn test_cuboid_topology() {
features/12-cylinder/final-review.md:2408:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/12-cylinder/final-review.md:2410:        assert_eq!(solid.vertices.len(), 8, "cuboid should have 8 vertices");
features/12-cylinder/final-review.md:2411:        assert_eq!(solid.edges.len(), 12, "cuboid should have 12 edges");
features/12-cylinder/final-review.md:2412:        assert_eq!(solid.faces.len(), 6, "cuboid should have 6 faces");
features/12-cylinder/final-review.md:2413:        assert_eq!(solid.shells.len(), 1, "cuboid should have 1 shell");
features/12-cylinder/final-review.md:2414:        assert!(solid.shells[0].closed, "cuboid shell should be closed");
features/12-cylinder/final-review.md:2419:            "cuboid should have 24 half-edges"
features/12-cylinder/final-review.md:2422:        assert_eq!(solid.loops.len(), 6, "cuboid should have 6 loops");
features/12-cylinder/final-review.md:2426:    fn test_cuboid_vertex_positions() {
features/12-cylinder/final-review.md:2428:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
features/12-cylinder/final-review.md:2454:mod cuboid;
features/12-cylinder/final-review.md:2457:pub use cuboid::make_cuboid;
features/12-cylinder/final-review.md:2623:/// Minimum cross-product norm for a non-degenerate triangle.
features/12-cylinder/final-review.md:2861:/// Push a triangle, but skip degenerate (zero-area) ones.
features/12-cylinder/final-review.md:2902:    use crate::primitives::make_cuboid;
features/12-cylinder/final-review.md:2905:    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
features/12-cylinder/final-review.md:2924:    /// T07: Determinism — same input → same output.
features/12-cylinder/final-review.md:2941:    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
features/12-cylinder/final-review.md:2943:    fn test_tessellate_cuboid_unchanged() {
features/12-cylinder/final-review.md:2945:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
features/12-cylinder/final-review.md:2954:    /// T08 extended: exact positions and indices snapshot.
features/12-cylinder/final-review.md:2956:    fn test_cuboid_positions_indices_snapshot() {
features/12-cylinder/final-review.md:2960:        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
features/12-cylinder/final-review.md:2961:        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
features/12-cylinder/final-review.md:2971:    /// T09: Outward normals — all facet normals point away from centroid.
features/12-cylinder/final-review.md:3085:    fn test_tessellate_cuboid() {
features/12-cylinder/final-review.md:3087:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/12-cylinder/final-review.md:3101:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/12-cylinder/final-review.md:3102:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/12-cylinder/final-review.md:3157:    fn test_cylinder_degenerate_inputs() {
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
features/12-cylinder/final-review.md:3451:    use crate::primitives::make_cuboid;
features/12-cylinder/final-review.md:3454:    fn test_tessellate_cuboid() {
features/12-cylinder/final-review.md:3456:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/12-cylinder/final-review.md:3459:        // A cuboid has 6 faces, each tessellated as 2 triangles = 12 triangles
features/12-cylinder/final-review.md:3472:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/12-cylinder/final-review.md:3473:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/12-cylinder/final-review.md:3667:79:ROADMAP Phase 3「円柱・球・押し出しが作れる」の最初の一歩。現状カーネルは `make_cuboid` と平面専用の `tessellate_solid` しか持たない。本 Issue で **解析曲面(円柱)を厳密 B-rep として生成し、それを三角形メッシュへ離散化する土台**を確立し、`mycad export examples/cylinder.mycad -o out.stl` を動作させる。
features/12-cylinder/final-review.md:3669:127:**退化入力の検証（R02）**: `.mycad` 由来のユーザー入力は境界で弾く。`KernelError` に `InvalidParameter { kind: &'static str }` を追加し、`build_solid_from_features` の `CreateCylinder` 分岐で `radius > 0.0 && height > 0.0` を検証（非正なら `InvalidParameter` を返し `make_cylinder` を呼ばない）。`make_cylinder` 自体は `make_cuboid` と同じ `-> Solid` シグネチャを保ち、正値前提（検証は呼び出し側＝境界に集約）。前提条件はドキュメントコメントに明記し、`debug_assert!(radius > 0.0 && height > 0.0)` を先頭に置いて開発時の契約違反を検出（リリースでは境界検証済み前提で素通り）。解像度の退化は `TessellationOptions` のクランプ（前述）で吸収。
features/12-cylinder/final-review.md:3724:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/12-cylinder/final-review.md:3755:    section: "テスト計画 > T01"
features/12-cylinder/final-review.md:3756:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/12-cylinder/final-review.md:3785:    section: "テスト計画 > T01"
features/12-cylinder/final-review.md:3786:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/12-cylinder/final-review.md:3809:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/12-cylinder/final-review.md:3840:    section: "テスト計画 > T01"
features/12-cylinder/final-review.md:3841:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/12-cylinder/final-review.md:3870:    section: "テスト計画 > T01"
features/12-cylinder/final-review.md:3886:    12	/// Minimum cross-product norm for a non-degenerate triangle.
features/12-cylinder/final-review.md:4124:   250	/// Push a triangle, but skip degenerate (zero-area) ones.
features/12-cylinder/final-review.md:4168:   289	    use crate::primitives::make_cuboid;
features/12-cylinder/final-review.md:4171:   292	    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
features/12-cylinder/final-review.md:4190:   311	    /// T07: Determinism — same input → same output.
features/12-cylinder/final-review.md:4207:   328	    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
features/12-cylinder/final-review.md:4209:   330	    fn test_tessellate_cuboid_unchanged() {
features/12-cylinder/final-review.md:4211:   332	        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
features/12-cylinder/final-review.md:4220:   341	    /// T08 extended: exact positions and indices snapshot.
features/12-cylinder/final-review.md:4222:   343	    fn test_cuboid_positions_indices_snapshot() {
features/12-cylinder/final-review.md:4226:   347	        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
features/12-cylinder/final-review.md:4227:   348	        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
features/12-cylinder/final-review.md:4237:   358	    /// T09: Outward normals — all facet normals point away from centroid.
features/12-cylinder/final-review.md:4351:   472	    fn test_tessellate_cuboid() {
features/12-cylinder/final-review.md:4353:   474	        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/12-cylinder/final-review.md:4367:   488	        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/12-cylinder/final-review.md:4368:   489	        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/12-cylinder/final-review.md:4387:crates/mycad-kernel/src/primitives/cuboid.rs:237:            solid.half_edges.len(),
features/12-cylinder/final-review.md:4391:/bin/bash -lc "grep -n \"cross_norm\\|AREA_EPS\\|degenerate\" -i features/12-cylinder/final-review.md | head -80" in /home/bacon/mycad
features/12-cylinder/final-review.md:4394:863:+    fn test_cylinder_degenerate_inputs() {
features/12-cylinder/final-review.md:4395:963:+/// Minimum cross-product norm for a non-degenerate triangle.
features/12-cylinder/final-review.md:4397:1237:+/// Push a triangle, but skip degenerate (zero-area) ones.
features/12-cylinder/final-review.md:4400:2623:/// Minimum cross-product norm for a non-degenerate triangle.
features/12-cylinder/final-review.md:4402:2861:/// Push a triangle, but skip degenerate (zero-area) ones.
features/12-cylinder/final-review.md:4407:3157:    fn test_cylinder_degenerate_inputs() {
features/12-cylinder/final-review.md:4408:3886:    12	/// Minimum cross-product norm for a non-degenerate triangle.
features/12-cylinder/final-review.md:4410:4124:   250	/// Push a triangle, but skip degenerate (zero-area) ones.
features/18-api-skeleton/design-review-2.md:45:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/design-review-2.md:87:- 新規クレート `crates/mycad-api/`(lib + bin 構成)+ `make_cuboid` の入力検証(kernel)
features/18-api-skeleton/design-review-2.md:98:| `crates/mycad-kernel/src/primitives/cuboid.rs` | **(R02)** `make_cuboid` を `-> Result<Solid, KernelError>` 化。`dx/dy/dz` が非有限 or `<= 0` なら `InvalidParameter { kind }`(`make_cylinder` cylinder.rs:16-22 と対称)。インライン test の呼び出しを `.unwrap()` 化 |
features/18-api-skeleton/design-review-2.md:99:| `crates/mycad-build/src/lib.rs` | CreateBox 分岐を `Ok(make_cuboid(...))` → `make_cuboid(...)`(`?` 伝播)に変更 |
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
features/18-api-skeleton/design-review-2.md:173:| K01 | kernel 単体(R02) | `make_cuboid(0.0, 1.0, 1.0, ..)` 等 | `Err(InvalidParameter { kind: "..." })` |
features/18-api-skeleton/design-review-2.md:175:> **R05 反論**: Euler-Poincaré 検証は kernel(`cylinder.rs:180-195`)で、golden YAML ラウンドトリップは format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保済み。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。API 層は HTTP 契約(status・JSON 形・決定性・cylinder 経路疎通)に集中する。
features/18-api-skeleton/design-review-2.md:193:設計レビューとして、まず参照されている既存実装と規約が文書の主張と整合しているかを確認します。`run_export`、`make_cuboid`、build/tessellation、workspace 依存規約、既存テスト配置を先に見ます。
features/18-api-skeleton/design-review-2.md:324:./crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/design-review-2.md:346:./crates/mycad-kernel/src/primitives/cuboid.rs
features/18-api-skeleton/design-review-2.md:476:/bin/bash -lc 'rg -n "run_export|make_cuboid|tessellate_solid_with|test_assembly_roundtrip|workspace.dependencies|thiserror|TriangleMesh|build_solid_from_features|Document::from_path|InvalidExtension|make_cylinder|Euler|HalfEdge|Shell|Loop|IdGenerator" -S .' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:484:./ROADMAP.md:17:- `make_cuboid` → `tessellate_solid` のパイプラインが動作し、決定性テストが通る
features/18-api-skeleton/design-review-2.md:505:./.claude/skills/3ai/agents/codex-final-reviewer.md:49:    suggestion: "make_cuboid の edge 生成ロジックを見直す"
features/18-api-skeleton/design-review-2.md:535:./features/12-cylinder/design-review.md:79:ROADMAP Phase 3「円柱・球・押し出しが作れる」の最初の一歩。現状カーネルは `make_cuboid` と平面専用の `tessellate_solid` しか持たない。本 Issue で **解析曲面(円柱)を厳密 B-rep として生成し、それを三角形メッシュへ離散化する土台**を確立し、`mycad export examples/cylinder.mycad -o out.stl` を動作させる。
features/18-api-skeleton/design-review-2.md:539:./features/12-cylinder/design-review.md:125:**決定性**: ID 採番順を `make_cuboid` と同型に固定。solid → 頂点(v_bot,v_top) → エッジ(e_bot,e_top,e_seam) → 面ごとに[底,上,側]順で HE群→loop→face → shell。すべて `id_gen.next()` を固定順で呼ぶのみ。`HashMap`/乱数等の非決定要素なし。
features/18-api-skeleton/design-review-2.md:540:./features/12-cylinder/design-review.md:127:**退化**: `radius <= 0` または `height <= 0` はゼロ面積/ゼロ長を生むため将来的にエラー化対象だが、現状 `make_cuboid` も正値前提で検証していない。本 Issue は **正値前提**を踏襲し、退化はテストで「正値での妥当性」を担保（負値の明示エラーはスコープ外、必要なら Codex 指摘で追加検討）。
features/18-api-skeleton/design-review-2.md:546:./features/12-cylinder/design-review.md:176:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/18-api-skeleton/design-review-2.md:547:./features/12-cylinder/design-review.md:178:| T03 | Euler | V−E+F == 2 | `assert_eq!(2-3+3, 2)` 相当の検証 |
features/18-api-skeleton/design-review-2.md:548:./features/12-cylinder/design-review.md:183:| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index | 既存テスト維持 |
features/18-api-skeleton/design-review-2.md:549:./features/12-cylinder/design-review.md:185:| T10 | ディスパッチ統合 | `examples/cylinder.mycad` を `build_solid_from_features` でビルドし T02 と同じ counts | `feature_dispatcher.rs` |
features/18-api-skeleton/design-review-2.md:551:./features/12-cylinder/design-review.md:201:設計ドキュメントを MyCad の実装規約に照らして確認します。まず既存の `make_cuboid`、トポロジー型、テッセレーション、ADR とテスト構成を読み、提案内容と矛盾する点を特定します。
features/18-api-skeleton/design-review-2.md:552:./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:568:./features/12-cylinder/design-review.md:421:CLAUDE.md:29:│   ├── primitives/   プリミティブ生成 (make_cuboid)
features/18-api-skeleton/design-review-2.md:579:./features/12-cylinder/design-review.md:434:.claude/skills/3ai/agents/codex-final-reviewer.md:49:    suggestion: "make_cuboid の edge 生成ロジックを見直す"
features/18-api-skeleton/design-review-2.md:581:./features/12-cylinder/design-review.md:438:ROADMAP.md:17:- `make_cuboid` → `tessellate_solid` のパイプラインが動作し、決定性テストが通る
features/18-api-skeleton/design-review-2.md:589:./features/12-cylinder/design-review.md:450:features/12-cylinder/design-review.md:79:ROADMAP Phase 3「円柱・球・押し出しが作れる」の最初の一歩。現状カーネルは `make_cuboid` と平面専用の `tessellate_solid` しか持たない。本 Issue で **解析曲面(円柱)を厳密 B-rep として生成し、それを三角形メッシュへ離散化する土台**を確立し、`mycad export examples/cylinder.mycad -o out.stl` を動作させる。
features/18-api-skeleton/design-review-2.md:590:./features/12-cylinder/design-review.md:454:features/12-cylinder/design-review.md:125:**決定性**: ID 採番順を `make_cuboid` と同型に固定。solid → 頂点(v_bot,v_top) → エッジ(e_bot,e_top,e_seam) → 面ごとに[底,上,側]順で HE群→loop→face → shell。すべて `id_gen.next()` を固定順で呼ぶのみ。`HashMap`/乱数等の非決定要素なし。
features/18-api-skeleton/design-review-2.md:591:./features/12-cylinder/design-review.md:455:features/12-cylinder/design-review.md:127:**退化**: `radius <= 0` または `height <= 0` はゼロ面積/ゼロ長を生むため将来的にエラー化対象だが、現状 `make_cuboid` も正値前提で検証していない。本 Issue は **正値前提**を踏襲し、退化はテストで「正値での妥当性」を担保（負値の明示エラーはスコープ外、必要なら Codex 指摘で追加検討）。
features/18-api-skeleton/design-review-2.md:597:./features/12-cylinder/design-review.md:463:features/12-cylinder/design-review.md:176:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/18-api-skeleton/design-review-2.md:598:./features/12-cylinder/design-review.md:465:features/12-cylinder/design-review.md:183:| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index | 既存テスト維持 |
features/18-api-skeleton/design-review-2.md:600:./features/12-cylinder/design-review.md:467:features/12-cylinder/design-review.md:201:設計ドキュメントを MyCad の実装規約に照らして確認します。まず既存の `make_cuboid`、トポロジー型、テッセレーション、ADR とテスト構成を読み、提案内容と矛盾する点を特定します。
features/18-api-skeleton/design-review-2.md:601:./features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:616:./features/12-cylinder/design-review.md:496:crates/mycad-format/CLAUDE.md:33:| `CreateBox`    | `make_cuboid`  |
features/18-api-skeleton/design-review-2.md:621:./features/12-cylinder/design-review.md:509:crates/mycad-build/src/lib.rs:4:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:623:./features/12-cylinder/design-review.md:511:crates/mycad-build/src/lib.rs:26:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
features/18-api-skeleton/design-review-2.md:627:./features/12-cylinder/design-review.md:523:crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-2.md:634:./features/12-cylinder/design-review.md:531:crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:636:./features/12-cylinder/design-review.md:533:crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:639:./features/12-cylinder/design-review.md:537:crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:640:./features/12-cylinder/design-review.md:538:crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:645:./features/12-cylinder/design-review.md:545:crates/mycad-kernel/src/tessellation/stl.rs:52:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:647:./features/12-cylinder/design-review.md:548:crates/mycad-kernel/src/tessellation/stl.rs:58:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/18-api-skeleton/design-review-2.md:649:./features/12-cylinder/design-review.md:551:crates/mycad-kernel/src/tessellation/stl.rs:75:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/18-api-skeleton/design-review-2.md:650:./features/12-cylinder/design-review.md:553:crates/mycad-kernel/src/primitives/cuboid.rs:1:use crate::brep::topology::{IdGenerator, Solid};
features/18-api-skeleton/design-review-2.md:651:./features/12-cylinder/design-review.md:554:crates/mycad-kernel/src/primitives/cuboid.rs:16:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/18-api-skeleton/design-review-2.md:652:./features/12-cylinder/design-review.md:555:crates/mycad-kernel/src/primitives/cuboid.rs:223:    use crate::brep::topology::IdGenerator;
features/18-api-skeleton/design-review-2.md:653:./features/12-cylinder/design-review.md:556:crates/mycad-kernel/src/primitives/cuboid.rs:227:        let mut id_gen = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:654:./features/12-cylinder/design-review.md:557:crates/mycad-kernel/src/primitives/cuboid.rs:228:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:655:./features/12-cylinder/design-review.md:558:crates/mycad-kernel/src/primitives/cuboid.rs:247:        let mut id_gen = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:656:./features/12-cylinder/design-review.md:559:crates/mycad-kernel/src/primitives/cuboid.rs:248:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:657:./features/12-cylinder/design-review.md:560:crates/mycad-kernel/src/primitives/cuboid.rs:276:        let mut id_gen1 = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:658:./features/12-cylinder/design-review.md:561:crates/mycad-kernel/src/primitives/cuboid.rs:277:        let mut id_gen2 = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:659:./features/12-cylinder/design-review.md:562:crates/mycad-kernel/src/primitives/cuboid.rs:279:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:660:./features/12-cylinder/design-review.md:563:crates/mycad-kernel/src/primitives/cuboid.rs:280:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:671:./features/12-cylinder/design-review.md:575:crates/mycad-kernel/benches/tessellation.rs:2:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:673:./features/12-cylinder/design-review.md:578:crates/mycad-kernel/benches/tessellation.rs:15:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:674:./features/12-cylinder/design-review.md:580:crates/mycad-kernel/tests/cuboid_roundtrip.rs:1:use mycad_kernel::brep::topology::IdGenerator;
features/18-api-skeleton/design-review-2.md:675:./features/12-cylinder/design-review.md:581:crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:676:./features/12-cylinder/design-review.md:583:crates/mycad-kernel/tests/cuboid_roundtrip.rs:8:    let mut id_gen = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:677:./features/12-cylinder/design-review.md:584:crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:678:./features/12-cylinder/design-review.md:586:crates/mycad-kernel/tests/cuboid_roundtrip.rs:30:        let mut id_gen = IdGenerator::new(42);
features/18-api-skeleton/design-review-2.md:679:./features/12-cylinder/design-review.md:587:crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:700:./features/12-cylinder/design-review.md:844:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/18-api-skeleton/design-review-2.md:703:./features/12-cylinder/design-review.md:1056:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:705:./features/12-cylinder/design-review.md:1076:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:708:./features/12-cylinder/design-review.md:1107:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:709:./features/12-cylinder/design-review.md:1108:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:716:./features/12-cylinder/design-review.md:1270:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:718:./features/12-cylinder/design-review.md:1275:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:721:./features/12-cylinder/design-review.md:1291:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:722:./features/12-cylinder/design-review.md:1292:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:732:./features/12-cylinder/design-review.md:1660:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:735:./features/12-cylinder/design-review.md:1682:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
features/18-api-skeleton/design-review-2.md:756:./features/12-cylinder/design-review.md:2230:    fn test_assembly_roundtrip() {
features/18-api-skeleton/design-review-2.md:759:./features/12-cylinder/design-review.md:2262:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:761:./features/12-cylinder/design-review.md:2269:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:763:./features/12-cylinder/design-review.md:2291:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:777:./features/12-cylinder/design-review.md:2520:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/design-review-2.md:778:./features/12-cylinder/design-review.md:2521:    suggestion: "`sample_segment` を HalfEdge 向き込みの API に変更し、rev では `t1` から降順にサンプルするか、先頭点を必ず `HalfEdge.start_vertex` から取得すること。reversed line edge を含む cuboid 回帰テストも追加すること。"
features/18-api-skeleton/design-review-2.md:780:./features/12-cylinder/design-review.md:2530:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/design-review-2.md:783:./features/12-cylinder/design-review.md:2550:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/design-review-2.md:784:./features/12-cylinder/design-review.md:2551:    suggestion: "`sample_segment` を HalfEdge 向き込みの API に変更し、rev では `t1` から降順にサンプルするか、先頭点を必ず `HalfEdge.start_vertex` から取得すること。reversed line edge を含む cuboid 回帰テストも追加すること。"
features/18-api-skeleton/design-review-2.md:786:./features/12-cylinder/design-review.md:2560:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/design-review-2.md:805:./features/12-cylinder/design-review-2.md:79:ROADMAP Phase 3「円柱・球・押し出しが作れる」の最初の一歩。現状カーネルは `make_cuboid` と平面専用の `tessellate_solid` しか持たない。本 Issue で **解析曲面(円柱)を厳密 B-rep として生成し、それを三角形メッシュへ離散化する土台**を確立し、`mycad export examples/cylinder.mycad -o out.stl` を動作させる。
features/18-api-skeleton/design-review-2.md:809:./features/12-cylinder/design-review-2.md:125:**決定性**: ID 採番順を `make_cuboid` と同型に固定。solid → 頂点(v_bot,v_top) → エッジ(e_bot,e_top,e_seam) → 面ごとに[底,上,側]順で HE群→loop→face → shell。すべて `id_gen.next()` を固定順で呼ぶのみ。`HashMap`/乱数等の非決定要素なし。
features/18-api-skeleton/design-review-2.md:810:./features/12-cylinder/design-review-2.md:127:**退化入力の検証（R02）**: `.mycad` 由来のユーザー入力は境界で弾く。`KernelError` に `InvalidParameter { kind: &'static str }` を追加し、`build_solid_from_features` の `CreateCylinder` 分岐で `radius > 0.0 && height > 0.0` を検証（非正なら `InvalidParameter` を返し `make_cylinder` を呼ばない）。`make_cylinder` 自体は `make_cuboid` と同じ `-> Solid` シグネチャを保ち、正値前提（検証は呼び出し側＝境界に集約）。解像度の退化は `TessellationOptions` のクランプ（前述）で吸収。
features/18-api-skeleton/design-review-2.md:813:./features/12-cylinder/design-review-2.md:147:  1. 外側 loop の各 HE について、`forward` なら `(t0,t1)`、`!forward` なら `(t1,t0)` を `sample_segment` に渡して折れ線化 → 連結して境界点列。**先頭点が必ず `HalfEdge.start_vertex` に一致**するため、逆向き Line を含む cuboid でも従来と同一の角点列＝**平面出力不変**。
features/18-api-skeleton/design-review-2.md:818:./features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/design-review-2.md:819:./features/12-cylinder/design-review-2.md:187:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/18-api-skeleton/design-review-2.md:820:./features/12-cylinder/design-review-2.md:189:| T03 | Euler | V−E+F == 2 | `assert_eq!(2-3+3, 2)` 相当の検証 |
features/18-api-skeleton/design-review-2.md:821:./features/12-cylinder/design-review-2.md:194:| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index。cuboid は逆向き Line HE を含むため R01 の向き対応も同時に担保 | 既存テスト維持 + positions が従来値と一致 |
features/18-api-skeleton/design-review-2.md:822:./features/12-cylinder/design-review-2.md:196:| T10 | ディスパッチ統合 | `examples/cylinder.mycad` を `build_solid_from_features` でビルドし T02 と同じ counts | `feature_dispatcher.rs` |
features/18-api-skeleton/design-review-2.md:826:./features/12-cylinder/design-review-2.md:214:設計レビューとして、まず既存カーネルのトポロジー型・`make_cuboid`・テッセレーション実装・エラー/依存規約を確認します。文書内の前提が既存設計と噛み合っているかを見てから、重大度付きで指摘を整理します。
features/18-api-skeleton/design-review-2.md:827:./features/12-cylinder/design-review-2.md:244:既存実装の骨格が見えました。次に `make_cuboid`、B-rep 型、テッセレータ、`build`/CLI の現状を読み、今回の設計がどこで破綻しうるかを詰めます。
features/18-api-skeleton/design-review-2.md:829:./features/12-cylinder/design-review-2.md:263:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/18-api-skeleton/design-review-2.md:832:./features/12-cylinder/design-review-2.md:475:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:834:./features/12-cylinder/design-review-2.md:495:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:835:./features/12-cylinder/design-review-2.md:514:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-2.md:842:./features/12-cylinder/design-review-2.md:657:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:844:./features/12-cylinder/design-review-2.md:662:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:847:./features/12-cylinder/design-review-2.md:678:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:848:./features/12-cylinder/design-review-2.md:679:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:849:./features/12-cylinder/design-review-2.md:939:`make_cuboid` は `HalfEdge.start_vertex` の順序だけでループを表現しており、現状テッセレータもそこを前提にしています。次は B-rep 型定義と `build` 側の feature/history の扱いを見ます。
features/18-api-skeleton/design-review-2.md:851:./features/12-cylinder/design-review-2.md:946:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:854:./features/12-cylinder/design-review-2.md:968:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
features/18-api-skeleton/design-review-2.md:877:./features/12-cylinder/design-review-2.md:1065:crates/mycad-kernel/src/primitives/cuboid.rs:1:use crate::brep::topology::{IdGenerator, Solid};
features/18-api-skeleton/design-review-2.md:878:./features/12-cylinder/design-review-2.md:1066:crates/mycad-kernel/src/primitives/cuboid.rs:16:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/18-api-skeleton/design-review-2.md:879:./features/12-cylinder/design-review-2.md:1076:crates/mycad-kernel/src/primitives/cuboid.rs:223:    use crate::brep::topology::IdGenerator;
features/18-api-skeleton/design-review-2.md:880:./features/12-cylinder/design-review-2.md:1077:crates/mycad-kernel/src/primitives/cuboid.rs:227:        let mut id_gen = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:881:./features/12-cylinder/design-review-2.md:1078:crates/mycad-kernel/src/primitives/cuboid.rs:247:        let mut id_gen = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:882:./features/12-cylinder/design-review-2.md:1079:crates/mycad-kernel/src/primitives/cuboid.rs:276:        let mut id_gen1 = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:883:./features/12-cylinder/design-review-2.md:1080:crates/mycad-kernel/src/primitives/cuboid.rs:277:        let mut id_gen2 = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:912:./features/12-cylinder/design-review-2.md:1732:    fn test_assembly_roundtrip() {
features/18-api-skeleton/design-review-2.md:919:./features/12-cylinder/design-review-2.md:1789:/bin/bash -lc 'rg -n "golden|roundtrip|tessellate_solid_with|TessellationOptions|export" crates/mycad-cli tests crates/mycad-kernel crates/mycad-build' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:922:./features/12-cylinder/design-review-2.md:1863:./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:923:./features/12-cylinder/design-review-2.md:1875:./features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:924:./features/12-cylinder/design-review-2.md:1883:./features/12-cylinder/design-review.md:523:crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-2.md:931:./features/12-cylinder/design-review-2.md:1891:./features/12-cylinder/design-review.md:531:crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:933:./features/12-cylinder/design-review-2.md:1893:./features/12-cylinder/design-review.md:533:crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:936:./features/12-cylinder/design-review-2.md:1897:./features/12-cylinder/design-review.md:537:crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:937:./features/12-cylinder/design-review-2.md:1898:./features/12-cylinder/design-review.md:538:crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:940:./features/12-cylinder/design-review-2.md:1928:./crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-2.md:941:./features/12-cylinder/design-review-2.md:1936:./features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/design-review-2.md:942:./features/12-cylinder/design-review-2.md:1941:./features/12-cylinder/design-review-2.md:514:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-2.md:947:./features/12-cylinder/design-review-2.md:2016:レビュー項目はほぼ固まりました。既存コードとの照合上、重大なのは「汎用曲面テッセレーションの抽象が cylinder 専用の仮定を漏らしている点」と「Euler テストが実体を検証していない点」です。最終的に YAML だけ返すので、最後に指摘を絞ります。
features/18-api-skeleton/design-review-2.md:964:./features/12-cylinder/design-review-2.md:2223:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:966:./features/12-cylinder/design-review-2.md:2229:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/18-api-skeleton/design-review-2.md:968:./features/12-cylinder/design-review-2.md:2246:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/18-api-skeleton/design-review-2.md:971:./features/12-cylinder/design-review-2.md:2275:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:972:./features/12-cylinder/design-review-2.md:2276:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:973:./features/12-cylinder/design-review-2.md:2307:    finding: "「cuboid は既に外向きなのでスワップは発生しない／平面出力不変」という前提が既存 `make_cuboid` と整合していない。現状の cuboid ループ順では front face `0→1→3→2` などの幾何法線が outward normal と逆向きで、提案どおりの法線整合スワップを入れると既存 STL/indices は変わる。"
features/18-api-skeleton/design-review-2.md:974:./features/12-cylinder/design-review-2.md:2312:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/18-api-skeleton/design-review-2.md:977:./features/12-cylinder/design-review-2.md:2332:    finding: "「cuboid は既に外向きなのでスワップは発生しない／平面出力不変」という前提が既存 `make_cuboid` と整合していない。現状の cuboid ループ順では front face `0→1→3→2` などの幾何法線が outward normal と逆向きで、提案どおりの法線整合スワップを入れると既存 STL/indices は変わる。"
features/18-api-skeleton/design-review-2.md:978:./features/12-cylinder/design-review-2.md:2337:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/18-api-skeleton/design-review-2.md:986:./crates/mycad-format/src/document.rs:116:    fn test_assembly_roundtrip() {
features/18-api-skeleton/design-review-2.md:988:./crates/mycad-format/CLAUDE.md:33:| `CreateBox`    | `make_cuboid`  |
features/18-api-skeleton/design-review-2.md:1007:./features/12-cylinder/final-review-2.md:412:         let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1010:./features/12-cylinder/final-review-2.md:426:         let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:1011:./features/12-cylinder/final-review-2.md:427:         let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:1025:./features/12-cylinder/final-review-2.md:948:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review-2.md:1084:./features/12-cylinder/design-review-3.md:79:ROADMAP Phase 3「円柱・球・押し出しが作れる」の最初の一歩。現状カーネルは `make_cuboid` と平面専用の `tessellate_solid` しか持たない。本 Issue で **解析曲面(円柱)を厳密 B-rep として生成し、それを三角形メッシュへ離散化する土台**を確立し、`mycad export examples/cylinder.mycad -o out.stl` を動作させる。
features/18-api-skeleton/design-review-2.md:1088:./features/12-cylinder/design-review-3.md:125:**決定性**: ID 採番順を `make_cuboid` と同型に固定。solid → 頂点(v_bot,v_top) → エッジ(e_bot,e_top,e_seam) → 面ごとに[底,上,側]順で HE群→loop→face → shell。すべて `id_gen.next()` を固定順で呼ぶのみ。`HashMap`/乱数等の非決定要素なし。
features/18-api-skeleton/design-review-2.md:1089:./features/12-cylinder/design-review-3.md:127:**退化入力の検証（R02）**: `.mycad` 由来のユーザー入力は境界で弾く。`KernelError` に `InvalidParameter { kind: &'static str }` を追加し、`build_solid_from_features` の `CreateCylinder` 分岐で `radius > 0.0 && height > 0.0` を検証（非正なら `InvalidParameter` を返し `make_cylinder` を呼ばない）。`make_cylinder` 自体は `make_cuboid` と同じ `-> Solid` シグネチャを保ち、正値前提（検証は呼び出し側＝境界に集約）。前提条件はドキュメントコメントに明記し、`debug_assert!(radius > 0.0 && height > 0.0)` を先頭に置いて開発時の契約違反を検出（リリースでは境界検証済み前提で素通り）。解像度の退化は `TessellationOptions` のクランプ（前述）で吸収。
features/18-api-skeleton/design-review-2.md:1092:./features/12-cylinder/design-review-3.md:148:  1. 外側 loop の各 HE について、`forward` なら `(t0,t1)`、`!forward` なら `(t1,t0)` を `sample_segment` に渡して折れ線化 → 連結して境界点列。**先頭点が必ず `HalfEdge.start_vertex` に一致**するため、逆向き Line を含む cuboid でも従来と同一の角点列＝**平面出力不変**。
features/18-api-skeleton/design-review-2.md:1098:./features/12-cylinder/design-review-3.md:180:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/design-review-2.md:1101:./features/12-cylinder/design-review-3.md:188:- T03 を「実カウントから Euler を計算」へ修正（定数ハードコード撤廃）。
features/18-api-skeleton/design-review-2.md:1102:./features/12-cylinder/design-review-3.md:194:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/18-api-skeleton/design-review-2.md:1103:./features/12-cylinder/design-review-3.md:196:| T03 | Euler | 実 solid の `vertices.len() − edges.len() + faces.len() == 2 * (shells.len() − genus)`（genus=0）を**実カウントから**計算して検証（定数ハードコードでなく構造体から） | `assert_eq!` |
features/18-api-skeleton/design-review-2.md:1104:./features/12-cylinder/design-review-3.md:201:| T08 | 回帰(平面バイト不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index。**positions も indices も従来値と完全一致**（スワップ撤去によりバイト不変、R02）。cuboid は逆向き Line HE を含むため R01 の向き対応も同時に担保 | 既存テスト維持 + positions/indices 一致 |
features/18-api-skeleton/design-review-2.md:1105:./features/12-cylinder/design-review-3.md:203:| T10 | ディスパッチ統合 | `examples/cylinder.mycad` を `build_solid_from_features` でビルドし T02 と同じ counts | `feature_dispatcher.rs` |
features/18-api-skeleton/design-review-2.md:1109:./features/12-cylinder/design-review-3.md:221:設計ドキュメントをコードベースの前提と照らしてレビューします。まず既存の `make_cuboid`、トポロジー型、テッセレーション実装、エラー型を確認して、この提案が MyCad の規約と実装実態に噛み合うかを見ます。
features/18-api-skeleton/design-review-2.md:1110:./features/12-cylinder/design-review-3.md:223:/bin/bash -lc "rg -n \"make_cuboid|tessellate_solid|IdGenerator|KernelError|HalfEdge|inner_loops|Surface::|Curve::|TriangleMesh|Feature::CreateCylinder|CreateCylinder|UnsupportedFeature|InvalidParameter|mod.rs|pub use\" crates/mycad-kernel crates/mycad-build crates/mycad-cli -g '"'!target'"'" in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:1114:./features/12-cylinder/design-review-3.md:235:crates/mycad-build/src/lib.rs:4:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1116:./features/12-cylinder/design-review-3.md:240:crates/mycad-build/src/lib.rs:26:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
features/18-api-skeleton/design-review-2.md:1118:./features/12-cylinder/design-review-3.md:248:crates/mycad-kernel/benches/tessellation.rs:2:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1120:./features/12-cylinder/design-review-3.md:251:crates/mycad-kernel/benches/tessellation.rs:15:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1127:./features/12-cylinder/design-review-3.md:260:crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1129:./features/12-cylinder/design-review-3.md:262:crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1132:./features/12-cylinder/design-review-3.md:266:crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:1133:./features/12-cylinder/design-review-3.md:267:crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:1143:./features/12-cylinder/design-review-3.md:280:crates/mycad-kernel/src/tessellation/stl.rs:52:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1145:./features/12-cylinder/design-review-3.md:283:crates/mycad-kernel/src/tessellation/stl.rs:58:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/18-api-skeleton/design-review-2.md:1147:./features/12-cylinder/design-review-3.md:286:crates/mycad-kernel/src/tessellation/stl.rs:75:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/18-api-skeleton/design-review-2.md:1150:./features/12-cylinder/design-review-3.md:290:crates/mycad-kernel/tests/cuboid_roundtrip.rs:1:use mycad_kernel::brep::topology::IdGenerator;
features/18-api-skeleton/design-review-2.md:1151:./features/12-cylinder/design-review-3.md:291:crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1152:./features/12-cylinder/design-review-3.md:293:crates/mycad-kernel/tests/cuboid_roundtrip.rs:8:    let mut id_gen = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:1153:./features/12-cylinder/design-review-3.md:294:crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1154:./features/12-cylinder/design-review-3.md:296:crates/mycad-kernel/tests/cuboid_roundtrip.rs:30:        let mut id_gen = IdGenerator::new(42);
features/18-api-skeleton/design-review-2.md:1155:./features/12-cylinder/design-review-3.md:297:crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1156:./features/12-cylinder/design-review-3.md:319:crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-2.md:1157:./features/12-cylinder/design-review-3.md:322:crates/mycad-kernel/src/primitives/cuboid.rs:1:use crate::brep::topology::{IdGenerator, Solid};
features/18-api-skeleton/design-review-2.md:1158:./features/12-cylinder/design-review-3.md:323:crates/mycad-kernel/src/primitives/cuboid.rs:16:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/18-api-skeleton/design-review-2.md:1159:./features/12-cylinder/design-review-3.md:331:crates/mycad-kernel/src/primitives/cuboid.rs:223:    use crate::brep::topology::IdGenerator;
features/18-api-skeleton/design-review-2.md:1160:./features/12-cylinder/design-review-3.md:332:crates/mycad-kernel/src/primitives/cuboid.rs:227:        let mut id_gen = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:1161:./features/12-cylinder/design-review-3.md:333:crates/mycad-kernel/src/primitives/cuboid.rs:228:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1162:./features/12-cylinder/design-review-3.md:334:crates/mycad-kernel/src/primitives/cuboid.rs:247:        let mut id_gen = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:1163:./features/12-cylinder/design-review-3.md:335:crates/mycad-kernel/src/primitives/cuboid.rs:248:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1164:./features/12-cylinder/design-review-3.md:336:crates/mycad-kernel/src/primitives/cuboid.rs:276:        let mut id_gen1 = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:1165:./features/12-cylinder/design-review-3.md:337:crates/mycad-kernel/src/primitives/cuboid.rs:277:        let mut id_gen2 = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:1166:./features/12-cylinder/design-review-3.md:338:crates/mycad-kernel/src/primitives/cuboid.rs:279:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:1167:./features/12-cylinder/design-review-3.md:339:crates/mycad-kernel/src/primitives/cuboid.rs:280:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:1191:./features/12-cylinder/design-review-3.md:621:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/18-api-skeleton/design-review-2.md:1194:./features/12-cylinder/design-review-3.md:833:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1196:./features/12-cylinder/design-review-3.md:853:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1203:./features/12-cylinder/design-review-3.md:968:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1205:./features/12-cylinder/design-review-3.md:973:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1208:./features/12-cylinder/design-review-3.md:989:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:1209:./features/12-cylinder/design-review-3.md:990:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:1212:./features/12-cylinder/design-review-3.md:1310:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1215:./features/12-cylinder/design-review-3.md:1332:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
features/18-api-skeleton/design-review-2.md:1229:./features/12-cylinder/design-review-3.md:1463:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1231:./features/12-cylinder/design-review-3.md:1470:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1233:./features/12-cylinder/design-review-3.md:1492:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1249:./features/12-cylinder/design-review-3.md:1625:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-2.md:1255:./features/12-cylinder/design-review-3.md:1912:    fn test_assembly_roundtrip() {
features/18-api-skeleton/design-review-2.md:1268:./features/12-cylinder/design-review-3.md:2112:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1270:./features/12-cylinder/design-review-3.md:2118:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/18-api-skeleton/design-review-2.md:1272:./features/12-cylinder/design-review-3.md:2135:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/18-api-skeleton/design-review-2.md:1283:./features/12-cylinder/final-review.md:44:    - make_cylinder(radius, height) -> Result<Solid, KernelError> with is_finite+positive validation
features/18-api-skeleton/design-review-2.md:1285:./features/12-cylinder/final-review.md:64:-use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1286:./features/12-cylinder/final-review.md:65:+use mycad_kernel::primitives::{make_cuboid, make_cylinder};
features/18-api-skeleton/design-review-2.md:1289:./features/12-cylinder/final-review.md:87:         } => Ok(make_cuboid(*width, *height, *depth, gen)),
features/18-api-skeleton/design-review-2.md:1303:./features/12-cylinder/final-review.md:184: use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1305:./features/12-cylinder/final-review.md:197:         let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1316:./features/12-cylinder/final-review.md:742:+    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review-2.md:1334:./features/12-cylinder/final-review.md:945: pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-2.md:1352:./features/12-cylinder/final-review.md:1276:     use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1361:./features/12-cylinder/final-review.md:1319:+        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
features/18-api-skeleton/design-review-2.md:1364:./features/12-cylinder/final-review.md:1334:+        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
features/18-api-skeleton/design-review-2.md:1365:./features/12-cylinder/final-review.md:1335:+        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
features/18-api-skeleton/design-review-2.md:1373:./features/12-cylinder/final-review.md:1461:         let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1376:./features/12-cylinder/final-review.md:1478:         let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:1377:./features/12-cylinder/final-review.md:1479:         let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:1381:./features/12-cylinder/final-review.md:1547:     use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1383:./features/12-cylinder/final-review.md:1553:         let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/18-api-skeleton/design-review-2.md:1385:./features/12-cylinder/final-review.md:1571:             let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/18-api-skeleton/design-review-2.md:1387:./features/12-cylinder/final-review.md:1584: use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1389:./features/12-cylinder/final-review.md:1591:     let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1391:./features/12-cylinder/final-review.md:1614:         let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1414:./features/12-cylinder/final-review.md:2050:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review-2.md:1424:./features/12-cylinder/final-review.md:2196:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/18-api-skeleton/design-review-2.md:1427:./features/12-cylinder/final-review.md:2408:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1429:./features/12-cylinder/final-review.md:2428:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1430:./features/12-cylinder/final-review.md:2457:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-2.md:1448:./features/12-cylinder/final-review.md:2902:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1457:./features/12-cylinder/final-review.md:2945:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
features/18-api-skeleton/design-review-2.md:1460:./features/12-cylinder/final-review.md:2960:        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
features/18-api-skeleton/design-review-2.md:1461:./features/12-cylinder/final-review.md:2961:        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
features/18-api-skeleton/design-review-2.md:1469:./features/12-cylinder/final-review.md:3087:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1472:./features/12-cylinder/final-review.md:3101:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:1473:./features/12-cylinder/final-review.md:3102:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:1503:./features/12-cylinder/final-review.md:3451:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1505:./features/12-cylinder/final-review.md:3456:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1508:./features/12-cylinder/final-review.md:3472:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:1509:./features/12-cylinder/final-review.md:3473:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:1510:./features/12-cylinder/final-review.md:3667:79:ROADMAP Phase 3「円柱・球・押し出しが作れる」の最初の一歩。現状カーネルは `make_cuboid` と平面専用の `tessellate_solid` しか持たない。本 Issue で **解析曲面(円柱)を厳密 B-rep として生成し、それを三角形メッシュへ離散化する土台**を確立し、`mycad export examples/cylinder.mycad -o out.stl` を動作させる。
features/18-api-skeleton/design-review-2.md:1511:./features/12-cylinder/final-review.md:3669:127:**退化入力の検証（R02）**: `.mycad` 由来のユーザー入力は境界で弾く。`KernelError` に `InvalidParameter { kind: &'static str }` を追加し、`build_solid_from_features` の `CreateCylinder` 分岐で `radius > 0.0 && height > 0.0` を検証（非正なら `InvalidParameter` を返し `make_cylinder` を呼ばない）。`make_cylinder` 自体は `make_cuboid` と同じ `-> Solid` シグネチャを保ち、正値前提（検証は呼び出し側＝境界に集約）。前提条件はドキュメントコメントに明記し、`debug_assert!(radius > 0.0 && height > 0.0)` を先頭に置いて開発時の契約違反を検出（リリースでは境界検証済み前提で素通り）。解像度の退化は `TessellationOptions` のクランプ（前述）で吸収。
features/18-api-skeleton/design-review-2.md:1519:./features/12-cylinder/final-review.md:3724:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/18-api-skeleton/design-review-2.md:1529:./features/12-cylinder/final-review.md:3809:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/18-api-skeleton/design-review-2.md:1553:./features/12-cylinder/final-review.md:4168:   289	    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1562:./features/12-cylinder/final-review.md:4211:   332	        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
features/18-api-skeleton/design-review-2.md:1565:./features/12-cylinder/final-review.md:4226:   347	        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
features/18-api-skeleton/design-review-2.md:1566:./features/12-cylinder/final-review.md:4227:   348	        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
features/18-api-skeleton/design-review-2.md:1574:./features/12-cylinder/final-review.md:4353:   474	        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1577:./features/12-cylinder/final-review.md:4367:   488	        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:1578:./features/12-cylinder/final-review.md:4368:   489	        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:1590:./crates/mycad-build/src/lib.rs:4:use mycad_kernel::primitives::{make_cuboid, make_cylinder};
features/18-api-skeleton/design-review-2.md:1593:./crates/mycad-build/src/lib.rs:26:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
features/18-api-skeleton/design-review-2.md:1601:./CLAUDE.md:29:│   ├── primitives/   プリミティブ生成 (make_cuboid)
features/18-api-skeleton/design-review-2.md:1623:./crates/mycad-kernel/src/tessellation/mod.rs:315:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1632:./crates/mycad-kernel/src/tessellation/mod.rs:358:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
features/18-api-skeleton/design-review-2.md:1635:./crates/mycad-kernel/src/tessellation/mod.rs:373:        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
features/18-api-skeleton/design-review-2.md:1636:./crates/mycad-kernel/src/tessellation/mod.rs:374:        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
features/18-api-skeleton/design-review-2.md:1644:./crates/mycad-kernel/src/tessellation/mod.rs:500:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1647:./crates/mycad-kernel/src/tessellation/mod.rs:514:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:1648:./crates/mycad-kernel/src/tessellation/mod.rs:515:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:1656:./crates/mycad-kernel/src/tessellation/stl.rs:52:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1658:./crates/mycad-kernel/src/tessellation/stl.rs:58:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/18-api-skeleton/design-review-2.md:1660:./crates/mycad-kernel/src/tessellation/stl.rs:75:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/18-api-skeleton/design-review-2.md:1684:./features/18-api-skeleton/design-review.md:146:| T01 | 正常系 | `GET /api/v0/mesh?file=examples/simple_box.mycad` を oneshot で叩く | 200、body を `TriangleMesh` に deser でき `positions`/`indices` が非空、`indices.len() % 3 == 0` |
features/18-api-skeleton/design-review-2.md:1685:./features/18-api-skeleton/design-review.md:149:| T04 | 400 | 非 `.mycad` 拡張子 `file=Cargo.toml` | 400(InvalidExtension) |
features/18-api-skeleton/design-review-2.md:1718:./features/18-api-skeleton/design-review.md:276:crates/mycad-kernel/src/primitives/cylinder.rs:180:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review-2.md:1745:./features/18-api-skeleton/design-review.md:590:use mycad_kernel::primitives::{make_cuboid, make_cylinder};
features/18-api-skeleton/design-review-2.md:1748:./features/18-api-skeleton/design-review.md:612:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
features/18-api-skeleton/design-review-2.md:1755:./features/18-api-skeleton/design-review.md:742:    fn test_assembly_roundtrip() {
features/18-api-skeleton/design-review-2.md:1774:./features/18-api-skeleton/design-review.md:1112:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1783:./features/18-api-skeleton/design-review.md:1155:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
features/18-api-skeleton/design-review-2.md:1786:./features/18-api-skeleton/design-review.md:1170:        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
features/18-api-skeleton/design-review-2.md:1787:./features/18-api-skeleton/design-review.md:1171:        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
features/18-api-skeleton/design-review-2.md:1795:./features/18-api-skeleton/design-review.md:1297:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1798:./features/18-api-skeleton/design-review.md:1311:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:1799:./features/18-api-skeleton/design-review.md:1312:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:1801:./features/18-api-skeleton/design-review.md:1337:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/18-api-skeleton/design-review-2.md:1804:./features/18-api-skeleton/design-review.md:1549:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1806:./features/18-api-skeleton/design-review.md:1569:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1816:./features/18-api-skeleton/design-review.md:1765:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review-2.md:1845:./features/18-api-skeleton/design-review.md:2442:./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:1846:./features/18-api-skeleton/design-review.md:2450:./features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:1847:./features/18-api-skeleton/design-review.md:2458:./features/12-cylinder/design-review.md:523:crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-2.md:1854:./features/18-api-skeleton/design-review.md:2466:./features/12-cylinder/design-review.md:531:crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1856:./features/18-api-skeleton/design-review.md:2468:./features/12-cylinder/design-review.md:533:crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1859:./features/18-api-skeleton/design-review.md:2472:./features/12-cylinder/design-review.md:537:crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:1860:./features/18-api-skeleton/design-review.md:2473:./features/12-cylinder/design-review.md:538:crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:1873:./features/18-api-skeleton/design-review.md:2539:./features/12-cylinder/design-review-3.md:223:/bin/bash -lc "rg -n \"make_cuboid|tessellate_solid|IdGenerator|KernelError|HalfEdge|inner_loops|Surface::|Curve::|TriangleMesh|Feature::CreateCylinder|CreateCylinder|UnsupportedFeature|InvalidParameter|mod.rs|pub use\" crates/mycad-kernel crates/mycad-build crates/mycad-cli -g '"'!target'"'" in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:1880:./features/18-api-skeleton/design-review.md:2549:./features/12-cylinder/design-review-3.md:260:crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1882:./features/18-api-skeleton/design-review.md:2551:./features/12-cylinder/design-review-3.md:262:crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1885:./features/18-api-skeleton/design-review.md:2555:./features/12-cylinder/design-review-3.md:266:crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:1886:./features/18-api-skeleton/design-review.md:2556:./features/12-cylinder/design-review-3.md:267:crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:1887:./features/18-api-skeleton/design-review.md:2560:./features/12-cylinder/design-review-3.md:319:crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-2.md:1889:./features/18-api-skeleton/design-review.md:2574:./features/12-cylinder/design-review-3.md:1625:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-2.md:1890:./features/18-api-skeleton/design-review.md:2583:./features/12-cylinder/design-review-2.md:514:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-2.md:1895:./features/18-api-skeleton/design-review.md:2609:./features/12-cylinder/design-review-2.md:1863:./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:1896:./features/18-api-skeleton/design-review.md:2617:./features/12-cylinder/design-review-2.md:1875:./features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:1897:./features/18-api-skeleton/design-review.md:2625:./features/12-cylinder/design-review-2.md:1883:./features/12-cylinder/design-review.md:523:crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-2.md:1904:./features/18-api-skeleton/design-review.md:2633:./features/12-cylinder/design-review-2.md:1891:./features/12-cylinder/design-review.md:531:crates/mycad-kernel/src/tessellation/mod.rs:99:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:1906:./features/18-api-skeleton/design-review.md:2635:./features/12-cylinder/design-review-2.md:1893:./features/12-cylinder/design-review.md:533:crates/mycad-kernel/src/tessellation/mod.rs:104:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:1909:./features/18-api-skeleton/design-review.md:2639:./features/12-cylinder/design-review-2.md:1897:./features/12-cylinder/design-review.md:537:crates/mycad-kernel/src/tessellation/mod.rs:120:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:1910:./features/18-api-skeleton/design-review.md:2640:./features/12-cylinder/design-review-2.md:1898:./features/12-cylinder/design-review.md:538:crates/mycad-kernel/src/tessellation/mod.rs:121:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:1913:./features/18-api-skeleton/design-review.md:2656:./features/12-cylinder/design-review-2.md:1928:./crates/mycad-kernel/src/primitives/mod.rs:3:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-2.md:1914:./features/18-api-skeleton/design-review.md:2665:./features/12-cylinder/design-review-2.md:1941:./features/12-cylinder/design-review-2.md:514:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-2.md:1919:./features/18-api-skeleton/design-review.md:2687:./features/12-cylinder/final-review.md:945: pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-2.md:1921:./features/18-api-skeleton/design-review.md:2696:./features/12-cylinder/final-review.md:2457:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-2.md:1944:./features/18-api-skeleton/design-review.md:2788:./crates/mycad-kernel/src/primitives/mod.rs:4:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-2.md:1947:./features/18-api-skeleton/design-review.md:2824:│   ├── primitives/   プリミティブ生成 (make_cuboid)
features/18-api-skeleton/design-review-2.md:1969:./features/18-api-skeleton/design-review.md:2898:features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/design-review-2.md:1970:./features/18-api-skeleton/design-review.md:2909:features/12-cylinder/design-review-2.md:1936:./features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/design-review-2.md:1973:./features/18-api-skeleton/design-review.md:2920:features/12-cylinder/design-review-3.md:180:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/design-review-2.md:1981:./features/18-api-skeleton/design-review.md:3025:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-2.md:1987:./features/18-api-skeleton/design-review.md:3071:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/design-review-2.md:1988:./features/18-api-skeleton/design-review.md:3072:    suggestion: "`sample_segment` を HalfEdge 向き込みの API に変更し、rev では `t1` から降順にサンプルするか、先頭点を必ず `HalfEdge.start_vertex` から取得すること。reversed line edge を含む cuboid 回帰テストも追加すること。"
features/18-api-skeleton/design-review-2.md:1990:./features/18-api-skeleton/design-review.md:3081:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/design-review-2.md:1993:./features/18-api-skeleton/design-review.md:3101:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/design-review-2.md:1994:./features/18-api-skeleton/design-review.md:3102:    suggestion: "`sample_segment` を HalfEdge 向き込みの API に変更し、rev では `t1` から降順にサンプルするか、先頭点を必ず `HalfEdge.start_vertex` から取得すること。reversed line edge を含む cuboid 回帰テストも追加すること。"
features/18-api-skeleton/design-review-2.md:1996:./features/18-api-skeleton/design-review.md:3111:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/design-review-2.md:2024:./features/18-api-skeleton/design-review.md:3354:    finding: "`build_solid_from_features` をそのまま再利用する方針では、`make_cuboid` 側に寸法バリデーションが無いため `width/height/depth <= 0` の `.mycad` を API が受理しうる。ゼロ長 edge・面積ゼロ face・縮退 loop をどう拒否するか、また数値境界を何で比較するかが計画に無い。"
features/18-api-skeleton/design-review-2.md:2025:./features/18-api-skeleton/design-review.md:3359:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/design-review-2.md:2026:./features/18-api-skeleton/design-review.md:3369:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/design-review-2.md:2027:./features/18-api-skeleton/design-review.md:3383:    finding: "`build_solid_from_features` をそのまま再利用する方針では、`make_cuboid` 側に寸法バリデーションが無いため `width/height/depth <= 0` の `.mycad` を API が受理しうる。ゼロ長 edge・面積ゼロ face・縮退 loop をどう拒否するか、また数値境界を何で比較するかが計画に無い。"
features/18-api-skeleton/design-review-2.md:2028:./features/18-api-skeleton/design-review.md:3388:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/design-review-2.md:2029:./features/18-api-skeleton/design-review.md:3398:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/design-review-2.md:2031:./crates/mycad-kernel/benches/tessellation.rs:2:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:2033:./crates/mycad-kernel/benches/tessellation.rs:15:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:2047:./features/18-api-skeleton/design-review-2.md:87:- 新規クレート `crates/mycad-api/`(lib + bin 構成)+ `make_cuboid` の入力検証(kernel)
features/18-api-skeleton/design-review-2.md:2048:./features/18-api-skeleton/design-review-2.md:98:| `crates/mycad-kernel/src/primitives/cuboid.rs` | **(R02)** `make_cuboid` を `-> Result<Solid, KernelError>` 化。`dx/dy/dz` が非有限 or `<= 0` なら `InvalidParameter { kind }`(`make_cylinder` cylinder.rs:16-22 と対称)。インライン test の呼び出しを `.unwrap()` 化 |
features/18-api-skeleton/design-review-2.md:2049:./features/18-api-skeleton/design-review-2.md:99:| `crates/mycad-build/src/lib.rs` | CreateBox 分岐を `Ok(make_cuboid(...))` → `make_cuboid(...)`(`?` 伝播)に変更 |
features/18-api-skeleton/design-review-2.md:2050:./features/18-api-skeleton/design-review-2.md:100:| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
features/18-api-skeleton/design-review-2.md:2061:./features/18-api-skeleton/design-review-2.md:164:| T01 | 正常系(box) | `GET /api/v0/mesh?file=<abs>/examples/simple_box.mycad` を oneshot | 200、`TriangleMesh` deser 可、`positions`/`indices` 非空、`indices.len() % 3 == 0` |
features/18-api-skeleton/design-review-2.md:2062:./features/18-api-skeleton/design-review-2.md:168:| T05 | 400 | 非 `.mycad` 拡張子(`Cargo.toml`) | 400(InvalidExtension) |
features/18-api-skeleton/design-review-2.md:2063:./features/18-api-skeleton/design-review-2.md:173:| K01 | kernel 単体(R02) | `make_cuboid(0.0, 1.0, 1.0, ..)` 等 | `Err(InvalidParameter { kind: "..." })` |
features/18-api-skeleton/design-review-2.md:2064:./features/18-api-skeleton/design-review-2.md:175:> **R05 反論**: Euler-Poincaré 検証は kernel(`cylinder.rs:180-195`)で、golden YAML ラウンドトリップは format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保済み。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。API 層は HTTP 契約(status・JSON 形・決定性・cylinder 経路疎通)に集中する。
features/18-api-skeleton/design-review-2.md:2066:./features/18-api-skeleton/design-review-2.md:193:設計レビューとして、まず参照されている既存実装と規約が文書の主張と整合しているかを確認します。`run_export`、`make_cuboid`、build/tessellation、workspace 依存規約、既存テスト配置を先に見ます。
features/18-api-skeleton/design-review-2.md:2067:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:1:use mycad_kernel::brep::topology::IdGenerator;
features/18-api-skeleton/design-review-2.md:2068:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:2069:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:8:    let mut id_gen = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:2070:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:2071:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:30:        let mut id_gen = IdGenerator::new(42);
features/18-api-skeleton/design-review-2.md:2072:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:2075:./features/18-api-skeleton/plan.md:13:- 新規クレート `crates/mycad-api/`(lib + bin 構成)+ `make_cuboid` の入力検証(kernel)
features/18-api-skeleton/design-review-2.md:2076:./features/18-api-skeleton/plan.md:24:| `crates/mycad-kernel/src/primitives/cuboid.rs` | **(R02)** `make_cuboid` を `-> Result<Solid, KernelError>` 化。`dx/dy/dz` が非有限 or `<= 0` なら `InvalidParameter { kind }`(`make_cylinder` cylinder.rs:16-22 と対称)。インライン test の呼び出しを `.unwrap()` 化 |
features/18-api-skeleton/design-review-2.md:2077:./features/18-api-skeleton/plan.md:25:| `crates/mycad-build/src/lib.rs` | CreateBox 分岐を `Ok(make_cuboid(...))` → `make_cuboid(...)`(`?` 伝播)に変更 |
features/18-api-skeleton/design-review-2.md:2078:./features/18-api-skeleton/plan.md:26:| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
features/18-api-skeleton/design-review-2.md:2089:./features/18-api-skeleton/plan.md:90:| T01 | 正常系(box) | `GET /api/v0/mesh?file=<abs>/examples/simple_box.mycad` を oneshot | 200、`TriangleMesh` deser 可、`positions`/`indices` 非空、`indices.len() % 3 == 0` |
features/18-api-skeleton/design-review-2.md:2090:./features/18-api-skeleton/plan.md:94:| T05 | 400 | 非 `.mycad` 拡張子(`Cargo.toml`) | 400(InvalidExtension) |
features/18-api-skeleton/design-review-2.md:2091:./features/18-api-skeleton/plan.md:99:| K01 | kernel 単体(R02) | `make_cuboid(0.0, 1.0, 1.0, ..)` 等 | `Err(InvalidParameter { kind: "..." })` |
features/18-api-skeleton/design-review-2.md:2092:./features/18-api-skeleton/plan.md:101:> **R05 反論**: Euler-Poincaré 検証は kernel(`cylinder.rs:180-195`)で、golden YAML ラウンドトリップは format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保済み。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。API 層は HTTP 契約(status・JSON 形・決定性・cylinder 経路疎通)に集中する。
features/18-api-skeleton/design-review-2.md:2115:./crates/mycad-kernel/src/primitives/mod.rs:4:pub use cuboid::make_cuboid;
features/18-api-skeleton/design-review-2.md:2117:./crates/mycad-kernel/src/primitives/cuboid.rs:1:use crate::brep::topology::{IdGenerator, Solid};
features/18-api-skeleton/design-review-2.md:2118:./crates/mycad-kernel/src/primitives/cuboid.rs:16:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/18-api-skeleton/design-review-2.md:2119:./crates/mycad-kernel/src/primitives/cuboid.rs:223:    use crate::brep::topology::IdGenerator;
features/18-api-skeleton/design-review-2.md:2120:./crates/mycad-kernel/src/primitives/cuboid.rs:227:        let mut id_gen = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:2121:./crates/mycad-kernel/src/primitives/cuboid.rs:228:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:2122:./crates/mycad-kernel/src/primitives/cuboid.rs:247:        let mut id_gen = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:2123:./crates/mycad-kernel/src/primitives/cuboid.rs:248:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:2124:./crates/mycad-kernel/src/primitives/cuboid.rs:276:        let mut id_gen1 = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:2125:./crates/mycad-kernel/src/primitives/cuboid.rs:277:        let mut id_gen2 = IdGenerator::new(0);
features/18-api-skeleton/design-review-2.md:2126:./crates/mycad-kernel/src/primitives/cuboid.rs:279:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:2127:./crates/mycad-kernel/src/primitives/cuboid.rs:280:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:2137:./crates/mycad-kernel/src/primitives/cylinder.rs:180:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review-2.md:2157:既存コードの実際の境界条件とテスト分担を確認しています。特に `make_cuboid` の退化対策、`build_solid_from_features` の assembly 扱い、`Document::from_path` の拡張子・存在判定、既存の決定性/Euler テスト有無を見ます。
features/18-api-skeleton/design-review-2.md:2235:use mycad_kernel::primitives::{make_cuboid, make_cylinder};
features/18-api-skeleton/design-review-2.md:2257:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
features/18-api-skeleton/design-review-2.md:2316:    /// Serialize to YAML string.
features/18-api-skeleton/design-review-2.md:2321:    /// Deserialize from YAML string.
features/18-api-skeleton/design-review-2.md:2346:    fn test_roundtrip_yaml() {
features/18-api-skeleton/design-review-2.md:2377:        assert_eq!(yaml1, yaml2, "YAML serialization must be deterministic");
features/18-api-skeleton/design-review-2.md:2407:    fn test_assembly_roundtrip() {
features/18-api-skeleton/design-review-2.md:2428:        // Roundtrip: serialize then deserialize yields the same YAML
features/18-api-skeleton/design-review-2.md:2436:/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/primitives/cuboid.rs" in /home/bacon/mycad
features/18-api-skeleton/design-review-2.md:2444:/// Create a cuboid (box) B-rep solid centered at the origin.
features/18-api-skeleton/design-review-2.md:2452:/// The cuboid spans from (-dx/2, -dy/2, -dz/2) to (dx/2, dy/2, dz/2).
features/18-api-skeleton/design-review-2.md:2453:pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/18-api-skeleton/design-review-2.md:2460:    // 8 vertices of the cuboid
features/18-api-skeleton/design-review-2.md:2486:    // 12 edges of the cuboid
features/18-api-skeleton/design-review-2.md:2521:    // 6 faces of the cuboid
features/18-api-skeleton/design-review-2.md:2663:    fn test_cuboid_topology() {
features/18-api-skeleton/design-review-2.md:2665:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:2667:        assert_eq!(solid.vertices.len(), 8, "cuboid should have 8 vertices");
features/18-api-skeleton/design-review-2.md:2668:        assert_eq!(solid.edges.len(), 12, "cuboid should have 12 edges");
features/18-api-skeleton/design-review-2.md:2669:        assert_eq!(solid.faces.len(), 6, "cuboid should have 6 faces");
features/18-api-skeleton/design-review-2.md:2670:        assert_eq!(solid.shells.len(), 1, "cuboid should have 1 shell");
features/18-api-skeleton/design-review-2.md:2671:        assert!(solid.shells[0].closed, "cuboid shell should be closed");
features/18-api-skeleton/design-review-2.md:2676:            "cuboid should have 24 half-edges"
features/18-api-skeleton/design-review-2.md:2679:        assert_eq!(solid.loops.len(), 6, "cuboid should have 6 loops");
features/18-api-skeleton/design-review-2.md:2683:    fn test_cuboid_vertex_positions() {
features/18-api-skeleton/design-review-2.md:2685:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:2834:    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
features/18-api-skeleton/design-review-2.md:2862:    /// T02: Topology counts.
features/18-api-skeleton/design-review-2.md:2881:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/design-review-2.md:2900:    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
features/18-api-skeleton/design-review-2.md:2955:    /// T05: Geometry — seam vertices and surface coefficients.
features/18-api-skeleton/design-review-2.md:3353:/// Minimum cross-product norm for a non-degenerate triangle.
features/18-api-skeleton/design-review-2.md:3871:/// Push a triangle, but skip degenerate (zero-area) ones.
features/18-api-skeleton/design-review-2.md:3910:    use crate::primitives::make_cuboid;
features/18-api-skeleton/design-review-2.md:3913:    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
features/18-api-skeleton/design-review-2.md:3932:    /// T07: Determinism — same input → same output.
features/18-api-skeleton/design-review-2.md:3949:    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
features/18-api-skeleton/design-review-2.md:3951:    fn test_tessellate_cuboid_unchanged() {
features/18-api-skeleton/design-review-2.md:3953:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
features/18-api-skeleton/design-review-2.md:3962:    /// T08 extended: exact positions and indices snapshot.
features/18-api-skeleton/design-review-2.md:3964:    fn test_cuboid_positions_indices_snapshot() {
features/18-api-skeleton/design-review-2.md:3968:        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
features/18-api-skeleton/design-review-2.md:3969:        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
features/18-api-skeleton/design-review-2.md:3979:    /// T09: Outward normals — all facet normals point away from centroid.
features/18-api-skeleton/design-review-2.md:4093:    fn test_tessellate_cuboid() {
features/18-api-skeleton/design-review-2.md:4095:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/design-review-2.md:4109:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/design-review-2.md:4110:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/design-review-2.md:4141:1. `src/primitives/` に新ファイルを作成（`cuboid.rs` をテンプレートとして使用）
features/18-api-skeleton/design-review-2.md:4181:設計原則: **決定性** (同じ入力は常に同じ出力)、**パラメトリック履歴** (feature historyが真実の源)、**YAML-based format** (人間可読・diff可能)。
features/18-api-skeleton/design-review-2.md:4190:│   ├── primitives/   プリミティブ生成 (make_cuboid)
features/18-api-skeleton/design-review-2.md:4192:├── mycad-format/     .mycadファイルのYAMLスキーマ・パーサー
features/18-api-skeleton/design-review-2.md:4249:    // simple_box.mycad: width=10, height=20, depth=30 → cuboid topology
features/18-api-skeleton/design-review-2.md:4286:        "cuboid should produce 12 facets (6 faces * 2 triangles)"
features/18-api-skeleton/design-review-2.md:4300:    #[error("YAML parse error: {0}")]
features/18-api-skeleton/design-review-2.md:4373:- server の multi-user 対応: `.mycad` は YAML で Git 管理可能な文字列 Feature 履歴。realtime 協調編集 (OT/CRDT) が不要 (Git で合流) なため、サーバ実装を大幅に簡略化できる。
features/18-api-skeleton/design-review-2.md:4515:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/design-review-2.md:4557:- 新規クレート `crates/mycad-api/`(lib + bin 構成)+ `make_cuboid` の入力検証(kernel)
features/18-api-skeleton/design-review-2.md:4568:| `crates/mycad-kernel/src/primitives/cuboid.rs` | **(R02)** `make_cuboid` を `-> Result<Solid, KernelError>` 化。`dx/dy/dz` が非有限 or `<= 0` なら `InvalidParameter { kind }`(`make_cylinder` cylinder.rs:16-22 と対称)。インライン test の呼び出しを `.unwrap()` 化 |
features/18-api-skeleton/design-review-2.md:4569:| `crates/mycad-build/src/lib.rs` | CreateBox 分岐を `Ok(make_cuboid(...))` → `make_cuboid(...)`(`?` 伝播)に変更 |
features/18-api-skeleton/design-review-2.md:4570:| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
features/18-api-skeleton/design-review-2.md:4720:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` の再利用方針と T09 の `EmptyFeatureList` 期待値では、`children` / `ref` を持つ assembly/reference 文書と、本当に feature が空の不正 part 文書を区別できない。これでは文書自身が要求している『assembly は明確なエラー + 422』を満たせない。"
features/18-api-skeleton/design-review-2.md:4724:    section: "影響ファイル > crates/mycad-kernel/src/primitives/cuboid.rs"
features/18-api-skeleton/design-review-2.md:4725:    finding: "`make_cuboid` の退化入力対策が `!is_finite() || <= 0` だけで、近接ゼロの正値をどう扱うか、既存の `AREA_EPS` や 1e-9 系判定とどう整合させるかが明記されていない。非常に小さい正寸法は B-rep 生成を通っても、後段テッセレーションで実質的に縮退しうる。"
features/18-api-skeleton/design-review-2.md:4726:    suggestion: "この Issue で exact-zero 方針を採るなら理由と影響範囲を明記し、少なくとも near-zero ケースを K01/API fixture に追加すること。可能なら共有公差定数を導入し `<= EPS` を `InvalidParameter` に統一すること。"
features/18-api-skeleton/design-review-2.md:4730:    finding: "golden YAML ラウンドトリップが既存 `examples/*` だけに依存しており、新規 `crates/mycad-api/tests/fixtures/*.mycad` の unsupported/退化 fixture については format crate 側の roundtrip 保証が計画されていない。API が依存する入力形状の YAML 安定性が未検証のまま残る。"
features/18-api-skeleton/design-review-2.md:4731:    suggestion: "`mycad-format` 側に新規 fixture を読み込む roundtrip/golden テストを追加するか、API fixture を共有して `to_yaml`/`from_yaml` の安定性を format crate の所有テストで担保すること。"
features/18-api-skeleton/design-review-2.md:4745:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` の再利用方針と T09 の `EmptyFeatureList` 期待値では、`children` / `ref` を持つ assembly/reference 文書と、本当に feature が空の不正 part 文書を区別できない。これでは文書自身が要求している『assembly は明確なエラー + 422』を満たせない。"
features/18-api-skeleton/design-review-2.md:4749:    section: "影響ファイル > crates/mycad-kernel/src/primitives/cuboid.rs"
features/18-api-skeleton/design-review-2.md:4750:    finding: "`make_cuboid` の退化入力対策が `!is_finite() || <= 0` だけで、近接ゼロの正値をどう扱うか、既存の `AREA_EPS` や 1e-9 系判定とどう整合させるかが明記されていない。非常に小さい正寸法は B-rep 生成を通っても、後段テッセレーションで実質的に縮退しうる。"
features/18-api-skeleton/design-review-2.md:4751:    suggestion: "この Issue で exact-zero 方針を採るなら理由と影響範囲を明記し、少なくとも near-zero ケースを K01/API fixture に追加すること。可能なら共有公差定数を導入し `<= EPS` を `InvalidParameter` に統一すること。"
features/18-api-skeleton/design-review-2.md:4755:    finding: "golden YAML ラウンドトリップが既存 `examples/*` だけに依存しており、新規 `crates/mycad-api/tests/fixtures/*.mycad` の unsupported/退化 fixture については format crate 側の roundtrip 保証が計画されていない。API が依存する入力形状の YAML 安定性が未検証のまま残る。"
features/18-api-skeleton/design-review-2.md:4756:    suggestion: "`mycad-format` 側に新規 fixture を読み込む roundtrip/golden テストを追加するか、API fixture を共有して `to_yaml`/`from_yaml` の安定性を format crate の所有テストで担保すること。"
features/18-api-skeleton/final-review.md:43:- 設計レビュー時に合意したテスト計画（ID T01〜）が全て実装されているか
features/18-api-skeleton/final-review.md:45:- golden YAML ラウンドトリップが含まれているか
features/18-api-skeleton/final-review.md:62:    suggestion: "make_cuboid の edge 生成ロジックを見直す"
features/18-api-skeleton/final-review.md:84:crates/mycad-kernel/src/primitives/cuboid.rs
features/18-api-skeleton/final-review.md:87:crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:98: M crates/mycad-kernel/src/primitives/cuboid.rs
features/18-api-skeleton/final-review.md:101: M crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:117: crates/mycad-kernel/src/primitives/cuboid.rs  | 115 +++++++-
features/18-api-skeleton/final-review.md:120: crates/mycad-kernel/tests/cuboid_roundtrip.rs |   4 +-
features/18-api-skeleton/final-review.md:159:/bin/bash -lc 'git diff -- Cargo.toml Cargo.lock crates/mycad-build/src/lib.rs crates/mycad-kernel/benches/tessellation.rs crates/mycad-kernel/src/primitives/cuboid.rs crates/mycad-kernel/src/tessellation/mod.rs crates/mycad-kernel/src/tessellation/stl.rs crates/mycad-kernel/tests/cuboid_roundtrip.rs' in /home/bacon/mycad
features/18-api-skeleton/final-review.md:654:-        } => Ok(make_cuboid(*width, *height, *depth, gen)),
features/18-api-skeleton/final-review.md:655:+        } => make_cuboid(*width, *height, *depth, gen),
features/18-api-skeleton/final-review.md:667:-        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/final-review.md:668:+        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review.md:672:diff --git a/crates/mycad-kernel/src/primitives/cuboid.rs b/crates/mycad-kernel/src/primitives/cuboid.rs
features/18-api-skeleton/final-review.md:674:--- a/crates/mycad-kernel/src/primitives/cuboid.rs
features/18-api-skeleton/final-review.md:675:+++ b/crates/mycad-kernel/src/primitives/cuboid.rs
features/18-api-skeleton/final-review.md:685: /// The cuboid spans from (-dx/2, -dy/2, -dz/2) to (dx/2, dy/2, dz/2).
features/18-api-skeleton/final-review.md:686:-pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
features/18-api-skeleton/final-review.md:687:+pub fn make_cuboid(
features/18-api-skeleton/final-review.md:705:@@ -214,18 +229,19 @@ pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid
features/18-api-skeleton/final-review.md:720:     fn test_cuboid_topology() {
features/18-api-skeleton/final-review.md:722:-        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/final-review.md:723:+        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review.md:725:         assert_eq!(solid.vertices.len(), 8, "cuboid should have 8 vertices");
features/18-api-skeleton/final-review.md:726:         assert_eq!(solid.edges.len(), 12, "cuboid should have 12 edges");
features/18-api-skeleton/final-review.md:729:     fn test_cuboid_vertex_positions() {
features/18-api-skeleton/final-review.md:731:-        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);
features/18-api-skeleton/final-review.md:732:+        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review.md:740:-        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
features/18-api-skeleton/final-review.md:741:-        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);
features/18-api-skeleton/final-review.md:742:+        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1).unwrap();
features/18-api-skeleton/final-review.md:743:+        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2).unwrap();
features/18-api-skeleton/final-review.md:752:+    // K01: make_cuboid rejects zero, negative, NaN, Infinity dimensions
features/18-api-skeleton/final-review.md:754:+    fn test_make_cuboid_invalid_dimensions() {
features/18-api-skeleton/final-review.md:759:+            make_cuboid(0.0, 1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:763:+            make_cuboid(1.0, 0.0, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:767:+            make_cuboid(1.0, 1.0, 0.0, &mut gen),
features/18-api-skeleton/final-review.md:773:+            make_cuboid(-1.0, 1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:777:+            make_cuboid(1.0, -1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:781:+            make_cuboid(1.0, 1.0, -1.0, &mut gen),
features/18-api-skeleton/final-review.md:787:+            make_cuboid(f64::NAN, 1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:791:+            make_cuboid(1.0, f64::NAN, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:795:+            make_cuboid(1.0, 1.0, f64::NAN, &mut gen),
features/18-api-skeleton/final-review.md:801:+            make_cuboid(f64::INFINITY, 1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:805:+            make_cuboid(1.0, f64::INFINITY, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:809:+            make_cuboid(1.0, 1.0, f64::INFINITY, &mut gen),
features/18-api-skeleton/final-review.md:815:+            make_cuboid(f64::NEG_INFINITY, 1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:819:+            make_cuboid(1.0, f64::NEG_INFINITY, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:823:+            make_cuboid(1.0, 1.0, f64::NEG_INFINITY, &mut gen),
features/18-api-skeleton/final-review.md:828:+    // K02: near-zero positive value is Ok (exact <= 0.0 policy)
features/18-api-skeleton/final-review.md:830:+    fn test_make_cuboid_near_zero_positive_ok() {
features/18-api-skeleton/final-review.md:832:+        let result = make_cuboid(1e-9, 1.0, 1.0, &mut gen);
features/18-api-skeleton/final-review.md:845:     fn test_tessellate_cuboid_unchanged() {
features/18-api-skeleton/final-review.md:847:-        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
features/18-api-skeleton/final-review.md:848:+        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen).unwrap();
features/18-api-skeleton/final-review.md:856:-        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
features/18-api-skeleton/final-review.md:857:-        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
features/18-api-skeleton/final-review.md:858:+        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1).unwrap();
features/18-api-skeleton/final-review.md:859:+        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2).unwrap();
features/18-api-skeleton/final-review.md:865:     fn test_tessellate_cuboid() {
features/18-api-skeleton/final-review.md:867:-        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/final-review.md:868:+        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review.md:876:-        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/final-review.md:877:-        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/final-review.md:878:+        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1).unwrap();
features/18-api-skeleton/final-review.md:879:+        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2).unwrap();
features/18-api-skeleton/final-review.md:889:     fn ascii_stl_cuboid_structure() {
features/18-api-skeleton/final-review.md:891:-        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/18-api-skeleton/final-review.md:892:+        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g).unwrap();
features/18-api-skeleton/final-review.md:900:-            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
features/18-api-skeleton/final-review.md:901:+            let s = make_cuboid(1.0, 1.0, 1.0, &mut g).unwrap();
features/18-api-skeleton/final-review.md:905:diff --git a/crates/mycad-kernel/tests/cuboid_roundtrip.rs b/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:907:--- a/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:908:+++ b/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:911: fn cuboid_to_mesh_pipeline() {
features/18-api-skeleton/final-review.md:913:-    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/final-review.md:914:+    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review.md:918:@@ -28,7 +28,7 @@ fn cuboid_to_mesh_pipeline() {
features/18-api-skeleton/final-review.md:919: fn cuboid_pipeline_is_deterministic() {
features/18-api-skeleton/final-review.md:922:-        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/18-api-skeleton/final-review.md:923:+        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review.md:1230:    50	// T01: normal box
features/18-api-skeleton/final-review.md:1242:    62	// T02: normal cylinder
features/18-api-skeleton/final-review.md:1253:    73	// T03: determinism
features/18-api-skeleton/final-review.md:1273:    93	// T04: 404 not found
features/18-api-skeleton/final-review.md:1283:   103	// T05: 400 non-.mycad extension
features/18-api-skeleton/final-review.md:1301:   121	// T06: 403 Host header (DNS rebinding protection)
features/18-api-skeleton/final-review.md:1313:   133	    // Without explicit host validation middleware, this will pass through.
features/18-api-skeleton/final-review.md:1314:   134	    // The plan says to add Host header validation middleware.
features/18-api-skeleton/final-review.md:1320:   140	// T07: 422 unsupported feature (sphere)
features/18-api-skeleton/final-review.md:1329:   149	// T08: 422 degenerate dimension (zero-width box)
features/18-api-skeleton/final-review.md:1331:   151	async fn t08_degenerate_dimension() {
features/18-api-skeleton/final-review.md:1338:   158	// T09: 422 assembly unsupported
features/18-api-skeleton/final-review.md:1387:次はコンパイル観点と既存型定義を確認します。`make_cuboid` のシグネチャ変更が波及しているので、依存側の型制約やエラー型の実装も見ます。
features/18-api-skeleton/final-review.md:1389:/bin/bash -lc "rg -n \"pub struct TriangleMesh|derive\\(|thiserror|InvalidParameter|tessellate_solid_with|pub enum KernelError|pub enum TessellationError|make_cuboid\\(\" crates/mycad-kernel crates/mycad-build crates/mycad-format -g '*.rs'" in /home/bacon/mycad
features/18-api-skeleton/final-review.md:1395:crates/mycad-build/src/lib.rs:26:        } => make_cuboid(*width, *height, *depth, gen),
features/18-api-skeleton/final-review.md:1408:crates/mycad-kernel/src/tessellation/mod.rs:358:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen).unwrap();
features/18-api-skeleton/final-review.md:1409:crates/mycad-kernel/src/tessellation/mod.rs:373:        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1).unwrap();
features/18-api-skeleton/final-review.md:1410:crates/mycad-kernel/src/tessellation/mod.rs:374:        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2).unwrap();
features/18-api-skeleton/final-review.md:1413:crates/mycad-kernel/src/tessellation/mod.rs:500:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review.md:1414:crates/mycad-kernel/src/tessellation/mod.rs:514:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1).unwrap();
features/18-api-skeleton/final-review.md:1415:crates/mycad-kernel/src/tessellation/mod.rs:515:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2).unwrap();
features/18-api-skeleton/final-review.md:1418:crates/mycad-kernel/benches/tessellation.rs:15:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review.md:1421:crates/mycad-kernel/src/tessellation/stl.rs:58:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g).unwrap();
features/18-api-skeleton/final-review.md:1422:crates/mycad-kernel/src/tessellation/stl.rs:75:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g).unwrap();
features/18-api-skeleton/final-review.md:1448:crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review.md:1449:crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review.md:1450:crates/mycad-kernel/src/primitives/cuboid.rs:17:pub fn make_cuboid(
features/18-api-skeleton/final-review.md:1451:crates/mycad-kernel/src/primitives/cuboid.rs:24:        return Err(KernelError::InvalidParameter { kind: "width" });
features/18-api-skeleton/final-review.md:1452:crates/mycad-kernel/src/primitives/cuboid.rs:27:        return Err(KernelError::InvalidParameter { kind: "height" });
features/18-api-skeleton/final-review.md:1453:crates/mycad-kernel/src/primitives/cuboid.rs:30:        return Err(KernelError::InvalidParameter { kind: "depth" });
features/18-api-skeleton/final-review.md:1454:crates/mycad-kernel/src/primitives/cuboid.rs:244:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review.md:1455:crates/mycad-kernel/src/primitives/cuboid.rs:264:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review.md:1456:crates/mycad-kernel/src/primitives/cuboid.rs:295:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1).unwrap();
features/18-api-skeleton/final-review.md:1457:crates/mycad-kernel/src/primitives/cuboid.rs:296:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2).unwrap();
features/18-api-skeleton/final-review.md:1458:crates/mycad-kernel/src/primitives/cuboid.rs:323:            make_cuboid(0.0, 1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:1459:crates/mycad-kernel/src/primitives/cuboid.rs:324:            Err(KernelError::InvalidParameter { kind: "width" })
features/18-api-skeleton/final-review.md:1460:crates/mycad-kernel/src/primitives/cuboid.rs:327:            make_cuboid(1.0, 0.0, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:1461:crates/mycad-kernel/src/primitives/cuboid.rs:328:            Err(KernelError::InvalidParameter { kind: "height" })
features/18-api-skeleton/final-review.md:1462:crates/mycad-kernel/src/primitives/cuboid.rs:331:            make_cuboid(1.0, 1.0, 0.0, &mut gen),
features/18-api-skeleton/final-review.md:1463:crates/mycad-kernel/src/primitives/cuboid.rs:332:            Err(KernelError::InvalidParameter { kind: "depth" })
features/18-api-skeleton/final-review.md:1464:crates/mycad-kernel/src/primitives/cuboid.rs:337:            make_cuboid(-1.0, 1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:1465:crates/mycad-kernel/src/primitives/cuboid.rs:338:            Err(KernelError::InvalidParameter { kind: "width" })
features/18-api-skeleton/final-review.md:1466:crates/mycad-kernel/src/primitives/cuboid.rs:341:            make_cuboid(1.0, -1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:1467:crates/mycad-kernel/src/primitives/cuboid.rs:342:            Err(KernelError::InvalidParameter { kind: "height" })
features/18-api-skeleton/final-review.md:1468:crates/mycad-kernel/src/primitives/cuboid.rs:345:            make_cuboid(1.0, 1.0, -1.0, &mut gen),
features/18-api-skeleton/final-review.md:1469:crates/mycad-kernel/src/primitives/cuboid.rs:346:            Err(KernelError::InvalidParameter { kind: "depth" })
features/18-api-skeleton/final-review.md:1470:crates/mycad-kernel/src/primitives/cuboid.rs:351:            make_cuboid(f64::NAN, 1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:1471:crates/mycad-kernel/src/primitives/cuboid.rs:352:            Err(KernelError::InvalidParameter { kind: "width" })
features/18-api-skeleton/final-review.md:1472:crates/mycad-kernel/src/primitives/cuboid.rs:355:            make_cuboid(1.0, f64::NAN, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:1473:crates/mycad-kernel/src/primitives/cuboid.rs:356:            Err(KernelError::InvalidParameter { kind: "height" })
features/18-api-skeleton/final-review.md:1474:crates/mycad-kernel/src/primitives/cuboid.rs:359:            make_cuboid(1.0, 1.0, f64::NAN, &mut gen),
features/18-api-skeleton/final-review.md:1475:crates/mycad-kernel/src/primitives/cuboid.rs:360:            Err(KernelError::InvalidParameter { kind: "depth" })
features/18-api-skeleton/final-review.md:1476:crates/mycad-kernel/src/primitives/cuboid.rs:365:            make_cuboid(f64::INFINITY, 1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:1477:crates/mycad-kernel/src/primitives/cuboid.rs:366:            Err(KernelError::InvalidParameter { kind: "width" })
features/18-api-skeleton/final-review.md:1478:crates/mycad-kernel/src/primitives/cuboid.rs:369:            make_cuboid(1.0, f64::INFINITY, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:1479:crates/mycad-kernel/src/primitives/cuboid.rs:370:            Err(KernelError::InvalidParameter { kind: "height" })
features/18-api-skeleton/final-review.md:1480:crates/mycad-kernel/src/primitives/cuboid.rs:373:            make_cuboid(1.0, 1.0, f64::INFINITY, &mut gen),
features/18-api-skeleton/final-review.md:1481:crates/mycad-kernel/src/primitives/cuboid.rs:374:            Err(KernelError::InvalidParameter { kind: "depth" })
features/18-api-skeleton/final-review.md:1482:crates/mycad-kernel/src/primitives/cuboid.rs:379:            make_cuboid(f64::NEG_INFINITY, 1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:1483:crates/mycad-kernel/src/primitives/cuboid.rs:380:            Err(KernelError::InvalidParameter { kind: "width" })
features/18-api-skeleton/final-review.md:1484:crates/mycad-kernel/src/primitives/cuboid.rs:383:            make_cuboid(1.0, f64::NEG_INFINITY, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:1485:crates/mycad-kernel/src/primitives/cuboid.rs:384:            Err(KernelError::InvalidParameter { kind: "height" })
features/18-api-skeleton/final-review.md:1486:crates/mycad-kernel/src/primitives/cuboid.rs:387:            make_cuboid(1.0, 1.0, f64::NEG_INFINITY, &mut gen),
features/18-api-skeleton/final-review.md:1487:crates/mycad-kernel/src/primitives/cuboid.rs:388:            Err(KernelError::InvalidParameter { kind: "depth" })
features/18-api-skeleton/final-review.md:1488:crates/mycad-kernel/src/primitives/cuboid.rs:396:        let result = make_cuboid(1e-9, 1.0, 1.0, &mut gen);
features/18-api-skeleton/final-review.md:1492:2026-05-25T06:37:17.744507Z ERROR codex_core::tools::router: error=exec_command failed for `/bin/bash -lc 'CARGO_TARGET_DIR=/tmp/mycad-target cargo test -p mycad-kernel --no-run'`: SandboxDenied { message: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", output: ExecToolCallOutput { exit_code: 101, stdout: StreamOutput { text: "", truncated_after_lines: None }, stderr: StreamOutput { text: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", truncated_after_lines: None }, aggregated_output: StreamOutput { text: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", truncated_after_lines: None }, duration: 0ns, timed_out: false } }
features/18-api-skeleton/final-review.md:1501:2026-05-25T06:37:17.862952Z ERROR codex_core::tools::router: error=exec_command failed for `/bin/bash -lc 'CARGO_TARGET_DIR=/tmp/mycad-target cargo test -p mycad-api --no-run'`: SandboxDenied { message: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", output: ExecToolCallOutput { exit_code: 101, stdout: StreamOutput { text: "", truncated_after_lines: None }, stderr: StreamOutput { text: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", truncated_after_lines: None }, aggregated_output: StreamOutput { text: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", truncated_after_lines: None }, duration: 0ns, timed_out: false } }
features/18-api-skeleton/final-review.md:1509:コンパイル確認は `sccache` 制約で止まったので、ラッパーを外して再試行します。あわせて、差分内コメントの `T01` 系テストが設計レビュー合意と一致しているかも確認します。
features/18-api-skeleton/final-review.md:1515:/bin/bash -lc "rg -n \"T0[1-9]|T10|T11|T12|T13|T14|Host header|golden|roundtrip|plan\" features crates/mycad-kernel crates/mycad-api -g '*.md' -g '*.rs'" in /home/bacon/mycad
features/18-api-skeleton/final-review.md:1519:crates/mycad-kernel/src/tessellation/mod.rs:318:    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
features/18-api-skeleton/final-review.md:1520:crates/mycad-kernel/src/tessellation/mod.rs:337:    /// T07: Determinism — same input → same output.
features/18-api-skeleton/final-review.md:1521:crates/mycad-kernel/src/tessellation/mod.rs:354:    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
features/18-api-skeleton/final-review.md:1522:crates/mycad-kernel/src/tessellation/mod.rs:367:    /// T08 extended: exact positions and indices snapshot.
features/18-api-skeleton/final-review.md:1523:crates/mycad-kernel/src/tessellation/mod.rs:384:    /// T09: Outward normals — all facet normals point away from centroid.
features/18-api-skeleton/final-review.md:1525:features/12-cylinder/design-review.md:45:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/final-review.md:1526:features/12-cylinder/design-review.md:176:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/18-api-skeleton/final-review.md:1527:features/12-cylinder/design-review.md:177:| T02 | トポロジー | 頂点2・エッジ3・面3・シェル1(closed)・HE6・loop3 | 各 `len()` 一致 |
features/18-api-skeleton/final-review.md:1528:features/12-cylinder/design-review.md:178:| T03 | Euler | V−E+F == 2 | `assert_eq!(2-3+3, 2)` 相当の検証 |
features/18-api-skeleton/final-review.md:1529:features/12-cylinder/design-review.md:179:| T04 | マニフォルド | 各エッジがちょうど 2 HE に参照され向きが逆 | 集計検証 |
features/18-api-skeleton/final-review.md:1530:features/12-cylinder/design-review.md:180:| T05 | 幾何 | シーム頂点が (0,r,0)/(0,r,h)、各 Surface/Curve の係数 | `approx` で一致 |
features/18-api-skeleton/final-review.md:1531:features/12-cylinder/design-review.md:181:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
features/18-api-skeleton/final-review.md:1532:features/12-cylinder/design-review.md:182:| T07 | 決定性(tessellate) | 同一入力2回で positions/normals/indices 一致 | `assert_eq!` |
features/18-api-skeleton/final-review.md:1533:features/12-cylinder/design-review.md:183:| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index | 既存テスト維持 |
features/18-api-skeleton/final-review.md:1534:features/12-cylinder/design-review.md:184:| T09 | 外向き法線 | 全 facet の幾何法線が重心からの外向きと同符号（凸形状なので判定可） | 検証 |
features/18-api-skeleton/final-review.md:1535:features/12-cylinder/design-review.md:185:| T10 | ディスパッチ統合 | `examples/cylinder.mycad` を `build_solid_from_features` でビルドし T02 と同じ counts | `feature_dispatcher.rs` |
features/18-api-skeleton/final-review.md:1536:features/12-cylinder/design-review.md:186:| T11 | export 統合 | `mycad export examples/cylinder.mycad -o tmp.stl` が成功し、facet 数が T06 と整合、ASCII STL として妥当 | `export.rs`（`CARGO_BIN_EXE_mycad`） |
features/18-api-skeleton/final-review.md:1538:features/12-cylinder/design-review.md:191:1. `cargo test --workspace` で T01–T12 green。
features/18-api-skeleton/final-review.md:1539:features/12-cylinder/design-review.md:269:crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:1540:features/12-cylinder/design-review.md:463:features/12-cylinder/design-review.md:176:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/18-api-skeleton/final-review.md:1541:features/12-cylinder/design-review.md:464:features/12-cylinder/design-review.md:181:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
features/18-api-skeleton/final-review.md:1542:features/12-cylinder/design-review.md:465:features/12-cylinder/design-review.md:183:| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index | 既存テスト維持 |
features/18-api-skeleton/final-review.md:1544:features/12-cylinder/design-review.md:580:crates/mycad-kernel/tests/cuboid_roundtrip.rs:1:use mycad_kernel::brep::topology::IdGenerator;
features/18-api-skeleton/final-review.md:1545:features/12-cylinder/design-review.md:581:crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/final-review.md:1546:features/12-cylinder/design-review.md:582:crates/mycad-kernel/tests/cuboid_roundtrip.rs:3:use mycad_kernel::tessellation::tessellate_solid;
features/18-api-skeleton/final-review.md:1547:features/12-cylinder/design-review.md:583:crates/mycad-kernel/tests/cuboid_roundtrip.rs:8:    let mut id_gen = IdGenerator::new(0);
features/18-api-skeleton/final-review.md:1548:features/12-cylinder/design-review.md:584:crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/final-review.md:1549:features/12-cylinder/design-review.md:585:crates/mycad-kernel/tests/cuboid_roundtrip.rs:17:    let mesh = tessellate_solid(&solid);
features/18-api-skeleton/final-review.md:1550:features/12-cylinder/design-review.md:586:crates/mycad-kernel/tests/cuboid_roundtrip.rs:30:        let mut id_gen = IdGenerator::new(42);
features/18-api-skeleton/final-review.md:1551:features/12-cylinder/design-review.md:587:crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/18-api-skeleton/final-review.md:1552:features/12-cylinder/design-review.md:588:crates/mycad-kernel/tests/cuboid_roundtrip.rs:32:        tessellate_solid(&solid)
features/18-api-skeleton/final-review.md:1557:features/12-cylinder/design-review.md:1473:    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
features/18-api-skeleton/final-review.md:1558:features/12-cylinder/design-review.md:1481:    fn test_uv_of_roundtrip_plane() {
features/18-api-skeleton/final-review.md:1559:features/12-cylinder/design-review.md:1488:        assert_uv_roundtrip(&surface, 0.7, -1.3);
features/18-api-skeleton/final-review.md:1560:features/12-cylinder/design-review.md:1492:    fn test_uv_of_roundtrip_cylinder() {
features/18-api-skeleton/final-review.md:1561:features/12-cylinder/design-review.md:1498:        assert_uv_roundtrip(&surface, 0.9, 1.7);
features/18-api-skeleton/final-review.md:1562:features/12-cylinder/design-review.md:1502:    fn test_uv_of_roundtrip_sphere() {
features/18-api-skeleton/final-review.md:1563:features/12-cylinder/design-review.md:1507:        assert_uv_roundtrip(&surface, 1.1, 0.6);
features/18-api-skeleton/final-review.md:1564:features/12-cylinder/design-review.md:1511:    fn test_uv_of_roundtrip_cone() {
features/18-api-skeleton/final-review.md:1565:features/12-cylinder/design-review.md:1517:        assert_uv_roundtrip(&surface, 0.8, 2.0);
features/18-api-skeleton/final-review.md:1568:features/12-cylinder/design-review.md:2169:    fn test_roundtrip_yaml() {
features/18-api-skeleton/final-review.md:1569:features/12-cylinder/design-review.md:2230:    fn test_assembly_roundtrip() {
features/18-api-skeleton/final-review.md:1570:features/12-cylinder/design-review.md:2259:/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/tests/cuboid_roundtrip.rs" in /home/bacon/mycad
features/18-api-skeleton/final-review.md:1571:features/12-cylinder/design-review.md:2486:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/final-review.md:1572:features/12-cylinder/design-review.md:2520:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:1573:features/12-cylinder/design-review.md:2529:    section: "設計方針 > 1. トポロジー / テスト計画 > T04"
features/18-api-skeleton/final-review.md:1574:features/12-cylinder/design-review.md:2530:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:1575:features/12-cylinder/design-review.md:2540:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
features/18-api-skeleton/final-review.md:1576:features/12-cylinder/design-review.md:2550:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:1577:features/12-cylinder/design-review.md:2559:    section: "設計方針 > 1. トポロジー / テスト計画 > T04"
features/18-api-skeleton/final-review.md:1578:features/12-cylinder/design-review.md:2560:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:1579:features/12-cylinder/design-review.md:2570:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
features/18-api-skeleton/final-review.md:1580:crates/mycad-api/tests/mesh_api.rs:50:// T01: normal box
features/18-api-skeleton/final-review.md:1581:crates/mycad-api/tests/mesh_api.rs:62:// T02: normal cylinder
features/18-api-skeleton/final-review.md:1582:crates/mycad-api/tests/mesh_api.rs:73:// T03: determinism
features/18-api-skeleton/final-review.md:1583:crates/mycad-api/tests/mesh_api.rs:93:// T04: 404 not found
features/18-api-skeleton/final-review.md:1584:crates/mycad-api/tests/mesh_api.rs:103:// T05: 400 non-.mycad extension
features/18-api-skeleton/final-review.md:1585:crates/mycad-api/tests/mesh_api.rs:121:// T06: 403 Host header (DNS rebinding protection)
features/18-api-skeleton/final-review.md:1586:crates/mycad-api/tests/mesh_api.rs:134:    // The plan says to add Host header validation middleware.
features/18-api-skeleton/final-review.md:1587:crates/mycad-api/tests/mesh_api.rs:140:// T07: 422 unsupported feature (sphere)
features/18-api-skeleton/final-review.md:1588:crates/mycad-api/tests/mesh_api.rs:149:// T08: 422 degenerate dimension (zero-width box)
features/18-api-skeleton/final-review.md:1589:crates/mycad-api/tests/mesh_api.rs:158:// T09: 422 assembly unsupported
features/18-api-skeleton/final-review.md:1592:crates/mycad-kernel/src/primitives/cylinder.rs:133:    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
features/18-api-skeleton/final-review.md:1593:crates/mycad-kernel/src/primitives/cylinder.rs:161:    /// T02: Topology counts.
features/18-api-skeleton/final-review.md:1594:crates/mycad-kernel/src/primitives/cylinder.rs:180:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/final-review.md:1595:crates/mycad-kernel/src/primitives/cylinder.rs:199:    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
features/18-api-skeleton/final-review.md:1596:crates/mycad-kernel/src/primitives/cylinder.rs:254:    /// T05: Geometry — seam vertices and surface coefficients.
features/18-api-skeleton/final-review.md:1599:features/12-cylinder/design-review-2.md:45:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/final-review.md:1600:features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/final-review.md:1601:features/12-cylinder/design-review-2.md:181:- R05(medium) 反映: golden YAML ラウンドトリップテスト T13 追加。
features/18-api-skeleton/final-review.md:1602:features/12-cylinder/design-review-2.md:187:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/18-api-skeleton/final-review.md:1603:features/12-cylinder/design-review-2.md:188:| T02 | トポロジー | 頂点2・エッジ3・面3・シェル1(closed)・HE6・loop3 | 各 `len()` 一致 |
features/18-api-skeleton/final-review.md:1604:features/12-cylinder/design-review-2.md:189:| T03 | Euler | V−E+F == 2 | `assert_eq!(2-3+3, 2)` 相当の検証 |
features/18-api-skeleton/final-review.md:1605:features/12-cylinder/design-review-2.md:190:| T04 | マニフォルド+ループ閉包 | 各エッジがちょうど 2 HE に参照され向きが逆。**かつ**各 loop で連続 HE の終点 ≈ 次 HE の始点、最後の HE 終点 ≈ 先頭 HE 始点（`approx`）。seam 接続も検証 | 集計 + 幾何連続性検証 (R03) |
features/18-api-skeleton/final-review.md:1606:features/12-cylinder/design-review-2.md:191:| T05 | 幾何 | シーム頂点が (0,r,0)/(0,r,h)、各 Surface/Curve の係数 | `approx` で一致 |
features/18-api-skeleton/final-review.md:1607:features/12-cylinder/design-review-2.md:192:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
features/18-api-skeleton/final-review.md:1608:features/12-cylinder/design-review-2.md:193:| T07 | 決定性(tessellate) | 同一入力2回で positions/normals/indices 一致 | `assert_eq!` |
features/18-api-skeleton/final-review.md:1609:features/12-cylinder/design-review-2.md:194:| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index。cuboid は逆向き Line HE を含むため R01 の向き対応も同時に担保 | 既存テスト維持 + positions が従来値と一致 |
features/18-api-skeleton/final-review.md:1610:features/12-cylinder/design-review-2.md:195:| T09 | 外向き法線 | 全 facet の幾何法線が重心からの外向きと同符号（凸形状なので判定可） | 検証 |
features/18-api-skeleton/final-review.md:1611:features/12-cylinder/design-review-2.md:196:| T10 | ディスパッチ統合 | `examples/cylinder.mycad` を `build_solid_from_features` でビルドし T02 と同じ counts | `feature_dispatcher.rs` |
features/18-api-skeleton/final-review.md:1612:features/12-cylinder/design-review-2.md:197:| T11 | export 統合 | `mycad export examples/cylinder.mycad -o tmp.stl` が成功し、facet 数が T06 と整合、ASCII STL として妥当 | `export.rs`（`CARGO_BIN_EXE_mycad`） |
features/18-api-skeleton/final-review.md:1614:features/12-cylinder/design-review-2.md:199:| T13 | golden YAML ラウンドトリップ | `examples/cylinder.mycad` を `from_path`→`to_yaml`→`from_yaml`→`to_yaml` で equality。`create_cylinder` の serialize/schema 回帰を固定 | `assert_eq!` (R05) |
features/18-api-skeleton/final-review.md:1616:features/12-cylinder/design-review-2.md:204:1. `cargo test --workspace` で T01–T12 green。
features/18-api-skeleton/final-review.md:1621:features/12-cylinder/design-review-2.md:860:    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
features/18-api-skeleton/final-review.md:1622:features/12-cylinder/design-review-2.md:868:    fn test_uv_of_roundtrip_plane() {
features/18-api-skeleton/final-review.md:1623:features/12-cylinder/design-review-2.md:875:        assert_uv_roundtrip(&surface, 0.7, -1.3);
features/18-api-skeleton/final-review.md:1624:features/12-cylinder/design-review-2.md:879:    fn test_uv_of_roundtrip_cylinder() {
features/18-api-skeleton/final-review.md:1625:features/12-cylinder/design-review-2.md:885:        assert_uv_roundtrip(&surface, 0.9, 1.7);
features/18-api-skeleton/final-review.md:1626:features/12-cylinder/design-review-2.md:889:    fn test_uv_of_roundtrip_sphere() {
features/18-api-skeleton/final-review.md:1627:features/12-cylinder/design-review-2.md:894:        assert_uv_roundtrip(&surface, 1.1, 0.6);
features/18-api-skeleton/final-review.md:1628:features/12-cylinder/design-review-2.md:898:    fn test_uv_of_roundtrip_cone() {
features/18-api-skeleton/final-review.md:1629:features/12-cylinder/design-review-2.md:904:        assert_uv_roundtrip(&surface, 0.8, 2.0);
features/18-api-skeleton/final-review.md:1631:features/12-cylinder/design-review-2.md:1671:    fn test_roundtrip_yaml() {
features/18-api-skeleton/final-review.md:1632:features/12-cylinder/design-review-2.md:1732:    fn test_assembly_roundtrip() {
features/18-api-skeleton/final-review.md:1633:features/12-cylinder/design-review-2.md:1789:/bin/bash -lc 'rg -n "golden|roundtrip|tessellate_solid_with|TessellationOptions|export" crates/mycad-cli tests crates/mycad-kernel crates/mycad-build' in /home/bacon/mycad
features/18-api-skeleton/final-review.md:1634:features/12-cylinder/design-review-2.md:1798:crates/mycad-kernel/src/geometry/surface.rs:168:    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
features/18-api-skeleton/final-review.md:1635:features/12-cylinder/design-review-2.md:1799:crates/mycad-kernel/src/geometry/surface.rs:176:    fn test_uv_of_roundtrip_plane() {
features/18-api-skeleton/final-review.md:1636:features/12-cylinder/design-review-2.md:1800:crates/mycad-kernel/src/geometry/surface.rs:183:        assert_uv_roundtrip(&surface, 0.7, -1.3);
features/18-api-skeleton/final-review.md:1637:features/12-cylinder/design-review-2.md:1801:crates/mycad-kernel/src/geometry/surface.rs:187:    fn test_uv_of_roundtrip_cylinder() {
features/18-api-skeleton/final-review.md:1638:features/12-cylinder/design-review-2.md:1802:crates/mycad-kernel/src/geometry/surface.rs:193:        assert_uv_roundtrip(&surface, 0.9, 1.7);
features/18-api-skeleton/final-review.md:1639:features/12-cylinder/design-review-2.md:1803:crates/mycad-kernel/src/geometry/surface.rs:197:    fn test_uv_of_roundtrip_sphere() {
features/18-api-skeleton/final-review.md:1640:features/12-cylinder/design-review-2.md:1804:crates/mycad-kernel/src/geometry/surface.rs:202:        assert_uv_roundtrip(&surface, 1.1, 0.6);
features/18-api-skeleton/final-review.md:1641:features/12-cylinder/design-review-2.md:1805:crates/mycad-kernel/src/geometry/surface.rs:206:    fn test_uv_of_roundtrip_cone() {
features/18-api-skeleton/final-review.md:1642:features/12-cylinder/design-review-2.md:1806:crates/mycad-kernel/src/geometry/surface.rs:212:        assert_uv_roundtrip(&surface, 0.8, 2.0);
features/18-api-skeleton/final-review.md:1643:features/12-cylinder/design-review-2.md:1936:./features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/final-review.md:1644:features/12-cylinder/design-review-2.md:2308:    suggestion: "この変更を意図的な winding 修正として明記し、`T08` を『counts は維持するが indices / STL facet normal は変わり得る』前提に更新するか、先に cuboid 側の loop orientation を正規化して本当に不変にすること。"
features/18-api-skeleton/final-review.md:1645:features/12-cylinder/design-review-2.md:2311:    section: "テスト計画 > T03"
features/18-api-skeleton/final-review.md:1646:features/12-cylinder/design-review-2.md:2312:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/18-api-skeleton/final-review.md:1647:features/12-cylinder/design-review-2.md:2333:    suggestion: "この変更を意図的な winding 修正として明記し、`T08` を『counts は維持するが indices / STL facet normal は変わり得る』前提に更新するか、先に cuboid 側の loop orientation を正規化して本当に不変にすること。"
features/18-api-skeleton/final-review.md:1648:features/12-cylinder/design-review-2.md:2336:    section: "テスト計画 > T03"
features/18-api-skeleton/final-review.md:1649:features/12-cylinder/design-review-2.md:2337:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/18-api-skeleton/final-review.md:1656:features/12-cylinder/final-review-2.md:712:    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
features/18-api-skeleton/final-review.md:1657:features/12-cylinder/final-review-2.md:720:    fn test_uv_of_roundtrip_plane() {
features/18-api-skeleton/final-review.md:1658:features/12-cylinder/final-review-2.md:727:        assert_uv_roundtrip(&surface, 0.7, -1.3);
features/18-api-skeleton/final-review.md:1659:features/12-cylinder/final-review-2.md:731:    fn test_uv_of_roundtrip_cylinder() {
features/18-api-skeleton/final-review.md:1660:features/12-cylinder/final-review-2.md:737:        assert_uv_roundtrip(&surface, 0.9, 1.7);
features/18-api-skeleton/final-review.md:1661:features/12-cylinder/final-review-2.md:741:    fn test_uv_of_roundtrip_sphere() {
features/18-api-skeleton/final-review.md:1662:features/12-cylinder/final-review-2.md:746:        assert_uv_roundtrip(&surface, 1.1, 0.6);
features/18-api-skeleton/final-review.md:1663:features/12-cylinder/final-review-2.md:750:    fn test_uv_of_roundtrip_cone() {
features/18-api-skeleton/final-review.md:1664:features/12-cylinder/final-review-2.md:756:        assert_uv_roundtrip(&surface, 0.8, 2.0);
features/18-api-skeleton/final-review.md:1665:features/12-cylinder/final-review-2.md:901:    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
features/18-api-skeleton/final-review.md:1666:features/12-cylinder/final-review-2.md:929:    /// T02: Topology counts.
features/18-api-skeleton/final-review.md:1667:features/12-cylinder/final-review-2.md:948:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/final-review.md:1668:features/12-cylinder/final-review-2.md:967:    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
features/18-api-skeleton/final-review.md:1669:features/18-api-skeleton/plan.md:9:> Codex 設計レビュー round 1-3 反映済み(3回上限)。round3 残存指摘は全て受諾し最終修正済み: R01=絶対パスのみ受理(決定的)、R02=v0 専用テッセレーション定数で Default 依存排除、R03=InvalidReference→422、R04=T09/T10 で assembly と空 part を分離、R05=K01 に INFINITY 分岐追加。round3 の verdict は fail のままだが指摘内容は本改訂で解消済み(ユーザー判断で再レビューせず確定)。
features/18-api-skeleton/final-review.md:1670:features/18-api-skeleton/plan.md:26:| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
features/18-api-skeleton/final-review.md:1672:features/18-api-skeleton/plan.md:107:| T01 | 正常系(box) | `GET /api/v0/mesh?file=<abs>/examples/simple_box.mycad` を oneshot | 200、`TriangleMesh` deser 可、`positions`/`indices` 非空、`indices.len() % 3 == 0` |
features/18-api-skeleton/final-review.md:1673:features/18-api-skeleton/plan.md:108:| T02 | 正常系(cylinder)(R05) | 同上 `examples/cylinder.mycad`(周期面経路) | 200、mesh 非空 |
features/18-api-skeleton/final-review.md:1674:features/18-api-skeleton/plan.md:109:| T03 | 決定性 | T01/T02 を 2 回実行 | 返る JSON バイト列が完全一致 |
features/18-api-skeleton/final-review.md:1675:features/18-api-skeleton/plan.md:110:| T04 | 404 | 存在しない `file` | 404、body `{"error": ...}` |
features/18-api-skeleton/final-review.md:1676:features/18-api-skeleton/plan.md:111:| T05 | 400 | 非 `.mycad` 拡張子(`Cargo.toml`) | 400(InvalidExtension) |
features/18-api-skeleton/final-review.md:1677:features/18-api-skeleton/plan.md:112:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/final-review.md:1678:features/18-api-skeleton/plan.md:113:| T07 | 422 未対応 feature(R03) | fixture `create_sphere` 単一 feature | 422(UnsupportedFeature) |
features/18-api-skeleton/final-review.md:1679:features/18-api-skeleton/plan.md:114:| T08 | 422 退化寸法(R02) | fixture `create_box` width=0(または負値) | 422(InvalidParameter) |
features/18-api-skeleton/final-review.md:1680:features/18-api-skeleton/plan.md:115:| T09 | 422 assembly 非対応(R04) | fixture: root features 空 + children あり | 422、body に assembly/reference 専用メッセージ(空 part とは別経路) |
features/18-api-skeleton/final-review.md:1683:features/18-api-skeleton/plan.md:121:> **R05 反論**(round 1): Euler-Poincaré は kernel(`cylinder.rs:180-195`)、golden YAML roundtrip は format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。
features/18-api-skeleton/final-review.md:1684:features/18-api-skeleton/plan.md:125:> **R04 反論**(round 2): 新規 `crates/mycad-api/tests/fixtures/*.mycad` は T07-T09 で API 経由で必ずパースされる(パースできなければ期待する 422 が出ず test fail)ため、入力形状の妥当性は API テストで担保済み。`Document` の YAML roundtrip 安定性は `mycad-format` の既存 roundtrip テストが所有。クロスクレートの roundtrip 追加は「テストは所有クレートに置く」規約に反するため行わない。fixture は最小に保つ。
features/18-api-skeleton/final-review.md:1686:features/12-cylinder/final-review.md:29: crates/mycad-kernel/tests/cuboid_roundtrip.rs  |   4 +-
features/18-api-skeleton/final-review.md:1688:features/12-cylinder/final-review.md:49:    - examples/cylinder.mycad + T01-T15 test suite (32 tests green)
features/18-api-skeleton/final-review.md:1693:features/12-cylinder/final-review.md:484:     fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
features/18-api-skeleton/final-review.md:1694:features/12-cylinder/final-review.md:492:     fn test_uv_of_roundtrip_plane() {
features/18-api-skeleton/final-review.md:1695:features/12-cylinder/final-review.md:499:         assert_uv_roundtrip(&surface, 0.7, -1.3);
features/18-api-skeleton/final-review.md:1696:features/12-cylinder/final-review.md:503:     fn test_uv_of_roundtrip_cylinder() {
features/18-api-skeleton/final-review.md:1697:features/12-cylinder/final-review.md:509:         assert_uv_roundtrip(&surface, 0.9, 1.7);
features/18-api-skeleton/final-review.md:1698:features/12-cylinder/final-review.md:513:     fn test_uv_of_roundtrip_sphere() {
features/18-api-skeleton/final-review.md:1699:features/12-cylinder/final-review.md:518:         assert_uv_roundtrip(&surface, 1.1, 0.6);
features/18-api-skeleton/final-review.md:1700:features/12-cylinder/final-review.md:522:     fn test_uv_of_roundtrip_cone() {
features/18-api-skeleton/final-review.md:1701:features/12-cylinder/final-review.md:528:         assert_uv_roundtrip(&surface, 0.8, 2.0);
features/18-api-skeleton/final-review.md:1703:features/12-cylinder/final-review.md:695:+    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
features/18-api-skeleton/final-review.md:1704:features/12-cylinder/final-review.md:723:+    /// T02: Topology counts.
features/18-api-skeleton/final-review.md:1705:features/12-cylinder/final-review.md:742:+    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/final-review.md:1706:features/12-cylinder/final-review.md:761:+    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
features/18-api-skeleton/final-review.md:1707:features/12-cylinder/final-review.md:816:+    /// T05: Geometry — seam vertices and surface coefficients.
features/18-api-skeleton/final-review.md:1712:features/12-cylinder/final-review.md:1279:+    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
features/18-api-skeleton/final-review.md:1713:features/12-cylinder/final-review.md:1298:+    /// T07: Determinism — same input → same output.
features/18-api-skeleton/final-review.md:1714:features/12-cylinder/final-review.md:1315:+    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
features/18-api-skeleton/final-review.md:1715:features/12-cylinder/final-review.md:1328:+    /// T08 extended: exact positions and indices snapshot.
features/18-api-skeleton/final-review.md:1716:features/12-cylinder/final-review.md:1345:+    /// T09: Outward normals — all facet normals point away from centroid.
features/18-api-skeleton/final-review.md:1718:features/12-cylinder/final-review.md:1578:diff --git a/crates/mycad-kernel/tests/cuboid_roundtrip.rs b/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:1719:features/12-cylinder/final-review.md:1580:--- a/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:1720:features/12-cylinder/final-review.md:1581:+++ b/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:1721:features/12-cylinder/final-review.md:2003:    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
features/18-api-skeleton/final-review.md:1722:features/12-cylinder/final-review.md:2031:    /// T02: Topology counts.
features/18-api-skeleton/final-review.md:1723:features/12-cylinder/final-review.md:2050:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/final-review.md:1724:features/12-cylinder/final-review.md:2069:    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
features/18-api-skeleton/final-review.md:1725:features/12-cylinder/final-review.md:2124:    /// T05: Geometry — seam vertices and surface coefficients.
features/18-api-skeleton/final-review.md:1728:features/12-cylinder/final-review.md:2905:    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
features/18-api-skeleton/final-review.md:1729:features/12-cylinder/final-review.md:2924:    /// T07: Determinism — same input → same output.
features/18-api-skeleton/final-review.md:1730:features/12-cylinder/final-review.md:2941:    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
features/18-api-skeleton/final-review.md:1731:features/12-cylinder/final-review.md:2954:    /// T08 extended: exact positions and indices snapshot.
features/18-api-skeleton/final-review.md:1732:features/12-cylinder/final-review.md:2971:    /// T09: Outward normals — all facet normals point away from centroid.
features/18-api-skeleton/final-review.md:1735:features/12-cylinder/final-review.md:3236:./features/12-cylinder/design-review-3.md:199:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
features/18-api-skeleton/final-review.md:1736:features/12-cylinder/final-review.md:3245:./features/12-cylinder/design-review-3.md:295:crates/mycad-kernel/tests/cuboid_roundtrip.rs:17:    let mesh = tessellate_solid(&solid);
features/18-api-skeleton/final-review.md:1737:features/12-cylinder/final-review.md:3246:./features/12-cylinder/design-review-3.md:298:crates/mycad-kernel/tests/cuboid_roundtrip.rs:32:        tessellate_solid(&solid)
features/18-api-skeleton/final-review.md:1738:features/12-cylinder/final-review.md:3298:./features/12-cylinder/design-review-2.md:192:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
features/18-api-skeleton/final-review.md:1739:features/12-cylinder/final-review.md:3311:./features/12-cylinder/design-review.md:181:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
features/18-api-skeleton/final-review.md:1740:features/12-cylinder/final-review.md:3313:./features/12-cylinder/design-review.md:464:features/12-cylinder/design-review.md:181:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
features/18-api-skeleton/final-review.md:1741:features/12-cylinder/final-review.md:3322:./features/12-cylinder/design-review.md:585:crates/mycad-kernel/tests/cuboid_roundtrip.rs:17:    let mesh = tessellate_solid(&solid);
features/18-api-skeleton/final-review.md:1742:features/12-cylinder/final-review.md:3323:./features/12-cylinder/design-review.md:588:crates/mycad-kernel/tests/cuboid_roundtrip.rs:32:        tessellate_solid(&solid)
features/18-api-skeleton/final-review.md:1743:features/12-cylinder/final-review.md:3331:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:17:    let mesh = tessellate_solid(&solid).unwrap();
features/18-api-skeleton/final-review.md:1744:features/12-cylinder/final-review.md:3332:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:32:        tessellate_solid(&solid).unwrap()
features/18-api-skeleton/final-review.md:1750:features/12-cylinder/final-review.md:3755:    section: "テスト計画 > T01"
features/18-api-skeleton/final-review.md:1751:features/12-cylinder/final-review.md:3756:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/18-api-skeleton/final-review.md:1752:features/12-cylinder/final-review.md:3785:    section: "テスト計画 > T01"
features/18-api-skeleton/final-review.md:1753:features/12-cylinder/final-review.md:3786:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/18-api-skeleton/final-review.md:1754:features/12-cylinder/final-review.md:3840:    section: "テスト計画 > T01"
features/18-api-skeleton/final-review.md:1755:features/12-cylinder/final-review.md:3841:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/18-api-skeleton/final-review.md:1756:features/12-cylinder/final-review.md:3870:    section: "テスト計画 > T01"
features/18-api-skeleton/final-review.md:1759:features/12-cylinder/final-review.md:4171:   292	    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
features/18-api-skeleton/final-review.md:1760:features/12-cylinder/final-review.md:4190:   311	    /// T07: Determinism — same input → same output.
features/18-api-skeleton/final-review.md:1761:features/12-cylinder/final-review.md:4207:   328	    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
features/18-api-skeleton/final-review.md:1762:features/12-cylinder/final-review.md:4220:   341	    /// T08 extended: exact positions and indices snapshot.
features/18-api-skeleton/final-review.md:1763:features/12-cylinder/final-review.md:4237:   358	    /// T09: Outward normals — all facet normals point away from centroid.
features/18-api-skeleton/final-review.md:1773:crates/mycad-kernel/src/geometry/surface.rs:188:    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
features/18-api-skeleton/final-review.md:1774:crates/mycad-kernel/src/geometry/surface.rs:196:    fn test_uv_of_roundtrip_plane() {
features/18-api-skeleton/final-review.md:1775:crates/mycad-kernel/src/geometry/surface.rs:203:        assert_uv_roundtrip(&surface, 0.7, -1.3);
features/18-api-skeleton/final-review.md:1776:crates/mycad-kernel/src/geometry/surface.rs:207:    fn test_uv_of_roundtrip_cylinder() {
features/18-api-skeleton/final-review.md:1777:crates/mycad-kernel/src/geometry/surface.rs:213:        assert_uv_roundtrip(&surface, 0.9, 1.7);
features/18-api-skeleton/final-review.md:1778:crates/mycad-kernel/src/geometry/surface.rs:217:    fn test_uv_of_roundtrip_sphere() {
features/18-api-skeleton/final-review.md:1779:crates/mycad-kernel/src/geometry/surface.rs:222:        assert_uv_roundtrip(&surface, 1.1, 0.6);
features/18-api-skeleton/final-review.md:1780:crates/mycad-kernel/src/geometry/surface.rs:226:    fn test_uv_of_roundtrip_cone() {
features/18-api-skeleton/final-review.md:1781:crates/mycad-kernel/src/geometry/surface.rs:232:        assert_uv_roundtrip(&surface, 0.8, 2.0);
features/18-api-skeleton/final-review.md:1783:features/18-api-skeleton/final-review.md:43:- 設計レビュー時に合意したテスト計画（ID T01〜）が全て実装されているか
features/18-api-skeleton/final-review.md:1784:features/18-api-skeleton/final-review.md:45:- golden YAML ラウンドトリップが含まれているか
features/18-api-skeleton/final-review.md:1785:features/18-api-skeleton/final-review.md:87:crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:1786:features/18-api-skeleton/final-review.md:101: M crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:1787:features/18-api-skeleton/final-review.md:120: crates/mycad-kernel/tests/cuboid_roundtrip.rs |   4 +-
features/18-api-skeleton/final-review.md:1789:features/18-api-skeleton/final-review.md:159:/bin/bash -lc 'git diff -- Cargo.toml Cargo.lock crates/mycad-build/src/lib.rs crates/mycad-kernel/benches/tessellation.rs crates/mycad-kernel/src/primitives/cuboid.rs crates/mycad-kernel/src/tessellation/mod.rs crates/mycad-kernel/src/tessellation/stl.rs crates/mycad-kernel/tests/cuboid_roundtrip.rs' in /home/bacon/mycad
features/18-api-skeleton/final-review.md:1790:features/18-api-skeleton/final-review.md:905:diff --git a/crates/mycad-kernel/tests/cuboid_roundtrip.rs b/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:1791:features/18-api-skeleton/final-review.md:907:--- a/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:1792:features/18-api-skeleton/final-review.md:908:+++ b/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:1793:features/18-api-skeleton/final-review.md:1230:    50	// T01: normal box
features/18-api-skeleton/final-review.md:1794:features/18-api-skeleton/final-review.md:1242:    62	// T02: normal cylinder
features/18-api-skeleton/final-review.md:1795:features/18-api-skeleton/final-review.md:1253:    73	// T03: determinism
features/18-api-skeleton/final-review.md:1796:features/18-api-skeleton/final-review.md:1273:    93	// T04: 404 not found
features/18-api-skeleton/final-review.md:1797:features/18-api-skeleton/final-review.md:1283:   103	// T05: 400 non-.mycad extension
features/18-api-skeleton/final-review.md:1798:features/18-api-skeleton/final-review.md:1301:   121	// T06: 403 Host header (DNS rebinding protection)
features/18-api-skeleton/final-review.md:1799:features/18-api-skeleton/final-review.md:1314:   134	    // The plan says to add Host header validation middleware.
features/18-api-skeleton/final-review.md:1800:features/18-api-skeleton/final-review.md:1320:   140	// T07: 422 unsupported feature (sphere)
features/18-api-skeleton/final-review.md:1801:features/18-api-skeleton/final-review.md:1329:   149	// T08: 422 degenerate dimension (zero-width box)
features/18-api-skeleton/final-review.md:1802:features/18-api-skeleton/final-review.md:1338:   158	// T09: 422 assembly unsupported
features/18-api-skeleton/final-review.md:1806:features/18-api-skeleton/final-review.md:1448:crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review.md:1807:features/18-api-skeleton/final-review.md:1449:crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review.md:1808:features/18-api-skeleton/final-review.md:1492:2026-05-25T06:37:17.744507Z ERROR codex_core::tools::router: error=exec_command failed for `/bin/bash -lc 'CARGO_TARGET_DIR=/tmp/mycad-target cargo test -p mycad-kernel --no-run'`: SandboxDenied { message: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", output: ExecToolCallOutput { exit_code: 101, stdout: StreamOutput { text: "", truncated_after_lines: None }, stderr: StreamOutput { text: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", truncated_after_lines: None }, aggregated_output: StreamOutput { text: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", truncated_after_lines: None }, duration: 0ns, timed_out: false } }
features/18-api-skeleton/final-review.md:1809:features/18-api-skeleton/final-review.md:1501:2026-05-25T06:37:17.862952Z ERROR codex_core::tools::router: error=exec_command failed for `/bin/bash -lc 'CARGO_TARGET_DIR=/tmp/mycad-target cargo test -p mycad-api --no-run'`: SandboxDenied { message: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", output: ExecToolCallOutput { exit_code: 101, stdout: StreamOutput { text: "", truncated_after_lines: None }, stderr: StreamOutput { text: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", truncated_after_lines: None }, aggregated_output: StreamOutput { text: "error: process didn't exit successfully: `sccache /home/bacon/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc -vV` (exit status: 2)\n--- stderr\nsccache: error: Operation not permitted (os error 1)\n\n", truncated_after_lines: None }, duration: 0ns, timed_out: false } }
features/18-api-skeleton/final-review.md:1810:features/18-api-skeleton/final-review.md:1509:コンパイル確認は `sccache` 制約で止まったので、ラッパーを外して再試行します。あわせて、差分内コメントの `T01` 系テストが設計レビュー合意と一致しているかも確認します。
features/18-api-skeleton/final-review.md:1811:features/18-api-skeleton/design-review-3.md:45:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/final-review.md:1812:features/18-api-skeleton/design-review-3.md:83:> Codex 設計レビュー round 1 + round 2 反映済み。round1: R01/R03/R04 受諾、R02 kernel 検証追加(ユーザー決定)、R05 部分受諾。round2: R01(構造化エラー)受諾、R02(assembly 判別)受諾、R03(公差)は #17 委譲で反論、R04(fixture roundtrip)反論。
features/18-api-skeleton/final-review.md:1813:features/18-api-skeleton/design-review-3.md:100:| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
features/18-api-skeleton/final-review.md:1814:features/18-api-skeleton/design-review-3.md:181:| T01 | 正常系(box) | `GET /api/v0/mesh?file=<abs>/examples/simple_box.mycad` を oneshot | 200、`TriangleMesh` deser 可、`positions`/`indices` 非空、`indices.len() % 3 == 0` |
features/18-api-skeleton/final-review.md:1815:features/18-api-skeleton/design-review-3.md:182:| T02 | 正常系(cylinder)(R05) | 同上 `examples/cylinder.mycad`(周期面経路) | 200、mesh 非空 |
features/18-api-skeleton/final-review.md:1816:features/18-api-skeleton/design-review-3.md:183:| T03 | 決定性 | T01/T02 を 2 回実行 | 返る JSON バイト列が完全一致 |
features/18-api-skeleton/final-review.md:1817:features/18-api-skeleton/design-review-3.md:184:| T04 | 404 | 存在しない `file` | 404、body `{"error": ...}` |
features/18-api-skeleton/final-review.md:1818:features/18-api-skeleton/design-review-3.md:185:| T05 | 400 | 非 `.mycad` 拡張子(`Cargo.toml`) | 400(InvalidExtension) |
features/18-api-skeleton/final-review.md:1819:features/18-api-skeleton/design-review-3.md:186:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/final-review.md:1820:features/18-api-skeleton/design-review-3.md:187:| T07 | 422 未対応 feature(R03) | fixture `create_sphere` 単一 feature | 422(UnsupportedFeature) |
features/18-api-skeleton/final-review.md:1821:features/18-api-skeleton/design-review-3.md:188:| T08 | 422 退化寸法(R02) | fixture `create_box` width=0(または負値) | 422(InvalidParameter) |
features/18-api-skeleton/final-review.md:1822:features/18-api-skeleton/design-review-3.md:189:| T09 | 422 assembly 非対応(R03) | fixture: root features 空 + children あり | 422(EmptyFeatureList、クラッシュしない) |
features/18-api-skeleton/final-review.md:1823:features/18-api-skeleton/design-review-3.md:193:> **R05 反論**(round 1): Euler-Poincaré は kernel(`cylinder.rs:180-195`)、golden YAML roundtrip は format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。
features/18-api-skeleton/final-review.md:1824:features/18-api-skeleton/design-review-3.md:197:> **R04 反論**(round 2): 新規 `crates/mycad-api/tests/fixtures/*.mycad` は T07-T09 で API 経由で必ずパースされる(パースできなければ期待する 422 が出ず test fail)ため、入力形状の妥当性は API テストで担保済み。`Document` の YAML roundtrip 安定性は `mycad-format` の既存 roundtrip テストが所有。クロスクレートの roundtrip 追加は「テストは所有クレートに置く」規約に反するため行わない。fixture は最小に保つ。
features/18-api-skeleton/final-review.md:1826:features/18-api-skeleton/design-review-3.md:239:crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/final-review.md:1827:features/18-api-skeleton/design-review-3.md:240:crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/final-review.md:1828:features/18-api-skeleton/design-review-3.md:241:crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/18-api-skeleton/final-review.md:1829:features/18-api-skeleton/design-review-3.md:371:    fn test_roundtrip_yaml() {
features/18-api-skeleton/final-review.md:1830:features/18-api-skeleton/design-review-3.md:432:    fn test_assembly_roundtrip() {
features/18-api-skeleton/final-review.md:1832:features/18-api-skeleton/design-review-3.md:1210:crates/mycad-kernel/tests/cuboid_roundtrip.rs:26:/// Integration test: deterministic pipeline produces identical results.
features/18-api-skeleton/final-review.md:1833:features/18-api-skeleton/design-review-3.md:1211:crates/mycad-kernel/tests/cuboid_roundtrip.rs:28:fn cuboid_pipeline_is_deterministic() {
features/18-api-skeleton/final-review.md:1834:features/18-api-skeleton/design-review-3.md:1215:crates/mycad-kernel/src/primitives/cylinder.rs:180:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/final-review.md:1835:features/18-api-skeleton/design-review-3.md:1244:    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
features/18-api-skeleton/final-review.md:1836:features/18-api-skeleton/design-review-3.md:1272:    /// T02: Topology counts.
features/18-api-skeleton/final-review.md:1837:features/18-api-skeleton/design-review-3.md:1291:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/final-review.md:1838:features/18-api-skeleton/design-review-3.md:1310:    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
features/18-api-skeleton/final-review.md:1839:features/18-api-skeleton/design-review-3.md:1363:    /// T07: Determinism — same input → same output.
features/18-api-skeleton/final-review.md:1840:features/18-api-skeleton/design-review-3.md:1380:    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
features/18-api-skeleton/final-review.md:1841:features/18-api-skeleton/design-review-3.md:1393:    /// T08 extended: exact positions and indices snapshot.
features/18-api-skeleton/final-review.md:1842:features/18-api-skeleton/design-review-3.md:1410:    /// T09: Outward normals — all facet normals point away from centroid.
features/18-api-skeleton/final-review.md:1844:features/18-api-skeleton/design-review-3.md:1816:/bin/bash -lc "sed -n '1,140p' crates/mycad-kernel/tests/cuboid_roundtrip.rs" in /home/bacon/mycad
features/18-api-skeleton/final-review.md:1845:features/18-api-skeleton/design-review-3.md:1882:    section: "テスト計画 > T09"
features/18-api-skeleton/final-review.md:1846:features/18-api-skeleton/design-review-3.md:1883:    finding: "設計本文では assembly/reference を build 前に検出して専用の 422 にするとしている一方、T09 の期待値は `422(EmptyFeatureList)` のままで矛盾している。これでは assembly と空 part を区別しない旧挙動でもテストが通る。"
features/18-api-skeleton/final-review.md:1847:features/18-api-skeleton/design-review-3.md:1884:    suggestion: "T09 は assembly/reference 専用メッセージを明示的に assert し、`features.is_empty()` の空 part は別テストに分離すること。"
features/18-api-skeleton/final-review.md:1848:features/18-api-skeleton/design-review-3.md:1916:    section: "テスト計画 > T09"
features/18-api-skeleton/final-review.md:1849:features/18-api-skeleton/design-review-3.md:1917:    finding: "設計本文では assembly/reference を build 前に検出して専用の 422 にするとしている一方、T09 の期待値は `422(EmptyFeatureList)` のままで矛盾している。これでは assembly と空 part を区別しない旧挙動でもテストが通る。"
features/18-api-skeleton/final-review.md:1850:features/18-api-skeleton/design-review-3.md:1918:    suggestion: "T09 は assembly/reference 専用メッセージを明示的に assert し、`features.is_empty()` の空 part は別テストに分離すること。"
features/18-api-skeleton/final-review.md:1851:features/12-cylinder/design-review-3.md:45:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/final-review.md:1852:features/12-cylinder/design-review-3.md:149:  2. `BoundaryFan`（平面） → 境界点列を **頂点0 から fan**（凸前提。cuboid 4 角=従来 2 三角形で不変、円盤キャップ=N角形 fan）。巻き方向は §1 で構築済み（底キャップ rev / 上キャップ fwd）なので fan の幾何法線が外向きになる。**スワップなし → cuboid のインデックス列もバイト不変**（T08 で固定）。
features/18-api-skeleton/final-review.md:1853:features/12-cylinder/design-review-3.md:180:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/final-review.md:1854:features/12-cylinder/design-review-3.md:182:- R05(medium) 反映: golden YAML ラウンドトリップテスト T13 追加。
features/18-api-skeleton/final-review.md:1856:features/12-cylinder/design-review-3.md:186:- R02(high) **検証して反映**: cuboid.rs / stl.rs を読み確認 → 「実行時の向き整合スワップは cuboid 出力不変」という前回主張は **誤り**（現行 cuboid のインデックス巻きが右手系外向き規約と一致する保証はなく、スワップ導入で index 列が変わり T08 が回帰しうる）。→ **テッセレータからスワップを撤去**し、外向きは §1 の**構築時巻き方向**（底キャップ=e_bot rev / 上キャップ=e_top fwd / 側面グリッド=∂u×∂v 外向き）で保証。平面 fan 経路は現行コードとバイト同一 → cuboid 完全不変（T08 で固定）。
features/18-api-skeleton/final-review.md:1857:features/12-cylinder/design-review-3.md:188:- T03 を「実カウントから Euler を計算」へ修正（定数ハードコード撤廃）。
features/18-api-skeleton/final-review.md:1858:features/12-cylinder/design-review-3.md:194:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/18-api-skeleton/final-review.md:1859:features/12-cylinder/design-review-3.md:195:| T02 | トポロジー | 頂点2・エッジ3・面3・シェル1(closed)・HE6・loop3 | 各 `len()` 一致 |
features/18-api-skeleton/final-review.md:1860:features/12-cylinder/design-review-3.md:196:| T03 | Euler | 実 solid の `vertices.len() − edges.len() + faces.len() == 2 * (shells.len() − genus)`（genus=0）を**実カウントから**計算して検証（定数ハードコードでなく構造体から） | `assert_eq!` |
features/18-api-skeleton/final-review.md:1861:features/12-cylinder/design-review-3.md:197:| T04 | マニフォルド+ループ閉包 | 各エッジがちょうど 2 HE に参照され向きが逆。**かつ**各 loop で連続 HE の終点 ≈ 次 HE の始点、最後の HE 終点 ≈ 先頭 HE 始点（`approx`）。seam 接続も検証 | 集計 + 幾何連続性検証 (R03) |
features/18-api-skeleton/final-review.md:1862:features/12-cylinder/design-review-3.md:198:| T05 | 幾何 | シーム頂点が (0,r,0)/(0,r,h)、各 Surface/Curve の係数 | `approx` で一致 |
features/18-api-skeleton/final-review.md:1863:features/12-cylinder/design-review-3.md:199:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
features/18-api-skeleton/final-review.md:1864:features/12-cylinder/design-review-3.md:200:| T07 | 決定性(tessellate) | 同一入力2回で positions/normals/indices 一致 | `assert_eq!` |
features/18-api-skeleton/final-review.md:1865:features/12-cylinder/design-review-3.md:201:| T08 | 回帰(平面バイト不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index。**positions も indices も従来値と完全一致**（スワップ撤去によりバイト不変、R02）。cuboid は逆向き Line HE を含むため R01 の向き対応も同時に担保 | 既存テスト維持 + positions/indices 一致 |
features/18-api-skeleton/final-review.md:1866:features/12-cylinder/design-review-3.md:202:| T09 | 外向き法線 | 全 facet の幾何法線が重心からの外向きと同符号（凸形状なので判定可） | 検証 |
features/18-api-skeleton/final-review.md:1867:features/12-cylinder/design-review-3.md:203:| T10 | ディスパッチ統合 | `examples/cylinder.mycad` を `build_solid_from_features` でビルドし T02 と同じ counts | `feature_dispatcher.rs` |
features/18-api-skeleton/final-review.md:1868:features/12-cylinder/design-review-3.md:204:| T11 | export 統合 | `mycad export examples/cylinder.mycad -o tmp.stl` が成功し、facet 数が T06 と整合、ASCII STL として妥当 | `export.rs`（`CARGO_BIN_EXE_mycad`） |
features/18-api-skeleton/final-review.md:1870:features/12-cylinder/design-review-3.md:206:| T13 | golden YAML ラウンドトリップ | `examples/cylinder.mycad` を `from_path`→`to_yaml`→`from_yaml`→`to_yaml` で equality。`create_cylinder` の serialize/schema 回帰を固定 | `assert_eq!` (R05) |
features/18-api-skeleton/final-review.md:1872:features/12-cylinder/design-review-3.md:211:1. `cargo test --workspace` で T01–T12 green。
features/18-api-skeleton/final-review.md:1873:features/12-cylinder/design-review-3.md:290:crates/mycad-kernel/tests/cuboid_roundtrip.rs:1:use mycad_kernel::brep::topology::IdGenerator;
features/18-api-skeleton/final-review.md:1874:features/12-cylinder/design-review-3.md:291:crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/final-review.md:1875:features/12-cylinder/design-review-3.md:292:crates/mycad-kernel/tests/cuboid_roundtrip.rs:3:use mycad_kernel::tessellation::tessellate_solid;
features/18-api-skeleton/final-review.md:1876:features/12-cylinder/design-review-3.md:293:crates/mycad-kernel/tests/cuboid_roundtrip.rs:8:    let mut id_gen = IdGenerator::new(0);
features/18-api-skeleton/final-review.md:1877:features/12-cylinder/design-review-3.md:294:crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/final-review.md:1878:features/12-cylinder/design-review-3.md:295:crates/mycad-kernel/tests/cuboid_roundtrip.rs:17:    let mesh = tessellate_solid(&solid);
features/18-api-skeleton/final-review.md:1879:features/12-cylinder/design-review-3.md:296:crates/mycad-kernel/tests/cuboid_roundtrip.rs:30:        let mut id_gen = IdGenerator::new(42);
features/18-api-skeleton/final-review.md:1880:features/12-cylinder/design-review-3.md:297:crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/18-api-skeleton/final-review.md:1881:features/12-cylinder/design-review-3.md:298:crates/mycad-kernel/tests/cuboid_roundtrip.rs:32:        tessellate_solid(&solid)
features/18-api-skeleton/final-review.md:1886:features/12-cylinder/design-review-3.md:1171:    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
features/18-api-skeleton/final-review.md:1887:features/12-cylinder/design-review-3.md:1179:    fn test_uv_of_roundtrip_plane() {
features/18-api-skeleton/final-review.md:1888:features/12-cylinder/design-review-3.md:1186:        assert_uv_roundtrip(&surface, 0.7, -1.3);
features/18-api-skeleton/final-review.md:1889:features/12-cylinder/design-review-3.md:1190:    fn test_uv_of_roundtrip_cylinder() {
features/18-api-skeleton/final-review.md:1890:features/12-cylinder/design-review-3.md:1196:        assert_uv_roundtrip(&surface, 0.9, 1.7);
features/18-api-skeleton/final-review.md:1891:features/12-cylinder/design-review-3.md:1200:    fn test_uv_of_roundtrip_sphere() {
features/18-api-skeleton/final-review.md:1892:features/12-cylinder/design-review-3.md:1205:        assert_uv_roundtrip(&surface, 1.1, 0.6);
features/18-api-skeleton/final-review.md:1893:features/12-cylinder/design-review-3.md:1209:    fn test_uv_of_roundtrip_cone() {
features/18-api-skeleton/final-review.md:1894:features/12-cylinder/design-review-3.md:1215:        assert_uv_roundtrip(&surface, 0.8, 2.0);
features/18-api-skeleton/final-review.md:1896:features/12-cylinder/design-review-3.md:1402:/bin/bash -lc "sed -n '1,260p' crates/mycad-build/tests/feature_dispatcher.rs && printf '\\n---\\n' && sed -n '1,260p' crates/mycad-cli/tests/export.rs && printf '\\n---\\n' && sed -n '1,220p' crates/mycad-kernel/tests/cuboid_roundtrip.rs" in /home/bacon/mycad
features/18-api-skeleton/final-review.md:1897:features/12-cylinder/design-review-3.md:1851:    fn test_roundtrip_yaml() {
features/18-api-skeleton/final-review.md:1898:features/12-cylinder/design-review-3.md:1912:    fn test_assembly_roundtrip() {
features/18-api-skeleton/final-review.md:1899:features/12-cylinder/design-review-3.md:2166:    section: "テスト計画 > T01"
features/18-api-skeleton/final-review.md:1900:features/12-cylinder/design-review-3.md:2167:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/18-api-skeleton/final-review.md:1901:features/12-cylinder/design-review-3.md:2196:    section: "テスト計画 > T01"
features/18-api-skeleton/final-review.md:1902:features/12-cylinder/design-review-3.md:2197:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/18-api-skeleton/final-review.md:1903:features/18-api-skeleton/design-review.md:45:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/final-review.md:1904:features/18-api-skeleton/design-review.md:146:| T01 | 正常系 | `GET /api/v0/mesh?file=examples/simple_box.mycad` を oneshot で叩く | 200、body を `TriangleMesh` に deser でき `positions`/`indices` が非空、`indices.len() % 3 == 0` |
features/18-api-skeleton/final-review.md:1905:features/18-api-skeleton/design-review.md:147:| T02 | 決定性 | 同一リクエストを 2 回実行 | 返る JSON バイト列が完全一致 |
features/18-api-skeleton/final-review.md:1906:features/18-api-skeleton/design-review.md:148:| T03 | 404 | 存在しない `file=examples/nope.mycad` | 404、body `{"error": ...}` |
features/18-api-skeleton/final-review.md:1907:features/18-api-skeleton/design-review.md:149:| T04 | 400 | 非 `.mycad` 拡張子 `file=Cargo.toml` | 400(InvalidExtension) |
features/18-api-skeleton/final-review.md:1908:features/18-api-skeleton/design-review.md:150:| T05 | 403 | `Host: evil.com` ヘッダ付与 | 403(Host 検証) |
features/18-api-skeleton/final-review.md:1909:features/18-api-skeleton/design-review.md:151:| T06 | 422 | 複数/未対応フィーチャ `file=examples/assembly.mycad` | 422 系エラー(build/tessellate 失敗を JSON で返す) |
features/18-api-skeleton/final-review.md:1911:features/18-api-skeleton/design-review.md:204:/home/bacon/mycad/crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:1912:features/18-api-skeleton/design-review.md:276:crates/mycad-kernel/src/primitives/cylinder.rs:180:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/final-review.md:1913:features/18-api-skeleton/design-review.md:681:    fn test_roundtrip_yaml() {
features/18-api-skeleton/final-review.md:1914:features/18-api-skeleton/design-review.md:742:    fn test_assembly_roundtrip() {
features/18-api-skeleton/final-review.md:1917:features/18-api-skeleton/design-review.md:1115:    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
features/18-api-skeleton/final-review.md:1918:features/18-api-skeleton/design-review.md:1134:    /// T07: Determinism — same input → same output.
features/18-api-skeleton/final-review.md:1919:features/18-api-skeleton/design-review.md:1151:    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
features/18-api-skeleton/final-review.md:1920:features/18-api-skeleton/design-review.md:1164:    /// T08 extended: exact positions and indices snapshot.
features/18-api-skeleton/final-review.md:1921:features/18-api-skeleton/design-review.md:1181:    /// T09: Outward normals — all facet normals point away from centroid.
features/18-api-skeleton/final-review.md:1923:features/18-api-skeleton/design-review.md:1718:    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
features/18-api-skeleton/final-review.md:1924:features/18-api-skeleton/design-review.md:1746:    /// T02: Topology counts.
features/18-api-skeleton/final-review.md:1925:features/18-api-skeleton/design-review.md:1765:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/final-review.md:1926:features/18-api-skeleton/design-review.md:1784:    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
features/18-api-skeleton/final-review.md:1927:features/18-api-skeleton/design-review.md:1839:    /// T05: Geometry — seam vertices and surface coefficients.
features/18-api-skeleton/final-review.md:1936:features/18-api-skeleton/design-review.md:2898:features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/final-review.md:1937:features/18-api-skeleton/design-review.md:2909:features/12-cylinder/design-review-2.md:1936:./features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/final-review.md:1938:features/18-api-skeleton/design-review.md:2919:features/12-cylinder/design-review-3.md:149:  2. `BoundaryFan`（平面） → 境界点列を **頂点0 から fan**（凸前提。cuboid 4 角=従来 2 三角形で不変、円盤キャップ=N角形 fan）。巻き方向は §1 で構築済み（底キャップ rev / 上キャップ fwd）なので fan の幾何法線が外向きになる。**スワップなし → cuboid のインデックス列もバイト不変**（T08 で固定）。
features/18-api-skeleton/final-review.md:1939:features/18-api-skeleton/design-review.md:2920:features/12-cylinder/design-review-3.md:180:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/final-review.md:1940:features/18-api-skeleton/design-review.md:2921:features/12-cylinder/design-review-3.md:186:- R02(high) **検証して反映**: cuboid.rs / stl.rs を読み確認 → 「実行時の向き整合スワップは cuboid 出力不変」という前回主張は **誤り**（現行 cuboid のインデックス巻きが右手系外向き規約と一致する保証はなく、スワップ導入で index 列が変わり T08 が回帰しうる）。→ **テッセレータからスワップを撤去**し、外向きは §1 の**構築時巻き方向**（底キャップ=e_bot rev / 上キャップ=e_top fwd / 側面グリッド=∂u×∂v 外向き）で保証。平面 fan 経路は現行コードとバイト同一 → cuboid 完全不変（T08 で固定）。
features/18-api-skeleton/final-review.md:1941:features/18-api-skeleton/design-review.md:3037:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/final-review.md:1942:features/18-api-skeleton/design-review.md:3071:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:1943:features/18-api-skeleton/design-review.md:3080:    section: "設計方針 > 1. トポロジー / テスト計画 > T04"
features/18-api-skeleton/final-review.md:1944:features/18-api-skeleton/design-review.md:3081:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:1945:features/18-api-skeleton/design-review.md:3091:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
features/18-api-skeleton/final-review.md:1946:features/18-api-skeleton/design-review.md:3101:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:1947:features/18-api-skeleton/design-review.md:3110:    section: "設計方針 > 1. トポロジー / テスト計画 > T04"
features/18-api-skeleton/final-review.md:1948:features/18-api-skeleton/design-review.md:3111:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:1949:features/18-api-skeleton/design-review.md:3121:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
features/18-api-skeleton/final-review.md:1951:features/18-api-skeleton/design-review.md:3181:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/final-review.md:1952:features/18-api-skeleton/design-review.md:3350:    suggestion: "絶対パスのみ受け付けるか、`app(state)` に明示的な base directory を持たせて相対パス解決を固定し、その規則を T02 と E2E で検証すること。"
features/18-api-skeleton/final-review.md:1953:features/18-api-skeleton/design-review.md:3359:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/final-review.md:1954:features/18-api-skeleton/design-review.md:3360:    suggestion: "v0 を『root_component に単一 part feature を持つ文書のみ対応』と明記して assembly 非対応を別エラーに分けるか、Document/Component 階層を解釈する document-level build API を設計すること。T06 は専用 fixture に差し替えること。"
features/18-api-skeleton/final-review.md:1955:features/18-api-skeleton/design-review.md:3369:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/final-review.md:1956:features/18-api-skeleton/design-review.md:3379:    suggestion: "絶対パスのみ受け付けるか、`app(state)` に明示的な base directory を持たせて相対パス解決を固定し、その規則を T02 と E2E で検証すること。"
features/18-api-skeleton/final-review.md:1957:features/18-api-skeleton/design-review.md:3388:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/final-review.md:1958:features/18-api-skeleton/design-review.md:3389:    suggestion: "v0 を『root_component に単一 part feature を持つ文書のみ対応』と明記して assembly 非対応を別エラーに分けるか、Document/Component 階層を解釈する document-level build API を設計すること。T06 は専用 fixture に差し替えること。"
features/18-api-skeleton/final-review.md:1959:features/18-api-skeleton/design-review.md:3398:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/final-review.md:1960:features/18-api-skeleton/design-review-2.md:45:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/final-review.md:1961:features/18-api-skeleton/design-review-2.md:100:| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
features/18-api-skeleton/final-review.md:1962:features/18-api-skeleton/design-review-2.md:164:| T01 | 正常系(box) | `GET /api/v0/mesh?file=<abs>/examples/simple_box.mycad` を oneshot | 200、`TriangleMesh` deser 可、`positions`/`indices` 非空、`indices.len() % 3 == 0` |
features/18-api-skeleton/final-review.md:1963:features/18-api-skeleton/design-review-2.md:165:| T02 | 正常系(cylinder)(R05) | 同上 `examples/cylinder.mycad`(周期面経路) | 200、mesh 非空 |
features/18-api-skeleton/final-review.md:1964:features/18-api-skeleton/design-review-2.md:166:| T03 | 決定性 | T01/T02 を 2 回実行 | 返る JSON バイト列が完全一致 |
features/18-api-skeleton/final-review.md:1965:features/18-api-skeleton/design-review-2.md:167:| T04 | 404 | 存在しない `file` | 404、body `{"error": ...}` |
features/18-api-skeleton/final-review.md:1966:features/18-api-skeleton/design-review-2.md:168:| T05 | 400 | 非 `.mycad` 拡張子(`Cargo.toml`) | 400(InvalidExtension) |
features/18-api-skeleton/final-review.md:1967:features/18-api-skeleton/design-review-2.md:169:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/final-review.md:1968:features/18-api-skeleton/design-review-2.md:170:| T07 | 422 未対応 feature(R03) | fixture `create_sphere` 単一 feature | 422(UnsupportedFeature) |
features/18-api-skeleton/final-review.md:1969:features/18-api-skeleton/design-review-2.md:171:| T08 | 422 退化寸法(R02) | fixture `create_box` width=0(または負値) | 422(InvalidParameter) |
features/18-api-skeleton/final-review.md:1970:features/18-api-skeleton/design-review-2.md:172:| T09 | 422 assembly 非対応(R03) | fixture: root features 空 + children あり | 422(EmptyFeatureList、クラッシュしない) |
features/18-api-skeleton/final-review.md:1971:features/18-api-skeleton/design-review-2.md:175:> **R05 反論**: Euler-Poincaré 検証は kernel(`cylinder.rs:180-195`)で、golden YAML ラウンドトリップは format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保済み。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。API 層は HTTP 契約(status・JSON 形・決定性・cylinder 経路疎通)に集中する。
features/18-api-skeleton/final-review.md:1974:features/18-api-skeleton/design-review-2.md:324:./crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:1975:features/18-api-skeleton/design-review-2.md:476:/bin/bash -lc 'rg -n "run_export|make_cuboid|tessellate_solid_with|test_assembly_roundtrip|workspace.dependencies|thiserror|TriangleMesh|build_solid_from_features|Document::from_path|InvalidExtension|make_cylinder|Euler|HalfEdge|Shell|Loop|IdGenerator" -S .' in /home/bacon/mycad
features/18-api-skeleton/final-review.md:1976:features/18-api-skeleton/design-review-2.md:546:./features/12-cylinder/design-review.md:176:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/18-api-skeleton/final-review.md:1977:features/18-api-skeleton/design-review-2.md:547:./features/12-cylinder/design-review.md:178:| T03 | Euler | V−E+F == 2 | `assert_eq!(2-3+3, 2)` 相当の検証 |
features/18-api-skeleton/final-review.md:1978:features/18-api-skeleton/design-review-2.md:548:./features/12-cylinder/design-review.md:183:| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index | 既存テスト維持 |
features/18-api-skeleton/final-review.md:1979:features/18-api-skeleton/design-review-2.md:549:./features/12-cylinder/design-review.md:185:| T10 | ディスパッチ統合 | `examples/cylinder.mycad` を `build_solid_from_features` でビルドし T02 と同じ counts | `feature_dispatcher.rs` |
features/18-api-skeleton/final-review.md:1981:features/18-api-skeleton/design-review-2.md:597:./features/12-cylinder/design-review.md:463:features/12-cylinder/design-review.md:176:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/18-api-skeleton/final-review.md:1982:features/18-api-skeleton/design-review-2.md:598:./features/12-cylinder/design-review.md:465:features/12-cylinder/design-review.md:183:| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index | 既存テスト維持 |
features/18-api-skeleton/final-review.md:1984:features/18-api-skeleton/design-review-2.md:674:./features/12-cylinder/design-review.md:580:crates/mycad-kernel/tests/cuboid_roundtrip.rs:1:use mycad_kernel::brep::topology::IdGenerator;
features/18-api-skeleton/final-review.md:1985:features/18-api-skeleton/design-review-2.md:675:./features/12-cylinder/design-review.md:581:crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/final-review.md:1986:features/18-api-skeleton/design-review-2.md:676:./features/12-cylinder/design-review.md:583:crates/mycad-kernel/tests/cuboid_roundtrip.rs:8:    let mut id_gen = IdGenerator::new(0);
features/18-api-skeleton/final-review.md:1987:features/18-api-skeleton/design-review-2.md:677:./features/12-cylinder/design-review.md:584:crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/final-review.md:1988:features/18-api-skeleton/design-review-2.md:678:./features/12-cylinder/design-review.md:586:crates/mycad-kernel/tests/cuboid_roundtrip.rs:30:        let mut id_gen = IdGenerator::new(42);
features/18-api-skeleton/final-review.md:1989:features/18-api-skeleton/design-review-2.md:679:./features/12-cylinder/design-review.md:587:crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/18-api-skeleton/final-review.md:1990:features/18-api-skeleton/design-review-2.md:756:./features/12-cylinder/design-review.md:2230:    fn test_assembly_roundtrip() {
features/18-api-skeleton/final-review.md:1991:features/18-api-skeleton/design-review-2.md:777:./features/12-cylinder/design-review.md:2520:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:1992:features/18-api-skeleton/design-review-2.md:780:./features/12-cylinder/design-review.md:2530:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:1993:features/18-api-skeleton/design-review-2.md:783:./features/12-cylinder/design-review.md:2550:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:1994:features/18-api-skeleton/design-review-2.md:786:./features/12-cylinder/design-review.md:2560:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:1995:features/18-api-skeleton/design-review-2.md:818:./features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/final-review.md:1996:features/18-api-skeleton/design-review-2.md:819:./features/12-cylinder/design-review-2.md:187:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/18-api-skeleton/final-review.md:1997:features/18-api-skeleton/design-review-2.md:820:./features/12-cylinder/design-review-2.md:189:| T03 | Euler | V−E+F == 2 | `assert_eq!(2-3+3, 2)` 相当の検証 |
features/18-api-skeleton/final-review.md:1998:features/18-api-skeleton/design-review-2.md:821:./features/12-cylinder/design-review-2.md:194:| T08 | 回帰(平面不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index。cuboid は逆向き Line HE を含むため R01 の向き対応も同時に担保 | 既存テスト維持 + positions が従来値と一致 |
features/18-api-skeleton/final-review.md:1999:features/18-api-skeleton/design-review-2.md:822:./features/12-cylinder/design-review-2.md:196:| T10 | ディスパッチ統合 | `examples/cylinder.mycad` を `build_solid_from_features` でビルドし T02 と同じ counts | `feature_dispatcher.rs` |
features/18-api-skeleton/final-review.md:2002:features/18-api-skeleton/design-review-2.md:912:./features/12-cylinder/design-review-2.md:1732:    fn test_assembly_roundtrip() {
features/18-api-skeleton/final-review.md:2003:features/18-api-skeleton/design-review-2.md:919:./features/12-cylinder/design-review-2.md:1789:/bin/bash -lc 'rg -n "golden|roundtrip|tessellate_solid_with|TessellationOptions|export" crates/mycad-cli tests crates/mycad-kernel crates/mycad-build' in /home/bacon/mycad
features/18-api-skeleton/final-review.md:2004:features/18-api-skeleton/design-review-2.md:941:./features/12-cylinder/design-review-2.md:1936:./features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/final-review.md:2005:features/18-api-skeleton/design-review-2.md:974:./features/12-cylinder/design-review-2.md:2312:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/18-api-skeleton/final-review.md:2006:features/18-api-skeleton/design-review-2.md:978:./features/12-cylinder/design-review-2.md:2337:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/18-api-skeleton/final-review.md:2007:features/18-api-skeleton/design-review-2.md:986:./crates/mycad-format/src/document.rs:116:    fn test_assembly_roundtrip() {
features/18-api-skeleton/final-review.md:2008:features/18-api-skeleton/design-review-2.md:1025:./features/12-cylinder/final-review-2.md:948:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/final-review.md:2009:features/18-api-skeleton/design-review-2.md:1098:./features/12-cylinder/design-review-3.md:180:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/final-review.md:2010:features/18-api-skeleton/design-review-2.md:1101:./features/12-cylinder/design-review-3.md:188:- T03 を「実カウントから Euler を計算」へ修正（定数ハードコード撤廃）。
features/18-api-skeleton/final-review.md:2011:features/18-api-skeleton/design-review-2.md:1102:./features/12-cylinder/design-review-3.md:194:| T01 | 決定性(primitive) | `make_cylinder(5,20)` を `IdGenerator::new(0)` で2回生成し全 ID・座標・面が一致 | `assert_eq!` |
features/18-api-skeleton/final-review.md:2012:features/18-api-skeleton/design-review-2.md:1103:./features/12-cylinder/design-review-3.md:196:| T03 | Euler | 実 solid の `vertices.len() − edges.len() + faces.len() == 2 * (shells.len() − genus)`（genus=0）を**実カウントから**計算して検証（定数ハードコードでなく構造体から） | `assert_eq!` |
features/18-api-skeleton/final-review.md:2013:features/18-api-skeleton/design-review-2.md:1104:./features/12-cylinder/design-review-3.md:201:| T08 | 回帰(平面バイト不変) | `make_cuboid` の `tessellate_solid` が従来どおり 12三角形/24頂点/36 index。**positions も indices も従来値と完全一致**（スワップ撤去によりバイト不変、R02）。cuboid は逆向き Line HE を含むため R01 の向き対応も同時に担保 | 既存テスト維持 + positions/indices 一致 |
features/18-api-skeleton/final-review.md:2014:features/18-api-skeleton/design-review-2.md:1105:./features/12-cylinder/design-review-3.md:203:| T10 | ディスパッチ統合 | `examples/cylinder.mycad` を `build_solid_from_features` でビルドし T02 と同じ counts | `feature_dispatcher.rs` |
features/18-api-skeleton/final-review.md:2017:features/18-api-skeleton/design-review-2.md:1150:./features/12-cylinder/design-review-3.md:290:crates/mycad-kernel/tests/cuboid_roundtrip.rs:1:use mycad_kernel::brep::topology::IdGenerator;
features/18-api-skeleton/final-review.md:2018:features/18-api-skeleton/design-review-2.md:1151:./features/12-cylinder/design-review-3.md:291:crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/final-review.md:2019:features/18-api-skeleton/design-review-2.md:1152:./features/12-cylinder/design-review-3.md:293:crates/mycad-kernel/tests/cuboid_roundtrip.rs:8:    let mut id_gen = IdGenerator::new(0);
features/18-api-skeleton/final-review.md:2020:features/18-api-skeleton/design-review-2.md:1153:./features/12-cylinder/design-review-3.md:294:crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/final-review.md:2021:features/18-api-skeleton/design-review-2.md:1154:./features/12-cylinder/design-review-3.md:296:crates/mycad-kernel/tests/cuboid_roundtrip.rs:30:        let mut id_gen = IdGenerator::new(42);
features/18-api-skeleton/final-review.md:2022:features/18-api-skeleton/design-review-2.md:1155:./features/12-cylinder/design-review-3.md:297:crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/18-api-skeleton/final-review.md:2023:features/18-api-skeleton/design-review-2.md:1255:./features/12-cylinder/design-review-3.md:1912:    fn test_assembly_roundtrip() {
features/18-api-skeleton/final-review.md:2024:features/18-api-skeleton/design-review-2.md:1316:./features/12-cylinder/final-review.md:742:+    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/final-review.md:2025:features/18-api-skeleton/design-review-2.md:1414:./features/12-cylinder/final-review.md:2050:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/final-review.md:2026:features/18-api-skeleton/design-review-2.md:1684:./features/18-api-skeleton/design-review.md:146:| T01 | 正常系 | `GET /api/v0/mesh?file=examples/simple_box.mycad` を oneshot で叩く | 200、body を `TriangleMesh` に deser でき `positions`/`indices` が非空、`indices.len() % 3 == 0` |
features/18-api-skeleton/final-review.md:2027:features/18-api-skeleton/design-review-2.md:1685:./features/18-api-skeleton/design-review.md:149:| T04 | 400 | 非 `.mycad` 拡張子 `file=Cargo.toml` | 400(InvalidExtension) |
features/18-api-skeleton/final-review.md:2028:features/18-api-skeleton/design-review-2.md:1718:./features/18-api-skeleton/design-review.md:276:crates/mycad-kernel/src/primitives/cylinder.rs:180:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/final-review.md:2029:features/18-api-skeleton/design-review-2.md:1755:./features/18-api-skeleton/design-review.md:742:    fn test_assembly_roundtrip() {
features/18-api-skeleton/final-review.md:2030:features/18-api-skeleton/design-review-2.md:1816:./features/18-api-skeleton/design-review.md:1765:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/final-review.md:2034:features/18-api-skeleton/design-review-2.md:1969:./features/18-api-skeleton/design-review.md:2898:features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/final-review.md:2035:features/18-api-skeleton/design-review-2.md:1970:./features/18-api-skeleton/design-review.md:2909:features/12-cylinder/design-review-2.md:1936:./features/12-cylinder/design-review-2.md:179:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/final-review.md:2036:features/18-api-skeleton/design-review-2.md:1973:./features/18-api-skeleton/design-review.md:2920:features/12-cylinder/design-review-3.md:180:- R03(high) **部分採用 / 反論**: 「`HalfEdge` に twin/next/prev を追加」は **却下**。本 kernel は index-based でループの順序配列から隣接を導く設計（ADR-001、既存 `make_cuboid`・`tessellate_solid` も隣接フィールド非依存）であり、隣接ポインタ追加は #12 のスコープを超える全 brep 改修・retessellation を招く。R03 の本質（参照数集計だけでは loop 閉包・seam 接続を検証できない）は妥当なので、**T04 をループ閉包の幾何検証へ強化**して代替担保する（隣接構造の導入は将来 Boolean/フィレットが必要とした時点で別途検討）。
features/18-api-skeleton/final-review.md:2037:features/18-api-skeleton/design-review-2.md:1987:./features/18-api-skeleton/design-review.md:3071:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:2038:features/18-api-skeleton/design-review-2.md:1990:./features/18-api-skeleton/design-review.md:3081:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:2039:features/18-api-skeleton/design-review-2.md:1993:./features/18-api-skeleton/design-review.md:3101:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:2040:features/18-api-skeleton/design-review-2.md:1996:./features/18-api-skeleton/design-review.md:3111:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:2041:features/18-api-skeleton/design-review-2.md:2025:./features/18-api-skeleton/design-review.md:3359:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/final-review.md:2042:features/18-api-skeleton/design-review-2.md:2026:./features/18-api-skeleton/design-review.md:3369:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/final-review.md:2043:features/18-api-skeleton/design-review-2.md:2028:./features/18-api-skeleton/design-review.md:3388:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/final-review.md:2044:features/18-api-skeleton/design-review-2.md:2029:./features/18-api-skeleton/design-review.md:3398:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/final-review.md:2045:features/18-api-skeleton/design-review-2.md:2050:./features/18-api-skeleton/design-review-2.md:100:| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
features/18-api-skeleton/final-review.md:2046:features/18-api-skeleton/design-review-2.md:2061:./features/18-api-skeleton/design-review-2.md:164:| T01 | 正常系(box) | `GET /api/v0/mesh?file=<abs>/examples/simple_box.mycad` を oneshot | 200、`TriangleMesh` deser 可、`positions`/`indices` 非空、`indices.len() % 3 == 0` |
features/18-api-skeleton/final-review.md:2047:features/18-api-skeleton/design-review-2.md:2062:./features/18-api-skeleton/design-review-2.md:168:| T05 | 400 | 非 `.mycad` 拡張子(`Cargo.toml`) | 400(InvalidExtension) |
features/18-api-skeleton/final-review.md:2048:features/18-api-skeleton/design-review-2.md:2064:./features/18-api-skeleton/design-review-2.md:175:> **R05 反論**: Euler-Poincaré 検証は kernel(`cylinder.rs:180-195`)で、golden YAML ラウンドトリップは format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保済み。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。API 層は HTTP 契約(status・JSON 形・決定性・cylinder 経路疎通)に集中する。
features/18-api-skeleton/final-review.md:2049:features/18-api-skeleton/design-review-2.md:2067:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:1:use mycad_kernel::brep::topology::IdGenerator;
features/18-api-skeleton/final-review.md:2050:features/18-api-skeleton/design-review-2.md:2068:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/final-review.md:2051:features/18-api-skeleton/design-review-2.md:2069:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:8:    let mut id_gen = IdGenerator::new(0);
features/18-api-skeleton/final-review.md:2052:features/18-api-skeleton/design-review-2.md:2070:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/final-review.md:2053:features/18-api-skeleton/design-review-2.md:2071:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:30:        let mut id_gen = IdGenerator::new(42);
features/18-api-skeleton/final-review.md:2054:features/18-api-skeleton/design-review-2.md:2072:./crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/18-api-skeleton/final-review.md:2057:features/18-api-skeleton/design-review-2.md:2075:./features/18-api-skeleton/plan.md:13:- 新規クレート `crates/mycad-api/`(lib + bin 構成)+ `make_cuboid` の入力検証(kernel)
features/18-api-skeleton/final-review.md:2058:features/18-api-skeleton/design-review-2.md:2076:./features/18-api-skeleton/plan.md:24:| `crates/mycad-kernel/src/primitives/cuboid.rs` | **(R02)** `make_cuboid` を `-> Result<Solid, KernelError>` 化。`dx/dy/dz` が非有限 or `<= 0` なら `InvalidParameter { kind }`(`make_cylinder` cylinder.rs:16-22 と対称)。インライン test の呼び出しを `.unwrap()` 化 |
features/18-api-skeleton/final-review.md:2059:features/18-api-skeleton/design-review-2.md:2077:./features/18-api-skeleton/plan.md:25:| `crates/mycad-build/src/lib.rs` | CreateBox 分岐を `Ok(make_cuboid(...))` → `make_cuboid(...)`(`?` 伝播)に変更 |
features/18-api-skeleton/final-review.md:2060:features/18-api-skeleton/design-review-2.md:2078:./features/18-api-skeleton/plan.md:26:| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
features/18-api-skeleton/final-review.md:2071:features/18-api-skeleton/design-review-2.md:2089:./features/18-api-skeleton/plan.md:90:| T01 | 正常系(box) | `GET /api/v0/mesh?file=<abs>/examples/simple_box.mycad` を oneshot | 200、`TriangleMesh` deser 可、`positions`/`indices` 非空、`indices.len() % 3 == 0` |
features/18-api-skeleton/final-review.md:2072:features/18-api-skeleton/design-review-2.md:2090:./features/18-api-skeleton/plan.md:94:| T05 | 400 | 非 `.mycad` 拡張子(`Cargo.toml`) | 400(InvalidExtension) |
features/18-api-skeleton/final-review.md:2073:features/18-api-skeleton/design-review-2.md:2091:./features/18-api-skeleton/plan.md:99:| K01 | kernel 単体(R02) | `make_cuboid(0.0, 1.0, 1.0, ..)` 等 | `Err(InvalidParameter { kind: "..." })` |
features/18-api-skeleton/final-review.md:2074:features/18-api-skeleton/design-review-2.md:2092:./features/18-api-skeleton/plan.md:101:> **R05 反論**: Euler-Poincaré 検証は kernel(`cylinder.rs:180-195`)で、golden YAML ラウンドトリップは format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保済み。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。API 層は HTTP 契約(status・JSON 形・決定性・cylinder 経路疎通)に集中する。
features/18-api-skeleton/final-review.md:2076:features/18-api-skeleton/design-review-2.md:2137:./crates/mycad-kernel/src/primitives/cylinder.rs:180:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/final-review.md:2077:features/18-api-skeleton/design-review-2.md:2346:    fn test_roundtrip_yaml() {
features/18-api-skeleton/final-review.md:2078:features/18-api-skeleton/design-review-2.md:2407:    fn test_assembly_roundtrip() {
features/18-api-skeleton/final-review.md:2079:features/18-api-skeleton/design-review-2.md:2834:    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
features/18-api-skeleton/final-review.md:2080:features/18-api-skeleton/design-review-2.md:2862:    /// T02: Topology counts.
features/18-api-skeleton/final-review.md:2081:features/18-api-skeleton/design-review-2.md:2881:    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
features/18-api-skeleton/final-review.md:2082:features/18-api-skeleton/design-review-2.md:2900:    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
features/18-api-skeleton/final-review.md:2083:features/18-api-skeleton/design-review-2.md:2955:    /// T05: Geometry — seam vertices and surface coefficients.
features/18-api-skeleton/final-review.md:2087:features/18-api-skeleton/design-review-2.md:3913:    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
features/18-api-skeleton/final-review.md:2088:features/18-api-skeleton/design-review-2.md:3932:    /// T07: Determinism — same input → same output.
features/18-api-skeleton/final-review.md:2089:features/18-api-skeleton/design-review-2.md:3949:    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
features/18-api-skeleton/final-review.md:2090:features/18-api-skeleton/design-review-2.md:3962:    /// T08 extended: exact positions and indices snapshot.
features/18-api-skeleton/final-review.md:2091:features/18-api-skeleton/design-review-2.md:3979:    /// T09: Outward normals — all facet normals point away from centroid.
features/18-api-skeleton/final-review.md:2093:features/18-api-skeleton/design-review-2.md:4515:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/final-review.md:2094:features/18-api-skeleton/design-review-2.md:4570:| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
features/18-api-skeleton/final-review.md:2095:features/18-api-skeleton/design-review-2.md:4720:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` の再利用方針と T09 の `EmptyFeatureList` 期待値では、`children` / `ref` を持つ assembly/reference 文書と、本当に feature が空の不正 part 文書を区別できない。これでは文書自身が要求している『assembly は明確なエラー + 422』を満たせない。"
features/18-api-skeleton/final-review.md:2096:features/18-api-skeleton/design-review-2.md:4730:    finding: "golden YAML ラウンドトリップが既存 `examples/*` だけに依存しており、新規 `crates/mycad-api/tests/fixtures/*.mycad` の unsupported/退化 fixture については format crate 側の roundtrip 保証が計画されていない。API が依存する入力形状の YAML 安定性が未検証のまま残る。"
features/18-api-skeleton/final-review.md:2097:features/18-api-skeleton/design-review-2.md:4731:    suggestion: "`mycad-format` 側に新規 fixture を読み込む roundtrip/golden テストを追加するか、API fixture を共有して `to_yaml`/`from_yaml` の安定性を format crate の所有テストで担保すること。"
features/18-api-skeleton/final-review.md:2098:features/18-api-skeleton/design-review-2.md:4745:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` の再利用方針と T09 の `EmptyFeatureList` 期待値では、`children` / `ref` を持つ assembly/reference 文書と、本当に feature が空の不正 part 文書を区別できない。これでは文書自身が要求している『assembly は明確なエラー + 422』を満たせない。"
features/18-api-skeleton/final-review.md:2099:features/18-api-skeleton/design-review-2.md:4755:    finding: "golden YAML ラウンドトリップが既存 `examples/*` だけに依存しており、新規 `crates/mycad-api/tests/fixtures/*.mycad` の unsupported/退化 fixture については format crate 側の roundtrip 保証が計画されていない。API が依存する入力形状の YAML 安定性が未検証のまま残る。"
features/18-api-skeleton/final-review.md:2100:features/18-api-skeleton/design-review-2.md:4756:    suggestion: "`mycad-format` 側に新規 fixture を読み込む roundtrip/golden テストを追加するか、API fixture を共有して `to_yaml`/`from_yaml` の安定性を format crate の所有テストで担保すること。"
features/18-api-skeleton/final-review.md:2102:2026-05-25T06:37:27.352616Z ERROR codex_core::tools::router: error=exec_command failed for `/bin/bash -lc 'RUSTC_WRAPPER= CARGO_TARGET_DIR=/tmp/mycad-target cargo test -p mycad-kernel --no-run'`: SandboxDenied { message: "error: Read-only file system (os error 30) at path \"/tmp/mycad-targetWKXIcx\"\n", output: ExecToolCallOutput { exit_code: 101, stdout: StreamOutput { text: "", truncated_after_lines: None }, stderr: StreamOutput { text: "error: Read-only file system (os error 30) at path \"/tmp/mycad-targetWKXIcx\"\n", truncated_after_lines: None }, aggregated_output: StreamOutput { text: "error: Read-only file system (os error 30) at path \"/tmp/mycad-targetWKXIcx\"\n", truncated_after_lines: None }, duration: 0ns, timed_out: false } }
features/18-api-skeleton/final-review.md:2103:2026-05-25T06:37:27.390183Z ERROR codex_core::tools::router: error=exec_command failed for `/bin/bash -lc 'RUSTC_WRAPPER= CARGO_TARGET_DIR=/tmp/mycad-target cargo test -p mycad-api --no-run'`: SandboxDenied { message: "error: Read-only file system (os error 30) at path \"/tmp/mycad-targetWmMi9p\"\n", output: ExecToolCallOutput { exit_code: 101, stdout: StreamOutput { text: "", truncated_after_lines: None }, stderr: StreamOutput { text: "error: Read-only file system (os error 30) at path \"/tmp/mycad-targetWmMi9p\"\n", truncated_after_lines: None }, aggregated_output: StreamOutput { text: "error: Read-only file system (os error 30) at path \"/tmp/mycad-targetWmMi9p\"\n", truncated_after_lines: None }, duration: 0ns, timed_out: false } }
features/18-api-skeleton/final-review.md:2121:> Codex 設計レビュー round 1-3 反映済み(3回上限)。round3 残存指摘は全て受諾し最終修正済み: R01=絶対パスのみ受理(決定的)、R02=v0 専用テッセレーション定数で Default 依存排除、R03=InvalidReference→422、R04=T09/T10 で assembly と空 part を分離、R05=K01 に INFINITY 分岐追加。round3 の verdict は fail のままだが指摘内容は本改訂で解消済み(ユーザー判断で再レビューせず確定)。
features/18-api-skeleton/final-review.md:2125:- 新規クレート `crates/mycad-api/`(lib + bin 構成)+ `make_cuboid` の入力検証(kernel)
features/18-api-skeleton/final-review.md:2136:| `crates/mycad-kernel/src/primitives/cuboid.rs` | `make_cuboid` を `-> Result<Solid, KernelError>` 化。`dx/dy/dz` が非有限 or `<= 0.0` なら `InvalidParameter { kind }`。**(R03)** 閾値は `make_cylinder`(cylinder.rs:16-22)と同じ exact `<= 0.0` で対称化。near-zero 正値の公差判定は導入しない(下記 R03 注記)。インライン test を `.unwrap()` 化 |
features/18-api-skeleton/final-review.md:2137:| `crates/mycad-build/src/lib.rs` | CreateBox 分岐を `Ok(make_cuboid(...))` → `make_cuboid(...)`(`?` 伝播)に変更 |
features/18-api-skeleton/final-review.md:2138:| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
features/18-api-skeleton/final-review.md:2219:| T01 | 正常系(box) | `GET /api/v0/mesh?file=<abs>/examples/simple_box.mycad` を oneshot | 200、`TriangleMesh` deser 可、`positions`/`indices` 非空、`indices.len() % 3 == 0` |
features/18-api-skeleton/final-review.md:2220:| T02 | 正常系(cylinder)(R05) | 同上 `examples/cylinder.mycad`(周期面経路) | 200、mesh 非空 |
features/18-api-skeleton/final-review.md:2221:| T03 | 決定性 | T01/T02 を 2 回実行 | 返る JSON バイト列が完全一致 |
features/18-api-skeleton/final-review.md:2222:| T04 | 404 | 存在しない `file` | 404、body `{"error": ...}` |
features/18-api-skeleton/final-review.md:2223:| T05 | 400 | 非 `.mycad` 拡張子(`Cargo.toml`) | 400(InvalidExtension) |
features/18-api-skeleton/final-review.md:2224:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/final-review.md:2225:| T07 | 422 未対応 feature(R03) | fixture `create_sphere` 単一 feature | 422(UnsupportedFeature) |
features/18-api-skeleton/final-review.md:2226:| T08 | 422 退化寸法(R02) | fixture `create_box` width=0(または負値) | 422(InvalidParameter) |
features/18-api-skeleton/final-review.md:2227:| T09 | 422 assembly 非対応(R04) | fixture: root features 空 + children あり | 422、body に assembly/reference 専用メッセージ(空 part とは別経路) |
features/18-api-skeleton/final-review.md:2230:| K01 | kernel 単体 | `make_cuboid` の `0.0` / 負値 / `NaN` / `INFINITY` / `-INFINITY`(dx/dy/dz 各々)(R05) | 全分岐で `Err(InvalidParameter { kind: "..." })` |
features/18-api-skeleton/final-review.md:2231:| K02 | kernel 単体(R03 境界可視化) | `make_cuboid(1e-9, 1.0, 1.0, ..)`(near-zero 正値) | 現状は `Ok`(exact `<= 0.0` 方針)であることを assert し境界を明文化 |
features/18-api-skeleton/final-review.md:2233:> **R05 反論**(round 1): Euler-Poincaré は kernel(`cylinder.rs:180-195`)、golden YAML roundtrip は format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。
features/18-api-skeleton/final-review.md:2235:> **R03 判断**(round 2): 退化寸法の閾値は `make_cylinder` と同じ exact `<= 0.0` で対称化する。**グローバル公差(near-zero の扱い)は ROADMAP の Issue #17「単位系・グローバル公差の早期固定(ADR-004 範囲)」が所有する未決事項**であり、#18 で独自 EPS を導入すると #17 の設計判断を先取りしてしまう。よって #18 は exact-zero 方針に留め、near-zero 正値が現状 `Ok` となる境界を K02 で明文化する(将来 #17 で公差確定後に統一)。
features/18-api-skeleton/final-review.md:2237:> **R04 反論**(round 2): 新規 `crates/mycad-api/tests/fixtures/*.mycad` は T07-T09 で API 経由で必ずパースされる(パースできなければ期待する 422 が出ず test fail)ため、入力形状の妥当性は API テストで担保済み。`Document` の YAML roundtrip 安定性は `mycad-format` の既存 roundtrip テストが所有。クロスクレートの roundtrip 追加は「テストは所有クレートに置く」規約に反するため行わない。fixture は最小に保つ。
features/18-api-skeleton/final-review.md:2301:- 設計レビュー時に合意したテスト計画（ID T01〜）が全て実装されているか
features/18-api-skeleton/final-review.md:2303:- golden YAML ラウンドトリップが含まれているか
features/18-api-skeleton/final-review.md:2320:    suggestion: "make_cuboid の edge 生成ロジックを見直す"
features/18-api-skeleton/final-review.md:2342:crates/mycad-kernel/src/primitives/cuboid.rs
features/18-api-skeleton/final-review.md:2345:crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:2356: M crates/mycad-kernel/src/primitives/cuboid.rs
features/18-api-skeleton/final-review.md:2359: M crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:2375: crates/mycad-kernel/src/primitives/cuboid.rs  | 115 +++++++-
features/18-api-skeleton/final-review.md:2378: crates/mycad-kernel/tests/cuboid_roundtrip.rs |   4 +-
features/18-api-skeleton/final-review.md:2417:/bin/bash -lc 'git diff -- Cargo.toml Cargo.lock crates/mycad-build/src/lib.rs crates/mycad-kernel/benches/tessellation.rs crates/mycad-kernel/src/primitives/cuboid.rs crates/mycad-kernel/src/tessellation/mod.rs crates/mycad-kernel/src/tessellation/stl.rs crates/mycad-kernel/tests/cuboid_roundtrip.rs' in /home/bacon/mycad
features/18-api-skeleton/final-review.md:2567:- golden YAML ラウンドトリップテストがあるか
features/18-api-skeleton/final-review.md:2605:> Codex 設計レビュー round 1 + round 2 反映済み。round1: R01/R03/R04 受諾、R02 kernel 検証追加(ユーザー決定)、R05 部分受諾。round2: R01(構造化エラー)受諾、R02(assembly 判別)受諾、R03(公差)は #17 委譲で反論、R04(fixture roundtrip)反論。
features/18-api-skeleton/final-review.md:2609:- 新規クレート `crates/mycad-api/`(lib + bin 構成)+ `make_cuboid` の入力検証(kernel)
features/18-api-skeleton/final-review.md:2620:| `crates/mycad-kernel/src/primitives/cuboid.rs` | `make_cuboid` を `-> Result<Solid, KernelError>` 化。`dx/dy/dz` が非有限 or `<= 0.0` なら `InvalidParameter { kind }`。**(R03)** 閾値は `make_cylinder`(cylinder.rs:16-22)と同じ exact `<= 0.0` で対称化。near-zero 正値の公差判定は導入しない(下記 R03 注記)。インライン test を `.unwrap()` 化 |
features/18-api-skeleton/final-review.md:2621:| `crates/mycad-build/src/lib.rs` | CreateBox 分岐を `Ok(make_cuboid(...))` → `make_cuboid(...)`(`?` 伝播)に変更 |
features/18-api-skeleton/final-review.md:2622:| `make_cuboid` 全呼び出し箇所 | `.unwrap()`/`.expect()` で追従。`tessellation/mod.rs`(L358,373-374,500,514-515)、`tessellation/stl.rs`(L58,75)、`tests/cuboid_roundtrip.rs`(L9,31)、`benches/tessellation.rs`(L15)。**全て更新し `cargo xtask ci` green を保つ** |
features/18-api-skeleton/final-review.md:2703:| T01 | 正常系(box) | `GET /api/v0/mesh?file=<abs>/examples/simple_box.mycad` を oneshot | 200、`TriangleMesh` deser 可、`positions`/`indices` 非空、`indices.len() % 3 == 0` |
features/18-api-skeleton/final-review.md:2704:| T02 | 正常系(cylinder)(R05) | 同上 `examples/cylinder.mycad`(周期面経路) | 200、mesh 非空 |
features/18-api-skeleton/final-review.md:2705:| T03 | 決定性 | T01/T02 を 2 回実行 | 返る JSON バイト列が完全一致 |
features/18-api-skeleton/final-review.md:2706:| T04 | 404 | 存在しない `file` | 404、body `{"error": ...}` |
features/18-api-skeleton/final-review.md:2707:| T05 | 400 | 非 `.mycad` 拡張子(`Cargo.toml`) | 400(InvalidExtension) |
features/18-api-skeleton/final-review.md:2708:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/final-review.md:2709:| T07 | 422 未対応 feature(R03) | fixture `create_sphere` 単一 feature | 422(UnsupportedFeature) |
features/18-api-skeleton/final-review.md:2710:| T08 | 422 退化寸法(R02) | fixture `create_box` width=0(または負値) | 422(InvalidParameter) |
features/18-api-skeleton/final-review.md:2711:| T09 | 422 assembly 非対応(R03) | fixture: root features 空 + children あり | 422(EmptyFeatureList、クラッシュしない) |
features/18-api-skeleton/final-review.md:2712:| K01 | kernel 単体 | `make_cuboid(0.0, 1.0, 1.0, ..)` / 負値 / NaN | `Err(InvalidParameter { kind: "..." })` |
features/18-api-skeleton/final-review.md:2713:| K02 | kernel 単体(R03 境界可視化) | `make_cuboid(1e-9, 1.0, 1.0, ..)`(near-zero 正値) | 現状は `Ok`(exact `<= 0.0` 方針)であることを assert し境界を明文化 |
features/18-api-skeleton/final-review.md:2715:> **R05 反論**(round 1): Euler-Poincaré は kernel(`cylinder.rs:180-195`)、golden YAML roundtrip は format crate(`document.rs` の `test_assembly_roundtrip` 等)で既に担保。API 層での重複は規約「テストは所有クレートに置く」に反するため追加しない。
features/18-api-skeleton/final-review.md:2717:> **R03 判断**(round 2): 退化寸法の閾値は `make_cylinder` と同じ exact `<= 0.0` で対称化する。**グローバル公差(near-zero の扱い)は ROADMAP の Issue #17「単位系・グローバル公差の早期固定(ADR-004 範囲)」が所有する未決事項**であり、#18 で独自 EPS を導入すると #17 の設計判断を先取りしてしまう。よって #18 は exact-zero 方針に留め、near-zero 正値が現状 `Ok` となる境界を K02 で明文化する(将来 #17 で公差確定後に統一)。
features/18-api-skeleton/final-review.md:2719:> **R04 反論**(round 2): 新規 `crates/mycad-api/tests/fixtures/*.mycad` は T07-T09 で API 経由で必ずパースされる(パースできなければ期待する 422 が出ず test fail)ため、入力形状の妥当性は API テストで担保済み。`Document` の YAML roundtrip 安定性は `mycad-format` の既存 roundtrip テストが所有。クロスクレートの roundtrip 追加は「テストは所有クレートに置く」規約に反するため行わない。fixture は最小に保つ。
features/18-api-skeleton/final-review.md:2739:/bin/bash -lc "rg -n \"enum FormatError|struct TriangleMesh|enum KernelError|enum TessellationError|fn from_path|build_solid_from_features|make_cuboid|make_cylinder|EmptyFeatureList|InvalidReference\" crates -g '*.rs'" in /home/bacon/mycad
features/18-api-skeleton/final-review.md:2748:crates/mycad-build/src/lib.rs:4:use mycad_kernel::primitives::{make_cuboid, make_cylinder};
features/18-api-skeleton/final-review.md:2751:crates/mycad-build/src/lib.rs:26:        } => Ok(make_cuboid(*width, *height, *depth, gen)),
features/18-api-skeleton/final-review.md:2757:crates/mycad-kernel/benches/tessellation.rs:2:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/final-review.md:2758:crates/mycad-kernel/benches/tessellation.rs:15:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/final-review.md:2759:crates/mycad-kernel/src/primitives/mod.rs:4:pub use cuboid::make_cuboid;
features/18-api-skeleton/final-review.md:2761:crates/mycad-kernel/tests/cuboid_roundtrip.rs:2:use mycad_kernel::primitives::make_cuboid;
features/18-api-skeleton/final-review.md:2762:crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
features/18-api-skeleton/final-review.md:2763:crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
features/18-api-skeleton/final-review.md:2766:crates/mycad-kernel/src/tessellation/mod.rs:315:    use crate::primitives::make_cuboid;
features/18-api-skeleton/final-review.md:2771:crates/mycad-kernel/src/tessellation/mod.rs:358:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
features/18-api-skeleton/final-review.md:2772:crates/mycad-kernel/src/tessellation/mod.rs:373:        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
features/18-api-skeleton/final-review.md:2773:crates/mycad-kernel/src/tessellation/mod.rs:374:        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
features/18-api-skeleton/final-review.md:2776:crates/mycad-kernel/src/tessellation/mod.rs:500:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
features/18-api-skeleton/final-review.md:2777:crates/mycad-kernel/src/tessellation/mod.rs:514:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
features/18-api-skeleton/final-review.md:2778:crates/mycad-kernel/src/tessellation/mod.rs:515:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
features/18-api-skeleton/final-review.md:2796:     8	    #[error("YAML parse error: {0}")]
features/18-api-skeleton/final-review.md:2810:     4	use mycad_kernel::primitives::{make_cuboid, make_cylinder};
features/18-api-skeleton/final-review.md:2832:    26	        } => make_cuboid(*width, *height, *depth, gen),
features/18-api-skeleton/final-review.md:2871:    25	    /// Serialize to YAML string.
features/18-api-skeleton/final-review.md:2876:    30	    /// Deserialize from YAML string.
features/18-api-skeleton/final-review.md:2901:    55	    fn test_roundtrip_yaml() {
features/18-api-skeleton/final-review.md:2932:    86	        assert_eq!(yaml1, yaml2, "YAML serialization must be deterministic");
features/18-api-skeleton/final-review.md:2962:   116	    fn test_assembly_roundtrip() {
features/18-api-skeleton/final-review.md:2983:   137	        // Roundtrip: serialize then deserialize yields the same YAML
features/18-api-skeleton/final-review.md:3014:/bin/bash -lc "nl -ba crates/mycad-kernel/src/primitives/cuboid.rs | sed -n '1,430p'" in /home/bacon/mycad
features/18-api-skeleton/final-review.md:3023:     8	/// Create a cuboid (box) B-rep solid centered at the origin.
features/18-api-skeleton/final-review.md:3031:    16	/// The cuboid spans from (-dx/2, -dy/2, -dz/2) to (dx/2, dy/2, dz/2).
features/18-api-skeleton/final-review.md:3032:    17	pub fn make_cuboid(
features/18-api-skeleton/final-review.md:3053:    38	    // 8 vertices of the cuboid
features/18-api-skeleton/final-review.md:3079:    64	    // 12 edges of the cuboid
features/18-api-skeleton/final-review.md:3114:    99	    // 6 faces of the cuboid
features/18-api-skeleton/final-review.md:3257:   242	    fn test_cuboid_topology() {
features/18-api-skeleton/final-review.md:3259:   244	        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review.md:3261:   246	        assert_eq!(solid.vertices.len(), 8, "cuboid should have 8 vertices");
features/18-api-skeleton/final-review.md:3262:   247	        assert_eq!(solid.edges.len(), 12, "cuboid should have 12 edges");
features/18-api-skeleton/final-review.md:3263:   248	        assert_eq!(solid.faces.len(), 6, "cuboid should have 6 faces");
features/18-api-skeleton/final-review.md:3264:   249	        assert_eq!(solid.shells.len(), 1, "cuboid should have 1 shell");
features/18-api-skeleton/final-review.md:3265:   250	        assert!(solid.shells[0].closed, "cuboid shell should be closed");
features/18-api-skeleton/final-review.md:3270:   255	            "cuboid should have 24 half-edges"
features/18-api-skeleton/final-review.md:3273:   258	        assert_eq!(solid.loops.len(), 6, "cuboid should have 6 loops");
features/18-api-skeleton/final-review.md:3277:   262	    fn test_cuboid_vertex_positions() {
features/18-api-skeleton/final-review.md:3279:   264	        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen).unwrap();
features/18-api-skeleton/final-review.md:3304:   289	    fn test_cuboid_deterministic() {
features/18-api-skeleton/final-review.md:3305:   290	        // Building the same cuboid twice with the same ID generator should
features/18-api-skeleton/final-review.md:3310:   295	        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1).unwrap();
features/18-api-skeleton/final-review.md:3311:   296	        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2).unwrap();
features/18-api-skeleton/final-review.md:3331:   316	    // K01: make_cuboid rejects zero, negative, NaN, Infinity dimensions
features/18-api-skeleton/final-review.md:3333:   318	    fn test_make_cuboid_invalid_dimensions() {
features/18-api-skeleton/final-review.md:3338:   323	            make_cuboid(0.0, 1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:3342:   327	            make_cuboid(1.0, 0.0, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:3346:   331	            make_cuboid(1.0, 1.0, 0.0, &mut gen),
features/18-api-skeleton/final-review.md:3352:   337	            make_cuboid(-1.0, 1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:3356:   341	            make_cuboid(1.0, -1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:3360:   345	            make_cuboid(1.0, 1.0, -1.0, &mut gen),
features/18-api-skeleton/final-review.md:3366:   351	            make_cuboid(f64::NAN, 1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:3370:   355	            make_cuboid(1.0, f64::NAN, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:3374:   359	            make_cuboid(1.0, 1.0, f64::NAN, &mut gen),
features/18-api-skeleton/final-review.md:3380:   365	            make_cuboid(f64::INFINITY, 1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:3384:   369	            make_cuboid(1.0, f64::INFINITY, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:3388:   373	            make_cuboid(1.0, 1.0, f64::INFINITY, &mut gen),
features/18-api-skeleton/final-review.md:3394:   379	            make_cuboid(f64::NEG_INFINITY, 1.0, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:3398:   383	            make_cuboid(1.0, f64::NEG_INFINITY, 1.0, &mut gen),
features/18-api-skeleton/final-review.md:3402:   387	            make_cuboid(1.0, 1.0, f64::NEG_INFINITY, &mut gen),
features/18-api-skeleton/final-review.md:3407:   392	    // K02: near-zero positive value is Ok (exact <= 0.0 policy)
features/18-api-skeleton/final-review.md:3409:   394	    fn test_make_cuboid_near_zero_positive_ok() {
features/18-api-skeleton/final-review.md:3411:   396	        let result = make_cuboid(1e-9, 1.0, 1.0, &mut gen);
features/18-api-skeleton/final-review.md:4029:features/18-api-skeleton/design-review-3.md:186:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/final-review.md:4040:features/18-api-skeleton/design-review-3.md:1883:    finding: "設計本文では assembly/reference を build 前に検出して専用の 422 にするとしている一方、T09 の期待値は `422(EmptyFeatureList)` のままで矛盾している。これでは assembly と空 part を区別しない旧挙動でもテストが通る。"
features/18-api-skeleton/final-review.md:4042:features/18-api-skeleton/design-review-3.md:1889:    finding: "`make_cuboid` の新仕様は `non-finite` を拒否すると書いているが、K01 の列挙は `0.0`/負値/`NaN` までで、`INFINITY`/`-INFINITY` の分岐が未検証である。"
features/18-api-skeleton/final-review.md:4050:features/18-api-skeleton/design-review-3.md:1917:    finding: "設計本文では assembly/reference を build 前に検出して専用の 422 にするとしている一方、T09 の期待値は `422(EmptyFeatureList)` のままで矛盾している。これでは assembly と空 part を区別しない旧挙動でもテストが通る。"
features/18-api-skeleton/final-review.md:4052:features/18-api-skeleton/design-review-3.md:1923:    finding: "`make_cuboid` の新仕様は `non-finite` を拒否すると書いているが、K01 の列挙は `0.0`/負値/`NaN` までで、`INFINITY`/`-INFINITY` の分岐が未検証である。"
features/18-api-skeleton/final-review.md:4068:features/18-api-skeleton/design-review-2.md:169:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/final-review.md:4069:features/18-api-skeleton/design-review-2.md:476:/bin/bash -lc 'rg -n "run_export|make_cuboid|tessellate_solid_with|test_assembly_roundtrip|workspace.dependencies|thiserror|TriangleMesh|build_solid_from_features|Document::from_path|InvalidExtension|make_cylinder|Euler|HalfEdge|Shell|Loop|IdGenerator" -S .' in /home/bacon/mycad
features/18-api-skeleton/final-review.md:4085:features/18-api-skeleton/design-review-2.md:552:./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/final-review.md:4095:features/18-api-skeleton/design-review-2.md:601:./features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/final-review.md:4109:features/18-api-skeleton/design-review-2.md:777:./features/12-cylinder/design-review.md:2520:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:4111:features/18-api-skeleton/design-review-2.md:780:./features/12-cylinder/design-review.md:2530:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:4112:features/18-api-skeleton/design-review-2.md:783:./features/12-cylinder/design-review.md:2550:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:4114:features/18-api-skeleton/design-review-2.md:786:./features/12-cylinder/design-review.md:2560:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:4123:features/18-api-skeleton/design-review-2.md:922:./features/12-cylinder/design-review-2.md:1863:./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/final-review.md:4124:features/18-api-skeleton/design-review-2.md:923:./features/12-cylinder/design-review-2.md:1875:./features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/final-review.md:4125:features/18-api-skeleton/design-review-2.md:973:./features/12-cylinder/design-review-2.md:2307:    finding: "「cuboid は既に外向きなのでスワップは発生しない／平面出力不変」という前提が既存 `make_cuboid` と整合していない。現状の cuboid ループ順では front face `0→1→3→2` などの幾何法線が outward normal と逆向きで、提案どおりの法線整合スワップを入れると既存 STL/indices は変わる。"
features/18-api-skeleton/final-review.md:4126:features/18-api-skeleton/design-review-2.md:974:./features/12-cylinder/design-review-2.md:2312:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/18-api-skeleton/final-review.md:4128:features/18-api-skeleton/design-review-2.md:977:./features/12-cylinder/design-review-2.md:2332:    finding: "「cuboid は既に外向きなのでスワップは発生しない／平面出力不変」という前提が既存 `make_cuboid` と整合していない。現状の cuboid ループ順では front face `0→1→3→2` などの幾何法線が outward normal と逆向きで、提案どおりの法線整合スワップを入れると既存 STL/indices は変わる。"
features/18-api-skeleton/final-review.md:4129:features/18-api-skeleton/design-review-2.md:978:./features/12-cylinder/design-review-2.md:2337:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/18-api-skeleton/final-review.md:4174:features/18-api-skeleton/design-review-2.md:1845:./features/18-api-skeleton/design-review.md:2442:./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/final-review.md:4175:features/18-api-skeleton/design-review-2.md:1846:./features/18-api-skeleton/design-review.md:2450:./features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/final-review.md:4176:features/18-api-skeleton/design-review-2.md:1895:./features/18-api-skeleton/design-review.md:2609:./features/12-cylinder/design-review-2.md:1863:./features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/final-review.md:4177:features/18-api-skeleton/design-review-2.md:1896:./features/18-api-skeleton/design-review.md:2617:./features/12-cylinder/design-review-2.md:1875:./features/12-cylinder/design-review.md:468:features/12-cylinder/design-review.md:205:/bin/bash -lc 'rg -n "ADR-004|make_cuboid|tessellate_solid|IdGenerator|HalfEdge|TriangleMesh|CreateCylinder|UnsupportedFeature|Feature::CreateCylinder|JsonSchema|thiserror|workspace.dependencies|pub use" -S' in /home/bacon/mycad
features/18-api-skeleton/final-review.md:4186:features/18-api-skeleton/design-review-2.md:1987:./features/18-api-skeleton/design-review.md:3071:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:4188:features/18-api-skeleton/design-review-2.md:1990:./features/18-api-skeleton/design-review.md:3081:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:4189:features/18-api-skeleton/design-review-2.md:1993:./features/18-api-skeleton/design-review.md:3101:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:4191:features/18-api-skeleton/design-review-2.md:1996:./features/18-api-skeleton/design-review.md:3111:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:4198:features/18-api-skeleton/design-review-2.md:2024:./features/18-api-skeleton/design-review.md:3354:    finding: "`build_solid_from_features` をそのまま再利用する方針では、`make_cuboid` 側に寸法バリデーションが無いため `width/height/depth <= 0` の `.mycad` を API が受理しうる。ゼロ長 edge・面積ゼロ face・縮退 loop をどう拒否するか、また数値境界を何で比較するかが計画に無い。"
features/18-api-skeleton/final-review.md:4199:features/18-api-skeleton/design-review-2.md:2025:./features/18-api-skeleton/design-review.md:3359:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/final-review.md:4200:features/18-api-skeleton/design-review-2.md:2026:./features/18-api-skeleton/design-review.md:3369:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/final-review.md:4201:features/18-api-skeleton/design-review-2.md:2027:./features/18-api-skeleton/design-review.md:3383:    finding: "`build_solid_from_features` をそのまま再利用する方針では、`make_cuboid` 側に寸法バリデーションが無いため `width/height/depth <= 0` の `.mycad` を API が受理しうる。ゼロ長 edge・面積ゼロ face・縮退 loop をどう拒否するか、また数値境界を何で比較するかが計画に無い。"
features/18-api-skeleton/final-review.md:4202:features/18-api-skeleton/design-review-2.md:2028:./features/18-api-skeleton/design-review.md:3388:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/final-review.md:4203:features/18-api-skeleton/design-review-2.md:2029:./features/18-api-skeleton/design-review.md:3398:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/final-review.md:4239:features/18-api-skeleton/design-review-2.md:4720:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` の再利用方針と T09 の `EmptyFeatureList` 期待値では、`children` / `ref` を持つ assembly/reference 文書と、本当に feature が空の不正 part 文書を区別できない。これでは文書自身が要求している『assembly は明確なエラー + 422』を満たせない。"
features/18-api-skeleton/final-review.md:4242:features/18-api-skeleton/design-review-2.md:4725:    finding: "`make_cuboid` の退化入力対策が `!is_finite() || <= 0` だけで、近接ゼロの正値をどう扱うか、既存の `AREA_EPS` や 1e-9 系判定とどう整合させるかが明記されていない。非常に小さい正寸法は B-rep 生成を通っても、後段テッセレーションで実質的に縮退しうる。"
features/18-api-skeleton/final-review.md:4244:features/18-api-skeleton/design-review-2.md:4730:    finding: "golden YAML ラウンドトリップが既存 `examples/*` だけに依存しており、新規 `crates/mycad-api/tests/fixtures/*.mycad` の unsupported/退化 fixture については format crate 側の roundtrip 保証が計画されていない。API が依存する入力形状の YAML 安定性が未検証のまま残る。"
features/18-api-skeleton/final-review.md:4249:features/18-api-skeleton/design-review-2.md:4745:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` の再利用方針と T09 の `EmptyFeatureList` 期待値では、`children` / `ref` を持つ assembly/reference 文書と、本当に feature が空の不正 part 文書を区別できない。これでは文書自身が要求している『assembly は明確なエラー + 422』を満たせない。"
features/18-api-skeleton/final-review.md:4252:features/18-api-skeleton/design-review-2.md:4750:    finding: "`make_cuboid` の退化入力対策が `!is_finite() || <= 0` だけで、近接ゼロの正値をどう扱うか、既存の `AREA_EPS` や 1e-9 系判定とどう整合させるかが明記されていない。非常に小さい正寸法は B-rep 生成を通っても、後段テッセレーションで実質的に縮退しうる。"
features/18-api-skeleton/final-review.md:4254:features/18-api-skeleton/design-review-2.md:4755:    finding: "golden YAML ラウンドトリップが既存 `examples/*` だけに依存しており、新規 `crates/mycad-api/tests/fixtures/*.mycad` の unsupported/退化 fixture については format crate 側の roundtrip 保証が計画されていない。API が依存する入力形状の YAML 安定性が未検証のまま残る。"
features/18-api-skeleton/final-review.md:4305:features/18-api-skeleton/final-review.md:1301:   121	// T06: 403 Host header (DNS rebinding protection)
features/18-api-skeleton/final-review.md:4307:features/18-api-skeleton/final-review.md:1314:   134	    // The plan says to add Host header validation middleware.
features/18-api-skeleton/final-review.md:4308:features/18-api-skeleton/final-review.md:1389:/bin/bash -lc "rg -n \"pub struct TriangleMesh|derive\\(|thiserror|InvalidParameter|tessellate_solid_with|pub enum KernelError|pub enum TessellationError|make_cuboid\\(\" crates/mycad-kernel crates/mycad-build crates/mycad-format -g '*.rs'" in /home/bacon/mycad
features/18-api-skeleton/final-review.md:4312:features/18-api-skeleton/final-review.md:1515:/bin/bash -lc "rg -n \"T0[1-9]|T10|T11|T12|T13|T14|Host header|golden|roundtrip|plan\" features crates/mycad-kernel crates/mycad-api -g '*.md' -g '*.rs'" in /home/bacon/mycad
features/18-api-skeleton/final-review.md:4313:features/18-api-skeleton/final-review.md:1572:features/12-cylinder/design-review.md:2520:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:4314:features/18-api-skeleton/final-review.md:1574:features/12-cylinder/design-review.md:2530:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:4315:features/18-api-skeleton/final-review.md:1575:features/12-cylinder/design-review.md:2540:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
features/18-api-skeleton/final-review.md:4316:features/18-api-skeleton/final-review.md:1576:features/12-cylinder/design-review.md:2550:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:4317:features/18-api-skeleton/final-review.md:1578:features/12-cylinder/design-review.md:2560:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:4318:features/18-api-skeleton/final-review.md:1579:features/12-cylinder/design-review.md:2570:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
features/18-api-skeleton/final-review.md:4319:features/18-api-skeleton/final-review.md:1585:crates/mycad-api/tests/mesh_api.rs:121:// T06: 403 Host header (DNS rebinding protection)
features/18-api-skeleton/final-review.md:4320:features/18-api-skeleton/final-review.md:1586:crates/mycad-api/tests/mesh_api.rs:134:    // The plan says to add Host header validation middleware.
features/18-api-skeleton/final-review.md:4321:features/18-api-skeleton/final-review.md:1646:features/12-cylinder/design-review-2.md:2312:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/18-api-skeleton/final-review.md:4322:features/18-api-skeleton/final-review.md:1649:features/12-cylinder/design-review-2.md:2337:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/18-api-skeleton/final-review.md:4323:features/18-api-skeleton/final-review.md:1677:features/18-api-skeleton/plan.md:112:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/final-review.md:4324:features/18-api-skeleton/final-review.md:1751:features/12-cylinder/final-review.md:3756:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/18-api-skeleton/final-review.md:4325:features/18-api-skeleton/final-review.md:1753:features/12-cylinder/final-review.md:3786:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/18-api-skeleton/final-review.md:4326:features/18-api-skeleton/final-review.md:1755:features/12-cylinder/final-review.md:3841:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/18-api-skeleton/final-review.md:4327:features/18-api-skeleton/final-review.md:1798:features/18-api-skeleton/final-review.md:1301:   121	// T06: 403 Host header (DNS rebinding protection)
features/18-api-skeleton/final-review.md:4328:features/18-api-skeleton/final-review.md:1799:features/18-api-skeleton/final-review.md:1314:   134	    // The plan says to add Host header validation middleware.
features/18-api-skeleton/final-review.md:4329:features/18-api-skeleton/final-review.md:1819:features/18-api-skeleton/design-review-3.md:186:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/final-review.md:4330:features/18-api-skeleton/final-review.md:1846:features/18-api-skeleton/design-review-3.md:1883:    finding: "設計本文では assembly/reference を build 前に検出して専用の 422 にするとしている一方、T09 の期待値は `422(EmptyFeatureList)` のままで矛盾している。これでは assembly と空 part を区別しない旧挙動でもテストが通る。"
features/18-api-skeleton/final-review.md:4331:features/18-api-skeleton/final-review.md:1849:features/18-api-skeleton/design-review-3.md:1917:    finding: "設計本文では assembly/reference を build 前に検出して専用の 422 にするとしている一方、T09 の期待値は `422(EmptyFeatureList)` のままで矛盾している。これでは assembly と空 part を区別しない旧挙動でもテストが通る。"
features/18-api-skeleton/final-review.md:4332:features/18-api-skeleton/final-review.md:1900:features/12-cylinder/design-review-3.md:2167:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/18-api-skeleton/final-review.md:4333:features/18-api-skeleton/final-review.md:1902:features/12-cylinder/design-review-3.md:2197:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
features/18-api-skeleton/final-review.md:4334:features/18-api-skeleton/final-review.md:1908:features/18-api-skeleton/design-review.md:150:| T05 | 403 | `Host: evil.com` ヘッダ付与 | 403(Host 検証) |
features/18-api-skeleton/final-review.md:4340:features/18-api-skeleton/final-review.md:1942:features/18-api-skeleton/design-review.md:3071:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:4341:features/18-api-skeleton/final-review.md:1944:features/18-api-skeleton/design-review.md:3081:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:4342:features/18-api-skeleton/final-review.md:1945:features/18-api-skeleton/design-review.md:3091:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
features/18-api-skeleton/final-review.md:4343:features/18-api-skeleton/final-review.md:1946:features/18-api-skeleton/design-review.md:3101:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:4344:features/18-api-skeleton/final-review.md:1948:features/18-api-skeleton/design-review.md:3111:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:4345:features/18-api-skeleton/final-review.md:1949:features/18-api-skeleton/design-review.md:3121:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
features/18-api-skeleton/final-review.md:4346:features/18-api-skeleton/final-review.md:1953:features/18-api-skeleton/design-review.md:3359:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/final-review.md:4347:features/18-api-skeleton/final-review.md:1955:features/18-api-skeleton/design-review.md:3369:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/final-review.md:4348:features/18-api-skeleton/final-review.md:1957:features/18-api-skeleton/design-review.md:3388:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/final-review.md:4349:features/18-api-skeleton/final-review.md:1959:features/18-api-skeleton/design-review.md:3398:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/final-review.md:4350:features/18-api-skeleton/final-review.md:1967:features/18-api-skeleton/design-review-2.md:169:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/final-review.md:4351:features/18-api-skeleton/final-review.md:1975:features/18-api-skeleton/design-review-2.md:476:/bin/bash -lc 'rg -n "run_export|make_cuboid|tessellate_solid_with|test_assembly_roundtrip|workspace.dependencies|thiserror|TriangleMesh|build_solid_from_features|Document::from_path|InvalidExtension|make_cylinder|Euler|HalfEdge|Shell|Loop|IdGenerator" -S .' in /home/bacon/mycad
features/18-api-skeleton/final-review.md:4352:features/18-api-skeleton/final-review.md:1991:features/18-api-skeleton/design-review-2.md:777:./features/12-cylinder/design-review.md:2520:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:4353:features/18-api-skeleton/final-review.md:1992:features/18-api-skeleton/design-review-2.md:780:./features/12-cylinder/design-review.md:2530:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:4354:features/18-api-skeleton/final-review.md:1993:features/18-api-skeleton/design-review-2.md:783:./features/12-cylinder/design-review.md:2550:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:4355:features/18-api-skeleton/final-review.md:1994:features/18-api-skeleton/design-review-2.md:786:./features/12-cylinder/design-review.md:2560:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:4356:features/18-api-skeleton/final-review.md:2005:features/18-api-skeleton/design-review-2.md:974:./features/12-cylinder/design-review-2.md:2312:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/18-api-skeleton/final-review.md:4357:features/18-api-skeleton/final-review.md:2006:features/18-api-skeleton/design-review-2.md:978:./features/12-cylinder/design-review-2.md:2337:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
features/18-api-skeleton/final-review.md:4359:features/18-api-skeleton/final-review.md:2037:features/18-api-skeleton/design-review-2.md:1987:./features/18-api-skeleton/design-review.md:3071:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:4360:features/18-api-skeleton/final-review.md:2038:features/18-api-skeleton/design-review-2.md:1990:./features/18-api-skeleton/design-review.md:3081:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:4361:features/18-api-skeleton/final-review.md:2039:features/18-api-skeleton/design-review-2.md:1993:./features/18-api-skeleton/design-review.md:3101:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
features/18-api-skeleton/final-review.md:4362:features/18-api-skeleton/final-review.md:2040:features/18-api-skeleton/design-review-2.md:1996:./features/18-api-skeleton/design-review.md:3111:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
features/18-api-skeleton/final-review.md:4363:features/18-api-skeleton/final-review.md:2041:features/18-api-skeleton/design-review-2.md:2025:./features/18-api-skeleton/design-review.md:3359:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/final-review.md:4364:features/18-api-skeleton/final-review.md:2042:features/18-api-skeleton/design-review-2.md:2026:./features/18-api-skeleton/design-review.md:3369:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/final-review.md:4365:features/18-api-skeleton/final-review.md:2043:features/18-api-skeleton/design-review-2.md:2028:./features/18-api-skeleton/design-review.md:3388:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
features/18-api-skeleton/final-review.md:4366:features/18-api-skeleton/final-review.md:2044:features/18-api-skeleton/design-review-2.md:2029:./features/18-api-skeleton/design-review.md:3398:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
features/18-api-skeleton/final-review.md:4371:features/18-api-skeleton/final-review.md:2095:features/18-api-skeleton/design-review-2.md:4720:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` の再利用方針と T09 の `EmptyFeatureList` 期待値では、`children` / `ref` を持つ assembly/reference 文書と、本当に feature が空の不正 part 文書を区別できない。これでは文書自身が要求している『assembly は明確なエラー + 422』を満たせない。"
features/18-api-skeleton/final-review.md:4372:features/18-api-skeleton/final-review.md:2096:features/18-api-skeleton/design-review-2.md:4730:    finding: "golden YAML ラウンドトリップが既存 `examples/*` だけに依存しており、新規 `crates/mycad-api/tests/fixtures/*.mycad` の unsupported/退化 fixture については format crate 側の roundtrip 保証が計画されていない。API が依存する入力形状の YAML 安定性が未検証のまま残る。"
features/18-api-skeleton/final-review.md:4373:features/18-api-skeleton/final-review.md:2098:features/18-api-skeleton/design-review-2.md:4745:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` の再利用方針と T09 の `EmptyFeatureList` 期待値では、`children` / `ref` を持つ assembly/reference 文書と、本当に feature が空の不正 part 文書を区別できない。これでは文書自身が要求している『assembly は明確なエラー + 422』を満たせない。"
features/18-api-skeleton/final-review.md:4374:features/18-api-skeleton/final-review.md:2099:features/18-api-skeleton/design-review-2.md:4755:    finding: "golden YAML ラウンドトリップが既存 `examples/*` だけに依存しており、新規 `crates/mycad-api/tests/fixtures/*.mycad` の unsupported/退化 fixture については format crate 側の roundtrip 保証が計画されていない。API が依存する入力形状の YAML 安定性が未検証のまま残る。"
features/18-api-skeleton/final-review.md:4390:features/18-api-skeleton/final-review.md:2224:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/final-review.md:4419:features/18-api-skeleton/final-review.md:2708:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/final-review.md:4585:    50	// T01: normal box
features/18-api-skeleton/final-review.md:4597:    62	// T02: normal cylinder
features/18-api-skeleton/final-review.md:4608:    73	// T03: determinism
features/18-api-skeleton/final-review.md:4628:    93	// T04: 404 not found
features/18-api-skeleton/final-review.md:4638:   103	// T05: 400 non-.mycad extension
features/18-api-skeleton/final-review.md:4656:   121	// T06: 403 Host header (DNS rebinding protection)
features/18-api-skeleton/final-review.md:4668:   133	    // Without explicit host validation middleware, this will pass through.
features/18-api-skeleton/final-review.md:4669:   134	    // The plan says to add Host header validation middleware.
features/18-api-skeleton/final-review.md:4675:   140	// T07: 422 unsupported feature (sphere)
features/18-api-skeleton/final-review.md:4684:   149	// T08: 422 degenerate dimension (zero-width box)
features/18-api-skeleton/final-review.md:4686:   151	async fn t08_degenerate_dimension() {
features/18-api-skeleton/final-review.md:4693:   158	// T09: 422 assembly unsupported
features/18-api-skeleton/final-review.md:4874:    suggestion: "make_cuboid の edge 生成ロジックを見直す"
features/18-api-skeleton/final-review.md:4896:crates/mycad-kernel/src/primitives/cuboid.rs
features/18-api-skeleton/final-review.md:4899:crates/mycad-kernel/tests/cuboid_roundtrip.rs
features/18-api-skeleton/final-review.md:4932:features/18-api-skeleton/design-review-3.md:186:| T06 | 403 | `Host: evil.com` | 403(Host 検証) |
features/18-api-skeleton/final-review.md:4943:features/18-api-skeleton/design-review-3.md:1883:    finding: "設計本文では assembly/reference を build 前に検出して専用の 422 にするとしている一方、T09 の期待値は `422(EmptyFeatureList)` のままで矛盾している。これでは assembly と空 part を区別しない旧挙動でもテストが通る。"
features/18-api-skeleton/final-review.md:4945:features/18-api-skeleton/design-review-3.md:1889:    finding: "`make_cuboid` の新仕様は `non-finite` を拒否すると書いているが、K01 の列挙は `0.0`/負値/`NaN` までで、`INFINITY`/`-INFINITY` の分岐が未検証である。"
features/18-api-skeleton/final-review.md:4962:    finding: "`app()` が素の `Router` を返しており、合意済みの Host ヘッダ検証ミドルウェアが入っていない。`mesh_api.rs` の T06 も `Host: evil.com` に対して `200 OK` を期待しているため、403 防御とテスト計画 T06 が未実装のままになっている。"
features/18-api-skeleton/final-review.md:4963:    suggestion: "`/api/v0` に loopback Host だけを許可する middleware を追加し、T06 を `StatusCode::FORBIDDEN` 期待に戻す"
features/18-api-skeleton/final-review.md:4990:    finding: "`app()` が素の `Router` を返しており、合意済みの Host ヘッダ検証ミドルウェアが入っていない。`mesh_api.rs` の T06 も `Host: evil.com` に対して `200 OK` を期待しているため、403 防御とテスト計画 T06 が未実装のままになっている。"
features/18-api-skeleton/final-review.md:4991:    suggestion: "`/api/v0` に loopback Host だけを許可する middleware を追加し、T06 を `StatusCode::FORBIDDEN` 期待に戻す"

codex
`Cargo.toml` が `crates/mycad-api` を workspace member に追加しているため、未追跡でもこの新規クレートは今回の実装範囲です。ここを読まないと T01〜T09 とスコープ逸脱を正しく判定できません。
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
/bin/bash -lc 'rg --files crates/mycad-api features/18-api-skeleton' in /home/bacon/mycad
 succeeded in 0ms:
features/18-api-skeleton/fix-plan.md
features/18-api-skeleton/ci.log
features/18-api-skeleton/design-review.md
features/18-api-skeleton/glm-result-fix.json
features/18-api-skeleton/final-review.md.verdict.json
features/18-api-skeleton/design-review-2.md
features/18-api-skeleton/final-review-2.md
features/18-api-skeleton/plan.md
features/18-api-skeleton/glm-result.json
features/18-api-skeleton/final-review.md
features/18-api-skeleton/design-review-3.md
features/18-api-skeleton/state.json
crates/mycad-api/Cargo.toml
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
/bin/bash -lc "sed -n '1,260p' crates/mycad-api/Cargo.toml" in /home/bacon/mycad
 succeeded in 0ms:
[package]
name = "mycad-api"
description = "HTTP API server for MyCad"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
axum = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
mycad-format = { workspace = true }
mycad-kernel = { workspace = true }
mycad-build = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tower = { workspace = true, features = ["util"] }

[[bin]]
name = "mycad-api"
path = "src/main.rs"

exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-api/src/lib.rs" in /home/bacon/mycad
 succeeded in 0ms:
pub mod error;
pub mod handler;
pub mod router;

exec
/bin/bash -lc "sed -n '1,360p' crates/mycad-api/src/error.rs" in /home/bacon/mycad
 succeeded in 0ms:
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use mycad_format::error::FormatError;
use mycad_kernel::error::KernelError;
use mycad_kernel::tessellation::TessellationError;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Unprocessable(String),
    #[error("{0}")]
    Internal(String),
}

impl From<FormatError> for ApiError {
    fn from(err: FormatError) -> Self {
        match &err {
            FormatError::InvalidExtension(_) | FormatError::Yaml(_) => {
                ApiError::BadRequest(err.to_string())
            }
            FormatError::Io(io_err) => match io_err.kind() {
                std::io::ErrorKind::NotFound => ApiError::NotFound(err.to_string()),
                _ => ApiError::Internal(err.to_string()),
            },
            FormatError::InvalidReference { .. } => ApiError::Unprocessable(err.to_string()),
        }
    }
}

impl From<KernelError> for ApiError {
    fn from(err: KernelError) -> Self {
        ApiError::Unprocessable(err.to_string())
    }
}

impl From<TessellationError> for ApiError {
    fn from(err: TessellationError) -> Self {
        ApiError::Unprocessable(err.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            ApiError::Unprocessable(_) => (StatusCode::UNPROCESSABLE_ENTITY, self.to_string()),
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };
        let body = json!({ "error": msg });
        (status, axum::Json(body)).into_response()
    }
}

exec
/bin/bash -lc "sed -n '1,360p' crates/mycad-api/src/handler.rs" in /home/bacon/mycad
 succeeded in 0ms:
use crate::error::ApiError;
use axum::extract::Query;
use axum::Json;
use mycad_build::build_solid_from_features;
use mycad_format::Document;
use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::tessellation::{tessellate_solid_with, TessellationOptions, TriangleMesh};
use serde::Deserialize;
use std::path::Path;

const V0_TESSELLATION: TessellationOptions = TessellationOptions {
    angular_segments: 32,
    axial_segments: 1,
};

#[derive(Deserialize)]
pub(crate) struct MeshQuery {
    pub file: String,
}

pub(crate) async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError> {
    let path = Path::new(&q.file);

    if path.is_relative() {
        return Err(ApiError::BadRequest(
            "relative path not allowed: file must be an absolute path".to_string(),
        ));
    }

    let canonical = std::fs::canonicalize(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => {
            ApiError::NotFound(format!("file not found: {}", path.display()))
        }
        _ => ApiError::BadRequest(format!("invalid path: {}", path.display())),
    })?;

    let doc = Document::from_path(&canonical)?;

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
    let solid = build_solid_from_features(&root.features, &mut gen)?;
    let mesh = tessellate_solid_with(&solid, &V0_TESSELLATION)?;
    Ok(Json(mesh))
}

exec
/bin/bash -lc "sed -n '1,240p' crates/mycad-api/src/main.rs" in /home/bacon/mycad
 succeeded in 0ms:
use mycad_api::router::app;

#[tokio::main]
async fn main() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("failed to bind to 127.0.0.1:3000");
    axum::serve(listener, app()).await.expect("server error");
}

exec
/bin/bash -lc "sed -n '1,360p' crates/mycad-api/tests/mesh_api.rs" in /home/bacon/mycad
 succeeded in 0ms:
use axum::body::Body;
use axum::http::{Request, StatusCode};
use mycad_api::router::app;
use serde::Deserialize;
use tower::ServiceExt;

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
}

fn fixture_path(name: &str) -> String {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = std::path::Path::new(&dir)
        .join("tests")
        .join("fixtures")
        .join(name);
    std::fs::canonicalize(&path)
        .unwrap_or_else(|_| panic!("fixture not found: {:?}", path))
        .to_str()
        .unwrap()
        .to_string()
}

fn example_path(name: &str) -> String {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = std::path::Path::new(&dir)
        .join("..")
        .join("..")
        .join("examples")
        .join(name);
    std::fs::canonicalize(&path)
        .unwrap_or_else(|_| panic!("example not found: {:?}", path))
        .to_str()
        .unwrap()
        .to_string()
}

async fn send_mesh_request(uri: &str) -> (StatusCode, String) {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let app = app();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

// T01: normal box
#[tokio::test]
async fn t01_normal_box() {
    let file = example_path("simple_box.mycad");
    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
    let (status, body) = send_mesh_request(&uri).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let mesh: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(mesh["positions"].as_array().unwrap().len() > 0);
    assert!(mesh["indices"].as_array().unwrap().len() % 3 == 0);
}

// T02: normal cylinder
#[tokio::test]
async fn t02_normal_cylinder() {
    let file = example_path("cylinder.mycad");
    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
    let (status, body) = send_mesh_request(&uri).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let mesh: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(mesh["positions"].as_array().unwrap().len() > 0);
}

// T03: determinism
#[tokio::test]
async fn t03_determinism() {
    let file = example_path("simple_box.mycad");
    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));

    let (_, body1) = send_mesh_request(&uri).await;
    let (_, body2) = send_mesh_request(&uri).await;
    assert_eq!(body1, body2, "mesh responses must be deterministic");

    let file2 = example_path("cylinder.mycad");
    let uri2 = format!("/api/v0/mesh?file={}", urlencoding(&file2));
    let (_, body3) = send_mesh_request(&uri2).await;
    let (_, body4) = send_mesh_request(&uri2).await;
    assert_eq!(
        body3, body4,
        "cylinder mesh responses must be deterministic"
    );
}

// T04: 404 not found
#[tokio::test]
async fn t04_not_found() {
    let uri = "/api/v0/mesh?file=/tmp/absolutely_nonexistent_file.mycad";
    let (status, body) = send_mesh_request(uri).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
    assert!(!err.error.is_empty());
}

// T05: 400 non-.mycad extension
#[tokio::test]
async fn t05_invalid_extension() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = std::path::Path::new(&dir)
        .join("..")
        .join("..")
        .join("Cargo.toml");
    let abs = std::fs::canonicalize(&path)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let uri = format!("/api/v0/mesh?file={}", urlencoding(&abs));
    let (status, body) = send_mesh_request(&uri).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
}

// T06: 403 Host header (DNS rebinding protection)
#[tokio::test]
async fn t06_host_header_rebinding() {
    let file = example_path("simple_box.mycad");
    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
    let req = Request::builder()
        .uri(&uri)
        .header("Host", "evil.com")
        .body(Body::empty())
        .unwrap();
    let app = app();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// T07: 422 unsupported feature (sphere)
#[tokio::test]
async fn t07_unsupported_feature() {
    let file = fixture_path("create_sphere.mycad");
    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
    let (status, body) = send_mesh_request(&uri).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
}

// T08: 422 degenerate dimension (zero-width box)
#[tokio::test]
async fn t08_degenerate_dimension() {
    let file = fixture_path("zero_box.mycad");
    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
    let (status, body) = send_mesh_request(&uri).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
}

// T09: 422 assembly unsupported
#[tokio::test]
async fn t09_assembly_unsupported() {
    let file = fixture_path("assembly.mycad");
    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
    let (status, body) = send_mesh_request(&uri).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
    assert!(
        err.error.contains("assembly") || err.error.contains("reference"),
        "expected assembly/reference message, got: {}",
        err.error
    );
}

// T10: 422 empty part
#[tokio::test]
async fn t10_empty_part() {
    let file = fixture_path("empty_part.mycad");
    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
    let (status, body) = send_mesh_request(&uri).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
    assert!(
        err.error.contains("empty part") || err.error.contains("no features"),
        "expected empty part message, got: {}",
        err.error
    );
}

// T11: 400 relative path rejected
#[tokio::test]
async fn t11_relative_path_rejected() {
    let uri = "/api/v0/mesh?file=examples/simple_box.mycad";
    let (status, body) = send_mesh_request(uri).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
    assert!(
        err.error.contains("relative"),
        "expected relative path error, got: {}",
        err.error
    );
}

fn urlencoding(s: &str) -> String {
    s.replace('/', "%2F")
}

exec
/bin/bash -lc "sed -n '1,360p' crates/mycad-api/src/router.rs" in /home/bacon/mycad
 succeeded in 0ms:
use crate::handler::get_mesh;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

async fn host_guard(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let host = req
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let host_base = host.split(':').next().unwrap_or("");
    if matches!(host_base, "127.0.0.1" | "localhost" | "") {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

pub fn app() -> Router {
    Router::new()
        .nest("/api/v0", Router::new().route("/mesh", get(get_mesh)))
        .layer(middleware::from_fn(host_guard))
}

codex
API 側はほぼ計画どおりですが、エラー変換と入力境界の扱いに仕様とのズレがないかを下層実装まで確認します。`Document::from_path` と `FormatError` の経路を追うと、見落としやすい 400/500 の取り違えが出やすい箇所です。
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
/bin/bash -lc "sed -n '1,260p' crates/mycad-build/src/lib.rs" in /home/bacon/mycad
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
        } => make_cuboid(*width, *height, *depth, gen),
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
/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/src/primitives/mod.rs" in /home/bacon/mycad
 succeeded in 0ms:
mod cuboid;
mod cylinder;

pub use cuboid::make_cuboid;
pub use cylinder::make_cylinder;

exec
/bin/bash -lc "sed -n '1,240p' crates/mycad-api/tests/fixtures/assembly.mycad" in /home/bacon/mycad
 succeeded in 0ms:
version: "0.1.0"
root_component:
  name: "Simple Assembly"
  children:
    - name: "Part"
      features:
        - type: create_box
          id: box_1
          width: 10.0
          height: 10.0
          depth: 10.0

exec
/bin/bash -lc "sed -n '1,240p' crates/mycad-api/tests/fixtures/empty_part.mycad" in /home/bacon/mycad
 succeeded in 0ms:
version: "0.1.0"
root_component:
  name: "Empty Part"

exec
/bin/bash -lc "sed -n '1,240p' crates/mycad-api/tests/fixtures/zero_box.mycad" in /home/bacon/mycad
 succeeded in 0ms:
version: "0.1.0"
root_component:
  name: "Zero Box"
  features:
    - type: create_box
      id: box_1
      width: 0.0
      height: 1.0
      depth: 1.0

exec
/bin/bash -lc "sed -n '1,240p' crates/mycad-api/tests/fixtures/create_sphere.mycad" in /home/bacon/mycad
 succeeded in 0ms:
version: "0.1.0"
root_component:
  name: "Sphere"
  features:
    - type: create_sphere
      id: sphere_1
      radius: 5.0

exec
/bin/bash -lc "sed -n '1,280p' crates/mycad-format/src/feature.rs" in /home/bacon/mycad
exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-format/src/component.rs" in /home/bacon/mycad
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

codex
ここまでで計画テスト自体は実装されています。残る論点は「仕様どおりの失敗経路か」と「新規 API 追加が kernel 専用レビュー観点から逸脱していないか」の2点です。
exec
/bin/bash -lc "sed -n '1,220p' features/18-api-skeleton/final-review.md.verdict.json" in /home/bacon/mycad
 succeeded in 0ms:
{"verdict": "fail", "severity_counts": {"critical": 10, "high": 19, "medium": 20, "low": 5}, "blocking": 29}

exec
/bin/bash -lc 'rg -n "issues:|severity:|finding:|suggestion:" features/18-api-skeleton/final-review.md | head -n 80' in /home/bacon/mycad
 succeeded in 0ms:
56:issues:
58:    severity: critical  # critical | high | medium | low
61:    finding: "HalfEdge の twin インデックスが self を指している（ループ不整合）"
62:    suggestion: "make_cuboid の edge 生成ロジックを見直す"
64:    severity: high
71:`issues` が空の場合は `issues: []` と書く。
1572:features/12-cylinder/design-review.md:2520:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
1574:features/12-cylinder/design-review.md:2530:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
1575:features/12-cylinder/design-review.md:2540:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
1576:features/12-cylinder/design-review.md:2550:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
1578:features/12-cylinder/design-review.md:2560:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
1579:features/12-cylinder/design-review.md:2570:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
1644:features/12-cylinder/design-review-2.md:2308:    suggestion: "この変更を意図的な winding 修正として明記し、`T08` を『counts は維持するが indices / STL facet normal は変わり得る』前提に更新するか、先に cuboid 側の loop orientation を正規化して本当に不変にすること。"
1646:features/12-cylinder/design-review-2.md:2312:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
1647:features/12-cylinder/design-review-2.md:2333:    suggestion: "この変更を意図的な winding 修正として明記し、`T08` を『counts は維持するが indices / STL facet normal は変わり得る』前提に更新するか、先に cuboid 側の loop orientation を正規化して本当に不変にすること。"
1649:features/12-cylinder/design-review-2.md:2337:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
1751:features/12-cylinder/final-review.md:3756:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
1753:features/12-cylinder/final-review.md:3786:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
1755:features/12-cylinder/final-review.md:3841:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
1846:features/18-api-skeleton/design-review-3.md:1883:    finding: "設計本文では assembly/reference を build 前に検出して専用の 422 にするとしている一方、T09 の期待値は `422(EmptyFeatureList)` のままで矛盾している。これでは assembly と空 part を区別しない旧挙動でもテストが通る。"
1847:features/18-api-skeleton/design-review-3.md:1884:    suggestion: "T09 は assembly/reference 専用メッセージを明示的に assert し、`features.is_empty()` の空 part は別テストに分離すること。"
1849:features/18-api-skeleton/design-review-3.md:1917:    finding: "設計本文では assembly/reference を build 前に検出して専用の 422 にするとしている一方、T09 の期待値は `422(EmptyFeatureList)` のままで矛盾している。これでは assembly と空 part を区別しない旧挙動でもテストが通る。"
1850:features/18-api-skeleton/design-review-3.md:1918:    suggestion: "T09 は assembly/reference 専用メッセージを明示的に assert し、`features.is_empty()` の空 part は別テストに分離すること。"
1900:features/12-cylinder/design-review-3.md:2167:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
1902:features/12-cylinder/design-review-3.md:2197:    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
1942:features/18-api-skeleton/design-review.md:3071:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
1944:features/18-api-skeleton/design-review.md:3081:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
1945:features/18-api-skeleton/design-review.md:3091:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
1946:features/18-api-skeleton/design-review.md:3101:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
1948:features/18-api-skeleton/design-review.md:3111:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
1949:features/18-api-skeleton/design-review.md:3121:    finding: "`examples/cylinder.mycad` または `Feature::CreateCylinder` の golden YAML ラウンドトリップ/決定的シリアライズ検証が計画に含まれていない。`build`/`export` が通っても `create_cylinder` のフォーマット回帰は検出できない。"
1950:features/18-api-skeleton/design-review.md:3127:/bin/bash -lc 'rg -n "''^issues:|''^verdict:|id: R" -n features/18-api-skeleton/design-review.md features/18-api-skeleton/plan.md' in /home/bacon/mycad
1952:features/18-api-skeleton/design-review.md:3350:    suggestion: "絶対パスのみ受け付けるか、`app(state)` に明示的な base directory を持たせて相対パス解決を固定し、その規則を T02 と E2E で検証すること。"
1953:features/18-api-skeleton/design-review.md:3359:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
1954:features/18-api-skeleton/design-review.md:3360:    suggestion: "v0 を『root_component に単一 part feature を持つ文書のみ対応』と明記して assembly 非対応を別エラーに分けるか、Document/Component 階層を解釈する document-level build API を設計すること。T06 は専用 fixture に差し替えること。"
1955:features/18-api-skeleton/design-review.md:3369:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
1956:features/18-api-skeleton/design-review.md:3379:    suggestion: "絶対パスのみ受け付けるか、`app(state)` に明示的な base directory を持たせて相対パス解決を固定し、その規則を T02 と E2E で検証すること。"
1957:features/18-api-skeleton/design-review.md:3388:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
1958:features/18-api-skeleton/design-review.md:3389:    suggestion: "v0 を『root_component に単一 part feature を持つ文書のみ対応』と明記して assembly 非対応を別エラーに分けるか、Document/Component 階層を解釈する document-level build API を設計すること。T06 は専用 fixture に差し替えること。"
1959:features/18-api-skeleton/design-review.md:3398:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
1991:features/18-api-skeleton/design-review-2.md:777:./features/12-cylinder/design-review.md:2520:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
1992:features/18-api-skeleton/design-review-2.md:780:./features/12-cylinder/design-review.md:2530:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
1993:features/18-api-skeleton/design-review-2.md:783:./features/12-cylinder/design-review.md:2550:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
1994:features/18-api-skeleton/design-review-2.md:786:./features/12-cylinder/design-review.md:2560:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
2005:features/18-api-skeleton/design-review-2.md:974:./features/12-cylinder/design-review-2.md:2312:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
2006:features/18-api-skeleton/design-review-2.md:978:./features/12-cylinder/design-review-2.md:2337:    finding: "T03 の期待結果が `assert_eq!(2-3+3, 2)` 相当になっており、生成した `Solid` の `V/E/F/S/H` を参照していない。これでは Euler-Poincaré 検証にならない。"
2037:features/18-api-skeleton/design-review-2.md:1987:./features/18-api-skeleton/design-review.md:3071:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
2038:features/18-api-skeleton/design-review-2.md:1990:./features/18-api-skeleton/design-review.md:3081:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
2039:features/18-api-skeleton/design-review-2.md:1993:./features/18-api-skeleton/design-review.md:3101:    finding: "`Curve::sample_segment` が `t_range` だけを受け取り、rev HalfEdge を単純反転で処理する設計になっている。`Line` は `evaluate(t0)` の 1 点しか返さないため、既存 cuboid の reversed edge が誤った始点からサンプルされ、T08 の『平面不変』はこのままでは成立しない。"
2040:features/18-api-skeleton/design-review-2.md:1996:./features/18-api-skeleton/design-review.md:3111:    finding: "設計は各 edge の HalfEdge 使用回数だけを規定しているが、現行 `HalfEdge` 型に `twin`/`next`/`prev` がなく、T04 も参照数と向きの集計しか見ない。これでは loop 閉包や `e_seam` 両側の接続誤りを検出できず、レビュー観点のトポロジー妥当性を満たせない。"
2041:features/18-api-skeleton/design-review-2.md:2025:./features/18-api-skeleton/design-review.md:3359:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
2042:features/18-api-skeleton/design-review-2.md:2026:./features/18-api-skeleton/design-review.md:3369:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
2043:features/18-api-skeleton/design-review-2.md:2028:./features/18-api-skeleton/design-review.md:3388:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` に固定すると、`children` / `ref` を持つ有効な `.mycad` 文書を全て root feature なしの build error として扱う。`examples/assembly.mycad` も実際は『複数/未対応フィーチャ』ではなく assembly 文書であり、T06 が意図した失敗経路を検証していない。"
2044:features/18-api-skeleton/design-review-2.md:2029:./features/18-api-skeleton/design-review.md:3398:    finding: "T01-T06 は HTTP status と mesh JSON しか見ておらず、新しい Document→Solid→Mesh 経路に対する Euler-Poincare 検証と golden YAML ラウンドトリップが無い。supported case も `simple_box.mycad` に偏っており、`examples/cylinder.mycad` の周期面テッセレーション経路を API 経由で固定していない。"
2095:features/18-api-skeleton/design-review-2.md:4720:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` の再利用方針と T09 の `EmptyFeatureList` 期待値では、`children` / `ref` を持つ assembly/reference 文書と、本当に feature が空の不正 part 文書を区別できない。これでは文書自身が要求している『assembly は明確なエラー + 422』を満たせない。"
2096:features/18-api-skeleton/design-review-2.md:4730:    finding: "golden YAML ラウンドトリップが既存 `examples/*` だけに依存しており、新規 `crates/mycad-api/tests/fixtures/*.mycad` の unsupported/退化 fixture については format crate 側の roundtrip 保証が計画されていない。API が依存する入力形状の YAML 安定性が未検証のまま残る。"
2097:features/18-api-skeleton/design-review-2.md:4731:    suggestion: "`mycad-format` 側に新規 fixture を読み込む roundtrip/golden テストを追加するか、API fixture を共有して `to_yaml`/`from_yaml` の安定性を format crate の所有テストで担保すること。"
2098:features/18-api-skeleton/design-review-2.md:4745:    finding: "`build_solid_from_features(&doc.root_component.features, ...)` の再利用方針と T09 の `EmptyFeatureList` 期待値では、`children` / `ref` を持つ assembly/reference 文書と、本当に feature が空の不正 part 文書を区別できない。これでは文書自身が要求している『assembly は明確なエラー + 422』を満たせない。"
2099:features/18-api-skeleton/design-review-2.md:4755:    finding: "golden YAML ラウンドトリップが既存 `examples/*` だけに依存しており、新規 `crates/mycad-api/tests/fixtures/*.mycad` の unsupported/退化 fixture については format crate 側の roundtrip 保証が計画されていない。API が依存する入力形状の YAML 安定性が未検証のまま残る。"
2100:features/18-api-skeleton/design-review-2.md:4756:    suggestion: "`mycad-format` 側に新規 fixture を読み込む roundtrip/golden テストを追加するか、API fixture を共有して `to_yaml`/`from_yaml` の安定性を format crate の所有テストで担保すること。"
2314:issues:
2316:    severity: critical  # critical | high | medium | low
2319:    finding: "HalfEdge の twin インデックスが self を指している（ループ不整合）"
2320:    suggestion: "make_cuboid の edge 生成ロジックを見直す"
2322:    severity: high
2329:`issues` が空の場合は `issues: []` と書く。
2579:issues:
2581:    severity: critical  # critical | high | medium | low
2583:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
2584:    suggestion: "IdGenerator::next() に置き換えること"
2586:    severity: medium
2593:`issues` が空の場合は `issues: []` と書く。
4008:/bin/bash -lc "rg -n \"finding:|severity:|ApiError|Host|tower|ServiceExt|thiserror|MeshQuery\" features/18-api-skeleton/final-review.md features/18-api-skeleton/design-review-3.md features/18-api-skeleton/design-review-2.md -g '*.md'" in /home/bacon/mycad
4011:features/18-api-skeleton/design-review-3.md:59:    severity: critical  # critical | high | medium | low
4012:features/18-api-skeleton/design-review-3.md:61:    finding: "IdGenerator を使わず Uuid::new_v4() を使用している"
4013:features/18-api-skeleton/design-review-3.md:64:    severity: medium
4033:features/18-api-skeleton/design-review-3.md:1863:    severity: high
4034:features/18-api-skeleton/design-review-3.md:1865:    finding: "相対 `file` を `std::env::current_dir()` 基準で解決するため、同じ `GET /api/v0/mesh?file=examples/simple_box.mycad` でもサーバの起動 CWD によって参照先と結果が変わる。これは MyCad の『同一入力→同一出力』に対する隠れた環境依存になる。"
4035:features/18-api-skeleton/design-review-3.md:1869:    severity: high
4036:features/18-api-skeleton/design-review-3.md:1871:    finding: "`tessellate_solid_with(..., &TessellationOptions::default())` を API 契約にしており、kernel 側の `Default` 値変更がそのまま `/api/v0` の三角形数・頂点順序を変えうる。URL で `v0` を切っていてもメッシュ仕様が静かにドリフトする。"

exec
/bin/bash -lc "sed -n '1,260p' .gitignore" in /home/bacon/mycad
 succeeded in 0ms:
/target
*.swp
*.swo
*~
.DS_Store
*.stl

# /3ai フロー
.env
features/**/*.raw

codex
問題候補は少数まで絞れています。行番号を確定して、実際に仕様逸脱か単なる好みかを切り分けます。
exec
/bin/bash -lc "nl -ba crates/mycad-api/src/handler.rs | sed -n '1,220p'" in /home/bacon/mycad
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
    17	pub(crate) struct MeshQuery {
    18	    pub file: String,
    19	}
    20	
    21	pub(crate) async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError> {
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
/bin/bash -lc "nl -ba crates/mycad-api/src/error.rs | sed -n '1,220p'" in /home/bacon/mycad
 succeeded in 0ms:
     1	use axum::http::StatusCode;
     2	use axum::response::{IntoResponse, Response};
     3	use mycad_format::error::FormatError;
     4	use mycad_kernel::error::KernelError;
     5	use mycad_kernel::tessellation::TessellationError;
     6	use serde_json::json;
     7	
     8	#[derive(Debug, thiserror::Error)]
     9	pub enum ApiError {
    10	    #[error("{0}")]
    11	    BadRequest(String),
    12	    #[error("{0}")]
    13	    NotFound(String),
    14	    #[error("{0}")]
    15	    Unprocessable(String),
    16	    #[error("{0}")]
    17	    Internal(String),
    18	}
    19	
    20	impl From<FormatError> for ApiError {
    21	    fn from(err: FormatError) -> Self {
    22	        match &err {
    23	            FormatError::InvalidExtension(_) | FormatError::Yaml(_) => {
    24	                ApiError::BadRequest(err.to_string())
    25	            }
    26	            FormatError::Io(io_err) => match io_err.kind() {
    27	                std::io::ErrorKind::NotFound => ApiError::NotFound(err.to_string()),
    28	                _ => ApiError::Internal(err.to_string()),
    29	            },
    30	            FormatError::InvalidReference { .. } => ApiError::Unprocessable(err.to_string()),
    31	        }
    32	    }
    33	}
    34	
    35	impl From<KernelError> for ApiError {
    36	    fn from(err: KernelError) -> Self {
    37	        ApiError::Unprocessable(err.to_string())
    38	    }
    39	}
    40	
    41	impl From<TessellationError> for ApiError {
    42	    fn from(err: TessellationError) -> Self {
    43	        ApiError::Unprocessable(err.to_string())
    44	    }
    45	}
    46	
    47	impl IntoResponse for ApiError {
    48	    fn into_response(self) -> Response {
    49	        let (status, msg) = match &self {
    50	            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
    51	            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
    52	            ApiError::Unprocessable(_) => (StatusCode::UNPROCESSABLE_ENTITY, self.to_string()),
    53	            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
    54	        };
    55	        let body = json!({ "error": msg });
    56	        (status, axum::Json(body)).into_response()
    57	    }
    58	}

exec
/bin/bash -lc "nl -ba crates/mycad-api/tests/mesh_api.rs | sed -n '1,260p'" in /home/bacon/mycad
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
   133	    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
   134	}
   135	
   136	// T07: 422 unsupported feature (sphere)
   137	#[tokio::test]
   138	async fn t07_unsupported_feature() {
   139	    let file = fixture_path("create_sphere.mycad");
   140	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
   141	    let (status, body) = send_mesh_request(&uri).await;
   142	    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
   143	}
   144	
   145	// T08: 422 degenerate dimension (zero-width box)
   146	#[tokio::test]
   147	async fn t08_degenerate_dimension() {
   148	    let file = fixture_path("zero_box.mycad");
   149	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
   150	    let (status, body) = send_mesh_request(&uri).await;
   151	    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
   152	}
   153	
   154	// T09: 422 assembly unsupported
   155	#[tokio::test]
   156	async fn t09_assembly_unsupported() {
   157	    let file = fixture_path("assembly.mycad");
   158	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
   159	    let (status, body) = send_mesh_request(&uri).await;
   160	    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
   161	    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
   162	    assert!(
   163	        err.error.contains("assembly") || err.error.contains("reference"),
   164	        "expected assembly/reference message, got: {}",
   165	        err.error
   166	    );
   167	}
   168	
   169	// T10: 422 empty part
   170	#[tokio::test]
   171	async fn t10_empty_part() {
   172	    let file = fixture_path("empty_part.mycad");
   173	    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
   174	    let (status, body) = send_mesh_request(&uri).await;
   175	    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
   176	    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
   177	    assert!(
   178	        err.error.contains("empty part") || err.error.contains("no features"),
   179	        "expected empty part message, got: {}",
   180	        err.error
   181	    );
   182	}
   183	
   184	// T11: 400 relative path rejected
   185	#[tokio::test]
   186	async fn t11_relative_path_rejected() {
   187	    let uri = "/api/v0/mesh?file=examples/simple_box.mycad";
   188	    let (status, body) = send_mesh_request(uri).await;
   189	    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
   190	    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
   191	    assert!(
   192	        err.error.contains("relative"),
   193	        "expected relative path error, got: {}",
   194	        err.error
   195	    );
   196	}
   197	
   198	fn urlencoding(s: &str) -> String {
   199	    s.replace('/', "%2F")
   200	}

exec
/bin/bash -lc "nl -ba crates/mycad-api/src/router.rs | sed -n '1,220p'" in /home/bacon/mycad
 succeeded in 0ms:
     1	use crate::handler::get_mesh;
     2	use axum::body::Body;
     3	use axum::http::{Request, StatusCode};
     4	use axum::middleware::{self, Next};
     5	use axum::response::Response;
     6	use axum::routing::get;
     7	use axum::Router;
     8	
     9	async fn host_guard(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    10	    let host = req
    11	        .headers()
    12	        .get("host")
    13	        .and_then(|v| v.to_str().ok())
    14	        .unwrap_or("");
    15	    let host_base = host.split(':').next().unwrap_or("");
    16	    if matches!(host_base, "127.0.0.1" | "localhost" | "") {
    17	        Ok(next.run(req).await)
    18	    } else {
    19	        Err(StatusCode::FORBIDDEN)
    20	    }
    21	}
    22	
    23	pub fn app() -> Router {
    24	    Router::new()
    25	        .nest("/api/v0", Router::new().route("/mesh", get(get_mesh)))
    26	        .layer(middleware::from_fn(host_guard))
    27	}

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
/bin/bash -lc "nl -ba crates/mycad-format/src/document.rs | sed -n '1,120p'" in /home/bacon/mycad
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

exec
/bin/bash -lc 'rg -n "Uuid::new_v4|rand::|thread_rng|SystemTime|HashMap|HashSet|BTreeMap|IndexMap" crates/mycad-api crates/mycad-kernel/src/primitives/cuboid.rs crates/mycad-build/src/lib.rs crates/mycad-kernel/tests/cuboid_roundtrip.rs crates/mycad-kernel/src/tessellation crates/mycad-kernel/benches/tessellation.rs' in /home/bacon/mycad
 exited 1 in 0ms:
exec
/bin/bash -lc "rg -n \"==\\s*0\\.0|==\\s*f64|==\\s*.*point|==\\s*.*norm|==\\s*.*radius|==\\s*.*height\" crates/mycad-kernel/src/primitives/cuboid.rs crates/mycad-api crates/mycad-build/src/lib.rs" in /home/bacon/mycad
 exited 1 in 0ms:
exec
/bin/bash -lc 'rg -n "golden|roundtrip" crates/mycad-api crates/mycad-kernel/tests crates/mycad-format/src' in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-format/src/document.rs:55:    fn test_roundtrip_yaml() {
crates/mycad-format/src/document.rs:116:    fn test_assembly_roundtrip() {

codex
差分外の noise ではなく、実際に `git diff` に入っているプラン外変更も一応確認します。スコープ逸脱は小さくてもレビュー観点に明記されているため、見逃さない方がいいです。
exec
/bin/bash -lc "sed -n '1,220p' .claude/skills/3ai/SKILL.md" in /home/bacon/mycad
 succeeded in 0ms:
---
name: 3ai
description: Claude × Codex × GLM マルチエージェント CAD 開発フロー。GitHub Issue を起点に設計(Claude)→設計レビュー(Codex)→実装+テスト(GLM)→最終レビュー(Codex)を一周する。
tools: Read, Write, Edit, Bash, Glob, Grep
---

# /3ai — マルチエージェント CAD 開発フロー

このスキルが呼ばれたら以下の手順を**必ず順番通りに**実行すること。
普段のプランモードとは独立したフローである。

---

## STEP 0: プランモードへ移行

**`EnterPlanMode` を呼ぶ。** 以降 STEP 4 まではプランモード内で動作する。

> **重要 — プランモード内でも STEP 1・STEP 3 の Bash dispatch は必ず実行する。**
> harness のプランモードは「read-only except plan file」と指示するが、これが deny するのは Edit/Write 等のファイル変更ツールであって、**Bash サブプロセス起動は通る**。
> `mkdir features/...`・`state.sh`・`dispatch-codex.sh`（Codex 設計レビュー）はすべて Bash であり、プランモード内でそのまま走る。
> したがって **STEP 3 の Codex 設計レビューを「ExitPlanMode 後に回す」と後ろ倒ししてはならない。** プランファイル記述完了後、プランモード内で Codex レビューを回して Critical/High を潰し、その後で STEP 4 の `ExitPlanMode` を呼ぶ（STEP 3→4 の順序厳守）。
> ファイル変更で deny されるのは `crates/**` への Edit/Write のみ（`guard-crates.sh` フック）。

---

## STEP 1: Issue 選択・作業ブランチ作成

```bash
gh issue list --state open
```

ROADMAP.md の現 Phase に紐づく Milestone の未着手 Issue を提案し、ユーザーに選んでもらう。
選んだ Issue 番号を `$ISSUE_NUM`、スラッグを `$ISSUE_SLUG` とする（例: `42-make-cylinder`）。

作業ディレクトリと状態ファイルを作成（後で使う）:
```bash
mkdir -p features/$ISSUE_NUM-$ISSUE_SLUG
bash .claude/skills/3ai/scripts/state.sh init features/$ISSUE_NUM-$ISSUE_SLUG/state.json $ISSUE_NUM $ISSUE_SLUG
```

---

## STEP 2: 壁打ち・設計・テスト設計

ユーザーと議論しながら設計を固める。CLAUDE.md の原則を守ること:
- **推測禁止** — 不明点は `grep`/`Read` で調査してから書く
- **反論・客観的視点を提供** — 問題を多角的に検討する

プランファイルに以下を記述する（`features/$ISSUE_NUM-$ISSUE_SLUG/plan.md` と対応させる）:

```markdown
## 実装対象
- Issue: #NNN
- 影響クレート/ファイル: (具体パス列挙)
- 変更する型・関数のシグネチャ

## 設計方針
- 決定性要件: (IdGenerator の使い方、同一入力→同一出力の保証方法)
- B-rep トポロジー妥当性: (Euler-Poincaré が成立するか: V - E + F = 2)
- 退化幾何の扱い: (ゼロ長エッジ、面積ゼロ面などの排除・エラー条件)
- derive 規約: Debug/Clone/Serialize/Deserialize (+JsonSchema が必要か)
- エラーハンドリング: thiserror の使い方
- workspace.dependencies 規約

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | 同一入力を2回実行し全 ID・座標が一致 | assert_eq! |
| T02 | 正常系 | ... | ... |
| TXX | エッジケース | 退化入力 / 境界数値 | エラーまたは正常処理 |
| TXX | golden YAML | .mycad → rebuild → 再出力が一致 | assert_eq! |
```

---

## STEP 3: Codex 設計レビュー（プランモード内）

プランファイルへの記述が完成したら、Codex にレビューを委託する。
ループ上限: **3回**。それでも Critical/High が残れば停止してユーザーに報告。

```bash
# プランファイルをまとめてレビュー用 input として使う
bash .claude/skills/3ai/scripts/dispatch-codex.sh \
  --mode design \
  --input <プランファイルパス> \
  --instruction .claude/skills/3ai/agents/codex-design-reviewer.md \
  --result features/$ISSUE_NUM-$ISSUE_SLUG/design-review.md
```

**完了通知を待つ（ポーリングしない）。** 結果ファイルを読み、Critical/High があれば:
1. プランを改訂
2. 上記コマンドを再実行（ループカウント +1）
3. 上限(3回)に達したら **停止してユーザーにエスカレーション**

全 Critical/High 解消後:
```bash
bash .claude/skills/3ai/scripts/state.sh set features/$ISSUE_NUM-$ISSUE_SLUG/state.json design_review passed
```

---

## STEP 4: 確定プラン提出

**`ExitPlanMode` を呼ぶ。** ユーザーに承認を求める。
承認されたら STEP 5 へ進む。

---

## STEP 5: 作業ブランチ作成

```bash
git checkout -b cad/$ISSUE_NUM-$ISSUE_SLUG
```

---

## STEP 6: GLM-5.1 実装（背景実行・完了通知）

GLM に実装+テスト+`cargo xtask ci` green ループを**全て内部で**回させる。
**自分（Claude）は crates/** を編集しない**（フックが deny する）。

```bash
bash .claude/skills/3ai/scripts/dispatch-glm.sh \
  --agent .claude/skills/3ai/agents/glm-implementer.md \
  --plan-file <プランファイルパス> \
  --feature-dir features/$ISSUE_NUM-$ISSUE_SLUG \
  --result-file features/$ISSUE_NUM-$ISSUE_SLUG/glm-result.json \
  --max-turns 80
```

**`run_in_background: true` で起動し、完了通知を待つ。**

`glm-result.json` を読んで確認:
- `status: success` かつ `ci_passed: true` → 以下を実行して STEP 7 へ:
  ```bash
  bash .claude/skills/3ai/scripts/state.sh set features/$ISSUE_NUM-$ISSUE_SLUG/state.json glm_impl passed
  ```
- `status: failed` → **停止してユーザーにエスカレーション**（Anthropic claude への自動フォールバック禁止）

---

## STEP 7: Codex 最終レビュー（背景実行・完了通知）

**ゲート: glm_impl が passed であることを確認。未通過なら STEP 6 へ戻ること。**
```bash
bash .claude/skills/3ai/scripts/state.sh assert features/$ISSUE_NUM-$ISSUE_SLUG/state.json glm_impl
```

ループ上限: **2回**。残れば停止してエスカレーション。

```bash
bash .claude/skills/3ai/scripts/dispatch-codex.sh \
  --mode review \
  --instruction .claude/skills/3ai/agents/codex-final-reviewer.md \
  --result features/$ISSUE_NUM-$ISSUE_SLUG/final-review.md
```

**完了通知を待つ。** `final-review.md.verdict.json` の `blocking` が 0 かつ `verdict: pass` なら:
```bash
bash .claude/skills/3ai/scripts/state.sh set features/$ISSUE_NUM-$ISSUE_SLUG/state.json final_review passed
```

Critical/High があれば:
1. GLM 修正ディスパッチ（dispatch-glm.sh を再実行、フォーカスは指摘箇所のみ）
2. Codex 再レビュー（ループ +1）
3. 上限到達で **停止・エスカレーション**

---

## STEP 8: 確定・squash マージ

**ゲート: final_review が passed であることを確認。未通過なら STEP 7 へ戻ること。**
```bash
bash .claude/skills/3ai/scripts/state.sh assert features/$ISSUE_NUM-$ISSUE_SLUG/state.json final_review
```

```bash
cargo xtask ci   # 最終 green 確認
git checkout main
git merge --squash cad/$ISSUE_NUM-$ISSUE_SLUG
git commit -m "feat: <内容の一行要約>

<詳細（任意）>

Closes #$ISSUE_NUM

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
git push origin main
git branch -d cad/$ISSUE_NUM-$ISSUE_SLUG
bash .claude/skills/3ai/scripts/state.sh set features/$ISSUE_NUM-$ISSUE_SLUG/state.json merge passed
```

---

## 禁止事項（常に守ること）

- `crates/**` を自分（Claude）が直接 Edit/Write **しない** — フックが deny する
- `glm`（Z.AI ラッパー）以外の `claude -p` ワーカーを spawn **しない**（課金制約）
- GLM が詰まっても Anthropic claude へ自動フォールバック**しない**
- dispatch 完了をポーリング**しない** — 背景実行 + 完了通知で受け取る
- git commit/push は STEP 8 以外で行わない

exec
/bin/bash -lc 'git diff -- .claude/skills/3ai/SKILL.md .gitignore' in /home/bacon/mycad
 succeeded in 0ms:
diff --git a/.claude/skills/3ai/SKILL.md b/.claude/skills/3ai/SKILL.md
index 2363420..f326d22 100644
--- a/.claude/skills/3ai/SKILL.md
+++ b/.claude/skills/3ai/SKILL.md
@@ -15,6 +15,12 @@ tools: Read, Write, Edit, Bash, Glob, Grep
 
 **`EnterPlanMode` を呼ぶ。** 以降 STEP 4 まではプランモード内で動作する。
 
+> **重要 — プランモード内でも STEP 1・STEP 3 の Bash dispatch は必ず実行する。**
+> harness のプランモードは「read-only except plan file」と指示するが、これが deny するのは Edit/Write 等のファイル変更ツールであって、**Bash サブプロセス起動は通る**。
+> `mkdir features/...`・`state.sh`・`dispatch-codex.sh`（Codex 設計レビュー）はすべて Bash であり、プランモード内でそのまま走る。
+> したがって **STEP 3 の Codex 設計レビューを「ExitPlanMode 後に回す」と後ろ倒ししてはならない。** プランファイル記述完了後、プランモード内で Codex レビューを回して Critical/High を潰し、その後で STEP 4 の `ExitPlanMode` を呼ぶ（STEP 3→4 の順序厳守）。
+> ファイル変更で deny されるのは `crates/**` への Edit/Write のみ（`guard-crates.sh` フック）。
+
 ---
 
 ## STEP 1: Issue 選択・作業ブランチ作成
diff --git a/.gitignore b/.gitignore
index ab2958a..5e44e21 100644
--- a/.gitignore
+++ b/.gitignore
@@ -4,3 +4,7 @@
 *~
 .DS_Store
 *.stl
+
+# /3ai フロー
+.env
+features/**/*.raw

exec
/bin/bash -lc "sed -n '1,120p' crates/mycad-kernel/src/tessellation/mod.rs" in /home/bacon/mycad
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

exec
/bin/bash -lc "rg -n \"derive\\(.*Serialize|struct TriangleMesh|pub struct TriangleMesh\" crates/mycad-kernel/src/tessellation/mod.rs" in /home/bacon/mycad
 succeeded in 0ms:
17:#[derive(Debug, Clone, Serialize, Deserialize)]
18:pub struct TriangleMesh {
49:#[derive(Debug, Clone, Copy, Serialize, Deserialize)]

exec
/bin/bash -lc "sed -n '1,200p' crates/mycad-api/src/main.rs" in /home/bacon/mycad
 succeeded in 0ms:
use mycad_api::router::app;

#[tokio::main]
async fn main() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("failed to bind to 127.0.0.1:3000");
    axum::serve(listener, app()).await.expect("server error");
}

exec
/bin/bash -lc 'rg -n "invalid path|directory|relative path|assembly/reference documents|empty part" crates/mycad-api crates/mycad-format' in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-api/src/handler.rs:26:            "relative path not allowed: file must be an absolute path".to_string(),
crates/mycad-api/src/handler.rs:34:        _ => ApiError::BadRequest(format!("invalid path: {}", path.display())),
crates/mycad-api/src/handler.rs:42:            "assembly/reference documents are not supported in v0".to_string(),
crates/mycad-api/src/handler.rs:47:            "empty part: document has no features".to_string(),
crates/mycad-api/tests/mesh_api.rs:169:// T10: 422 empty part
crates/mycad-api/tests/mesh_api.rs:178:        err.error.contains("empty part") || err.error.contains("no features"),
crates/mycad-api/tests/mesh_api.rs:179:        "expected empty part message, got: {}",
crates/mycad-api/tests/mesh_api.rs:184:// T11: 400 relative path rejected
crates/mycad-api/tests/mesh_api.rs:193:        "expected relative path error, got: {}",

exec
/bin/bash -lc "sed -n '1,160p' crates/mycad-api/src/lib.rs" in /home/bacon/mycad
 succeeded in 0ms:
pub mod error;
pub mod handler;
pub mod router;

exec
/bin/bash -lc 'rg -n "ApiError::BadRequest|ApiError::NotFound|ApiError::Unprocessable|ApiError::Internal" crates/mycad-api' in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-api/src/handler.rs:25:        return Err(ApiError::BadRequest(
crates/mycad-api/src/handler.rs:32:            ApiError::NotFound(format!("file not found: {}", path.display()))
crates/mycad-api/src/handler.rs:34:        _ => ApiError::BadRequest(format!("invalid path: {}", path.display())),
crates/mycad-api/src/handler.rs:41:        return Err(ApiError::Unprocessable(
crates/mycad-api/src/handler.rs:46:        return Err(ApiError::Unprocessable(
crates/mycad-api/src/error.rs:24:                ApiError::BadRequest(err.to_string())
crates/mycad-api/src/error.rs:27:                std::io::ErrorKind::NotFound => ApiError::NotFound(err.to_string()),
crates/mycad-api/src/error.rs:28:                _ => ApiError::Internal(err.to_string()),
crates/mycad-api/src/error.rs:30:            FormatError::InvalidReference { .. } => ApiError::Unprocessable(err.to_string()),
crates/mycad-api/src/error.rs:37:        ApiError::Unprocessable(err.to_string())
crates/mycad-api/src/error.rs:43:        ApiError::Unprocessable(err.to_string())
crates/mycad-api/src/error.rs:50:            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
crates/mycad-api/src/error.rs:51:            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
crates/mycad-api/src/error.rs:52:            ApiError::Unprocessable(_) => (StatusCode::UNPROCESSABLE_ENTITY, self.to_string()),
crates/mycad-api/src/error.rs:53:            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),

exec
/bin/bash -lc "rg -n \"make_cuboid\\(\" . -g '"'!target'"' -g '"'!features/**/ci.log'"' -g '"'!features/**/final-review*.md'"' -g '"'!features/**/design-review*.md'"' -g '"'!features/**/glm-result*.json*'"'" in /home/bacon/mycad
 succeeded in 0ms:
./features/18-api-skeleton/plan.md:25:| `crates/mycad-build/src/lib.rs` | CreateBox 分岐を `Ok(make_cuboid(...))` → `make_cuboid(...)`(`?` 伝播)に変更 |
./features/18-api-skeleton/plan.md:119:| K02 | kernel 単体(R03 境界可視化) | `make_cuboid(1e-9, 1.0, 1.0, ..)`(near-zero 正値) | 現状は `Ok`(exact `<= 0.0` 方針)であることを assert し境界を明文化 |
./crates/mycad-build/src/lib.rs:26:        } => make_cuboid(*width, *height, *depth, gen),
./crates/mycad-kernel/src/tessellation/mod.rs:358:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:373:        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:374:        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:500:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:514:        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:515:        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2).unwrap();
./crates/mycad-kernel/src/tessellation/stl.rs:58:        let solid = make_cuboid(1.0, 1.0, 1.0, &mut g).unwrap();
./crates/mycad-kernel/src/tessellation/stl.rs:75:            let s = make_cuboid(1.0, 1.0, 1.0, &mut g).unwrap();
./crates/mycad-kernel/benches/tessellation.rs:15:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
./crates/mycad-kernel/tests/cuboid_roundtrip.rs:9:    let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
./crates/mycad-kernel/tests/cuboid_roundtrip.rs:31:        let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen).unwrap();
./crates/mycad-kernel/src/primitives/cuboid.rs:17:pub fn make_cuboid(
./crates/mycad-kernel/src/primitives/cuboid.rs:244:        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();
./crates/mycad-kernel/src/primitives/cuboid.rs:264:        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen).unwrap();
./crates/mycad-kernel/src/primitives/cuboid.rs:295:        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1).unwrap();
./crates/mycad-kernel/src/primitives/cuboid.rs:296:        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2).unwrap();
./crates/mycad-kernel/src/primitives/cuboid.rs:323:            make_cuboid(0.0, 1.0, 1.0, &mut gen),
./crates/mycad-kernel/src/primitives/cuboid.rs:327:            make_cuboid(1.0, 0.0, 1.0, &mut gen),
./crates/mycad-kernel/src/primitives/cuboid.rs:331:            make_cuboid(1.0, 1.0, 0.0, &mut gen),
./crates/mycad-kernel/src/primitives/cuboid.rs:337:            make_cuboid(-1.0, 1.0, 1.0, &mut gen),
./crates/mycad-kernel/src/primitives/cuboid.rs:341:            make_cuboid(1.0, -1.0, 1.0, &mut gen),
./crates/mycad-kernel/src/primitives/cuboid.rs:345:            make_cuboid(1.0, 1.0, -1.0, &mut gen),
./crates/mycad-kernel/src/primitives/cuboid.rs:351:            make_cuboid(f64::NAN, 1.0, 1.0, &mut gen),
./crates/mycad-kernel/src/primitives/cuboid.rs:355:            make_cuboid(1.0, f64::NAN, 1.0, &mut gen),
./crates/mycad-kernel/src/primitives/cuboid.rs:359:            make_cuboid(1.0, 1.0, f64::NAN, &mut gen),
./crates/mycad-kernel/src/primitives/cuboid.rs:365:            make_cuboid(f64::INFINITY, 1.0, 1.0, &mut gen),
./crates/mycad-kernel/src/primitives/cuboid.rs:369:            make_cuboid(1.0, f64::INFINITY, 1.0, &mut gen),
./crates/mycad-kernel/src/primitives/cuboid.rs:373:            make_cuboid(1.0, 1.0, f64::INFINITY, &mut gen),
./crates/mycad-kernel/src/primitives/cuboid.rs:379:            make_cuboid(f64::NEG_INFINITY, 1.0, 1.0, &mut gen),
./crates/mycad-kernel/src/primitives/cuboid.rs:383:            make_cuboid(1.0, f64::NEG_INFINITY, 1.0, &mut gen),
./crates/mycad-kernel/src/primitives/cuboid.rs:387:            make_cuboid(1.0, 1.0, f64::NEG_INFINITY, &mut gen),
./crates/mycad-kernel/src/primitives/cuboid.rs:396:        let result = make_cuboid(1e-9, 1.0, 1.0, &mut gen);

codex
issues:
  - id: F01
    severity: medium
    file: "crates/mycad-api/src/error.rs"
    line_hint: 26
    finding: "`.mycad` 拡張子のディレクトリや読取不可ファイルは `Document::from_path` で `FormatError::Io` になり、`NotFound` 以外を一律 500 に落としている。設計では user input 起因の path エラーは 400 扱いのはずで、現状は不正入力で Internal Error を返しうる。"
    suggestion: "`handler` 側で `canonical.is_file()` を先に確認するか、`FormatError::Io` の `IsADirectory`/`PermissionDenied` などを `BadRequest` にマップする。"
  - id: F02
    severity: medium
    file: "crates/mycad-api/tests/mesh_api.rs"
    line_hint: 57
    finding: "T01 は成功 body を `serde_json::Value` で見るだけで、T05/T07/T08 も status しか見ていない。これでは計画で合意した `TriangleMesh` deserialization / `InvalidExtension` / `UnsupportedFeature` / `InvalidParameter` 契約が崩れてもテストが通る。"
    suggestion: "T01/T02 は `TriangleMesh` に deserialize し、T05/T07/T08 は error body を parse して期待するメッセージまたは variant 相当を assert する。"
  - id: F03
    severity: low
    file: "crates/mycad-api/tests/mesh_api.rs"
    line_hint: 136
    finding: "新規 fixture YAML (`create_sphere.mycad` / `zero_box.mycad` / `empty_part.mycad`) は API 経由でしか読まれておらず、owner crate (`mycad-format`) 側の golden YAML roundtrip が追加されていない。フォーマット回帰を API テストだけでは固定しきれない。"
    suggestion: "`mycad-format` に fixture 読み込みの `from_path -> to_yaml -> from_yaml -> to_yaml` roundtrip/golden test を追加する。"

verdict: pass
tokens used
202,819
issues:
  - id: F01
    severity: medium
    file: "crates/mycad-api/src/error.rs"
    line_hint: 26
    finding: "`.mycad` 拡張子のディレクトリや読取不可ファイルは `Document::from_path` で `FormatError::Io` になり、`NotFound` 以外を一律 500 に落としている。設計では user input 起因の path エラーは 400 扱いのはずで、現状は不正入力で Internal Error を返しうる。"
    suggestion: "`handler` 側で `canonical.is_file()` を先に確認するか、`FormatError::Io` の `IsADirectory`/`PermissionDenied` などを `BadRequest` にマップする。"
  - id: F02
    severity: medium
    file: "crates/mycad-api/tests/mesh_api.rs"
    line_hint: 57
    finding: "T01 は成功 body を `serde_json::Value` で見るだけで、T05/T07/T08 も status しか見ていない。これでは計画で合意した `TriangleMesh` deserialization / `InvalidExtension` / `UnsupportedFeature` / `InvalidParameter` 契約が崩れてもテストが通る。"
    suggestion: "T01/T02 は `TriangleMesh` に deserialize し、T05/T07/T08 は error body を parse して期待するメッセージまたは variant 相当を assert する。"
  - id: F03
    severity: low
    file: "crates/mycad-api/tests/mesh_api.rs"
    line_hint: 136
    finding: "新規 fixture YAML (`create_sphere.mycad` / `zero_box.mycad` / `empty_part.mycad`) は API 経由でしか読まれておらず、owner crate (`mycad-format`) 側の golden YAML roundtrip が追加されていない。フォーマット回帰を API テストだけでは固定しきれない。"
    suggestion: "`mycad-format` に fixture 読み込みの `from_path -> to_yaml -> from_yaml -> to_yaml` roundtrip/golden test を追加する。"

verdict: pass
