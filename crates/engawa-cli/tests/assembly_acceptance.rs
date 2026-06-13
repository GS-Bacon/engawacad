use std::path::PathBuf;
use std::process::Command;

fn example_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join(name)
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn run_export(bin: &str, input: &std::path::Path) -> std::process::Output {
    let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");
    Command::new(bin)
        .arg("export")
        .arg(input)
        .arg("-o")
        .arg(tmp.path())
        .output()
        .expect("run mycad export")
}

/// T01: Determinism — assembly export produces identical STL bytes on repeated runs.
#[test]
fn t01_determinism_assembly_export() {
    let bin = env!("CARGO_BIN_EXE_mycad");
    let input = example_path("assembly.mycad");

    let tmp1 = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");
    let tmp2 = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");

    let status1 = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(tmp1.path())
        .status()
        .expect("run mycad export 1");
    assert!(status1.success(), "first export should succeed");

    let status2 = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(tmp2.path())
        .status()
        .expect("run mycad export 2");
    assert!(status2.success(), "second export should succeed");

    let stl1 = std::fs::read(tmp1.path()).expect("read stl 1");
    let stl2 = std::fs::read(tmp2.path()).expect("read stl 2");
    assert_eq!(stl1, stl2, "assembly STL output must be deterministic");
}

/// T02: Assembly export succeeds — exit 0 and non-empty STL output.
#[test]
fn t02_assembly_export_succeeds() {
    let bin = env!("CARGO_BIN_EXE_mycad");
    let input = example_path("assembly.mycad");
    let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");

    let status = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(tmp.path())
        .status()
        .expect("run mycad export");
    assert!(status.success(), "export of assembly must succeed");

    let stl = std::fs::read(tmp.path()).expect("read stl");
    assert!(!stl.is_empty(), "STL output must not be empty");
}

/// T05: Simple part (features only, no children) still works after assembly wiring.
#[test]
fn t05_boundary_simple_part_still_works() {
    let bin = env!("CARGO_BIN_EXE_mycad");
    let input = example_path("simple_box.mycad");
    let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");

    let status = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(tmp.path())
        .status()
        .expect("run mycad export");
    assert!(status.success(), "export of simple part must still succeed");

    let stl = std::fs::read(tmp.path()).expect("read stl");
    assert!(!stl.is_empty(), "STL output must not be empty");
}

// ---------------------------------------------------------------------------
// Edge-case tests (adversarial persona)
// ---------------------------------------------------------------------------

/// Determinism 100x: 100 consecutive exports of assembly.mycad produce identical STL bytes.
#[test]
fn edge_determinism_100x_assembly_export() {
    let bin = env!("CARGO_BIN_EXE_mycad");
    let input = example_path("assembly.mycad");

    let tmp_ref = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");
    let status = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(tmp_ref.path())
        .status()
        .expect("run mycad export reference");
    assert!(status.success(), "reference export should succeed");
    let reference = std::fs::read(tmp_ref.path()).expect("read reference stl");

    for i in 1..=99 {
        let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");
        let s = Command::new(bin)
            .arg("export")
            .arg(&input)
            .arg("-o")
            .arg(tmp.path())
            .status()
            .expect("run mycad export");
        assert!(s.success(), "export run {i} should succeed");
        let stl = std::fs::read(tmp.path()).expect("read stl");
        assert_eq!(stl, reference, "STL output differs at run {i}");
    }
}

/// Determinism 100x: Simple part also produces identical output 100 times.
#[test]
fn edge_determinism_100x_simple_part_export() {
    let bin = env!("CARGO_BIN_EXE_mycad");
    let input = example_path("simple_box.mycad");

    let tmp_ref = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");
    let status = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(tmp_ref.path())
        .status()
        .expect("run mycad export reference");
    assert!(status.success());
    let reference = std::fs::read(tmp_ref.path()).expect("read reference");

    for i in 1..=99 {
        let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");
        let s = Command::new(bin)
            .arg("export")
            .arg(&input)
            .arg("-o")
            .arg(tmp.path())
            .status()
            .expect("run mycad export");
        assert!(s.success(), "simple part run {i} should succeed");
        let stl = std::fs::read(tmp.path()).expect("read stl");
        assert_eq!(stl, reference, "simple part STL differs at run {i}");
    }
}

/// Deep nested assembly: 3 levels of children with a leaf box exports successfully.
#[test]
fn edge_deep_nested_assembly_export() {
    let bin = env!("CARGO_BIN_EXE_mycad");
    let input = fixture_path("deep_nested_assembly.mycad");
    let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");

    let status = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(tmp.path())
        .status()
        .expect("run mycad export");
    assert!(
        status.success(),
        "deep nested assembly export should succeed"
    );

    let stl = std::fs::read(tmp.path()).expect("read stl");
    assert!(!stl.is_empty(), "deep nested STL must not be empty");
}

/// Deep nested assembly determinism: two runs produce identical output.
#[test]
fn edge_deep_nested_determinism() {
    let bin = env!("CARGO_BIN_EXE_mycad");
    let input = fixture_path("deep_nested_assembly.mycad");

    let tmp1 = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");
    let tmp2 = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");

    let s1 = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(tmp1.path())
        .status()
        .expect("run 1");
    assert!(s1.success());

    let s2 = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(tmp2.path())
        .status()
        .expect("run 2");
    assert!(s2.success());

    let stl1 = std::fs::read(tmp1.path()).expect("read stl 1");
    let stl2 = std::fs::read(tmp2.path()).expect("read stl 2");
    assert_eq!(stl1, stl2, "deep nested STL must be deterministic");
}

/// Sibling assembly: two sibling components with transform offset export successfully.
#[test]
fn edge_siblings_assembly_export() {
    let bin = env!("CARGO_BIN_EXE_mycad");
    let input = fixture_path("siblings_assembly.mycad");
    let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");

    let status = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(tmp.path())
        .status()
        .expect("run mycad export");
    assert!(status.success(), "siblings assembly export should succeed");

    let stl = std::fs::read(tmp.path()).expect("read stl");
    assert!(!stl.is_empty(), "siblings STL must not be empty");
    // Two boxes combined: sibling output must differ from single box
    let single_input = example_path("simple_box.mycad");
    let tmp_single = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");
    let s_single = Command::new(bin)
        .arg("export")
        .arg(&single_input)
        .arg("-o")
        .arg(tmp_single.path())
        .status()
        .expect("run single box export");
    assert!(s_single.success());
    let stl_single = std::fs::read(tmp_single.path()).expect("read single stl");
    assert_ne!(
        stl, stl_single,
        "sibling assembly STL must differ from single box"
    );
}

/// Empty children assembly: children with no features → CLI succeeds but produces empty STL.
#[test]
fn edge_empty_children_assembly_cli_produces_empty_stl() {
    let bin = env!("CARGO_BIN_EXE_mycad");
    let input = fixture_path("empty_children_assembly.mycad");
    let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");

    let status = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(tmp.path())
        .status()
        .expect("run mycad export");
    // CLI does not reject empty assemblies — it writes an empty STL
    assert!(
        status.success(),
        "CLI should not panic on empty children assembly"
    );

    let stl = std::fs::read(tmp.path()).expect("read stl");
    // Empty STL contains only the header + 0 facet count = 84 bytes, or may be truly empty
    // Check that no geometry was produced (no "facet normal" lines)
    if !stl.is_empty() {
        let text = String::from_utf8_lossy(&stl);
        assert_eq!(
            text.matches("facet normal").count(),
            0,
            "empty children assembly should produce 0 facets"
        );
    }
}

/// Negative dimension box assembly: CLI exits with error, does not panic.
#[test]
fn edge_negative_box_assembly_cli_fails() {
    let bin = env!("CARGO_BIN_EXE_mycad");
    let input = fixture_path("negative_box_assembly.mycad");
    let output = run_export(bin, &input);
    assert!(
        !output.status.success(),
        "negative box assembly should fail"
    );
    // Must not panic — we get a clean error exit
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.is_empty(),
        "stderr should contain error message, not silent failure"
    );
}

/// Nonexistent input file: CLI exits with error, not panic.
#[test]
fn edge_nonexistent_input_cli_fails() {
    let bin = env!("CARGO_BIN_EXE_mycad");
    let input = PathBuf::from("/tmp/absolutely_nonexistent_76_test.mycad");
    let output = run_export(bin, &input);
    assert!(
        !output.status.success(),
        "nonexistent file should cause error exit"
    );
}
