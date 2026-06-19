//! Schema version migration hooks for .engawa file format evolution.
//!
//! When the .engawa format changes in a breaking way, increment `CURRENT_SCHEMA_VERSION`
//! and provide a `MigrationHook` implementation that transforms older documents to the
//! current version.

use crate::document::Document;
use crate::error::FormatError;

/// Migration hook for converting documents between schema versions.
///
/// Implementers provide the logic to transform a `Document` from an older schema version
/// to a newer one in-place. This trait defines the migration *contract*; the actual driver
/// (loader that selects and invokes hooks during deserialization) will be added in a
/// follow-up Issue. Currently, no API automatically applies registered hooks.
///
/// # Example
///
/// ```ignore
/// struct V1ToV2;
///
/// impl MigrationHook for V1ToV2 {
///     fn migrate(&self, from: u32, to: u32, doc: &mut Document) -> Result<(), FormatError> {
///         // Transform v1 documents to v2
///         Ok(())
///     }
/// }
/// ```
pub trait MigrationHook {
    /// Apply the migration transformation to `doc` in-place.
    fn migrate(&self, from: u32, to: u32, doc: &mut Document) -> Result<(), FormatError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::Feature;

    /// T_TRAIT_migration_hook_signature: MigrationHook trait 実装サンプルが compile し
    /// migrate() を呼べること。
    #[test]
    fn t_trait_migration_hook_signature() {
        // Dummy implementation for compile-time verification
        struct DummyMigration;

        impl MigrationHook for DummyMigration {
            fn migrate(
                &self,
                _from: u32,
                _to: u32,
                _doc: &mut Document,
            ) -> Result<(), FormatError> {
                Ok(())
            }
        }

        let hook = DummyMigration;

        // Verify migrate can be called on a real Document
        let mut doc = Document::new("test");
        doc.root_component.features.push(Feature::CreateBox {
            id: "box_1".to_string(),
            width: 10.0,
            height: 20.0,
            depth: 30.0,
            suppressed: false,
        });
        assert!(hook.migrate(1, 2, &mut doc).is_ok());
    }
}
