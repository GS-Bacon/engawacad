use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BooleanOp {
    Cut,
    Fuse,
    Intersect,
}

impl BooleanOp {
    pub fn op_str(&self) -> &'static str {
        match self {
            BooleanOp::Cut => "cut",
            BooleanOp::Fuse => "fuse",
            BooleanOp::Intersect => "intersect",
        }
    }

    pub fn isect_op_str(&self) -> &'static str {
        match self {
            BooleanOp::Cut => "cut_isect_edge",
            BooleanOp::Fuse => "fuse_isect_edge",
            BooleanOp::Intersect => "intersect_isect_edge",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentLabel {
    InsideOther,
    OutsideOther,
    SharedSameDirection,
    SharedOppositeDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum HalfSpaceClass {
    Inside,
    Outside,
    OnBoundary,
}
