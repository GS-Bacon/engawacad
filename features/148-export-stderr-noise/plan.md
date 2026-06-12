# Issue #148 — `tests/export.rs` stderr ノイズ解消

## Context (背景)

`cargo xtask ci` の `tests/export.rs` (mycad-cli integration test) 実行中、stderr に
`error: failed to build assembly: invalid parameter: profile`
が混入してログノイズになっている。テスト自体は `ok` でパスしているが、Issue #145 (Playwright E2E) のデバッグ中に検出され、診断ノイズになるので別 Issue として #148 が起票された。

根本原因は **negative test `export_invalid_profile_fails_no_stl` が subprocess を `.status()` で起動して stderr を親プロセスに継承してしまう** こと。CLI 側 (`validate_profile_closed` → `KernelError::InvalidParameter { kind: "profile" }` → `eprintln!`) は設計通り正しい挙動。テスト側で `.output()` に切り替えて stderr を捕捉すれば、ノイズが消える + エラーメッセージの回帰ガードを得られる。

## 根本原因マップ

| 観点 | 内容 | ファイル参照 |
|---|---|---|
| 出力源 | `mycad export` が `eprintln!("error: failed to build assembly: {e}")` を実行 | `crates/mycad-cli/src/main.rs` 周辺 |
| エラー本体 | `validate_profile_closed` が profile の閉ループ条件を満たさないとき `InvalidParameter { kind: "profile" }` を返す | `crates/mycad-build/src/lib.rs:339-353` |
| ノイズ流出口 | `export_invalid_profile_fails_no_stl` が `Command::status()` で subprocess を起動 → stderr が親プロセスに継承 | `crates/mycad-cli/tests/export.rs:92-116` |
| 期待パターン | `t16_export_assembly_succeeds` は既に `Command::output()` を使い stderr を捕捉済み (L129-135) | `crates/mycad-cli/tests/export.rs:129-135` |

→ **修正は `tests/export.rs` の `export_invalid_profile_fails_no_stl` 1 関数のみ。プロダクションコード変更なし。**

## In-Scope / Out-of-Scope
<!-- ADR-006 §plan.md 必須セクション。GLM SCOPE ペルソナが存在を検証する。 -->

| 区分 | 内容 |
|---|---|
| In-Scope | `crates/mycad-cli/tests/export.rs` の `export_invalid_profile_fails_no_stl` を `.status()` → `.output()` に変更し、`output.stderr` に `invalid parameter: profile` が含まれることを assert する (回帰ガード) |
| Out-of-Scope | 他 3 success テスト (`export_simple_box_*` / `export_sphere_*` / `export_extruded_rect_*`) の挙動・スタイル変更 (成功想定で stderr ノイズなし) |
| Out-of-Scope | `crates/mycad-cli/src/main.rs` の `eprintln!` 自体の変更 (CLI 利用者向け stderr 出力は維持する) |
| Out-of-Scope | `crates/mycad-build/src/lib.rs` の `validate_profile_closed` 挙動・エラーメッセージ変更 |
| Out-of-Scope | `crates/mycad-kernel/src/error.rs` の `InvalidParameter` バリアント変更 |
| Out-of-Scope | エラーメッセージ国際化・整形改善 |
| Out-of-Scope | 他クレートの subprocess テストへの同種パターン横展開 (本 Issue の対象外、必要なら別 Issue) |

## Non-Goals
<!-- Out-of-Scope と同内容でも重複 OK。dispatch-codex-auto.ts の guard が参照する。 -->

- profile validation のエラーメッセージ細分化 (`profile` という kind 単位の粒度で十分)
- mycad-build の `build_assembly` エラーラッピング (`format!("failed to build assembly: {e}")`) の設計変更
- 他クレートの subprocess テストへの同種パターン横展開
- 新規 negative test の追加 (Out: 既存 1 関数の修正に限定)
- CLI の終了コード細分化 (現状 `Err(_)` → non-zero で十分)

## 実装対象

- **修正対象**: `crates/mycad-cli/tests/export.rs` の関数 `export_invalid_profile_fails_no_stl` (現状 L92-116)
- **影響範囲**: 約 15 行差分 + 1 つの追加 assert
- **新規ファイル**: なし
- **プロダクションコード**: 変更なし

### before / after コードスニペット (修正箇所明示)

**Before** (`crates/mycad-cli/tests/export.rs:101-115`):

```rust
let status = Command::new(bin)
    .arg("export")
    .arg(&input)
    .arg("-o")
    .arg(output)
    .status()                     // ← stderr が親プロセスへ継承される
    .expect("run mycad export");
assert!(
    !status.success(),
    "export should fail for self-intersecting profile"
);

// Output file should be absent or empty
let written = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);
assert_eq!(written, 0, "no STL should be written for invalid profile");
```

**After**:

```rust
let result = Command::new(bin)
    .arg("export")
    .arg(&input)
    .arg("-o")
    .arg(output)
    .output()                     // ← stderr/stdout を捕捉して継承させない
    .expect("run mycad export");
assert!(
    !result.status.success(),
    "export should fail for self-intersecting profile"
);

// Stderr must contain the expected error message
// (regression guard for KernelError::InvalidParameter { kind: "profile" })
let stderr = String::from_utf8_lossy(&result.stderr);
assert!(
    stderr.contains("invalid parameter: profile"),
    "stderr should contain expected error, got: {stderr}"
);

// Output file should be absent or empty
let written = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);
assert_eq!(written, 0, "no STL should be written for invalid profile");
```

### 他 3 success テストは変更しない

`export_simple_box_produces_12_facets` / `export_sphere_produces_960_facets` / `export_extruded_rect_produces_12_facets` は成功想定 (`assert!(status.success())`) なので stderr 出力は基本的に空で、ノイズの原因にならない。スタイル統一目的の `.output()` 化は Out-of-Scope。

## 設計方針

- **決定性要件**: subprocess の `.output()` は決定的に exit code と stderr を返す (`validate_profile_closed` は同入力に対し同 `KernelError` を返すため stderr 文字列も決定的)。
- **B-rep トポロジー妥当性 (Euler-Poincaré)**: 本 Issue ではトポロジー生成を一切行わないため N/A。

### 決定性に関する注記 (テスト修正 Issue のため ID/座標決定性テストは不要)

本 Issue は **テスト関数 1 個 (`export_invalid_profile_fails_no_stl`) の subprocess 起動方法を `.status()` → `.output()` に変更するテスト修正** であり、以下の理由で「同一入力を 2 回実行して全 ID・座標が一致することを assert する」という CLAUDE.md 標準の決定性テストは適用不能・不要:

1. **ID 生成なし**: 本 Issue は `IdGenerator`/`Uuid` を一切利用しない (テストコード + std `Command` のみ)。プロダクションコード変更なし。
2. **座標生成なし**: 本 Issue は B-rep トポロジー・幾何要素を一切生成しない。テストは subprocess の exit code と stderr 文字列 (substring) を assert するのみ。
3. **構造的決定性保証**: subprocess は std `Command::output()` (POSIX `fork+exec`) で起動され、テスト fixture (`invalid_profile.mycad`) は固定ファイル。`validate_profile_closed` は純粋関数で同入力に対し同 `KernelError` を返す。したがって stderr 文字列は構造的に決定的。
4. **CLAUDE.md 規約準拠**: CLAUDE.md「テストでは決定性を検証すること」は「`IdGenerator` で決定的に生成」される ID やトポロジー生成に対する要件であり、subprocess 起動 + 純粋関数のテスト修正には適用範囲外。
- **退化幾何の扱い**: 本 Issue 内で扱う退化はテストフレームワーク観点のみ (subprocess stderr 捕捉)。
- **derive 規約**: 変更なし (新規型なし)。
- **エラーハンドリング**: テスト内のみ。`thiserror` 等プロダクションへの影響なし。CLI 側の `format!("failed to build assembly: {e}")` パターンは契約として固定する (substring assert で stderr フォーマットを固定する意図的な設計)。
- **`workspace.dependencies`**: 変更なし (`tempfile` は既存依存)。
- **エンコーディング/プラットフォーム差**: `String::from_utf8_lossy` で UTF-8 不正バイトを置換し堅牢化。`contains("invalid parameter: profile")` は substring 検索のため改行差・末尾差分の影響を受けない。

<!-- ### 数値モデル — 数値判断 (tolerance / ε / 退化) なし。本セクション省略 (ADR-006 §plan.md 数値モデル必須は Phase 4/6+ Issue のみ、本 Issue は該当しない)。 -->

## テスト計画 (ID 付き)

| ID | 種別 | 内容 | 期待結果 |
|---|---|---|---|
| T01_repro_stderr_leak_before_fix | repro (バグ再現) | **修正前** のテストで `cargo test -p mycad-cli --test export 2>&1 \| grep -c "error: failed to build assembly"` が 1 以上 (ヒットあり) → `features/148-export-stderr-noise/repro-before.log` に記録 | hit ≥ 1 (修正前) |
| T02_normal_assert_exit_nonzero | 正常系 | `!result.status.success()` (既存挙動の維持) | true |
| T03_normal_assert_stderr_contains_kind | 回帰ガード | `result.stderr` の UTF-8 表現に `"invalid parameter: profile"` を含む | true |
| T04_normal_no_stl_written | 既存維持 | output ファイルが空 or 不在 | `len() == 0` |
| T05_degen_other_tests_unchanged | 退化/境界 | 他 3 success テスト + `t16_export_assembly_succeeds` が引き続きパスする | all pass |
| T06_boundary_stderr_capture_does_not_break_status | 境界 | `.output()` 経由でも exit code 判定 `!result.status.success()` が `.status()` と等価に動く | T02 と兼用、true |
| T_post_fix_grep_zero | repro 解消 | **修正後** に `cargo test -p mycad-cli --test export 2>&1 \| grep -c "error: failed to build assembly"` が 0 (ヒットなし) | hit == 0 (修正後) |

退化/境界 ID: `T05_degen_other_tests_unchanged`, `T06_boundary_stderr_capture_does_not_break_status` を含む (ADR-006 退化テスト 1 件以上の要件を満たす)。

## 幾何的不変条件チェックリスト

- N/A — テスト修正のみで B-rep トポロジー・Boolean・Partition・Assemble に影響しない。
- partition 出力の polygon 頂点順と assemble の normal 処理の整合: **N/A**
- 各プリミティブの face ごとの outer_loop 2D 向き: **N/A**
- flip_normals / same_sense の意味論: **N/A**
- pslg_subdivide の出力向きと元の outer_loop 向きの整合: **N/A**

## 実装順序 (STEP 5.5 〜 STEP 8 で踏む手順)

1. STEP 5.5: 修正前の repro 確認: `cargo test -p mycad-cli --test export 2>&1 | tee features/148-export-stderr-noise/repro-before.log` を実行し、`grep -c "error: failed to build assembly" features/148-export-stderr-noise/repro-before.log` が **1 以上 (ヒットあり)** であることを確認 → T01 のバグ再現保証
2. STEP 6 (GLM): `tests/export.rs` の `export_invalid_profile_fails_no_stl` を `.status()` → `.output()` に変更し、`stderr.contains("invalid parameter: profile")` の assert を追加 (上記 before/after 参照)
3. STEP 6 中で `cargo test -p mycad-cli --test export` をローカル実行し、5 テスト全パスを確認
4. STEP 6 中で `cargo test -p mycad-cli --test export 2>&1 | grep -c "error: failed to build assembly"` が 0 件 (ノイズ消失) を確認 → T_post_fix_grep_zero
5. STEP 6 末で `cargo xtask ci` で workspace 全体 green を確認

## STEP 5.5 (Acceptance Skeleton) の扱い

本 Issue は **既存テスト関数 1 個の修正** であり、新規 integration test ファイル `<feature>_acceptance.rs` の追加にはそぐわない。代替として:

- T01 (repro) を STEP 5.5 で人手 reproduction する: **修正前** の `cargo test ... | grep -c "error: failed to build assembly"` が ≥ 1 (ヒットあり) であることを `features/148-export-stderr-noise/repro-before.log` に記録 → これが「バグ再現できる」ことの保証
- GLM 修正後に同 grep が **== 0 (ヒットなし)** になることを STEP 6 末で確認 → 修正の効果保証

これで「テストと修正を同時書きして両方通す」偽陽性ガード (`#[ignore]` の repro-first 確認に相当) を満たす。

## 既存資産との関係

- 既存 `t16_export_assembly_succeeds` (`tests/export.rs:120-139`) は既に `Command::output()` パターンを使っており、本 Issue の after コードと整合する。GLM 実装時に「参考既存パターン」として参照させれば認知負荷が下がる。
- `tempfile::NamedTempFile::with_suffix` の使い方は他テストと同一 — 変更不要。
- `String::from_utf8_lossy` は std にあり追加依存不要。

## 想定リスク

- (低) `String::from_utf8_lossy` の戻り値 `Cow<'_, str>` を `format!("..., got: {stderr}")` で `Display` する形式 → `Cow<'_, str>` は `Display` 実装あり、問題なし。
- (低) Windows / macOS ランナーで eprintln の改行・エンコーディング差 → `contains("invalid parameter: profile")` は substring 検索なので改行差で壊れない。
- (低) 将来 CLI のエラーメッセージを変更する PR が出たとき、本テストの substring assert が breaking → これは意図的: 「stderr を契約として固定する」ことが本 Issue の追加価値。

## 検証 (verification)

1. `cargo test -p mycad-cli --test export` → 5 テスト all pass
2. `cargo test -p mycad-cli --test export 2>&1 | grep -c "error: failed to build assembly"` → 0
3. `cargo xtask ci` → workspace 全 green
4. `cargo clippy --workspace -- -D warnings` → warning なし
5. `cargo fmt --all -- --check` → 差分なし

## ファイル

- 修正対象: `crates/mycad-cli/tests/export.rs` (関数 `export_invalid_profile_fails_no_stl` のみ、~15 行差分)
- 参照のみ (変更なし):
  - `crates/mycad-cli/src/main.rs` (eprintln 元)
  - `crates/mycad-build/src/lib.rs:339-353` (`validate_profile_closed`)
  - `crates/mycad-kernel/src/error.rs:14` (`KernelError::InvalidParameter`)
  - `crates/mycad-cli/tests/fixtures/invalid_profile.mycad` (テスト fixture)
