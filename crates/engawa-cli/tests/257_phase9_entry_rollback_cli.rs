//! `engawa entry rollback` CLI subcommand tests.
//!
//! T03: 正常系 — 一時 .engawa → rollback → out.engawa が expected のみ含む

use tempfile::TempDir;

#[test]
fn t03_cli_rollback_normal() {
    // 一時 .engawa → engawa entry rollback <input> sphere_1 --output out.engawa
    // → out.engawa が box_1 のみ含む
    let tmp_dir = TempDir::new().unwrap();

    // 初期ドキュメントを作成: [box_1, sphere_1, cyl_1]
    let mut doc = engawa_format::Document::new("test");
    doc.root_component
        .features
        .push(engawa_format::Feature::CreateBox {
            id: "box_1".to_string(),
            width: 10.0,
            height: 20.0,
            depth: 30.0,
        });
    doc.root_component
        .features
        .push(engawa_format::Feature::CreateSphere {
            id: "sphere_1".to_string(),
            radius: 5.0,
            center: [0.0, 0.0, 0.0],
        });
    doc.root_component
        .features
        .push(engawa_format::Feature::CreateCylinder {
            id: "cyl_1".to_string(),
            radius: 3.0,
            height: 15.0,
            origin: [0.0, 0.0, 0.0],
        });

    let input_path = tmp_dir.path().join("input.engawa");
    let initial_yaml = doc.to_yaml().unwrap();
    std::fs::write(&input_path, &initial_yaml).unwrap();

    // 期待値を構築: rollback("sphere_1") → features.truncate(1) → box_1 のみ
    let expected_doc = {
        let mut d = engawa_format::Document::from_yaml(&initial_yaml).unwrap();
        d.root_component.features.truncate(1);
        d
    };
    let expected_yaml = expected_doc.to_yaml().unwrap();

    let output_path = tmp_dir.path().join("output.engawa");

    // CLI サブコマンドを実行
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_engawa"))
        .arg("entry")
        .arg("rollback")
        .arg(&input_path)
        .arg("sphere_1")
        .arg("--output")
        .arg(&output_path)
        .status()
        .unwrap();

    assert!(status.success(), "CLI command failed");

    // 出力ファイルが期待値と byte-equal
    let actual_yaml = std::fs::read_to_string(&output_path).unwrap();
    assert_eq!(actual_yaml, expected_yaml);

    // 出力ドキュメントの feature 列を検証
    let output_doc = engawa_format::Document::from_path(&output_path).unwrap();
    assert_eq!(output_doc.root_component.features.len(), 1);
    assert_eq!(output_doc.root_component.features[0].id(), "box_1");
}
