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

use engawa_build::build_assembly;
use engawa_format::component::ComponentRef;
use engawa_format::document::Document;
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::error::KernelError;
use serial_test::file_serial;
use std::fs;

#[test]
#[file_serial(engawa_stdlib_path)]
fn t03_meta_lock_protects_concurrent_resolve() {
    let tmp = tempfile::tempdir().unwrap();
    let stdlib_dir = tmp.path().join("stdlib");
    fs::create_dir_all(&stdlib_dir).unwrap();

    // stdlib 下に最小の参照先 component yaml を配置
    let part_doc = Document::new("dummy_part");
    fs::write(
        stdlib_dir.join("dummy_part.engawa"),
        part_doc.to_yaml().unwrap(),
    )
    .unwrap();

    // env を set し、build_assembly が stdlib reference を解決できることを確認
    let old = std::env::var("ENGAWA_STDLIB_PATH").ok();
    std::env::set_var("ENGAWA_STDLIB_PATH", &stdlib_dir);

    let mut doc = Document::new("root");
    doc.root_component.reference = Some(ComponentRef::StdLib("dummy_part".into()));

    let mut gen = IdGenerator::new(0);
    let result = build_assembly(&doc, tmp.path(), &mut gen);
    result.expect("stdlib reference must resolve when env is set");

    // restore
    match old {
        Some(v) => std::env::set_var("ENGAWA_STDLIB_PATH", v),
        None => std::env::remove_var("ENGAWA_STDLIB_PATH"),
    }
}

#[test]
#[file_serial(engawa_stdlib_path)]
fn t04_meta_boundary_remove_set_remove_is_isolated() {
    let old = std::env::var("ENGAWA_STDLIB_PATH").ok();
    let tmp = tempfile::tempdir().unwrap();
    let stdlib_dir = tmp.path().join("stdlib");
    fs::create_dir_all(&stdlib_dir).unwrap();

    // リポジトリローカル stdlib に存在しない名前を使用
    let part_doc = Document::new("unique_test_part");
    fs::write(
        stdlib_dir.join("unique_test_part.engawa"),
        part_doc.to_yaml().unwrap(),
    )
    .unwrap();

    let mut doc = Document::new("root");
    doc.root_component.reference = Some(ComponentRef::StdLib("unique_test_part".into()));

    // Phase 1: remove → エラー (repo-local stdlib に未配置の名前なので失敗)
    std::env::remove_var("ENGAWA_STDLIB_PATH");
    let r1 = build_assembly(&doc, tmp.path(), &mut IdGenerator::new(0));
    match r1 {
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

    // Phase 2: set → 成功
    std::env::set_var("ENGAWA_STDLIB_PATH", &stdlib_dir);
    let r2 = build_assembly(&doc, tmp.path(), &mut IdGenerator::new(0));
    assert!(r2.is_ok(), "env set で参照解決が成功すること");

    // Phase 3: remove → 再びエラー
    std::env::remove_var("ENGAWA_STDLIB_PATH");
    let r3 = build_assembly(&doc, tmp.path(), &mut IdGenerator::new(0));
    match r3 {
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

    // restore
    if let Some(v) = old {
        std::env::set_var("ENGAWA_STDLIB_PATH", v);
    }
}
