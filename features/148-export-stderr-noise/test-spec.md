# Test Specification — Issue #148

## サマリ

STEP 6 (GLM コア実装) で `crates/mycad-cli/tests/export.rs:export_invalid_profile_fails_no_stl` を `.status()` → `.output()` に変更し、`stderr.contains("invalid parameter: profile")` の回帰ガード assert を追加した。これにより plan の **T02 / T03 / T04** がテスト関数本体に内包され、**T01 / T05 / T06 / T_post_fix_grep_zero** も同時に充足されている。

**結論**: 追加の test 関数は **不要**。STEP 6.6 (GLM テスト実装) は no-op パスとして扱う (mode=test dispatch は走るが、GLM は新規テストを書く必要がないと判断するはず)。以下に充足根拠を ID 別に示す。

## plan テスト計画 ID × 実装充足マップ

| ID | 種別 | plan 期待値 | 実装での充足箇所 | 充足状態 |
|----|------|------|------|------|
| T01_repro_stderr_leak_before_fix | repro (バグ再現) | 修正前 `grep -c "error: failed to build assembly"` が ≥ 1 | `features/148-export-stderr-noise/repro-before.log` (修正前 cargo test 出力をキャプチャ済み、grep ヒット = 1) | ✓ 充足 (artifact 記録) |
| T02_normal_assert_exit_nonzero | 正常系 | `!result.status.success()` が成立 | `tests/export.rs:108-111` の `assert!(!result.status.success(), ...)` | ✓ 充足 |
| T03_normal_assert_stderr_contains_kind | 回帰ガード | `result.stderr` に `"invalid parameter: profile"` 含む | `tests/export.rs:113-119` の `assert!(stderr.contains("invalid parameter: profile"), ...)` | ✓ 充足 (本 Issue で新規追加) |
| T04_normal_no_stl_written | 既存維持 | output が空 or 不在 | `tests/export.rs:121-123` の `assert_eq!(written, 0, ...)` (既存) | ✓ 充足 |
| T05_degen_other_tests_unchanged | 退化/境界 | 他 4 テスト (`export_simple_box_*`, `export_sphere_*`, `export_extruded_rect_*`, `t16_export_assembly_succeeds`) が引き続きパス | `cargo test -p mycad-cli --test export` で 5 件 all pass を確認済み (repro-after.log 参照) | ✓ 充足 |
| T06_boundary_stderr_capture_does_not_break_status | 境界 | `.output()` 経由でも exit code 判定が `.status()` と等価に動く | T02 と同じ assertion で実証 (5 テスト all pass、退化なし) | ✓ 充足 (T02 と兼用) |
| T_post_fix_grep_zero | repro 解消 | 修正後 `grep -c "error: failed to build assembly"` が 0 | `features/148-export-stderr-noise/repro-after.log` で `0` を確認済み (修正後 cargo test 出力) | ✓ 充足 (artifact 記録) |

## 実装差分から追加すべきテスト

差分 (`crates/mycad-cli/tests/export.rs` のみ、+11/-3) を読む限り、plan に書かれていない新規分岐 / 隠れたケースは生じていない:

- `Command::output()` 戻り値は構造体 `Output { status, stdout, stderr }` で plan の after コードと一致
- `String::from_utf8_lossy(&result.stderr)` は plan 通り (`Cow<'_, str>`)
- assert メッセージで `{stderr}` を補完表示しているが、これも plan の after コードと一致
- 削除した `let status = ...` パスはなく、置換のみ

**追加テストなし。**

## エッジケース・退化入力

| 観点 | 検討結果 |
|------|---------|
| stderr が空のとき | subprocess 内で `eprintln!` が呼ばれない場合、assert が "expected ..., got: " で fail する。これは適切な fail メッセージ。テスト追加不要 |
| stderr が非 UTF-8 バイトを含むとき | `from_utf8_lossy` が `U+FFFD` に置換する。`contains("invalid parameter: profile")` は ASCII substring 検索なので影響なし。テスト追加不要 |
| Windows ランナーでの `\r\n` 改行 | substring 検索なので影響なし。テスト追加不要 |
| subprocess が即時クラッシュ (SIGSEGV 等) | `.output()` 内部で wait し、`status.success() == false` になる。stderr が空でも `assert!` の fail メッセージで状況判別可能。テスト追加不要 |

## 数値境界

N/A — 本 Issue は数値判定を含まない (substring 検索のみ)。

## 決定性

`tests/export.rs` の修正部分は subprocess 起動 (std `Command`) + 純粋関数 `validate_profile_closed` の挙動を assert する形になっており、構造的に決定的 (plan の「決定性に関する注記」セクション参照)。  
明示的な「同一入力 2 回実行で stderr が一致」テストは scope-inflation のため追加しない (Round 1 IN02 棄却根拠と同じ)。

## 類似ケース (バグ修正 Issue 構造類似性チェック)

bug `bug` ラベル付き Issue として、修正した「subprocess の stderr が親プロセスに継承される」と同種の問題が他に存在するかを確認。

- `grep -rn 'self-intersect\|self_intersect' crates/` でヒットした関連箇所:
  - `crates/mycad-kernel/src/primitives/extrusion.rs:692 test_self_intersecting_rejected` — **kernel 単体テスト**で subprocess を使わず、純粋関数を直接呼び出す形式。stderr leak の対象外 → **類似ケースなし**
  - `crates/mycad-kernel/src/booleans/classify.rs:212` / `partition.rs:1332` — コード内コメントのみ (UV polygon の self-intersecting 言及)、テスト無関係 → **類似ケースなし**

- subprocess を `.status()` で起動する他テスト:
  - 同ファイル内の他 3 テスト (`export_simple_box_*` / `export_sphere_*` / `export_extruded_rect_*`) は **成功想定** (`assert!(status.success())`) で、subprocess の stderr は基本的に空 → ノイズの原因にならない → **Out-of-Scope (plan 既記載)**
  - リポジトリ全体で `Command::status()` を grep → 多数あるが、いずれも non-zero exit を期待しない `cargo` / `git` / 内部スクリプト呼び出しが大半。本 Issue と同じ「失敗パス + eprintln stderr leak」構造のテストは他に未確認

→ **類似ケースの追加テスト不要。** 必要なら別 Issue で「subprocess negative test のスタイル統一」として横展開可。本 Issue は scope 内に集約。

## STEP 6.6 (GLM テスト実装) への指示

GLM 担当者へ:
- **追加すべきテスト関数はない。** plan の T01〜T_post_fix_grep_zero は STEP 6 でカバー済み
- 既存 `export_invalid_profile_fails_no_stl` の現状 (`.output()` + stderr substring assert) を維持する
- `cargo xtask ci` が green であることを再確認するのみで OK
- 万一 lint / format で warning が出た場合のみ修正する
