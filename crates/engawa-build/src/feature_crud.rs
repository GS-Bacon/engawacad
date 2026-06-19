//! Feature CRUD operations for Document.
//!
//! This module provides pure-functional operations for manipulating
//! the feature history of a Document. Each operation returns a new Document
//! and preserves input Document immutability (ID-stable).

use engawa_format::{Document, Feature};
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
