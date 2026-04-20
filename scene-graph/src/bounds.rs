use serde::{Deserialize, Serialize};

use crate::Coordinate;

/// The bounds of a node in the scene graph can either be:
/// 1. `Unbounded`, meaning that the node has bounds but can not yet be calculated, e.g. because the
///    node subtree has no coordinates yet, or
/// 2. A `BoundingBox`, which is an axis-aligned box that represents the bounds of the coordinates in
///    the subtree of a node.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum Bounds {
    /// State of which a node has bounds but they can not yet be calculated, e.g. because the node
    /// subtree has no coordinates yet. This is used to distinguish between nodes with no bounds
    /// and nodes with unknown bounds.
    #[default]
    Unbounded,
    /// A bounding box is a Axis Aligned Box that represents the bounds of the coordinates in the
    /// subtree of a node. It is used for efficient spatial queries and Field-of-View calculations.
    /// It is defined by two points: the all-minimum point (minx, miny, minz) and the all-maximum
    /// point (maxx, maxy, maxz).
    Bounded(BoundingBox),
}

/// A bounding box is a Axis Aligned Box that represents the bounds of the coordinates in the
/// subtree of a node. It is used for efficient spatial queries and Field-of-View calculations.
/// It is defined by two points: the all-minimum point (minx, miny, minz) and the all-maximum
/// point (maxx, maxy, maxz).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BoundingBox {
    x: (f32, f32),
    y: (f32, f32),
    z: (f32, f32),
}

impl BoundingBox {
    /// Create a new BoundingBox with the given minimum and maximum coordinates.
    fn new(minx: f32, maxx: f32, miny: f32, maxy: f32, minz: f32, maxz: f32) -> Self {
        Self {
            x: (minx, maxx),
            y: (miny, maxy),
            z: (minz, maxz),
        }
    }

    /// Create a bounding box for a single point, where the minimum and maximum coordinates are the same.
    fn point(coordinate: Coordinate) -> Self {
        Self::new(
            coordinate.x,
            coordinate.x,
            coordinate.y,
            coordinate.y,
            coordinate.z,
            coordinate.z,
        )
    }
    /// Create a BoundingBox that encompasses all the given coordinates.
    fn of(coordinates: Vec<Coordinate>) -> Self {
        let minx = coordinates
            .iter()
            .map(|c| c.x)
            .fold(f32::INFINITY, f32::min);
        let maxx = coordinates
            .iter()
            .map(|c| c.x)
            .fold(f32::NEG_INFINITY, f32::max);
        let miny = coordinates
            .iter()
            .map(|c| c.y)
            .fold(f32::INFINITY, f32::min);
        let maxy = coordinates
            .iter()
            .map(|c| c.y)
            .fold(f32::NEG_INFINITY, f32::max);
        let minz = coordinates
            .iter()
            .map(|c| c.z)
            .fold(f32::INFINITY, f32::min);
        let maxz = coordinates
            .iter()
            .map(|c| c.z)
            .fold(f32::NEG_INFINITY, f32::max);
        Self::new(minx, maxx, miny, maxy, minz, maxz)
    }

    /// Merge another BoundingBox into this one, expanding the bounds to encompass both if
    /// necessary.
    fn merge(&mut self, other: &Self) {
        self.x.0 = self.x.0.min(other.x.0);
        self.x.1 = self.x.1.max(other.x.1);
        self.y.0 = self.y.0.min(other.y.0);
        self.y.1 = self.y.1.max(other.y.1);
        self.z.0 = self.z.0.min(other.z.0);
        self.z.1 = self.z.1.max(other.z.1);
    }

    /// Check if this BoundingBox entierly encompasses another. The check fails if the other
    /// BoundingBox shares a border with this one, i.e. if any of the min or max coordinates are
    /// equal.
    fn encompasses(&self, other: &Self) -> bool {
        self.x.0 < other.x.0
            && self.x.1 > other.x.1
            && self.y.0 < other.y.0
            && self.y.1 > other.y.1
            && self.z.0 < other.z.0
            && self.z.1 > other.z.1
    }

    /// Check if this BoundingBox shares a border with another, i.e. if any of the min or max
    /// values are equal. This is useful huristic for determining if removal of a point from the
    /// subtree of a node will cause the bounds of the node to change, which can be expensive to
    /// recalculate.
    fn shares_border_with(&self, other: &Self) -> bool {
        self.x.0 == other.x.0
            || self.x.1 == other.x.1
            || self.y.0 == other.y.0
            || self.y.1 == other.y.1
            || self.z.0 == other.z.0
            || self.z.1 == other.z.1
    }
}

impl Bounds {
    /// Create a new BoundingBox with the given minimum and maximum coordinates.
    pub fn bounding_box(minx: f32, maxx: f32, miny: f32, maxy: f32, minz: f32, maxz: f32) -> Self {
        Self::Bounded(BoundingBox::new(minx, maxx, miny, maxy, minz, maxz))
    }

    /// Create a bounding box for a single point, where the minimum and maximum coordinates are the same.
    pub fn point(coordinate: Coordinate) -> Self {
        Self::bounding_box(
            coordinate.x,
            coordinate.x,
            coordinate.y,
            coordinate.y,
            coordinate.z,
            coordinate.z,
        )
    }
    /// Create a BoundingBox that encompasses all the given coordinates.
    pub fn of(coordinates: Vec<Coordinate>) -> Self {
        Self::Bounded(BoundingBox::of(coordinates))
    }

    /// Merge another Bounds into this one, expanding the bounds to encompass both if
    /// necessary.
    /// It would do no-op if other is unbounded, and if self is unbounded, it would simply take the
    /// other's bounds. In case of two bounding boxes, it would merge the two.
    pub fn merge(&mut self, other: &Self) -> &mut Self {
        use Bounds::*;
        match other {
            Unbounded => {
                return self;
            }
            Bounded(obb) => match self {
                Unbounded => {
                    *self = Bounds::Bounded(*obb);
                }
                Bounded(sbb) => {
                    sbb.merge(obb);
                }
            },
        }
        self
    }

    /// Check if these Bounds entierly encompasses another.
    /// It would return true if the other bound is unbounded, and false if self is unbounded and
    /// the other is a bounding box. In case of two bounding boxes, it would fall back to the
    /// BoundingBox implementation.
    pub fn encompasses(&self, other: &Self) -> bool {
        use Bounds::*;
        match (self, other) {
            (_, Unbounded) => true,
            (Unbounded, Bounded(..)) => false,
            (Bounded(sbb), Bounded(obb)) => sbb.encompasses(obb),
        }
    }

    /// Check if these Bounds shares a border with another.
    /// It would return false if either of the bounds is unbounded. In case of two bounding boces,
    /// it checks if the two bounding boxes share a border.
    pub fn shares_border_with(&self, other: &Self) -> bool {
        use Bounds::*;
        match (self, other) {
            (Bounded(sbb), Bounded(obb)) => sbb.shares_border_with(obb),
            _ => false,
        }
    }
}
