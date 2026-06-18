## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| proptest: `make_cuboid` 入力域の決定性 property を 1 件追加 (engawa-kernel/tests/) | proptest を Boolean / Tessellation 不変量に拡張 (Phase 10+) |
| criterion: 既存 `engawa-kernel/benches/tessellation.rs` を維持し、main の baseline JSON を `bench-results/baseline-phase9.json` に commit | bench 結果の CI 自動計測・回帰検出 (将来 Phase) |
| cargo-fuzz: `crates/engawa-format/fuzz/` ディレクトリを新設し、`from_yaml` parser の fuzz target を 1 件追加 | fuzz の corpus seed 提供・CI 実行 |
| cargo-llvm-cov: `docs/QUALITY_TOOLS.md` (新規) に実行手順を 1 章記述 | llvm-cov の CI 統合 (Phase 12 Refactor Pass で改めて) |
| Playwright: `web/tests/smoke.spec.ts` を 1 件追加 (viewer hello-world) | Playwright を `cargo xtask ci` に統合する変更 (既に部分統合済、追加 e2e は Phase 21+) |
| すべての setup について `cargo xtask ci` を red にしないこと | cargo-fuzz の `cargo xtask ci` への統合 (manual 実行のみ) |

## Non-Goals

- 5 ツールの全機能を導入すること (Issue は "最小 setup" と明記)
- bench / fuzz / coverage を CI に組み込むこと (`cargo xtask ci` には組み込まず手動実行を維持、と Issue で明示)
- proptest cases 数の調整 (デフォルト 256 で OK)
- 既存 boolean_proptest.rs / tessellation.rs の改変

## 実装対象

<!-- Issue: #243 -->
<!-- 影響範囲: crates/engawa-kernel/tests/, crates/engawa-format/fuzz/ (新規), docs/, web/tests/, bench-results/ (新規), Cargo.toml -->

### 新規ファイル

1. **`crates/engawa-kernel/tests/cuboid_determinism_proptest.rs`** — proptest 1 件
   - `t_proptest_make_cuboid_determinism`: w/h/d in 0.01..1000.0 で同一入力 2 回 → Debug 表現一致
   - default proptest cases 256

2. **`bench-results/baseline-phase9.json`** — criterion baseline (Phase 9 時点 bench スナップショット)
   - 生成手順: `cargo bench -p engawa-kernel --bench tessellation -- --save-baseline phase9` 実行後、`target/criterion/.../estimates.json` を 1 件抽出して commit
   - 内容最小限 (median + estimate のみ) で十分

3. **`crates/engawa-format/fuzz/Cargo.toml`** — cargo-fuzz スケルトン
   - `libfuzzer-sys = "0.4"` + `engawa-format = { path = ".." }`
   - 独立 workspace (cargo-fuzz 慣例)

4. **`crates/engawa-format/fuzz/fuzz_targets/from_yaml.rs`** — fuzz target
   - `fuzz_target!(|data: &[u8]| { ... Document::from_yaml(s) ... })`

5. **`docs/QUALITY_TOOLS.md`** — 5 ツール実行手順 (cargo-llvm-cov + 他 4 つの manual 実行 cmd 集)

6. **`web/tests/smoke.spec.ts`** — Playwright minimal smoke (Phase 21+ viewer 用 hello-world)

### 既存ファイル変更

- **`Cargo.toml` (workspace)**: `[workspace]` に `exclude = ["crates/engawa-format/fuzz"]` を追加 (cargo-fuzz dir は workspace 対象外、慣例に従う)

  **before**:
  ```toml
  [workspace]
  members = [
      "crates/engawa-kernel",
      ...
  ]
  resolver = "2"
  ```
  **after**:
  ```toml
  [workspace]
  members = [
      "crates/engawa-kernel",
      ...
  ]
  exclude = ["crates/engawa-format/fuzz"]
  resolver = "2"
  ```

- **`.gitignore`**: `bench-results/baseline-phase9.json` を track するため変更不要 (既存ルールで干渉なし)

### 確認事項

- `cargo xtask ci` を実行して red にしないこと (proptest 1 件は通常テストとして走る、benches は `cargo bench` のみで実行、fuzz は workspace 外で `cargo build` 影響なし、Playwright smoke は既存 viewer 系と同 dir で `cargo xtask ci` 経由実行 OK)

## 設計方針

- **5 ツール並列導入の正当性**: ADR-015 §4 で「Phase 9 で 5 ツールまとめて導入 (Option B 採用)」と決定済み。本 Issue はその実装。
- **CI 影響ゼロ**: bench / fuzz / coverage は manual 実行のみ、proptest 1 件は既存 test framework に乗せる、Playwright smoke は `web/tests/` に置けば既存 `cargo xtask ci` の Playwright stage で実行される。
- **決定性**: proptest test は `make_cuboid` の同一入力 → 同一 Debug 表現で IdGenerator の決定性を検証する。
- **derive 規約**: 該当なし (テスト / bench / fuzz / docs / Playwright のみ、公開型変更なし)
- **エラーハンドリング**: 該当なし
- **workspace.dependencies**: `proptest` は既に workspace.dependencies にあるので利用するだけ。`criterion` は engawa-kernel/Cargo.toml に dev-dependency 既に追加済。`libfuzzer-sys` は fuzz dir 独立 Cargo.toml のため workspace 対象外で OK。

## テスト計画 (ID 付き)

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 (proptest) | `make_cuboid` を同一入力で 2 回呼んで Debug 表現一致 | `prop_assert_eq!` 全成功 (256 cases) |
| T02 | smoke | `cargo bench -p engawa-kernel --bench tessellation` が compile + 1 回完走 | exit 0 (CI には含まれないが docs/QUALITY_TOOLS.md で手順記載) |
| T03 | smoke | `cd crates/engawa-format/fuzz && cargo build` が compile する | exit 0 — **manual 実行のみ、CI 検証 N/A** |
| T04 | smoke | `cargo llvm-cov --workspace --no-report` が exit 0 | manual 実行のみ |
| T_BOUNDARY_proptest_minimal_dim | 境界 | proptest 範囲下限 0.01 でも make_cuboid が panic しない | `prop_assert!` |
| T_DEG_zero_dim_excluded | 退化 | proptest 範囲が 0.01 から開始しているため、0 寸法は範囲外 (退化入力を意図的に排除しコメントで明示) | proptest range guard で reach せず |
| T_PLAYWRIGHT_smoke | smoke | `web/tests/smoke.spec.ts` で page.goto + title 取得が成功 | `expect(page).toHaveTitle(/.+/)` 成功 |
| T_CI_no_regression | 回帰 | `cargo xtask ci` が green を維持 | exit 0 |

### 退化/境界ケース ID チェック

- 退化系: `T_DEG_zero_dim_excluded`
- 境界系: `T_BOUNDARY_proptest_minimal_dim`

両系列とも 1 件以上を満たし STEP 5.5 grep pass。

## 幾何的不変条件チェックリスト

本 Issue は品質基盤 setup でカーネルの不変条件には触れない → すべて **N/A**。

- [ ] partition 出力の polygon 頂点順と assemble の normal 処理 → N/A
- [ ] 各プリミティブの face ごとの outer_loop 2D 向き → N/A
- [ ] flip_normals / same_sense の意味論 → N/A
- [ ] pslg_subdivide の出力向き → N/A
