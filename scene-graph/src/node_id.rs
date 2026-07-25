use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[repr(transparent)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord, Default,
)]
pub struct NodeId(u64);

impl Display for NodeId{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeId({}::{})", self.layer_id(), self.node_id())
    }
}

impl NodeId {
    pub fn new(nid: u32, lid: u32) -> Self {
        let id = ((lid as u64) << 32) | (nid as u64);
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

