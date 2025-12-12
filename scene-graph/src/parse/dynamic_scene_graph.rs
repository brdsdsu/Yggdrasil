use crate::error::SceneGraphError;
use crate::sg::SceneGraph;
use dsg::DynamicSceneGraph;

impl TryFrom<SceneGraph> for DynamicSceneGraph {
    type Error = SceneGraphError;

    /// Converts a SceneGraph into a DynamicSceneGraph.
    /// Note that edge descriptions are lost in the conversion.
    fn try_from(sg: SceneGraph) -> Result<Self, Self::Error> {
        fn try_from(sg: SceneGraph) -> Result<DynamicSceneGraph, String> {
            let mut dsg = DynamicSceneGraph::builder();
            for (lid, layer) in sg.layers.iter().enumerate() {
                dsg.add_layer(lid)?;
                for node in &layer.nodes {
                    dsg.add_node(lid, node.id, node.coordinates)?;
                    for child_id in &node.children {
                        dsg.nest(node.id, *child_id)?;
                    }
                }
                // edge description is lost
                for node in &layer.nodes {
                    for edge in &node.edges {
                        dsg.add_edge(node.id, edge.dst)?;
                    }
                }
            }
            Ok(dsg.build())
        }
        try_from(sg).map_err(SceneGraphError::ParseError)
    }
}
