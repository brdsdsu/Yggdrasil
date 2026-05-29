use serde::{Deserialize, Serialize};


/// A NodeId is a 64-bit integer that encodes both the layer id and the node id within that layer.
/// NodeId format:
/// |      lid        |       nlid      |
/// | - - - - - - - - | - - - - - - - - |
///
/// lid: layer id (32 bits)
/// nlid: node id within the layer (32 bits)
#[repr(transparent)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord, Default,
)]
pub struct NodeId(u64);

impl NodeId {
    pub fn new(lid: u32, nlid:u32) -> Self {
        let id = ((lid as u64) << 32) | (nlid as u64);
        Self(id)
    }

    pub fn layer_id(self) -> u32 {
        (self.0 >> 32) as u32
    }

    pub fn node_id(self) -> u32 {
        (self.0 & 0xFFFFFFFF) as u32
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl num_traits::Zero for NodeId {
    fn zero() -> Self {
        NodeId(0)
    }

    fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

impl num_traits::One for NodeId {
    fn one() -> Self {
        NodeId(1)
    }
}

impl std::ops::Add<NodeId> for NodeId {
    type Output = NodeId;

    fn add(self, other: NodeId) -> NodeId {
        NodeId(self.0 + other.0)
    }
}
impl std::ops::Mul<NodeId> for NodeId {
    type Output = NodeId;

    fn mul(self, other: NodeId) -> NodeId {
        NodeId(self.0 * other.0)
    }
}
impl std::ops::SubAssign<NodeId> for NodeId {
    fn sub_assign(&mut self, other: NodeId) {
        self.0 -= other.0;
    }
}
