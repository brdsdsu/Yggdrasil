use dsg_sys::*;
use std::ffi::CString;
use std::ptr::NonNull;

pub struct Dsg {
    raw: NonNull<dsg_graph_t>,
}

impl Dsg {
    pub fn new() -> Self {
        let ptr = unsafe { dsg_create() };
        let raw = NonNull::new(ptr).expect("dsg_create returned null");
        Self { raw }
    }

    pub fn add_layer(&mut self, layer: u16, name: &str) -> bool {
        let c = CString::new(name).unwrap();
        unsafe { dsg_add_layer(self.raw.as_ptr(), layer, c.as_ptr()) }
    }

    pub fn add_node(&mut self, layer: u16, id: u64, pos: [f64; 3]) -> bool {
        let v = dsg_vec3_t {
            x: pos[0],
            y: pos[1],
            z: pos[2],
        };
        unsafe { dsg_add_node(self.raw.as_ptr(), layer, id, v) }
    }

    pub fn set_label(&mut self, layer: u16, id: u64, label: &str) -> bool {
        let c = CString::new(label).unwrap();
        unsafe { dsg_set_node_label(self.raw.as_ptr(), layer, id, c.as_ptr()) }
    }

    pub fn add_edge(&mut self, a_id: u64, b_id: u64) -> bool {
        unsafe { dsg_add_edge(self.raw.as_ptr(), a_id, b_id) }
    }

    pub fn nest(&mut self, nester: u64, nestee: u64) -> bool {
        unsafe { dsg_nest(self.raw.as_ptr(), nester, nestee) }
    }

    pub fn save_json(&self, path: &str) -> bool {
        let c = CString::new(path).unwrap();
        unsafe { dsg_save_json(self.raw.as_ptr(), c.as_ptr()) }
    }

    pub fn as_raw(&self) -> *mut dsg_graph_t {
        self.raw.as_ptr()
    }
}

impl Drop for Dsg {
    fn drop(&mut self) {
        unsafe { dsg_free(self.raw.as_ptr()) }
    }
}
