use serde::{Deserialize, Serialize};

use crate::Coordinate;

/// A bounding box is a Axis Aligned Box that represents the bounds of the coordinates in the
/// subtree of a node. It is used for efficient spatial queries and Field-of-View calculations. It
/// is defined by two points: the all-minimum point (minx, miny, minz) and the all-maximum point
/// (maxx, maxy, maxz).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct BoundingBox {
    x: (f32, f32),
    y: (f32, f32),
    z: (f32, f32),
}

impl BoundingBox {
    /// Create a new BoundingBox with the given minimum and maximum coordinates.
    pub fn new(minx: f32, maxx: f32, miny: f32, maxy: f32, minz: f32, maxz: f32) -> Self {
        Self {
            x: (minx, maxx),
            y: (miny, maxy),
            z: (minz, maxz),
        }
    }

    /// Create a bounding box for a single point, where the minimum and maximum coordinates are the same.
    pub fn point(coordinate: Coordinate) -> Self {
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
    pub fn of(coordinates: Vec<Coordinate>) -> Self {
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
    pub fn merge(&mut self, other: &Self) {
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
    pub fn encompasses(&self, other: &Self) -> bool {
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
    pub fn shares_border_with(&self, other: &Self) -> bool {
        self.x.0 == other.x.0
            || self.x.1 == other.x.1
            || self.y.0 == other.y.0
            || self.y.1 == other.y.1
            || self.z.0 == other.z.0
            || self.z.1 == other.z.1
    }
}
