//! CLI tests for `engawa entry add` command.

use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

fn fixture_path(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../engawa-build/tests/fixtures/insert");
    path.push(name);
    path
}

/// T03: CLI dry-run — stdout に期待 YAML が出力され、input は変更されない
#[test]
fn t03_entry_add_dry_run_matches_golden() {
    let input = fixture_path("input.engawa");
    let feature = fixture_path("new_box.yaml");
    let tmp_dir = TempDir::new().expect("failed to create temp dir");

    // 一時ファイルにコピーして実行
    let tmp_input = tmp_dir.path().join("input.engawa");
    std::fs::copy(&input, &tmp_input).expect("failed to copy input");
    let original_bytes = std::fs::read(&tmp_input).expect("failed to read original");

    let mut cmd = Command::new(cargo_bin_exe());
    cmd.arg("entry")
        .arg("add")
        .arg(&tmp_input)
        .arg(&feature)
        .arg("--at")
        .arg("1")
        .arg("--dry-run");

    let output = cmd.output().expect("failed to execute");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected_yaml = std::fs::read_to_string(fixture_path("expected.engawa"))
        .expect("failed to read expected YAML");
    assert_eq!(stdout, expected_yaml);

    // --dry-run は input を変更しないことを確認
    let after_bytes = std::fs::read(&tmp_input).expect("failed to read after");
    assert_eq!(
        original_bytes, after_bytes,
        "input should not be modified with --dry-run"
    );
}

/// T04: CLI 出力先 — --output で指定したファイルに書き込まれ、input は変更されない
#[test]
fn t04_entry_add_output_flag_writes_separate_file() {
    let input = fixture_path("input.engawa");
    let feature = fixture_path("new_box.yaml");
    let tmp_dir = TempDir::new().expect("failed to create temp dir");

    // 一時ファイルにコピーして実行
    let tmp_input = tmp_dir.path().join("input.engawa");
    std::fs::copy(&input, &tmp_input).expect("failed to copy input");
    let original_bytes = std::fs::read(&tmp_input).expect("failed to read original");

    let output_path = tmp_dir.path().join("output.engawa");

    let mut cmd = Command::new(cargo_bin_exe());
    cmd.arg("entry")
        .arg("add")
        .arg(&tmp_input)
        .arg(&feature)
        .arg("--at")
        .arg("1")
        .arg("-o")
        .arg(&output_path);

    let output = cmd.output().expect("failed to execute");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let result = std::fs::read_to_string(&output_path).expect("failed to read output");
    let expected = std::fs::read_to_string(fixture_path("expected.engawa"))
        .expect("failed to read expected YAML");
    assert_eq!(result, expected);

    // --output 指定時は input を変更しないことを確認
    let after_bytes = std::fs::read(&tmp_input).expect("failed to read after");
    assert_eq!(
        original_bytes, after_bytes,
        "input should not be modified when --output is specified"
    );
}

/// T05: CLI 上書き — --output 未指定で input が上書きされる
#[test]
fn t05_entry_add_default_overwrites_input() {
    let input = fixture_path("input.engawa");
    let feature = fixture_path("new_box.yaml");
    let tmp_dir = TempDir::new().expect("failed to create temp dir");

    // 一時ファイルにコピー
    let tmp_input = tmp_dir.path().join("input.engawa");
    std::fs::copy(&input, &tmp_input).expect("failed to copy input");

    let original_content = std::fs::read_to_string(&tmp_input).expect("failed to read original");

    let mut cmd = Command::new(cargo_bin_exe());
    cmd.arg("entry")
        .arg("add")
        .arg(&tmp_input)
        .arg(&feature)
        .arg("--at")
        .arg("1");

    let output = cmd.output().expect("failed to execute");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let result = std::fs::read_to_string(&tmp_input).expect("failed to read updated input");
    let expected = std::fs::read_to_string(fixture_path("expected.engawa"))
        .expect("failed to read expected YAML");
    assert_eq!(result, expected);
    assert_ne!(result, original_content, "input should be modified");
}

/// T07: 無効なfeature.yaml — 空idでCLIがerror: prefix付きでexit 1
#[test]
fn t07_input_validation_propagates() {
    let input = fixture_path("input.engawa");
    let invalid_feature = fixture_path("invalid_empty_id.yaml");

    let mut cmd = Command::new(cargo_bin_exe());
    cmd.arg("entry")
        .arg("add")
        .arg(&input)
        .arg(&invalid_feature)
        .arg("--at")
        .arg("1")
        .arg("--dry-run");

    let output = cmd.output().expect("failed to execute");

    assert!(!output.status.success(), "should exit with non-zero status");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error:") || stderr.contains("Error"),
        "stderr should contain 'error:' prefix, got: {}",
        stderr
    );
    // validate 由来のエラー文言が含まれることを確認（empty または invalid）
    assert!(
        stderr.contains("empty") || stderr.contains("invalid"),
        "stderr should contain validation-related text like 'empty' or 'invalid', got: {}",
        stderr
    );
}

fn cargo_bin_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_engawa"))
}
