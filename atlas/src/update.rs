use crate::error::Result;
use crate::sg::SceneGraph;

#[derive(Default)]
pub struct UpdatePipeline {
    update_queue: Vec<SceneGraph>,
}

impl UpdatePipeline {
    pub fn new() -> Self {
        Default::default()
    }

    /// Push a new scene graph update to the pipeline for Lazy evaluation.
    /// It is guaranteed that the updates will be applied in the order they were pushed.
    pub fn push(&mut self, scene_graph: SceneGraph) {
        // todo: develop some mechanism for seperating conflict-free updates
        self.update_queue.push(scene_graph);
    }

    pub fn flush<'a>(&mut self, sg: &'a mut SceneGraph) -> Result<&'a mut SceneGraph> {
        let updates = std::mem::take(&mut self.update_queue);

        // todo: First resolve conflicts between updates and then apply the final sub-graph to the main scene graph
        updates.into_iter().try_for_each(|u| sg.merge(u))?;
        Ok(sg)
    }
}
