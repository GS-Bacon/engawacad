//! Feature CRUD operations for Document.
//!
//! This module provides pure-functional operations for manipulating
//! the feature history of a Document. Each operation returns a new Document
//! and preserves input Document immutability (ID-stable).

use engawa_format::{Document, EntityRef, Feature, PlaneRef};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

/// Zero-sized namespace for feature CRUD operations.
#[derive(Debug, Clone, Copy)]
pub struct FeatureCrud;

/// Errors that can occur during feature CRUD operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FeatureCrudError {
    /// Insert index is out of valid range.
    #[error("insert index {index} is out of range (root_component has {len} features)")]
    OutOfRange { index: usize, len: usize },

    /// Feature ID already exists in root_component.
    #[error("feature id {id:?} already exists in root_component")]
    DuplicateFeatureId { id: String },

    /// Document validation failed after the operation.
    #[error("document validation failed after insert: {source}")]
    Validation {
        #[from]
        source: engawa_format::FormatError,
    },

    /// Referenced sketch does not exist or is not yet created at insertion point.
    #[error("feature {feature_id} references sketch {sketch_ref:?} that does not exist or is created after this feature")]
    SketchNotFound {
        feature_id: String,
        sketch_ref: String,
    },

    /// Referenced body does not exist, is not yet created, or was already consumed.
    #[error("feature {feature_id} references body {body_ref:?} that does not exist, is created after this feature, or was already consumed")]
    BodyNotFound {
        feature_id: String,
        body_ref: String,
    },

    /// Insertion would place this feature before the producer of a referenced entity.
    #[error("feature {feature_id} references {ref_id:?} which is produced at index {producer_at}, but insertion requested at {requested_at}")]
    InsertBeforeProducer {
        feature_id: String,
        ref_id: String,
        producer_at: usize,
        requested_at: usize,
    },

    /// Insertion would place this feature before a consumer of a body it consumes.
    #[error("feature {consumed_ref:?} is consumed by {displaced_feature_id} at index {consumer_at}, but insertion at {requested_at} would remove that body before it can be consumed")]
    InsertBeforeConsumer {
        consumed_ref: String,
        displaced_feature_id: String,
        consumer_at: usize,
        requested_at: usize,
    },

    /// Feature references itself (logical degeneracy).
    #[error("feature {feature_id} contains a {ref_kind:?} reference to itself")]
    SelfReference {
        feature_id: String,
        ref_kind: &'static str,
    },
}

/// Extract sketch IDs referenced by a feature.
fn feature_sketch_refs(f: &Feature) -> Vec<&str> {
    match f {
        Feature::Extrude { sketch, .. } => vec![sketch.as_str()],
        Feature::ExtrudeCut { sketch, .. } => vec![sketch.as_str()],
        _ => vec![],
    }
}

/// Extract body IDs referenced by a feature.
fn feature_body_refs(f: &Feature) -> Vec<&str> {
    match f {
        Feature::Extrude { fuse_target, .. } => fuse_target.iter().map(|s| s.as_str()).collect(),
        Feature::ExtrudeCut { target, .. } => vec![target.as_str()],
        Feature::Cut { target, tool, .. } => vec![target.as_str(), tool.as_str()],
        Feature::Fuse { target, tool, .. } => vec![target.as_str(), tool.as_str()],
        Feature::Intersect { target, tool, .. } => vec![target.as_str(), tool.as_str()],
        _ => vec![],
    }
}

/// Extract body IDs implicitly referenced by a feature via topological entity refs.
///
/// Currently only `CreateSketch.plane_ref = PlaneRef::Entity(EntityRef)` is tracked:
/// face-attached sketches carry an implicit lifetime dependency on the body that owns
/// the referenced face. The provenance tree is walked recursively so `EntityRef::Derived`
/// chains resolve back to their `Named.feature_id` leaves.
///
/// Returns owned `String` (vs `&str`) because `Derived` traversal may produce values
/// not directly borrowable from `f` in the future (current impl only borrows, but the
/// owned shape keeps the API stable if Derived ever materialises new strings).
fn feature_implicit_body_refs(f: &Feature) -> Vec<String> {
    if let Feature::CreateSketch {
        plane_ref: Some(PlaneRef::Entity(eref)),
        ..
    } = f
    {
        collect_named_feature_ids(eref)
    } else {
        Vec::new()
    }
}

/// Extract implicit body refs **transitively** reachable from a consumer feature.
///
/// Includes:
/// - `feature_implicit_body_refs(f)` (direct: CreateSketch.plane_ref=Entity)
/// - For each sketch_id in `feature_sketch_refs(f)`, look up the matching
///   `CreateSketch` in `features` (full history) and add its `feature_implicit_body_refs`.
///
/// This captures the transitive lifetime dependency where e.g.
/// `Extrude { sketch: sk }` indirectly depends on the body that sk's plane is
/// attached to, even though Extrude itself carries no implicit body ref.
fn feature_transitive_implicit_body_refs(f: &Feature, features: &[Feature]) -> Vec<String> {
    let mut refs = feature_implicit_body_refs(f);
    for sketch_id in feature_sketch_refs(f) {
        for feat in features {
            if let Feature::CreateSketch { id, .. } = feat {
                if id == sketch_id {
                    refs.extend(feature_implicit_body_refs(feat));
                    break;
                }
            }
        }
    }
    refs
}

/// Extract body IDs consumed by a feature (same as body_refs in current spec).
fn feature_consumes(f: &Feature) -> Vec<&str> {
    feature_body_refs(f)
}

/// Check whether all direct refs of a body-producer feature resolve against the
/// running prefix state (`sketches_at` / `live_bodies_at`).
///
/// Returns `true` for features that have no refs to validate (CreateBox/Cylinder/Sphere).
/// Returns `false` if any sketch ref is missing from `sketches_at`, any body ref
/// (target/tool/fuse_target) is missing from `live_bodies_at`, or any implicit body ref
/// (e.g. CreateSketch.plane_ref=Entity) is missing from `live_bodies_at`.
///
/// This is the gate used by `simulate_history` to decide whether a prefix feature
/// "really executed". A feature that fails this check is treated as inert — its
/// inputs are not consumed and its output is not registered (atomic skip).
fn refs_resolve_in_state(
    f: &Feature,
    sketches_at: &HashMap<String, usize>,
    live_bodies_at: &HashMap<String, usize>,
) -> bool {
    match f {
        Feature::Extrude {
            sketch,
            fuse_target,
            ..
        } => {
            sketches_at.contains_key(sketch)
                && fuse_target
                    .as_ref()
                    .is_none_or(|t| live_bodies_at.contains_key(t))
        }
        Feature::ExtrudeCut { sketch, target, .. } => {
            sketches_at.contains_key(sketch) && live_bodies_at.contains_key(target)
        }
        Feature::Cut { target, tool, .. }
        | Feature::Fuse { target, tool, .. }
        | Feature::Intersect { target, tool, .. } => {
            live_bodies_at.contains_key(target) && live_bodies_at.contains_key(tool)
        }
        Feature::CreateSketch { plane_ref, .. } => {
            if let Some(PlaneRef::Entity(eref)) = plane_ref {
                // Check implicit body refs via EntityRef traversal
                for named_id in collect_named_feature_ids(eref) {
                    if !live_bodies_at.contains_key(&named_id) {
                        return false;
                    }
                }
            }
            true
        }
        // CreateBox/Cylinder/Sphere have no refs
        _ => true,
    }
}

/// Helper: collect all Named.feature_id from an EntityRef tree.
fn collect_named_feature_ids(eref: &EntityRef) -> Vec<String> {
    let mut ids = Vec::new();
    match eref {
        EntityRef::Named { feature_id, .. } => ids.push(feature_id.clone()),
        EntityRef::Derived { from, .. } => {
            for child in from {
                ids.extend(collect_named_feature_ids(child));
            }
        }
    }
    ids
}

/// Simulate feature history up to `up_to` index.
/// Returns (sketches_at: first occurrence index, live_bodies_at: last registered index, executed_at: indices where refs resolved).
fn simulate_history(
    features: &[Feature],
    up_to: usize,
) -> (
    HashMap<String, usize>,
    HashMap<String, usize>,
    HashSet<usize>,
) {
    let mut sketches_at: HashMap<String, usize> = HashMap::new();
    let mut live_bodies_at: HashMap<String, usize> = HashMap::new();
    let mut executed_at: HashSet<usize> = HashSet::new();

    for (i, f) in features.iter().enumerate() {
        if i >= up_to {
            break;
        }

        match f {
            Feature::CreateSketch { id, .. } => {
                if !refs_resolve_in_state(f, &sketches_at, &live_bodies_at) {
                    continue;
                }
                sketches_at.entry(id.clone()).or_insert(i);
                executed_at.insert(i);
            }
            Feature::CreateBox { id, .. }
            | Feature::CreateCylinder { id, .. }
            | Feature::CreateSphere { id, .. } => {
                live_bodies_at.insert(id.clone(), i);
                executed_at.insert(i);
            }
            Feature::Extrude {
                id, fuse_target, ..
            } => {
                if !refs_resolve_in_state(f, &sketches_at, &live_bodies_at) {
                    // Broken ref in prefix — atomic skip: no consume, no register.
                    continue;
                }
                if let Some(target) = fuse_target {
                    live_bodies_at.remove(target);
                }
                live_bodies_at.insert(id.clone(), i);
                executed_at.insert(i);
            }
            Feature::ExtrudeCut { id, target, .. } => {
                if !refs_resolve_in_state(f, &sketches_at, &live_bodies_at) {
                    continue;
                }
                live_bodies_at.remove(target);
                live_bodies_at.insert(id.clone(), i);
                executed_at.insert(i);
            }
            Feature::Cut {
                id, target, tool, ..
            } => {
                if !refs_resolve_in_state(f, &sketches_at, &live_bodies_at) {
                    continue;
                }
                live_bodies_at.remove(target);
                live_bodies_at.remove(tool);
                live_bodies_at.insert(id.clone(), i);
                executed_at.insert(i);
            }
            Feature::Fuse {
                id, target, tool, ..
            } => {
                if !refs_resolve_in_state(f, &sketches_at, &live_bodies_at) {
                    continue;
                }
                live_bodies_at.remove(target);
                live_bodies_at.remove(tool);
                live_bodies_at.insert(id.clone(), i);
                executed_at.insert(i);
            }
            Feature::Intersect {
                id, target, tool, ..
            } => {
                if !refs_resolve_in_state(f, &sketches_at, &live_bodies_at) {
                    continue;
                }
                live_bodies_at.remove(target);
                live_bodies_at.remove(tool);
                live_bodies_at.insert(id.clone(), i);
                executed_at.insert(i);
            }
        }
    }

    (sketches_at, live_bodies_at, executed_at)
}

/// Check for self-reference degeneracy.
fn check_self_reference(f: &Feature) -> Result<(), FeatureCrudError> {
    let id = f.id();

    for sketch_ref in feature_sketch_refs(f) {
        if sketch_ref == id {
            return Err(FeatureCrudError::SelfReference {
                feature_id: id.to_string(),
                ref_kind: "sketch",
            });
        }
    }

    for body_ref in feature_body_refs(f) {
        if body_ref == id {
            return Err(FeatureCrudError::SelfReference {
                feature_id: id.to_string(),
                ref_kind: "body",
            });
        }
    }

    Ok(())
}

/// Check that all refs resolve before insertion point.
fn check_refs_resolve_before(
    f: &Feature,
    features: &[Feature],
    at: usize,
) -> Result<(), FeatureCrudError> {
    let (sketches_at, live_bodies_at, _) = simulate_history(features, at);
    let fid = f.id();

    // Full simulation for forward scan executed_at
    let (_, _, executed_at_full) = simulate_history(features, features.len());

    // Check sketch refs
    for sketch_ref in feature_sketch_refs(f) {
        if let Some(&idx) = sketches_at.get(sketch_ref) {
            if idx >= at {
                return Err(FeatureCrudError::InsertBeforeProducer {
                    feature_id: fid.to_string(),
                    ref_id: sketch_ref.to_string(),
                    producer_at: idx,
                    requested_at: at,
                });
            }
        } else {
            // Check if a CreateSketch with this id exists after insertion point.
            // (body refs と同様に variant 限定で誤分類を防ぐ — Codex A-F02/M-F02)
            let producer_after = features.iter().enumerate().skip(at).find_map(|(i, feat)| {
                if !executed_at_full.contains(&i) {
                    return None;
                }
                match feat {
                    Feature::CreateSketch { id, .. } if id == sketch_ref => Some(i),
                    _ => None,
                }
            });

            if let Some(producer_idx) = producer_after {
                return Err(FeatureCrudError::InsertBeforeProducer {
                    feature_id: fid.to_string(),
                    ref_id: sketch_ref.to_string(),
                    producer_at: producer_idx,
                    requested_at: at,
                });
            } else {
                return Err(FeatureCrudError::SketchNotFound {
                    feature_id: fid.to_string(),
                    sketch_ref: sketch_ref.to_string(),
                });
            }
        }
    }

    // Check body refs
    for body_ref in feature_body_refs(f) {
        if let Some(&idx) = live_bodies_at.get(body_ref) {
            if idx >= at {
                return Err(FeatureCrudError::InsertBeforeProducer {
                    feature_id: fid.to_string(),
                    ref_id: body_ref.to_string(),
                    producer_at: idx,
                    requested_at: at,
                });
            }
        } else {
            // Check if body exists after insertion point or was consumed
            let producer_after = features.iter().enumerate().skip(at).find_map(|(i, feat)| {
                if !executed_at_full.contains(&i) {
                    return None;
                }
                if feat.id() == body_ref {
                    // Verify this is a body producer
                    match feat {
                        Feature::CreateBox { .. }
                        | Feature::CreateCylinder { .. }
                        | Feature::CreateSphere { .. }
                        | Feature::Extrude { .. }
                        | Feature::ExtrudeCut { .. }
                        | Feature::Cut { .. }
                        | Feature::Fuse { .. }
                        | Feature::Intersect { .. } => Some(i),
                        _ => None,
                    }
                } else {
                    None
                }
            });

            if let Some(producer_idx) = producer_after {
                return Err(FeatureCrudError::InsertBeforeProducer {
                    feature_id: fid.to_string(),
                    ref_id: body_ref.to_string(),
                    producer_at: producer_idx,
                    requested_at: at,
                });
            } else {
                return Err(FeatureCrudError::BodyNotFound {
                    feature_id: fid.to_string(),
                    body_ref: body_ref.to_string(),
                });
            }
        }
    }

    // Check implicit body refs (e.g. CreateSketch.plane_ref entity provenance).
    // Same semantics as body refs but resolved against live_bodies_at: the face must
    // belong to a body that is live at `at` (i.e. created earlier and not yet consumed).
    for implicit_ref in feature_transitive_implicit_body_refs(f, features) {
        if let Some(&idx) = live_bodies_at.get(&implicit_ref) {
            if idx >= at {
                return Err(FeatureCrudError::InsertBeforeProducer {
                    feature_id: fid.to_string(),
                    ref_id: implicit_ref,
                    producer_at: idx,
                    requested_at: at,
                });
            }
        } else {
            let producer_after = features.iter().enumerate().skip(at).find_map(|(i, feat)| {
                if !executed_at_full.contains(&i) {
                    return None;
                }
                if feat.id() == implicit_ref {
                    match feat {
                        Feature::CreateBox { .. }
                        | Feature::CreateCylinder { .. }
                        | Feature::CreateSphere { .. }
                        | Feature::Extrude { .. }
                        | Feature::ExtrudeCut { .. }
                        | Feature::Cut { .. }
                        | Feature::Fuse { .. }
                        | Feature::Intersect { .. } => Some(i),
                        _ => None,
                    }
                } else {
                    None
                }
            });

            if let Some(producer_idx) = producer_after {
                return Err(FeatureCrudError::InsertBeforeProducer {
                    feature_id: fid.to_string(),
                    ref_id: implicit_ref,
                    producer_at: producer_idx,
                    requested_at: at,
                });
            } else {
                return Err(FeatureCrudError::BodyNotFound {
                    feature_id: fid.to_string(),
                    body_ref: implicit_ref,
                });
            }
        }
    }

    Ok(())
}

/// Check that insertion doesn't break downstream consumers.
///
/// Uses post-insert hypothetical history to detect two cases:
/// 1. Inserted feature directly consumes a body still needed by a downstream consumer
///    (original pre-#267 semantics).
/// 2. Inserted feature activates a previously broken consumer, which then consumes
///    bodies needed by other consumers (#267 scope).
fn check_no_downstream_break(
    f: &Feature,
    features: &[Feature],
    at: usize,
) -> Result<(), FeatureCrudError> {
    // 1. Pre-insert simulation
    let (_, _, executed_at_pre) = simulate_history(features, features.len());

    // 2. Post-insert hypothetical
    let mut post_insert: Vec<Feature> = Vec::with_capacity(features.len() + 1);
    post_insert.extend_from_slice(&features[..at]);
    post_insert.push(f.clone());
    post_insert.extend_from_slice(&features[at..]);
    let (_, _, executed_at_post) = simulate_history(&post_insert, post_insert.len());

    let consumed_bodies_by_f = feature_consumes(f);

    // 3a. Cases where the inserted f directly consumes a body still needed by some
    //     downstream consumer (= original semantics, pre-insert view).
    //     This was the existing pre-#267 behavior — keep it for direct-conflict detection.
    for body_id in &consumed_bodies_by_f {
        for (consumer_idx, consumer_feat) in features.iter().enumerate().skip(at) {
            if !executed_at_pre.contains(&consumer_idx) {
                continue;
            }
            let consumer_refs: Vec<&str> = feature_consumes(consumer_feat);
            let implicit_consumer_refs =
                feature_transitive_implicit_body_refs(consumer_feat, features);
            let direct_match = consumer_refs.contains(body_id);
            let implicit_match = implicit_consumer_refs.iter().any(|r| r == body_id);
            if direct_match || implicit_match {
                // re_registered check (pre-insert space, between at+1 and consumer_idx)
                let mut re_registered = false;
                for (reg_idx, reg_feat) in features.iter().enumerate().skip(at + 1) {
                    if reg_idx >= consumer_idx {
                        break;
                    }
                    if !executed_at_pre.contains(&reg_idx) {
                        continue;
                    }
                    if reg_feat.id() == *body_id {
                        match reg_feat {
                            Feature::CreateBox { .. }
                            | Feature::CreateCylinder { .. }
                            | Feature::CreateSphere { .. }
                            | Feature::Extrude { .. }
                            | Feature::ExtrudeCut { .. }
                            | Feature::Cut { .. }
                            | Feature::Fuse { .. }
                            | Feature::Intersect { .. } => {
                                re_registered = true;
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                if !re_registered {
                    return Err(FeatureCrudError::InsertBeforeConsumer {
                        consumed_ref: body_id.to_string(),
                        displaced_feature_id: consumer_feat.id().to_string(),
                        consumer_at: consumer_idx,
                        requested_at: at,
                    });
                }
            }
        }
    }

    // 3b. Post-insert activation case (#267 scope):
    //     Some consumer that was executing in pre-insert sim becomes inert in post-insert sim,
    //     because newly-activated features consumed the bodies it depends on.
    //     This catches the "[box_b1, box_b2, Cut(c1, target=new_box, tool=box_b1), Cut(c2, target=box_b1, tool=box_b2)]
    //     + insert CreateBox(new_box) @ idx 2" scenario.
    for (orig_idx, consumer_feat) in features.iter().enumerate().skip(at) {
        let post_idx = orig_idx + 1; // shift by inserted feature
        if !executed_at_pre.contains(&orig_idx) {
            // wasn't working before
            continue;
        }
        if executed_at_post.contains(&post_idx) {
            // still works after
            continue;
        }
        // Was working pre, broken post — identify which body of consumer_feat is no longer live post.
        let consumer_refs: Vec<&str> = feature_consumes(consumer_feat);
        let implicit_consumer_refs =
            feature_transitive_implicit_body_refs(consumer_feat, &post_insert);

        // Combine direct + implicit refs into a single ordered list (direct first).
        let mut all_refs: Vec<String> = Vec::new();
        for r in &consumer_refs {
            all_refs.push(r.to_string());
        }
        for r in &implicit_consumer_refs {
            all_refs.push(r.clone());
        }

        // For each ref the consumer uses: was it consumed by an activator?
        // An activator is a feature at index in (at..post_idx) in post-insert space that
        // wasn't in executed_at_pre (at original idx, i.e. post_idx-1) but is in executed_at_post.
        // The inserted feature itself counts (executed_at_post contains `at`).
        for body_id in &all_refs {
            // Find activator that consumes body_id within post-insert prefix up to post_idx-1.
            let activator_consumed =
                post_insert
                    .iter()
                    .enumerate()
                    .take(post_idx)
                    .skip(at)
                    .any(|(pi, pf)| {
                        // pi == at: inserted feature. pi > at: shifted from original idx pi-1.
                        let executed_now = executed_at_post.contains(&pi);
                        let executed_before = if pi == at {
                            false
                        } else {
                            executed_at_pre.contains(&(pi - 1))
                        };
                        let is_activator_or_inserted = executed_now && !executed_before;
                        if !is_activator_or_inserted {
                            return false;
                        }
                        feature_consumes(pf).contains(&body_id.as_str())
                    });
            if activator_consumed {
                return Err(FeatureCrudError::InsertBeforeConsumer {
                    consumed_ref: body_id.to_string(),
                    displaced_feature_id: consumer_feat.id().to_string(),
                    consumer_at: orig_idx,
                    requested_at: at,
                });
            }
        }
    }

    Ok(())
}

impl FeatureCrud {
    /// Insert `feature` into `doc.root_component.features` at the given `at` index.
    ///
    /// Returns a NEW Document (in-place mutation is prohibited). Existing feature
    /// IDs are preserved (ID-stable).
    ///
    /// `at == doc.root_component.features.len()` is allowed (append at tail).
    /// `at > doc.root_component.features.len()` returns `Err(OutOfRange)`.
    ///
    /// # Determinism
    ///
    /// This function is deterministic: calling it twice with the same inputs
    /// produces byte-identical YAML when serialized via `to_yaml()`.
    pub fn insert(
        doc: &Document,
        feature: Feature,
        at: usize,
    ) -> Result<Document, FeatureCrudError> {
        let len = doc.root_component.features.len();
        if at > len {
            return Err(FeatureCrudError::OutOfRange { index: at, len });
        }

        let new_id = feature.id().to_string();
        if doc.root_component.features.iter().any(|f| f.id() == new_id) {
            return Err(FeatureCrudError::DuplicateFeatureId { id: new_id });
        }

        // Semantic validation
        check_self_reference(&feature)?;
        check_refs_resolve_before(&feature, &doc.root_component.features, at)?;
        check_no_downstream_break(&feature, &doc.root_component.features, at)?;

        let mut next = doc.clone();
        next.root_component.features.insert(at, feature);
        next.validate()?;
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engawa_format::Feature;

    #[test]
    fn test_insert_at_end() {
        let doc = Document::new("Test");
        let feature = Feature::CreateBox {
            id: "box_1".to_string(),
            width: 10.0,
            height: 20.0,
            depth: 30.0,
        };
        let result = FeatureCrud::insert(&doc, feature, 0).unwrap();
        assert_eq!(result.root_component.features.len(), 1);
        assert_eq!(result.root_component.features[0].id(), "box_1");
    }

    #[test]
    fn test_insert_out_of_range() {
        let doc = Document::new("Test");
        let feature = Feature::CreateBox {
            id: "box_1".to_string(),
            width: 10.0,
            height: 20.0,
            depth: 30.0,
        };
        let result = FeatureCrud::insert(&doc, feature, 1);
        assert!(matches!(
            result,
            Err(FeatureCrudError::OutOfRange { index: 1, len: 0 })
        ));
    }

    #[test]
    fn test_insert_duplicate_id() {
        let mut doc = Document::new("Test");
        doc.root_component.features.push(Feature::CreateBox {
            id: "box_1".to_string(),
            width: 10.0,
            height: 20.0,
            depth: 30.0,
        });
        let feature = Feature::CreateSphere {
            id: "box_1".to_string(),
            radius: 5.0,
            center: [0.0, 0.0, 0.0],
        };
        let result = FeatureCrud::insert(&doc, feature, 1);
        assert!(matches!(
            result,
            Err(FeatureCrudError::DuplicateFeatureId { .. })
        ));
    }

    #[test]
    fn test_insert_at_zero() {
        let mut doc = Document::new("Test");
        doc.root_component.features.push(Feature::CreateBox {
            id: "box_1".to_string(),
            width: 10.0,
            height: 20.0,
            depth: 30.0,
        });
        let feature = Feature::CreateSphere {
            id: "sphere_1".to_string(),
            radius: 5.0,
            center: [0.0, 0.0, 0.0],
        };
        let result = FeatureCrud::insert(&doc, feature, 0).unwrap();
        assert_eq!(result.root_component.features.len(), 2);
        assert_eq!(result.root_component.features[0].id(), "sphere_1");
        assert_eq!(result.root_component.features[1].id(), "box_1");
    }
}
