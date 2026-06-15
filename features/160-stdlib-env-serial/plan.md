# Plan: Issue #160 — assembly_acceptance の ENGAWA_STDLIB_PATH race を `serial_test::file_serial` で解消

## Context

`cargo clean` 後の `cargo test --workspace` で `engawa-build::assembly_acceptance::t02_stdlib_reference_resolved` が確実に FAILED する。
- 単独 `cargo test -p engawa-build --test assembly_acceptance` は全 16 件 OK。**同 binary 内の並列実行時のみ発生**。
- 根本原因: `assembly_acceptance.rs` の `t02` と `t08` がプロセスグローバルな env var `ENGAWA_STDLIB_PATH` を `set_var` / `remove_var` し、Rust の integration test が default で binary 内並列実行 (`--test-threads` 指定なし) のため race。
- `resolve_stdlib_root()` (`crates/engawa-build/src/lib.rs:520-535`) が env を直読み (`std::env::var`) のため、テスト側で env を mutate する限り race は不可避。
- 影響: `cargo xtask ci` (内部で `cargo test --workspace` を呼ぶ — `crates/xtask/src/main.rs:811`) が常に赤。engawa-build を一切触らない他 Issue (#158 等) もブロック。

本 Issue は **短期対処**: `serial_test::file_serial` でテスト関数を直列化する。中期対処 (`resolve_stdlib_root` を `BuildOptions { stdlib_root: PathBuf }` 注入に refactor) は Issue 本文どおり別 Issue へ送る。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `crates/engawa-build/Cargo.toml` の `[dev-dependencies]` に `serial_test = { workspace = true }` を追加 | `resolve_stdlib_root()` を env 直読みから引数注入 (`BuildOptions { stdlib_root: PathBuf }`) へ refactor する設計変更 (中期対処、別 Issue で対応) |
| `assembly_acceptance.rs` 冒頭に `use serial_test::file_serial;` を追加 | `cargo xtask ci` 側で `--test-threads=1` を強制する workaround |
| `t02_stdlib_reference_resolved` と `t08_stdlib_root_not_set_error` に `#[file_serial(engawa_stdlib_path)]` を付与 | 他の env var (将来の `ENGAWA_*` 系) への横展開 |
| 新規 `crates/engawa-build/tests/stdlib_env_race_acceptance.rs` にメタテスト 2 件 (t03/t04) を skeleton で配置し STEP 6.6 で中身を実装 | `set_stdlib_env` / `restore_stdlib_env` ヘルパー関数の API 改変 |
| `cargo test --workspace` を 3 回連続で全 green になることを検証 | engawa-api の serial_test 適用範囲拡張 (本 Issue は engawa-build のみ) |

## Non-Goals

- `engawa-build` 側プロダクションコード (`src/**`) の変更は行わない。env race の根治は別 Issue (中期対処) のスコープ。
- 新規プロダクション API・新型・新フィーチャの追加なし。
- `serial_test::serial` (in-process mutex) は使わず file_serial (cross-binary lock) に統一する (先例 `crates/engawa-api/tests/startup_log_acceptance.rs:60` の `#[file_serial(engawa_api_port_7878)]` と命名規約を揃える)。
- 該当なし: `examples_smoke.rs` 等他 integration test binary への file_serial 横展開 (本 Issue で env を touch する binary は `assembly_acceptance.rs` + 新規 `stdlib_env_race_acceptance.rs` のみ — grep 全件検証済み)。

## 実装対象

- Issue: #160
- 影響クレート/ファイル:
  - `crates/engawa-build/Cargo.toml` (1 行追加: `serial_test = { workspace = true }`)
  - `crates/engawa-build/tests/assembly_acceptance.rs` (use 1 行 + attribute 2 箇所)
  - `crates/engawa-build/tests/stdlib_env_race_acceptance.rs` (**新規ファイル**)
- 変更する型・関数のシグネチャ: なし (production コードに type/関数追加なし)
- `engawa-build/src/**` には一切手を入れない

### 既存ファイル修正の before/after

#### A. `crates/engawa-build/Cargo.toml` (dev-dep 追加)

```toml
# before
[dev-dependencies]
engawa-format = { workspace = true }
serde_json = { workspace = true }
serde_yaml = { workspace = true }
tempfile = { workspace = true }

# after
[dev-dependencies]
engawa-format = { workspace = true }
serde_json = { workspace = true }
serde_yaml = { workspace = true }
serial_test = { workspace = true }
tempfile = { workspace = true }
```

#### B. `crates/engawa-build/tests/assembly_acceptance.rs` use 追加 (l.1-9)

```rust
// before
use engawa_build::build_assembly;
use engawa_format::component::{Component, ComponentRef};
use engawa_format::document::Document;
use engawa_format::feature::Feature;
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::error::KernelError;
use std::fs;
use std::path::{Path, PathBuf};

// after (serial_test を std::* の前に挿入)
use engawa_build::build_assembly;
use engawa_format::component::{Component, ComponentRef};
use engawa_format::document::Document;
use engawa_format::feature::Feature;
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::error::KernelError;
use serial_test::file_serial;
use std::fs;
use std::path::{Path, PathBuf};
```

#### C. `assembly_acceptance.rs` の `t02` attribute 付与 (l.115-116 想定)

```rust
// before
#[test]
fn t02_stdlib_reference_resolved() {

// after
#[test]
#[file_serial(engawa_stdlib_path)]
fn t02_stdlib_reference_resolved() {
```

#### D. `assembly_acceptance.rs` の `t08` attribute 付与 (l.296-297 想定)

```rust
// before
#[test]
fn t08_stdlib_root_not_set_error() {

// after
#[test]
#[file_serial(engawa_stdlib_path)]
fn t08_stdlib_root_not_set_error() {
```

### 新規ファイル: `crates/engawa-build/tests/stdlib_env_race_acceptance.rs`

STEP 5.5 で Claude が skeleton を `#[ignore]` 付きで Write:

```rust
//! Acceptance tests for #160: cross-binary protection of ENGAWA_STDLIB_PATH via `#[file_serial(engawa_stdlib_path)]`.
//!
//! 既存 assembly_acceptance.rs と同じ lock を別 binary で参加させ、`file_serial` の
//! cross-binary lock (workspace.dependencies の `features = ["file_locks"]`) が
//! 壊れた瞬間に CI が赤くなる回帰保護網。

use serial_test::file_serial;

#[test]
#[ignore = "STEP 6 で実装後に解除"]
#[file_serial(engawa_stdlib_path)]
fn t03_meta_lock_protects_concurrent_resolve() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
#[file_serial(engawa_stdlib_path)]
fn t04_meta_boundary_remove_set_remove_is_isolated() {
    todo!()
}
```

STEP 6.6 (GLM, mode=test) で実装する中身:
- `t03_meta_lock_protects_concurrent_resolve`: 一時 stdlib ディレクトリを作り `ENGAWA_STDLIB_PATH` を set → `build_assembly` (簡単な `stdlib://` 参照を含む `Document`) で解決成功を assert → env restore (元値が None なら remove)
- `t04_meta_boundary_remove_set_remove_is_isolated`: 退化境界遷移 — env を `remove` → `build_assembly` が `StdlibRootNotFound` 系エラーを返すこと → 一時 path に `set` → 解決成功 → 再度 `remove` → 再びエラー、を順に assert

## 設計方針

### lock 命名規約

先例 `engawa_api_port_7878` (`crates/engawa-api/tests/startup_log_acceptance.rs:60`) に倣い、`engawa_<resource>` 形式で env var 名そのものを resource として `engawa_stdlib_path` を採用する。Issue 本文の提案 `stdlib_env` ではなく env var 名そのものを resource にすることで「何の lock か」がコメントなしで明示される。

### 保護範囲の確定

現在 `ENGAWA_STDLIB_PATH` を mutate しているテストは grep 全件で:
- `assembly_acceptance.rs:43-56` (`set_stdlib_env` / `restore_stdlib_env` ヘルパー定義)
- `assembly_acceptance.rs:126-129` (t02 がヘルパー経由で set/restore)
- `assembly_acceptance.rs:299-312` (t08 が手書きで `remove_var` → `set_var` → `set_var/remove_var` 復元)

`assembly_acceptance.rs` 外で `ENGAWA_STDLIB_PATH` を mutate するテストは存在しない (Explore 検証済み)。両テスト (t02, t08) に同 lock を付与すれば「片方付け忘れによる再発」を防げる。

### メタテスト binary を別に置く理由

同 lock を共有する binary が 1 つだけだと、「実は serial 化が壊れていても 1 binary 内に並列ペアが存在しないため気付けない」回帰盲点が残る。`stdlib_env_race_acceptance.rs` を独立 integration test binary として置き同 lock を共有させることで:
- `file_serial` の cross-binary lock 動作 (`workspace.dependencies` の `serial_test = { features = ["file_locks"] }`) が壊れた瞬間に CI が赤くなる
- 将来 t02/t08 のいずれかが attribute を失っても、新規 binary が別 binary として並列実行されることで race を露呈させる

### 決定性

production code path は触らないため決定性要件は不変。変更は test harness の並列度制御のみ。`IdGenerator` 等の決定的 ID 生成や `resolve_stdlib_root` 自体のロジックは未変更。

### derive 規約 / エラーハンドリング / workspace.dependencies

- derive 規約: 該当なし (新型を導入しない)
- エラーハンドリング: 該当なし (新エラー型なし)
- workspace.dependencies: `serial_test = { version = "3", features = ["file_locks"] }` は既存 (root `Cargo.toml:19-59`)。各 crate は `{ workspace = true }` で参照する規約に従う

### 数値モデル

N/A (test infrastructure 修正で tolerance/ε に触れない)。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01_existing_t02_attribute | バグ再現/正常系 | 既存 `assembly_acceptance::t02_stdlib_reference_resolved` (attribute 付与後の `cargo test --workspace`) | 修正前: FAILED / 修正後: PASS |
| T02_existing_t08_attribute | バグ再現/正常系 | 既存 `assembly_acceptance::t08_stdlib_root_not_set_error` (同上) | 修正前: t02 と相関で FAILED / 修正後: PASS |
| T03_meta_lock_protects_concurrent_resolve | 正常系 (回帰保護) | `stdlib_env_race_acceptance::t03` で env set → `build_assembly` の stdlib 参照解決成功 | PASS (resolved root が temp path と一致) |
| T04_meta_boundary_remove_set_remove_is_isolated | **退化/境界** | `stdlib_env_race_acceptance::t04` で env を remove→set→remove のシーケンス | `Err(StdlibRoot 系)` → `Ok` → `Err(StdlibRoot 系)` 遷移 |
| T_REGRESSION_workspace_3runs | 統合検証 (Issue 本文の検証手順) | `cargo test --workspace` を **3 回連続実行** | 3 回とも全 green (FAILED ゼロ) |

退化/境界 ID grep チェック (`_degen_|_boundary_`): `T04_meta_boundary_remove_set_remove_is_isolated` が `_boundary_` を含むため通過 ✓

### バグ修正 Issue の「再現ファースト」位置付け

- **再現テスト = 既存 `assembly_acceptance::t02_stdlib_reference_resolved`** (新規追加せず既存資産を再利用)
- **再現性に関する注記** (STEP 5.5 実測ログ): `cargo clean -p engawa-build` 後 + `cargo test --workspace` で本セッション内では **t02 FAILED を再現できなかった** (全 binary 緑)。Rust の `std::env::set_var` は内部 lock を持つため race 窓は時間的に狭く、現マシン scheduler 状態では衝突しない。Issue 報告者の環境では Issue 本文どおり「確実に再現」する条件 (CPU 数 / load) があった可能性が高い。
- **race 窓自体は構造的に存続している** (t02 と t08 が prozessglobal env を mutate するため): 修正は予防保護として実施し、`#[file_serial(engawa_stdlib_path)]` で race 窓を機械的に塞ぐことに意義がある。
- 新規 t03/t04 はバグ再現ではなく cross-binary 回帰保護網 (上記§設計方針) として位置付ける

### 類似ケース未カバー

- 他テストで env var を触る場所: grep `std::env::set_var\|std::env::remove_var` を `crates/engawa-build` 全体で実行済み、`assembly_acceptance.rs` 内 t02/t08 のみで他なし
- 他の env var への横展開: 本 Issue では `ENGAWA_STDLIB_PATH` のみ。将来 `ENGAWA_*` 系 env var が増えた場合は中期対処 (`BuildOptions` 注入) で根治する想定

## 幾何的不変条件チェックリスト

- [N/A] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか
- [N/A] 各プリミティブの face ごとの outer_loop 2D 向き（CW/CCW）が文書化されているか
- [N/A] flip_normals / same_sense の意味論が明確か（頂点順を変えるか vs 法線だけ変えるか）
- [N/A] pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか

(本 Issue は test infrastructure 修正で Boolean/Partition/Assemble いずれにも該当しない)

## 実装順序

1. **STEP 5.5 (Claude)**: skeleton ファイル `stdlib_env_race_acceptance.rs` を Write (`#[ignore]` 付き)。`cargo test --workspace 2>&1 | grep -E '(FAILED|t02_stdlib)'` で「修正前は FAILED」を確認 (バグ再現確認)
2. **STEP 6 (GLM, mode=core)**: `Cargo.toml` に dev-dep 追加 → `assembly_acceptance.rs` に `use` 文 + t02/t08 へ attribute 付与 → `cargo test --workspace` を 3 回連続で全 green 確認
3. **STEP 6.5 (Claude)**: `test-spec.md` を作成 (実装差分から追加すべきテスト = `stdlib_env_race_acceptance.rs` の t03/t04 中身)。期待値乖離チェック (production code 変更なし想定)
4. **STEP 6.6 (GLM, mode=test)**: `stdlib_env_race_acceptance.rs` の t03/t04 中身を実装、`#[ignore]` 解除、green 確認
5. **STEP 7 / 7.5 / 8**: 標準フロー (Codex 独立技術ゲート STEP 7.5 は batch:kernel のため保持)

## 検証手順

1. `cargo build --workspace` — コンパイルパス
2. `cargo test -p engawa-build --test assembly_acceptance` — 単独で全 16 件 green (元から OK)
3. `cargo test -p engawa-build --test stdlib_env_race_acceptance` — 新規メタテスト 2 件 green
4. **`cargo test --workspace` を 3 回連続実行** — 全 green (= race 解消の決定的検証)
5. `cargo xtask ci` — fmt → clippy → test → build 全 green
6. `cargo clippy --workspace -- -D warnings` — 警告ゼロ
7. `cargo fmt --all -- --check` — フォーマット OK

## Workflow 補足

- **Slug**: `stdlib-env-serial`
- **Phase / Milestone**: なし (engawa-build の test infrastructure 修正、Phase 横断 unblock 目的)
- **ラベル**: `bug` + `batch:kernel`
- **STEP 6 GLM dispatch 対象**: Cargo.toml dev-dep 追加 + assembly_acceptance.rs (`use` + t02/t08 attribute)
- **STEP 6.6 GLM dispatch 対象**: stdlib_env_race_acceptance.rs の t03/t04 中身
- **STEP 7.5 Codex 独立技術ゲート**: `batch:kernel` のため **保持** (skill ルール: kernel リスクが高いため keep_codex_gate)
- **マージ commit message (案)**: `fix(engawa-build): serial_test::file_serial で assembly_acceptance の env var race を解消 (Closes #160)`
- **マージ先**: `claude/add-claude-guidelines-BKKtD` (本リポジトリの effective default branch、`main` は不在)
- **commit Co-Authored-By**: `Claude Opus 4.7 <noreply@anthropic.com>`
