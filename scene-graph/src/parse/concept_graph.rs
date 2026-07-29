use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::{Result, SceneGraphError};
use crate::{SceneGraph, node::Feature};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConceptGraph {
    pub nodes: Vec<ConceptGraphNode>,
    pub edges: Vec<ConceptGraphEdge>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConceptGraphNode {
    pub node_id: usize,
    pub label: Vec<String>,
    pub label_affordance: Option<Vec<String>>,
    pub pcd_points: Vec<[f32; 3]>,
    pub pcd_colors: Vec<[f32; 3]>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConceptGraphEdge(pub usize, pub usize, pub ConceptGraphEdgeMeta);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConceptGraphEdgeMeta {
    pub desc: String,
}

impl TryFrom<ConceptGraph> for SceneGraph {
    type Error = SceneGraphError;

    /// Converts a ConceptGraph into a SceneGraph.
    /// The resulting SceneGraph will have two layers:
    /// 1. Pointcloud layer containing all point new_coordinates.
    /// 2. Semantic layer containing nodes with labels and affordances.
    ///
    /// Each ConceptGraphNode is converted into a semantic node in the SceneGraph, and several
    /// nodes in the pointcloud layer representing its pointcloud points.
    /// Edges in the ConceptGraph are preserved in the semantic layer of the SceneGraph.
    fn try_from(cg: ConceptGraph) -> Result<Self, Self::Error> {
        let mut sg = Self::default();
        // pointcloud layer
        let pcd_lid = sg.new_layer().id;
        // semantic layer
        let semantic_layer = sg.new_layer();
        semantic_layer.add_child(pcd_lid)?;

        // keeps track of id mapping between concept graph and scene graph
        let mut id_map = HashMap::new();

        // nodes
        for node in cg.nodes {
            // create a (<LABEL>,label) feature for each label in the node
            let mut features: Vec<Feature> = node
                .label
                .iter()
                .map(|l| Feature::new(l, "label"))
                .collect();

            // create a (<AFFORDANCE>,affordance) feature for each affordance label in the node
            if let Some(affordances) = &node.label_affordance {
                features.extend(affordances.iter().map(|l| Feature::new(l, "affordance")));
            }

            let new_node = sg.new_node(features);
            // push to the semantic layer
            let new_nod_id = sg.layer_mut(1)?.push_node(new_node)?;
            id_map.insert(node.node_id, new_nod_id);

            // pointcloud points as coordinates
            for (p, c) in node.pcd_points.iter().zip(node.pcd_colors.iter()) {
                let features = vec![Feature::new("color", format!("{},{},{}", c[0], c[1], c[2]))];
                let coord = sg.new_coordinates(p[0], p[1], p[2], features);
                // push to the pointcloud layer
                let coord_id = sg.layer_mut(0)?.push_node(coord)?;
                // nest the coordinate under its semantic node
                sg.nest(coord_id).under(new_nod_id)?;
            }
        }
        // edges
        for (src, dst, meta) in cg
            .edges
            .iter()
            .map(|e| (id_map.get(&e.0), id_map.get(&e.1), &e.2))
        {
            let src = src.ok_or(SceneGraphError::NodeNotFound)?;
            let dst = dst.ok_or(SceneGraphError::NodeNotFound)?;

            sg.layer_mut(1)?.add_edge(*src, *dst, &meta.desc)?
        }

        Ok(sg)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn create_test_graph() -> ConceptGraph {
        // Simple fixed 10 points pointcloud
        let pcd_points: Vec<[f32; 3]> = (0..10).map(|_| [0.1, 0.1, 0.1]).collect();
        let pcd_colors: Vec<[f32; 3]> = (0..10).map(|_| [0.1, 0.1, 0.1]).collect();

        // Create 5 nodes
        let nodes: Vec<ConceptGraphNode> = (0..5)
            .map(|i| ConceptGraphNode {
                node_id: i,
                label: vec![format!("Node_{i}")],
                label_affordance: Some(vec![format!("Affordance_{i}")]),
                pcd_points: pcd_points.clone(),
                pcd_colors: pcd_colors.clone(),
            })
            .collect();

        // Fully connect all nodes (undirected)
        let mut edges = Vec::new();
        for i in 0..nodes.len() {
            for j in 0..nodes.len() {
                edges.push(ConceptGraphEdge(
                    i,
                    j,
                    ConceptGraphEdgeMeta {
                        desc: "connected".into(),
                    },
                ));
            }
        }

        ConceptGraph { nodes, edges }
    }

    #[test]
    fn parse_from_concept_graph() -> Result<()> {
        let cg = create_test_graph();
        let sg = SceneGraph::try_from(cg)?;
        assert_eq!(sg.layers.len(), 2);
        assert_eq!(sg.layer(0)?.nodes.len(), 50); // 5 nodes * 10 points each
        assert_eq!(sg.layer(1)?.nodes.len(), 5);
        for node in &sg.layer(1)?.nodes {
            assert_eq!(node.features.len(), 2); // 1 label + 1 affordance
            assert_eq!(node.edges.len(), 5); // fully connected graph with 5 nodes
            assert_eq!(node.children().len(), 10); // each node has 10 pointcloud children
        }
        Ok(())
    }
}
