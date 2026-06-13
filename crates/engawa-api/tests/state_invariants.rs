use engawa_api::state::AppState;
use engawa_format::Document;
use std::path::PathBuf;

fn simple_box_path() -> PathBuf {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let p = std::path::Path::new(&dir)
        .join("..")
        .join("..")
        .join("examples")
        .join("simple_box.engawa");
    std::fs::canonicalize(&p).expect("simple_box.engawa fixture must exist")
}

/// ensure_loaded の戻り値が読み取り専用 &Document であることを
/// パブリック API 経由で検証する acceptance テスト。
#[test]
fn ensure_loaded_public_api_returns_read_only_ref() {
    let mut state = AppState::new(simple_box_path());
    let doc: &Document = state.ensure_loaded().expect("ensure_loaded must succeed");
    assert!(
        !doc.root_component.features.is_empty(),
        "loaded document must have features"
    );
}
