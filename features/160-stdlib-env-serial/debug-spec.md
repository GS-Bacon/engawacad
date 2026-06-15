# Debug Spec: Codex round 1 指摘への対応

Codex 独立技術ゲート (STEP 7.5 round 1) が `verdict: fail` / `blocking: 1` (high 1 / medium 1)。
本 debug-spec は GLM (mode=test) への修正指示。

## F01 (high) 対応方針: **部分採用 (docs only)**

### Codex の指摘 (該当箇所: `crates/engawa-build/tests/stdlib_env_race_acceptance.rs:16`)

> 新規メタテストは `ENGAWA_STDLIB_PATH` を変更して `build_assembly` を順に呼ぶだけで、別 integration test binary との同時実行や lock 待ちを強制していない。`#[file_serial(engawa_stdlib_path)]` の cross-binary 効力が壊れても、スケジューラ次第ではそのまま緑で通るため、Issue #160 の回帰保護網として決定的ではない。

### Claude 判定: 部分採用 (docstring 強化のみ、実装拡張はしない)

- **採用部分**: メタテストの「役割」が現状の docstring (l.1-5) では曖昧。明示的に「これは lock 違反の決定的検出ではない / 同 lock を共有する binary 数を 2 以上に保つ Static な保証」と再記述する。
- **rejection 部分**: sentinel file / child process / 待機ポート等による決定的 race 検出は **本 Issue のスコープ外**:
  - 既存先例 `crates/engawa-api/tests/startup_log_acceptance.rs:60` (Issue #153 で導入) も port 競合の決定的検出はせず `file_serial` 適用のみ。本 Issue はその先例に追随する形。
  - 決定的 race 検出は test 戦略全体に関わる (env var だけでなく port / temp file 等も対象になる) ため、独立した別 Issue として中期対処 (`BuildOptions { stdlib_root: PathBuf }` 注入による env 依存自体の根絶) と同水準のスコープに切り出すべき。
  - 本 Issue の plan.md §設計方針「メタテスト binary を別に置く理由」で「**lock 共有 binary 数 ≥ 2** が保証されることそのものが回帰保護の主目的」と明示済み。

### 修正対象

`crates/engawa-build/tests/stdlib_env_race_acceptance.rs` の **モジュール冒頭 docstring** (現 l.1-5) を以下に置き換える:

```rust
//! Acceptance tests for #160: cross-binary protection of ENGAWA_STDLIB_PATH via `#[file_serial(engawa_stdlib_path)]`.
//!
//! ## このテストの役割 (重要)
//!
//! 本ファイルは「lock 違反の決定的検出」ではなく、「同一 lock `engawa_stdlib_path`
//! を共有する integration test binary 数を **2 以上に保つ Static な保証**」を提供する。
//!
//! - 既存 `assembly_acceptance.rs` の `t02` / `t08` が lock を共有する 1 つ目の binary。
//! - 本ファイル `stdlib_env_race_acceptance.rs` が 2 つ目の binary。
//! - `serial_test = { features = ["file_locks"] }` の cross-binary lock が将来 regress した瞬間、
//!   両 binary が並列で `ENGAWA_STDLIB_PATH` を mutate → resolve_stdlib_root() が race し、
//!   t02/t08/t03/t04 のいずれかが workspace test で FAILED に転ぶ。
//!
//! 「lock が機能している」ことの決定的 (sentinel file 同期等) な検出は本 Issue の
//! スコープ外であり、中期対処 (BuildOptions による env 依存の根絶) の側で扱う。
```

実装 (テスト関数本体) には触らない。

## F02 (medium) 対応方針: **採用**

### Codex の指摘 (該当箇所: `crates/engawa-build/tests/stdlib_env_race_acceptance.rs:68`)

> t04 の Phase 1/3 は `is_err()` しか見ておらず、計画で要求している `stdlib` 解決失敗 (`KernelError::ReferenceResolution` / not-found 系) を検証していない。`build_assembly` が別理由で失敗してもテストが通るため、ENV の remove→set→remove 境界回帰を見逃す。

### 修正対象

`t04_meta_boundary_remove_set_remove_is_isolated` の `r1.is_err()` / `r3.is_err()` を、既存 `assembly_acceptance::t08_stdlib_root_not_set_error` (`crates/engawa-build/tests/assembly_acceptance.rs:317-327`) と同じ pattern で:

```rust
// 既存 t08 の pattern (参考):
match result {
    Err(KernelError::ReferenceResolution { reason, .. }) => {
        assert!(
            reason.contains("stdlib")
                || reason.contains("No such file")
                || reason.contains("not found"),
            "unexpected reason: {reason}"
        );
    }
    other => panic!("expected ReferenceResolution, got {other:?}"),
}
```

t04 の 3 phase それぞれを以下のように変更:

- **Phase 1 (env unset → エラー期待)**: 上記 match pattern を適用し `KernelError::ReferenceResolution` であることまで assert。reason は `stdlib` / `not found` / `No such file` のいずれかを含む。
- **Phase 2 (env set → Ok 期待)**: 現状の `assert!(r2.is_ok())` のままで OK (中身まで assert する必要なし)。
- **Phase 3 (env 再 remove → エラー期待)**: Phase 1 と同じ match pattern を適用。

`use engawa_kernel::error::KernelError;` を必要なら追加。

## やらないこと (確認)

- メタテスト関数本体への構造変更 (sentinel file / child process / 同期 primitives 等の追加)
- `assembly_acceptance.rs` への変更 (STEP 6 で完了済み、Codex 指摘対象外)
- `Cargo.toml` / production code (`crates/engawa-build/src/**`) への変更

## 修正後の検証

1. `cargo test -p engawa-build --test stdlib_env_race_acceptance` → 2 件 PASS
2. `cargo test --workspace` → 全 binary PASS
3. `cargo clippy --workspace -- -D warnings` → 警告ゼロ
4. `cargo fmt --all -- --check` → フォーマット OK

修正完了後、STEP 7.5-A から Codex 再レビューを行う。
