use serde::{Deserialize, Serialize};

use crate::Coordinate;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct BoundingBox {
    x: (f32, f32),
    y: (f32, f32),
    z: (f32, f32),
}

impl BoundingBox {
    pub fn new(minx: f32, maxx: f32, miny: f32, maxy: f32, minz: f32, maxz: f32) -> Self {
        Self {
            x: (minx, maxx),
            y: (miny, maxy),
            z: (minz, maxz),
        }
    }

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

    pub fn merge(&mut self, other: &Self) {
        self.x.0 = self.x.0.min(other.x.0);
        self.x.1 = self.x.1.max(other.x.1);
        self.y.0 = self.y.0.min(other.y.0);
        self.y.1 = self.y.1.max(other.y.1);
        self.z.0 = self.z.0.min(other.z.0);
        self.z.1 = self.z.1.max(other.z.1);
    }
}
