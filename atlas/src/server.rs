use crate::error::Result;
use crate::{UpdatePipeline, sg::SceneGraph};

#[derive(Default)]
pub struct Server {
    update_pipeline: UpdatePipeline,
    scene_graph: SceneGraph,
}

impl Server {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn scene_graph(&mut self) -> Result<&mut SceneGraph> {
        self.update_pipeline.flush(&mut self.scene_graph)
    }

    pub fn update(&mut self, update: SceneGraph) {
        self.update_pipeline.push(update);
    }
}
