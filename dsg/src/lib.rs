use std::ffi::CString;
use std::ptr::NonNull;

pub struct Dsg {
    raw: NonNull<dsg_sys::dsg_graph_t>,
}
pub type Result<T> = std::result::Result<T, String>;

impl Dsg {
    /// Creates a new empty DSG graph.
    pub fn new() -> Self {
        let ptr = unsafe { dsg_sys::dsg_create() };
        let raw = NonNull::new(ptr).expect("dsg_create returned null");
        Self { raw }
    }

    /// Adds a new layer with the given ID to the graph.
    pub fn add_layer(&mut self, layer_id: u16) -> Result<()> {
        let succeed = unsafe { dsg_sys::dsg_add_layer(self.raw.as_ptr(), layer_id) };
        match succeed {
            true => Ok(()),
            false => Err(format!("Failed to add layer {layer_id}")),
        }
    }

    /// Adds a new node with the given ID and position to the specified layer.
    /// Lables and other properties can be set later.
    pub fn add_node(&mut self, layer_id: u16, id: u64, pos: [f64; 3]) -> Result<()> {
        let v = dsg_sys::dsg_vec3_t {
            x: pos[0],
            y: pos[1],
            z: pos[2],
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
    pub fn set_label(&mut self, layer_id: u16, id: u64, label: &str) -> Result<()> {
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
    pub fn add_edge(&mut self, a_id: u64, b_id: u64) -> Result<()> {
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
    pub fn nest(&mut self, nester: u64, nestee: u64) -> Result<()> {
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

impl Drop for Dsg {
    fn drop(&mut self) {
        unsafe { dsg_sys::dsg_free(self.raw.as_ptr()) }
    }
}
