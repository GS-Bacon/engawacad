//! Issue #269: refs_resolve_in_state transitive plane_ref liveness — acceptance tests.
//!
//! 本ファイルは Issue #269 スコープ専用の slug-matched integration test binary。
//! T_269 系の大半は `feature_crud_prefix_validate_acceptance.rs` 側 (line 1185-) に
//! 既存 #266/#267 テストと隣接させて実装している。本ファイルでは:
//! - 成功パス決定性 (success-path determinism, B-6 cross-cut F02) を byte-equal で検証
//! - transitive plane_ref check のスコープ最小再現 (B-6 cross-cut F01: 1+ #[test] 確保)
//! の 2 件を残し、slug-matched extractor が `total_added`/`determinism` を拾えるようにする。

use engawa_build::FeatureCrud;
use engawa_format::{
    Document, EntityKind, EntityRef, Feature, PlaneRef, SketchElement, SketchPlane,
};

/// T_269 T01 (success-path determinism, B-6 F02 採用): clean history に Extrude を 2 回 insert し、
/// 返却 Document の YAML が完全一致することを byte-equal で確認する。
///
/// 失敗パスの error 比較ではなく、成功パスの to_yaml() byte-equal で
/// `refs_resolve_in_state` の transitive 分岐が決定的に振る舞うことを検証。
#[test]
fn test_269_success_path_determinism_byte_equal() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
        suppressed: false,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_2".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
        suppressed: false,
    });
    doc.root_component.features.push(Feature::CreateSketch {
        id: "sk".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![SketchElement::Line {
            id: "s1".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        }],
        plane_ref: Some(PlaneRef::Entity(EntityRef::Named {
            feature_id: "box_1".to_string(),
            kind: EntityKind::Face,
            role: "top".to_string(),
        })),
        suppressed: false,
    });

    let extrude = Feature::Extrude {
        id: "e1".to_string(),
        sketch: "sk".to_string(),
        depth: 5.0,
        fuse_target: Some("box_2".to_string()),
        suppressed: false,
    };

    let doc1 = FeatureCrud::insert(&doc, extrude.clone(), 3).expect("first insert should succeed");
    let doc2 = FeatureCrud::insert(&doc, extrude, 3).expect("second insert should succeed");

    let yaml1 = serde_yaml::to_string(&doc1).expect("yaml1");
    let yaml2 = serde_yaml::to_string(&doc2).expect("yaml2");
    assert_eq!(
        yaml1, yaml2,
        "two consecutive inserts must produce byte-identical YAML"
    );

    let ids1: Vec<&str> = doc1
        .root_component
        .features
        .iter()
        .map(|f| f.id())
        .collect();
    let ids2: Vec<&str> = doc2
        .root_component
        .features
        .iter()
        .map(|f| f.id())
        .collect();
    assert_eq!(
        ids1, ids2,
        "feature ID order must be identical across 2 runs"
    );
}

/// T_269 minimal transitive check (B-6 F01 採用): slug-matched file 内に
/// transitive plane_ref check のスコープ最小再現を 1 件残し、coverage_hints の
/// `total_added` が >=1 として拾われるようにする。
///
/// 実シナリオの広い覆いは `feature_crud_prefix_validate_acceptance.rs:1185-` 側で
/// `t_269_extrude_transitive_plane_ref_dead` 等が担う。
#[test]
fn test_269_minimal_transitive_check() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
        suppressed: false,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_other".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
        suppressed: false,
    });
    doc.root_component.features.push(Feature::CreateSketch {
        id: "sk".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![SketchElement::Line {
            id: "s1".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        }],
        plane_ref: Some(PlaneRef::Entity(EntityRef::Named {
            feature_id: "box_1".to_string(),
            kind: EntityKind::Face,
            role: "top".to_string(),
        })),
        suppressed: false,
    });
    // c1 が box_1 を consume → sk の plane_ref body が dead
    doc.root_component.features.push(Feature::Cut {
        id: "c1".to_string(),
        target: "box_1".to_string(),
        tool: "box_other".to_string(),
        suppressed: false,
    });
    doc.root_component.features.push(Feature::Extrude {
        id: "e1".to_string(),
        sketch: "sk".to_string(),
        depth: 5.0,
        fuse_target: None,
        suppressed: false,
    });

    // 末尾に Cut(target=e1) → e1 は transitive plane_ref dead で skip 済 → BodyNotFound
    let cut = Feature::Cut {
        id: "c2".to_string(),
        target: "e1".to_string(),
        tool: "box_other".to_string(),
        suppressed: false,
    };

    let err = FeatureCrud::insert(&doc, cut, 5);
    match err {
        Err(engawa_build::FeatureCrudError::BodyNotFound {
            feature_id,
            body_ref,
        }) => {
            assert_eq!(feature_id, "c2");
            assert_eq!(body_ref, "e1");
        }
        other => panic!("expected BodyNotFound for e1, got {:?}", other),
    }
}
