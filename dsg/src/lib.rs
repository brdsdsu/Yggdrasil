use std::ffi::CString;
use std::ptr::NonNull;

use glam::Vec3;

pub struct DynamicSceneGraph {
    raw: NonNull<dsg_sys::dsg_graph_t>,
    node_count: u64,
    layer_count: u64,
}
pub type Result<T> = std::result::Result<T, String>;

impl DynamicSceneGraph {
    /// Creates a new empty DSG graph.
    pub fn new() -> Self {
        let ptr = unsafe { dsg_sys::dsg_create() };
        let raw = NonNull::new(ptr).expect("dsg_create returned null");
        Self {
            raw,
            node_count: 0,
            layer_count: 0,
        }
    }

    /// Creates a new DSG graph builder.
    pub fn builder() -> DynamicSceneGraphBuilder {
        DynamicSceneGraphBuilder::new()
    }

    /// Adds a new layer with the given ID to the graph.
    /// The layer ID is automatically assigned and returned.
    pub fn add_layer(&mut self) -> Result<usize> {
        let layer_id = self.layer_count;
        self.layer_count += 1;
        let succeed = unsafe { dsg_sys::dsg_add_layer(self.raw.as_ptr(), layer_id) };
        match succeed {
            true => Ok(layer_id as usize),
            false => Err(format!("Failed to add layer {layer_id}")),
        }
    }

    /// Adds a new node with the given ID and position to the specified layer.
    /// Lables and other properties can be set later.
    /// The node ID is automatically assigned and returned.
    pub fn add_node(&mut self, layer_id: usize, pos: [f64; 3]) -> Result<usize> {
        let node_id = self.node_count;
        self.node_count += 1;

        let layer_id = layer_id as u64;
        let v = dsg_sys::dsg_vec3_t {
            x: pos[0],
            y: pos[1],
            z: pos[2],
        };
        let succeed = unsafe { dsg_sys::dsg_add_node(self.raw.as_ptr(), layer_id, node_id, v) };
        match succeed {
            true => Ok(node_id as usize),
            false => Err(format!("Failed to add node {node_id} to layer {layer_id}")),
        }
    }

    /// Sets the label of the specified node.
    /// Returns true if successful, false otherwise.
    /// Returns false if the node does not exist.
    pub fn set_label(&mut self, layer_id: usize, node_id: usize, label: &str) -> Result<()> {
        let layer_id = layer_id as u64;
        let node_id = node_id as u64;
        let c = CString::new(label).unwrap();
        let succeed = unsafe {
            dsg_sys::dsg_set_node_label(self.raw.as_ptr(), layer_id, node_id, c.as_ptr())
        };
        match succeed {
            true => Ok(()),
            false => Err(format!("Failed to set label for node {node_id}")),
        }
    }

    /// Adds an edge between two nodes identified by their IDs.
    /// These Two nodes must be on the same layer.
    /// for nesting edges, use the `nest` method.
    /// Returns true if successful, false otherwise.
    ///
    /// [`nest`]: Dsg::nest
    pub fn add_edge(&mut self, a_id: usize, b_id: usize) -> Result<()> {
        let a_id = a_id as u64;
        let b_id = b_id as u64;
        let succeed = unsafe { dsg_sys::dsg_add_edge(self.raw.as_ptr(), a_id, b_id) };
        match succeed {
            true => Ok(()),
            false => Err(format!("Failed to add edge between nodes [{a_id}, {b_id}]")),
        }
    }

    /// Nests one node (nestee) within another node (nester).
    /// Returns true if successful, false otherwise.
    /// Returns false if either node does not exist.
    /// Returns false if the nester is not exactly one layer above the nestee.
    /// for regular edges, use the `add_edge` method.
    ///
    /// [`add_edge`]: Dsg::add_edge
    pub fn nest(&mut self, nester: usize, nestee: usize) -> Result<()> {
        let nester = nester as u64;
        let nestee = nestee as u64;
        let succeed = unsafe { dsg_sys::dsg_nest(self.raw.as_ptr(), nester, nestee) };
        match succeed {
            true => Ok(()),
            false => Err(format!("Failed to nest node {nestee} within node {nester}")),
        }
    }

    /// Saves the graph to a JSON file at the specified path.
    pub fn save_json(&self, path: &str) -> Result<()> {
        let c = CString::new(path).unwrap();
        let succeed = unsafe { dsg_sys::dsg_save_json(self.raw.as_ptr(), c.as_ptr()) };
        match succeed {
            true => Ok(()),
            false => Err(format!("Failed to save graph to JSON file at {path}")),
        }
    }
}

impl Default for DynamicSceneGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for DynamicSceneGraph {
    fn drop(&mut self) {
        unsafe { dsg_sys::dsg_free(self.raw.as_ptr()) }
    }
}

pub struct DynamicSceneGraphBuilder {
    raw: NonNull<dsg_sys::dsg_graph_t>,
    node_max_id: u64,
    layer_max_id: u64,
}

impl DynamicSceneGraphBuilder {
    /// Creates a new empty DSG graph.
    fn new() -> Self {
        let ptr = unsafe { dsg_sys::dsg_create() };
        let raw = NonNull::new(ptr).expect("dsg_create returned null");
        Self {
            raw,
            node_max_id: 0,
            layer_max_id: 0,
        }
    }

    /// Adds a new layer with the given ID to the graph.
    /// Returns an error if the layer ID is already in use.
    pub fn add_layer(&mut self, layer_id: usize) -> Result<()> {
        let layer_id = layer_id as u64;
        self.layer_max_id = self.layer_max_id.max(layer_id + 1);
        let succeed = unsafe { dsg_sys::dsg_add_layer(self.raw.as_ptr(), layer_id) };
        match succeed {
            true => Ok(()),
            false => Err(format!("Failed to add layer {layer_id}")),
        }
    }

    /// Adds a new node with the given ID and position to the specified layer.
    /// Lables and other properties can be set later.
    /// Returns an error if the node ID is already in use.
    pub fn add_node(
        &mut self,
        layer_id: usize,
        id: usize,
        coordinates: Option<Vec3>,
    ) -> Result<()> {
        let layer_id = layer_id as u64;
        let id = id as u64;
        let v = match coordinates {
            Some(coords) => dsg_sys::dsg_vec3_t {
                x: coords.x as f64,
                y: coords.y as f64,
                z: coords.z as f64,
            },
            None => dsg_sys::dsg_vec3_t {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
        };
        let succeed = unsafe { dsg_sys::dsg_add_node(self.raw.as_ptr(), layer_id, id, v) };
        match succeed {
            true => Ok(()),
            false => Err(format!("Failed to add node {id} to layer {layer_id}")),
        }
    }

    /// Sets the label of the specified node.
    /// Returns true if successful, false otherwise.
    /// Returns false if the node does not exist.
    pub fn set_label(&mut self, layer_id: u64, id: u64, label: &str) -> Result<()> {
        let c = CString::new(label).unwrap();
        let succeed =
            unsafe { dsg_sys::dsg_set_node_label(self.raw.as_ptr(), layer_id, id, c.as_ptr()) };
        match succeed {
            true => Ok(()),
            false => Err(format!("Failed to set label for node {id}")),
        }
    }

    /// Adds an edge between two nodes identified by their IDs.
    /// These Two nodes must be on the same layer.
    /// for nesting edges, use the `nest` method.
    /// Returns true if successful, false otherwise.
    ///
    /// [`nest`]: Dsg::nest
    pub fn add_edge(&mut self, src: usize, dst: usize) -> Result<()> {
        let src = src as u64;
        let dst = dst as u64;
        let succeed = unsafe { dsg_sys::dsg_add_edge(self.raw.as_ptr(), src, dst) };
        match succeed {
            true => Ok(()),
            false => Err(format!("Failed to add edge between nodes [{src}, {dst}]")),
        }
    }

    /// Nests one node (nestee) within another node (nester).
    /// Returns true if successful, false otherwise.
    /// Returns false if either node does not exist.
    /// Returns false if the nester is not exactly one layer above the nestee.
    /// for regular edges, use the `add_edge` method.
    ///
    /// [`add_edge`]: Dsg::add_edge
    pub fn nest(&mut self, nester: usize, nestee: usize) -> Result<()> {
        let nester = nester as u64;
        let nestee = nestee as u64;
        let succeed = unsafe { dsg_sys::dsg_nest(self.raw.as_ptr(), nester, nestee) };
        match succeed {
            true => Ok(()),
            false => Err(format!("Failed to nest node {nestee} within node {nester}")),
        }
    }

    /// Finalizes the builder and returns the constructed DynamicSceneGraph.
    pub fn build(self) -> DynamicSceneGraph {
        DynamicSceneGraph {
            raw: self.raw,
            node_count: self.node_max_id,
            layer_count: self.layer_max_id,
        }
    }
}

impl Drop for DynamicSceneGraphBuilder {
    fn drop(&mut self) {
        unsafe { dsg_sys::dsg_free(self.raw.as_ptr()) }
    }
}
