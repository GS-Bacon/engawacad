use crate::error::FormatError;
use crate::variable::Variable;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Kind of topological entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
pub enum EntityKind {
    Face,
    Edge,
    Vertex,
}

impl EntityKind {
    /// Single uppercase letter used in canonical names.
    fn tag(&self) -> &'static str {
        match self {
            EntityKind::Face => "F",
            EntityKind::Edge => "E",
            EntityKind::Vertex => "V",
        }
    }
}

/// A reference to a topological entity, either directly named or derived from an operation.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "ref", rename_all = "snake_case")]
pub enum EntityRef {
    Named {
        feature_id: String,
        kind: EntityKind,
        role: String,
    },
    Derived {
        kind: EntityKind,
        op: String,
        from: Vec<EntityRef>,
        selector: String,
    },
}

impl EntityRef {
    /// Validated constructor for Named variant.
    pub fn try_named(
        feature_id: impl Into<String>,
        kind: EntityKind,
        role: impl Into<String>,
    ) -> Result<Self, FormatError> {
        let v = EntityRef::Named {
            feature_id: feature_id.into(),
            kind,
            role: role.into(),
        };
        v.validate()?;
        Ok(v)
    }

    /// Validated constructor for Derived variant.
    pub fn try_derived(
        kind: EntityKind,
        op: impl Into<String>,
        from: Vec<EntityRef>,
        selector: impl Into<String>,
    ) -> Result<Self, FormatError> {
        let v = EntityRef::Derived {
            kind,
            op: op.into(),
            from,
            selector: selector.into(),
        };
        v.validate()?;
        Ok(v)
    }

    /// Internal canonical stable name (for identity checks and map keys; not wire format).
    /// Pure function: identical structure always yields the same string.
    pub fn canonical_name(&self) -> String {
        match self {
            EntityRef::Named {
                feature_id,
                kind,
                role,
            } => {
                format!("N({};{}:{})", feature_id, kind.tag(), role)
            }
            EntityRef::Derived {
                kind,
                op,
                from,
                selector,
            } => {
                let children: Vec<String> = from.iter().map(|c| c.canonical_name()).collect();
                format!(
                    "D({};{};{};[{}])",
                    kind.tag(),
                    op,
                    selector,
                    children.join(",")
                )
            }
        }
    }

    /// Validate character set, empty segments, and provenance (recursive).
    pub fn validate(&self) -> Result<(), FormatError> {
        match self {
            EntityRef::Named {
                feature_id, role, ..
            } => {
                validate_identifier(feature_id, "feature_id")?;
                validate_identifier(role, "role")?;
                Ok(())
            }
            EntityRef::Derived {
                op, from, selector, ..
            } => {
                if from.is_empty() {
                    return Err(FormatError::EmptyProvenance { op: op.clone() });
                }
                validate_identifier(op, "op")?;
                validate_identifier(selector, "selector")?;
                for child in from {
                    child.validate()?;
                }
                Ok(())
            }
        }
    }
}

/// Validate that a string segment contains only `[A-Za-z0-9_-]` and is non-empty.
fn validate_identifier(value: &str, field: &'static str) -> Result<(), FormatError> {
    if value.is_empty() {
        return Err(FormatError::InvalidName {
            value: value.to_string(),
            reason: "must not be empty",
        });
    }
    if !value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(FormatError::InvalidName {
            value: value.to_string(),
            reason: "only alphanumeric, underscore, and hyphen characters allowed",
        });
    }
    let _ = field;
    Ok(())
}

impl Serialize for EntityRef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.validate().map_err(serde::ser::Error::custom)?;
        #[derive(Serialize)]
        #[serde(tag = "ref", rename_all = "snake_case")]
        enum Shadow<'a> {
            Named {
                feature_id: &'a str,
                kind: EntityKind,
                role: &'a str,
            },
            Derived {
                kind: EntityKind,
                op: &'a str,
                from: Vec<EntityRef>,
                selector: &'a str,
            },
        }
        match self {
            EntityRef::Named {
                feature_id,
                kind,
                role,
            } => Shadow::Named {
                feature_id,
                kind: *kind,
                role,
            }
            .serialize(serializer),
            EntityRef::Derived {
                kind,
                op,
                from,
                selector,
            } => Shadow::Derived {
                kind: *kind,
                op,
                from: from.clone(),
                selector,
            }
            .serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for EntityRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "ref", rename_all = "snake_case")]
        enum RawEntityRef {
            Named {
                feature_id: String,
                kind: EntityKind,
                role: String,
            },
            Derived {
                kind: EntityKind,
                op: String,
                from: Vec<EntityRef>,
                selector: String,
            },
        }

        let raw = RawEntityRef::deserialize(deserializer)?;
        let entity_ref = match raw {
            RawEntityRef::Named {
                feature_id,
                kind,
                role,
            } => EntityRef::Named {
                feature_id,
                kind,
                role,
            },
            RawEntityRef::Derived {
                kind,
                op,
                from,
                selector,
            } => EntityRef::Derived {
                kind,
                op,
                from,
                selector,
            },
        };
        entity_ref.validate().map_err(serde::de::Error::custom)?;
        Ok(entity_ref)
    }
}

/// The plane on which a sketch is drawn.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, TS, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SketchPlane {
    Xy,
    Xz,
    Yz,
}

/// Plane reference for a sketch. Either a named RefPlane (legacy) or an EntityRef pointing
/// to an existing Face. Serialized untagged so YAML keeps the simple string form for legacy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
pub enum PlaneRef {
    /// Reference to a `RefPlane.id` (legacy string form, e.g. "Front").
    RefPlane(String),
    /// Reference to a Face via topological naming (ADR-005 EntityRef::Named).
    Entity(EntityRef),
}

/// A line segment in a 2D sketch profile.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct SketchSegment {
    pub id: String,
    pub from: [f64; 2],
    pub to: [f64; 2],
}

/// A feature — one step in the operation history.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type")]
pub enum Feature {
    /// Create a box primitive.
    #[serde(rename = "create_box")]
    CreateBox {
        id: String,
        width: f64,
        height: f64,
        depth: f64,
    },

    /// Create a cylinder primitive.
    #[serde(rename = "create_cylinder")]
    CreateCylinder {
        id: String,
        radius: f64,
        height: f64,
        #[serde(default, skip_serializing_if = "is_origin")]
        origin: [f64; 3],
    },

    /// Create a sphere primitive.
    #[serde(rename = "create_sphere")]
    CreateSphere {
        id: String,
        radius: f64,
        #[serde(default, skip_serializing_if = "is_origin")]
        center: [f64; 3],
    },

    /// Create a sketch (2D closed profile on a plane).
    #[serde(rename = "create_sketch")]
    CreateSketch {
        id: String,
        plane: SketchPlane,
        #[serde(default, skip_serializing_if = "is_zero")]
        offset: f64,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        variables: Vec<Variable>,
        profile: Vec<SketchSegment>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        plane_ref: Option<PlaneRef>,
    },

    /// Extrude a sketch profile.
    #[serde(rename = "extrude")]
    Extrude {
        id: String,
        sketch: String,
        depth: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fuse_target: Option<String>,
    },

    /// Extrude a sketch profile and cut (boolean subtract) it from a target body.
    #[serde(rename = "extrude_cut")]
    ExtrudeCut {
        id: String,
        sketch: String,
        depth: f64,
        target: String,
    },

    /// Cut (boolean subtract) one body from another.
    #[serde(rename = "cut")]
    Cut {
        id: String,
        target: String,
        tool: String,
    },

    /// Fuse (boolean union) two bodies.
    #[serde(rename = "fuse")]
    Fuse {
        id: String,
        target: String,
        tool: String,
    },

    /// Intersect two bodies.
    #[serde(rename = "intersect")]
    Intersect {
        id: String,
        target: String,
        tool: String,
    },
}

fn is_origin(p: &[f64; 3]) -> bool {
    *p == [0.0, 0.0, 0.0]
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

impl Feature {
    /// Get the feature ID.
    pub fn id(&self) -> &str {
        match self {
            Feature::CreateBox { id, .. }
            | Feature::CreateCylinder { id, .. }
            | Feature::CreateSphere { id, .. }
            | Feature::CreateSketch { id, .. }
            | Feature::Extrude { id, .. }
            | Feature::ExtrudeCut { id, .. }
            | Feature::Cut { id, .. }
            | Feature::Fuse { id, .. }
            | Feature::Intersect { id, .. } => id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_serialization() {
        let feature = Feature::CreateBox {
            id: "box_1".to_string(),
            width: 10.0,
            height: 20.0,
            depth: 30.0,
        };

        let yaml = serde_yaml::to_string(&feature).unwrap();
        assert!(yaml.contains("create_box"));
        assert!(yaml.contains("box_1"));

        let deserialized: Feature = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.id(), "box_1");
    }

    #[test]
    fn test_create_sphere_yaml_golden() {
        let feature = Feature::CreateSphere {
            id: "sphere_1".to_string(),
            radius: 5.0,
            center: [0.0, 0.0, 0.0],
        };
        let yaml = serde_yaml::to_string(&feature).unwrap();
        // Byte-identical golden for schema drift detection
        assert!(yaml.contains("type: create_sphere"), "tag missing: {yaml}");
        assert!(yaml.contains("id: sphere_1"), "id missing: {yaml}");
        assert!(yaml.contains("radius: 5.0"), "radius missing: {yaml}");
        // Roundtrip
        let back: Feature = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.id(), "sphere_1");
        assert!(
            matches!(back, Feature::CreateSphere { radius, .. } if (radius - 5.0).abs() < 1e-12)
        );
    }

    #[test]
    fn test_entity_ref_serialization() {
        let entity_ref = EntityRef::Named {
            feature_id: "box_1".to_string(),
            kind: EntityKind::Face,
            role: "top_face".to_string(),
        };

        let yaml = serde_yaml::to_string(&entity_ref).unwrap();
        let deserialized: EntityRef = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized, entity_ref);
    }

    #[test]
    fn test_create_sketch_yaml_golden() {
        let f = Feature::CreateSketch {
            id: "sketch_1".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            variables: Vec::new(),
            profile: vec![
                SketchSegment {
                    id: "seg_a".to_string(),
                    from: [0.0, 0.0],
                    to: [10.0, 0.0],
                },
                SketchSegment {
                    id: "seg_b".to_string(),
                    from: [10.0, 0.0],
                    to: [10.0, 5.0],
                },
                SketchSegment {
                    id: "seg_c".to_string(),
                    from: [10.0, 5.0],
                    to: [0.0, 5.0],
                },
                SketchSegment {
                    id: "seg_d".to_string(),
                    from: [0.0, 5.0],
                    to: [0.0, 0.0],
                },
            ],
            plane_ref: None,
        };
        let yaml = serde_yaml::to_string(&f).unwrap();
        assert_eq!(
            yaml,
            "type: create_sketch\nid: sketch_1\nplane: xy\nprofile:\n- id: seg_a\n  from:\n  - 0.0\n  - 0.0\n  to:\n  - 10.0\n  - 0.0\n- id: seg_b\n  from:\n  - 10.0\n  - 0.0\n  to:\n  - 10.0\n  - 5.0\n- id: seg_c\n  from:\n  - 10.0\n  - 5.0\n  to:\n  - 0.0\n  - 5.0\n- id: seg_d\n  from:\n  - 0.0\n  - 5.0\n  to:\n  - 0.0\n  - 0.0\n",
            "CreateSketch YAML golden mismatch — field order or rename drifted"
        );
        let back: Feature = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.id(), "sketch_1");
    }

    #[test]
    fn test_create_sketch_yaml_golden_with_plane_ref_refplane() {
        let f = Feature::CreateSketch {
            id: "sketch_1".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            variables: Vec::new(),
            profile: vec![],
            plane_ref: Some(PlaneRef::RefPlane("Front".to_string())),
        };
        let yaml = serde_yaml::to_string(&f).unwrap();
        // legacy string 形式: plane_ref: Front
        assert!(yaml.contains("plane_ref: Front"), "got:\n{}", yaml);
        // roundtrip 同一 (Feature に PartialEq がないため文字列比較)
        let back: Feature = serde_yaml::from_str(&yaml).unwrap();
        let yaml2 = serde_yaml::to_string(&back).unwrap();
        assert_eq!(yaml, yaml2, "roundtrip 同一");
        assert_eq!(back.id(), "sketch_1");
    }

    #[test]
    fn test_create_sketch_yaml_golden_with_plane_ref_entity() {
        let f = Feature::CreateSketch {
            id: "sketch_1".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            variables: Vec::new(),
            profile: vec![],
            plane_ref: Some(PlaneRef::Entity(EntityRef::Named {
                feature_id: "cuboid".to_string(),
                kind: EntityKind::Face,
                role: "f_z_pos".to_string(),
            })),
        };
        let yaml = serde_yaml::to_string(&f).unwrap();
        // Entity 形式: map で ref: named, feature_id, kind, role
        assert!(yaml.contains("ref: named"), "got:\n{}", yaml);
        assert!(yaml.contains("feature_id: cuboid"), "got:\n{}", yaml);
        assert!(yaml.contains("kind: face"), "got:\n{}", yaml);
        assert!(yaml.contains("role: f_z_pos"), "got:\n{}", yaml);
        // legacy string 形式と衝突しない (untagged enum で variant が区別される)
        let back: Feature = serde_yaml::from_str(&yaml).unwrap();
        let yaml2 = serde_yaml::to_string(&back).unwrap();
        assert_eq!(yaml, yaml2, "roundtrip 同一");
        assert_eq!(back.id(), "sketch_1");
    }

    #[test]
    fn test_extrude_cut_yaml_golden() {
        let f = Feature::ExtrudeCut {
            id: "ec_1".to_string(),
            sketch: "sketch_0".to_string(),
            depth: 5.0,
            target: "box_1".to_string(),
        };
        let yaml = serde_yaml::to_string(&f).unwrap();
        assert!(yaml.contains("type: extrude_cut"), "tag missing: {yaml}");
        assert!(yaml.contains("id: ec_1"), "id missing: {yaml}");
        assert!(yaml.contains("sketch: sketch_0"), "sketch missing: {yaml}");
        assert!(yaml.contains("depth: 5.0"), "depth missing: {yaml}");
        assert!(yaml.contains("target: box_1"), "target missing: {yaml}");
        let back: Feature = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.id(), "ec_1");
        assert!(
            matches!(back, Feature::ExtrudeCut { depth, target, .. } if (depth - 5.0).abs() < 1e-12 && target == "box_1")
        );
    }

    // --- T01-T14: Topological naming tests ---

    /// T01: Deterministic canonical_name (pure function).
    #[test]
    fn t01_canonical_name_deterministic() {
        let a = EntityRef::Named {
            feature_id: "box_1".to_string(),
            kind: EntityKind::Face,
            role: "top".to_string(),
        };
        let b = EntityRef::Named {
            feature_id: "box_1".to_string(),
            kind: EntityKind::Face,
            role: "top".to_string(),
        };
        assert_eq!(a.canonical_name(), b.canonical_name());

        let child = EntityRef::Named {
            feature_id: "box_1".to_string(),
            kind: EntityKind::Face,
            role: "top".to_string(),
        };
        let d1 = EntityRef::Derived {
            kind: EntityKind::Face,
            op: "cut".to_string(),
            from: vec![child.clone()],
            selector: "s0".to_string(),
        };
        let d2 = EntityRef::Derived {
            kind: EntityKind::Face,
            op: "cut".to_string(),
            from: vec![child],
            selector: "s0".to_string(),
        };
        assert_eq!(d1.canonical_name(), d2.canonical_name());
    }

    /// T02: Provenance order preserved (non-commutative).
    #[test]
    fn t02_provenance_order_significant() {
        let a = EntityRef::Named {
            feature_id: "box_1".to_string(),
            kind: EntityKind::Face,
            role: "top".to_string(),
        };
        let b = EntityRef::Named {
            feature_id: "box_2".to_string(),
            kind: EntityKind::Face,
            role: "bottom".to_string(),
        };
        let d_ab = EntityRef::Derived {
            kind: EntityKind::Face,
            op: "cut".to_string(),
            from: vec![a.clone(), b.clone()],
            selector: "s0".to_string(),
        };
        let d_ba = EntityRef::Derived {
            kind: EntityKind::Face,
            op: "cut".to_string(),
            from: vec![b, a],
            selector: "s0".to_string(),
        };
        assert_ne!(d_ab.canonical_name(), d_ba.canonical_name());
    }

    /// T03: Named canonical name format.
    #[test]
    fn t03_named_canonical_name() {
        let e = EntityRef::Named {
            feature_id: "box_1".to_string(),
            kind: EntityKind::Face,
            role: "top".to_string(),
        };
        assert_eq!(e.canonical_name(), "N(box_1;F:top)");

        let e2 = EntityRef::Named {
            feature_id: "cyl_1".to_string(),
            kind: EntityKind::Edge,
            role: "side_0".to_string(),
        };
        assert_eq!(e2.canonical_name(), "N(cyl_1;E:side_0)");

        let e3 = EntityRef::Named {
            feature_id: "sph_1".to_string(),
            kind: EntityKind::Vertex,
            role: "north".to_string(),
        };
        assert_eq!(e3.canonical_name(), "N(sph_1;V:north)");
    }

    /// T04: YAML roundtrip for Named and Derived.
    #[test]
    fn t04_yaml_roundtrip() {
        let named = EntityRef::Named {
            feature_id: "box_1".to_string(),
            kind: EntityKind::Face,
            role: "top".to_string(),
        };
        let yaml = serde_yaml::to_string(&named).unwrap();
        let back: EntityRef = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back, named);

        let derived = EntityRef::Derived {
            kind: EntityKind::Edge,
            op: "cut".to_string(),
            from: vec![named.clone()],
            selector: "edge_0".to_string(),
        };
        let yaml2 = serde_yaml::to_string(&derived).unwrap();
        let back2: EntityRef = serde_yaml::from_str(&yaml2).unwrap();
        assert_eq!(back2, derived);
    }

    /// T04b: Deterministic wire format (serialize twice → same bytes).
    #[test]
    fn t04b_wire_deterministic() {
        let derived = EntityRef::Derived {
            kind: EntityKind::Face,
            op: "cut".to_string(),
            from: vec![
                EntityRef::Named {
                    feature_id: "a".to_string(),
                    kind: EntityKind::Face,
                    role: "top".to_string(),
                },
                EntityRef::Named {
                    feature_id: "b".to_string(),
                    kind: EntityKind::Face,
                    role: "bottom".to_string(),
                },
            ],
            selector: "s0".to_string(),
        };
        let yaml1 = serde_yaml::to_string(&derived).unwrap();
        let yaml2 = serde_yaml::to_string(&derived).unwrap();
        assert_eq!(yaml1, yaml2);
    }

    /// T05: Invalid characters and empty segments in EntityRef::validate().
    #[test]
    fn t05_validate_rejects_invalid() {
        // Empty feature_id
        let e = EntityRef::Named {
            feature_id: "".to_string(),
            kind: EntityKind::Face,
            role: "top".to_string(),
        };
        assert!(matches!(e.validate(), Err(FormatError::InvalidName { .. })));

        // Invalid char in role
        let e2 = EntityRef::Named {
            feature_id: "box".to_string(),
            kind: EntityKind::Face,
            role: "top;face".to_string(),
        };
        assert!(matches!(
            e2.validate(),
            Err(FormatError::InvalidName { .. })
        ));

        // Colon in op
        let e3 = EntityRef::Derived {
            kind: EntityKind::Face,
            op: "cut:special".to_string(),
            from: vec![EntityRef::Named {
                feature_id: "a".to_string(),
                kind: EntityKind::Face,
                role: "top".to_string(),
            }],
            selector: "s0".to_string(),
        };
        assert!(matches!(
            e3.validate(),
            Err(FormatError::InvalidName { .. })
        ));

        // Parens in selector
        let e4 = EntityRef::Derived {
            kind: EntityKind::Face,
            op: "cut".to_string(),
            from: vec![EntityRef::Named {
                feature_id: "a".to_string(),
                kind: EntityKind::Face,
                role: "top".to_string(),
            }],
            selector: "s(0)".to_string(),
        };
        assert!(matches!(
            e4.validate(),
            Err(FormatError::InvalidName { .. })
        ));
    }

    /// T05b: Deserialize rejects invalid EntityRef.
    #[test]
    fn t05b_deserialize_rejects_invalid() {
        let yaml = "ref: named\nfeature_id: \"\"\nkind: face\nrole: top\n";
        let result: Result<EntityRef, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());

        let yaml2 = "ref: named\nfeature_id: box\nkind: face\nrole: \"top;face\"\n";
        let result2: Result<EntityRef, _> = serde_yaml::from_str(yaml2);
        assert!(result2.is_err());
    }

    /// T06: Missing tag / unknown variant rejected.
    #[test]
    fn t06_deserialize_missing_tag_rejected() {
        let yaml = "feature_id: box_1\nkind: face\nrole: top\n";
        let result: Result<EntityRef, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());

        let yaml2 = "ref: unknown\nfeature_id: box_1\nkind: face\nrole: top\n";
        let result2: Result<EntityRef, _> = serde_yaml::from_str(yaml2);
        assert!(result2.is_err());
    }

    /// T11: JsonSchema golden for EntityRef.
    #[test]
    fn t11_json_schema_golden() {
        let schema = schemars::schema_for!(EntityRef);
        let json = serde_json::to_string_pretty(&schema).unwrap();
        let expected = r##"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "EntityRef",
  "description": "A reference to a topological entity, either directly named or derived from an operation.",
  "oneOf": [
    {
      "type": "object",
      "required": [
        "feature_id",
        "kind",
        "ref",
        "role"
      ],
      "properties": {
        "feature_id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/definitions/EntityKind"
        },
        "ref": {
          "type": "string",
          "enum": [
            "named"
          ]
        },
        "role": {
          "type": "string"
        }
      }
    },
    {
      "type": "object",
      "required": [
        "from",
        "kind",
        "op",
        "ref",
        "selector"
      ],
      "properties": {
        "from": {
          "type": "array",
          "items": {
            "$ref": "#/definitions/EntityRef"
          }
        },
        "kind": {
          "$ref": "#/definitions/EntityKind"
        },
        "op": {
          "type": "string"
        },
        "ref": {
          "type": "string",
          "enum": [
            "derived"
          ]
        },
        "selector": {
          "type": "string"
        }
      }
    }
  ],
  "definitions": {
    "EntityKind": {
      "description": "Kind of topological entity.",
      "type": "string",
      "enum": [
        "face",
        "edge",
        "vertex"
      ]
    },
    "EntityRef": {
      "description": "A reference to a topological entity, either directly named or derived from an operation.",
      "oneOf": [
        {
          "type": "object",
          "required": [
            "feature_id",
            "kind",
            "ref",
            "role"
          ],
          "properties": {
            "feature_id": {
              "type": "string"
            },
            "kind": {
              "$ref": "#/definitions/EntityKind"
            },
            "ref": {
              "type": "string",
              "enum": [
                "named"
              ]
            },
            "role": {
              "type": "string"
            }
          }
        },
        {
          "type": "object",
          "required": [
            "from",
            "kind",
            "op",
            "ref",
            "selector"
          ],
          "properties": {
            "from": {
              "type": "array",
              "items": {
                "$ref": "#/definitions/EntityRef"
              }
            },
            "kind": {
              "$ref": "#/definitions/EntityKind"
            },
            "op": {
              "type": "string"
            },
            "ref": {
              "type": "string",
              "enum": [
                "derived"
              ]
            },
            "selector": {
              "type": "string"
            }
          }
        }
      ]
    }
  }
}"##;
        assert_eq!(json, expected);
    }

    /// T12: Determinism — 100 runs.
    #[test]
    fn t12_canonical_name_100_runs() {
        let derived = EntityRef::Derived {
            kind: EntityKind::Face,
            op: "cut".to_string(),
            from: vec![
                EntityRef::Named {
                    feature_id: "a".to_string(),
                    kind: EntityKind::Face,
                    role: "top".to_string(),
                },
                EntityRef::Named {
                    feature_id: "b".to_string(),
                    kind: EntityKind::Edge,
                    role: "side".to_string(),
                },
            ],
            selector: "s0".to_string(),
        };
        let reference = derived.canonical_name();
        for i in 0..100 {
            assert_eq!(derived.canonical_name(), reference, "differs at run {i}");
        }
    }

    /// T13: Empty provenance rejected.
    #[test]
    fn t13_empty_provenance_rejected() {
        let e = EntityRef::Derived {
            kind: EntityKind::Face,
            op: "cut".to_string(),
            from: vec![],
            selector: "s0".to_string(),
        };
        assert!(matches!(
            e.validate(),
            Err(FormatError::EmptyProvenance { .. })
        ));
    }

    /// T13b: Empty provenance via deserialize.
    #[test]
    fn t13b_empty_provenance_deserialize_rejected() {
        let yaml = "ref: derived\nkind: face\nop: cut\nfrom: []\nselector: s0\n";
        let result: Result<EntityRef, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    /// Nested derived canonical name.
    #[test]
    fn test_nested_derived_canonical_name() {
        let leaf = EntityRef::Named {
            feature_id: "a".to_string(),
            kind: EntityKind::Face,
            role: "top".to_string(),
        };
        let mid = EntityRef::Derived {
            kind: EntityKind::Edge,
            op: "cut".to_string(),
            from: vec![leaf],
            selector: "e0".to_string(),
        };
        let top = EntityRef::Derived {
            kind: EntityKind::Vertex,
            op: "fuse".to_string(),
            from: vec![mid],
            selector: "v0".to_string(),
        };
        // N(a;F:top) -> D(E;cut;e0;[N(a;F:top)]) -> D(V;fuse;v0;[D(E;cut;e0;[N(a;F:top)])])
        assert_eq!(
            top.canonical_name(),
            "D(V;fuse;v0;[D(E;cut;e0;[N(a;F:top)])])"
        );
    }

    /// Recursive validation: invalid deep child.
    #[test]
    fn test_validate_recursive_invalid_child() {
        let bad_child = EntityRef::Named {
            feature_id: "".to_string(),
            kind: EntityKind::Face,
            role: "top".to_string(),
        };
        let parent = EntityRef::Derived {
            kind: EntityKind::Face,
            op: "cut".to_string(),
            from: vec![bad_child],
            selector: "s0".to_string(),
        };
        assert!(matches!(
            parent.validate(),
            Err(FormatError::InvalidName { .. })
        ));
    }

    /// Hyphen and underscore allowed in identifiers.
    #[test]
    fn test_valid_identifiers_with_hyphen_underscore() {
        let e = EntityRef::Named {
            feature_id: "my-box_1".to_string(),
            kind: EntityKind::Face,
            role: "top_face-0".to_string(),
        };
        assert!(e.validate().is_ok());
    }

    /// Validate rejects whitespace.
    #[test]
    fn test_validate_rejects_whitespace() {
        let e = EntityRef::Named {
            feature_id: "box 1".to_string(),
            kind: EntityKind::Face,
            role: "top".to_string(),
        };
        assert!(matches!(e.validate(), Err(FormatError::InvalidName { .. })));
    }

    // --- F02: try_new constructors and Serialize validate gate ---

    /// try_named succeeds with valid input.
    #[test]
    fn t_try_named_valid() {
        let e = EntityRef::try_named("box_1", EntityKind::Face, "top").unwrap();
        assert_eq!(e.canonical_name(), "N(box_1;F:top)");
    }

    /// try_named rejects invalid feature_id.
    #[test]
    fn t_try_named_invalid_feature_id() {
        let result = EntityRef::try_named("bad;id", EntityKind::Face, "top");
        assert!(result.is_err());
    }

    /// try_named rejects empty role.
    #[test]
    fn t_try_named_empty_role() {
        let result = EntityRef::try_named("box_1", EntityKind::Face, "");
        assert!(result.is_err());
    }

    /// try_derived succeeds with valid input.
    #[test]
    fn t_try_derived_valid() {
        let child = EntityRef::try_named("box_1", EntityKind::Face, "top").unwrap();
        let e = EntityRef::try_derived(EntityKind::Edge, "cut", vec![child], "e0").unwrap();
        assert_eq!(e.canonical_name(), "D(E;cut;e0;[N(box_1;F:top)])");
    }

    /// try_derived rejects empty provenance.
    #[test]
    fn t_try_derived_empty_provenance() {
        let result = EntityRef::try_derived(EntityKind::Face, "cut", vec![], "s0");
        assert!(matches!(result, Err(FormatError::EmptyProvenance { .. })));
    }

    /// try_derived rejects invalid op.
    #[test]
    fn t_try_derived_invalid_op() {
        let child = EntityRef::try_named("a", EntityKind::Face, "top").unwrap();
        let result = EntityRef::try_derived(EntityKind::Face, "cut:special", vec![child], "s0");
        assert!(result.is_err());
    }

    /// Serialize rejects directly-constructed invalid EntityRef.
    #[test]
    fn t_serialize_rejects_invalid_directly_constructed() {
        let invalid = EntityRef::Named {
            feature_id: "bad;id".into(),
            kind: EntityKind::Face,
            role: "top".into(),
        };
        let result = serde_yaml::to_string(&invalid);
        assert!(result.is_err(), "Serialize of invalid EntityRef must fail");
    }

    /// Serialize rejects directly-constructed Derived with invalid child.
    #[test]
    fn t_serialize_rejects_derived_with_invalid_child() {
        let bad_child = EntityRef::Named {
            feature_id: "".into(),
            kind: EntityKind::Face,
            role: "top".into(),
        };
        let parent = EntityRef::Derived {
            kind: EntityKind::Face,
            op: "cut".into(),
            from: vec![bad_child],
            selector: "s0".into(),
        };
        let result = serde_yaml::to_string(&parent);
        assert!(
            result.is_err(),
            "Serialize of EntityRef with invalid child must fail"
        );
    }

    /// try_derived rejects invalid selector.
    #[test]
    fn t_try_derived_invalid_selector() {
        let child = EntityRef::try_named("a", EntityKind::Face, "top").unwrap();
        let result = EntityRef::try_derived(EntityKind::Face, "cut", vec![child], "s(0)");
        assert!(result.is_err());
    }

    // --- Adversarial edge-case tests (F02) ---

    /// try_named rejects feature_id with dot.
    #[test]
    fn edge_try_named_dot_in_feature_id() {
        let result = EntityRef::try_named("box.1", EntityKind::Face, "top");
        assert!(result.is_err());
    }

    /// try_named rejects feature_id with slash.
    #[test]
    fn edge_try_named_slash_in_feature_id() {
        let result = EntityRef::try_named("box/1", EntityKind::Face, "top");
        assert!(result.is_err());
    }

    /// try_named rejects role with unicode.
    #[test]
    fn edge_try_named_unicode_in_role() {
        let result = EntityRef::try_named("box_1", EntityKind::Face, "トップ");
        assert!(result.is_err());
    }

    /// try_derived rejects op with space.
    #[test]
    fn edge_try_derived_space_in_op() {
        let child = EntityRef::try_named("a", EntityKind::Face, "top").unwrap();
        let result = EntityRef::try_derived(EntityKind::Face, "cut here", vec![child], "s0");
        assert!(result.is_err());
    }

    /// try_derived propagates error from invalid child.
    #[test]
    fn edge_try_derived_invalid_child_propagates() {
        let bad_child = EntityRef::Named {
            feature_id: "a b".into(),
            kind: EntityKind::Face,
            role: "top".into(),
        };
        let result = EntityRef::try_derived(EntityKind::Face, "cut", vec![bad_child], "s0");
        assert!(result.is_err());
    }

    /// Serialize rejects Named with empty feature_id (directly constructed).
    #[test]
    fn edge_serialize_empty_feature_id_rejected() {
        let e = EntityRef::Named {
            feature_id: "".into(),
            kind: EntityKind::Face,
            role: "top".into(),
        };
        assert!(serde_yaml::to_string(&e).is_err());
    }

    /// Serialize rejects Derived with empty selector (directly constructed).
    #[test]
    fn edge_serialize_empty_selector_rejected() {
        let child = EntityRef::Named {
            feature_id: "a".into(),
            kind: EntityKind::Face,
            role: "top".into(),
        };
        let e = EntityRef::Derived {
            kind: EntityKind::Face,
            op: "cut".into(),
            from: vec![child],
            selector: "".into(),
        };
        assert!(serde_yaml::to_string(&e).is_err());
    }

    /// Serialize accepts valid directly-constructed Named.
    #[test]
    fn edge_serialize_valid_directly_constructed() {
        let e = EntityRef::Named {
            feature_id: "box_1".into(),
            kind: EntityKind::Face,
            role: "top".into(),
        };
        assert!(serde_yaml::to_string(&e).is_ok());
    }

    /// Roundtrip via try_named → serialize → deserialize preserves all fields.
    #[test]
    fn edge_roundtrip_try_named() {
        let original = EntityRef::try_named("box_1", EntityKind::Edge, "side-0").unwrap();
        let yaml = serde_yaml::to_string(&original).unwrap();
        let back: EntityRef = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(original, back);
    }

    /// Roundtrip via try_derived → serialize → deserialize preserves structure.
    #[test]
    fn edge_roundtrip_try_derived() {
        let child = EntityRef::try_named("a", EntityKind::Vertex, "north").unwrap();
        let original = EntityRef::try_derived(EntityKind::Face, "cut", vec![child], "s-0").unwrap();
        let yaml = serde_yaml::to_string(&original).unwrap();
        let back: EntityRef = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(original, back);
    }

    /// Nested derived roundtrip.
    #[test]
    fn edge_nested_derived_roundtrip() {
        let leaf = EntityRef::try_named("a", EntityKind::Face, "top").unwrap();
        let mid = EntityRef::try_derived(EntityKind::Edge, "cut", vec![leaf], "e0").unwrap();
        let top = EntityRef::try_derived(EntityKind::Vertex, "fuse", vec![mid], "v0").unwrap();
        let yaml = serde_yaml::to_string(&top).unwrap();
        let back: EntityRef = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(top, back);
    }

    /// 100-run determinism: try_named produces same canonical_name.
    #[test]
    fn edge_try_named_deterministic_100() {
        let reference = EntityRef::try_named("box_1", EntityKind::Face, "top").unwrap();
        let ref_name = reference.canonical_name();
        for _ in 0..100 {
            let e = EntityRef::try_named("box_1", EntityKind::Face, "top").unwrap();
            assert_eq!(e.canonical_name(), ref_name);
        }
    }

    /// try_derived with 10 children.
    #[test]
    fn edge_try_derived_many_children() {
        let children: Vec<EntityRef> = (0..10)
            .map(|i| EntityRef::try_named(format!("f-{i}"), EntityKind::Face, "top").unwrap())
            .collect();
        let e = EntityRef::try_derived(EntityKind::Face, "fuse", children, "s0").unwrap();
        let yaml = serde_yaml::to_string(&e).unwrap();
        let back: EntityRef = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(e, back);
    }

    /// EntityKind all variants serialize/deserialize correctly.
    #[test]
    fn edge_entity_kind_roundtrip() {
        for kind in [EntityKind::Face, EntityKind::Edge, EntityKind::Vertex] {
            let e = EntityRef::try_named("a", kind, "r").unwrap();
            let yaml = serde_yaml::to_string(&e).unwrap();
            let back: EntityRef = serde_yaml::from_str(&yaml).unwrap();
            assert_eq!(e, back);
        }
    }
}
