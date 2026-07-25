mod bounds;
mod error;
mod fov;
mod layer;
mod node;
mod node_id;
pub mod parse;
mod pos;
mod sg;

pub use bounds::{BoundingBox, Bounds};
pub use error::{Result, SceneGraphError};
pub use fov::Observer;
pub use layer::{Layer, NodeMut};
pub use node::{AnonNode, Coordinate, Edge, Feature, Node};
pub use node_id::NodeId;
pub use pos::PosQueryCount;
pub use sg::SceneGraph;

use node::IntoNode;

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use super::error::Result;
    use super::*;

    #[test]
    fn api() -> Result<()> {
        let mut sg = SceneGraph::default();

        // create nodes
        let node1 = sg.new_node(vec![Feature::new("name", "Node 1")]);
        let node2 = sg.new_node(vec![Feature::new("name", "Node 2")]);
        let node3 = sg.new_node(vec![Feature::new("name", "Node 3")]);

        // create a semantic layer and add nodes to layers
        let semantic_layer = sg.new_layer();
        let id2 = semantic_layer.push_node(node2)?;
        let id3 = semantic_layer.push_node(node3)?;
        // create the second semantic layer
        let semantic_layer = sg.new_layer();
        let id1 = semantic_layer.push_node(node1)?;
        semantic_layer.add_child(0)?;

        // nesting
        assert!(sg.nest(id2).under(id1).is_ok());
        assert!(sg.nest(id3).under(id1).is_ok());
        assert_eq!(sg.node(id1)?.children(), vec![id2, id3]);
        assert_eq!(sg.node(id2)?.pid, Some(id1));
        assert_eq!(sg.node(id3)?.pid, Some(id1));

        // add edge
        sg.layer_mut(0)?.add_edge(id2, id3, "connected to")?;
        sg.layer_mut(0)?.add_edge(id3, id2, "is supporting")?;
        assert_eq!(sg.node(id2)?.edges.len(), 1);
        assert_eq!(sg.node(id3)?.edges.len(), 1);

        // delete edge
        sg.layer_mut(0)?.del_edge(id2, id3)?;
        assert_eq!(sg.node(id2)?.edges.len(), 0);
        assert_eq!(sg.node(id3)?.edges.len(), 1);

        // delete invalid edge
        assert!(sg.layer_mut(0)?.del_edge(id2, id3).is_err());

        // delete node

        // deleting a node should also delete its edges within the same layers
        sg.del_node(id2)?;
        assert!(sg.node(id2).is_err());
        assert_eq!(sg.node(id3)?.edges.len(), 0);
        // should also be removed from parent's children List
        assert_eq!(sg.node(id1)?.children(), vec![id3]);

        sg.del_node(id1)?;
        assert!(sg.node(id1).is_err());
        // children should also be deleted
        assert!(sg.node(id2).is_err());
        assert!(sg.node(id3).is_err());

        Ok(())
    }

    #[test]
    fn query() -> Result<()> {
        let mut sg = SceneGraph::default();

        // create nodes
        let chair = sg.new_node(vec![
            Feature::new("name", "chair"),
            Feature::new("type", "furniture"),
            Feature::new("affordance", "sit"),
        ]);
        let table = sg.new_node(vec![
            Feature::new("name", "table"),
            Feature::new("type", "furniture"),
            Feature::new("affordance", "place items"),
        ]);
        let wall = sg.new_node(vec![
            Feature::new("name", "wall"),
            Feature::new("type", "structure"),
            Feature::new("affordance", "support"),
        ]);
        let clock = sg.new_node(vec![
            Feature::new("name", "clock"),
            Feature::new("type", "appliance"),
        ]);

        // create layers and add nodes to layers
        let l = sg.new_layer();
        let table_id = l.push_node(table)?;
        let wall_id = l.push_node(wall)?;
        let chair_id = l.push_node(chair)?;
        let clock_id = l.push_node(clock)?;

        l.add_edge(clock_id, wall_id, "supported by")?;
        l.add_edge(table_id, chair_id, "next to")?;
        l.add_edge(chair_id, table_id, "next to")?;
        l.add_edge(table_id, wall_id, "in front of")?;

        // query nodes by label
        let furniture = sg.nodes_having(&["type"]);
        assert_eq!(furniture.len(), 1); // only one layer in the scene graph
        assert_eq!(furniture[0].len(), 4); // all nodes have "type" feature
        let furniture = sg.nodes_matching(&[&Feature::new("type", "furniture")]);
        assert_eq!(furniture.len(), 1); // only one layer in the scene graph
        assert_eq!(furniture[0].len(), 2); // only chair and table are furniture 
        assert!(furniture[0].iter().any(|n| n.id == chair_id));
        assert!(furniture[0].iter().any(|n| n.id == table_id));

        // query nodes by affordance
        let sit_nodes = sg.nodes_having(&["affordance"]);
        assert_eq!(sit_nodes.len(), 1); // only one layer in the scene graph
        assert_eq!(sit_nodes[0].len(), 3); // chair, table, wall have "affordance" feature
        let sit_nodes = sg.nodes_matching(&[&Feature::new("affordance", "sit")]);
        assert_eq!(sit_nodes.len(), 1); // only one layer in the scene graph
        assert_eq!(sit_nodes[0].len(), 1); // only chair has "sit" affordance
        assert_eq!(sit_nodes[0][0].id, chair_id);

        // query edges by src
        let edges_from_table = sg.edges_from(table_id);
        assert_eq!(edges_from_table.len(), 2);
        assert!(edges_from_table.iter().any(|e| e.dst == chair_id));
        assert!(edges_from_table.iter().any(|e| e.dst == wall_id));

        // query edges by dst;
        let edges_to_wall = sg.edges_to(wall_id);
        assert_eq!(edges_to_wall.len(), 2);
        assert!(edges_to_wall.iter().any(|e| e.src == clock_id));
        assert!(edges_to_wall.iter().any(|e| e.src == table_id));

        // query edges by description
        let next_to_edges = sg.edges_matching("next to");
        assert_eq!(next_to_edges.len(), 1); // only one layer in the scene graph
        assert_eq!(next_to_edges[0].len(), 2); // top layer
        assert!(
            next_to_edges[0]
                .iter()
                .any(|e| e.src == table_id && e.dst == chair_id)
        );
        assert!(
            next_to_edges[0]
                .iter()
                .any(|e| e.src == chair_id && e.dst == table_id)
        );

        Ok(())
    }

    fn cone() -> Observer {
        // Observer at origin, yaw=30°, pitch=5°, roll=0°
        let pos = Coordinate::new(0.0, 0.0, 0.0);
        let yaw = 0_f32.to_radians();
        let pitch = 0_f32.to_radians();
        let roll = 0_f32.to_radians();

        // Cone View Frustum: half-angle=35°, near=0.6, far=6.0
        let half_angle = 35_f32.to_radians();
        let near = 0.6;
        let far = 6.0;

        Observer::from_ypr(pos, yaw, pitch, roll, half_angle, near, far)
    }

    #[test]
    fn fov() -> Result<()> {
        let mut sg = SceneGraph::default();
        let inside_coords = Coordinate::new(0.0, 0.0, 1.0);
        let outside_coords = Coordinate::new(6.0, 6.0, 6.0);

        // first layer:
        // 100 coordinate nodes, half inside FOV, half outside, fully connected
        const NUM_COOR_NODES: usize = 150;
        let mut nodes = Vec::new();
        for id in 0..NUM_COOR_NODES {
            let coords = if (id / 15) % 2 == 0 {
                inside_coords
            } else {
                outside_coords
            };
            let node = sg.new_coordinates(coords.x, coords.y, coords.z, Vec::new());
            nodes.push(node);
        }
        let layer = sg.new_layer();
        for node in nodes {
            layer.push_node(node)?;
        }

        for src in 0..NUM_COOR_NODES {
            for dst in 0..NUM_COOR_NODES {
                let src = layer.node_id(src as u32);
                let dst = layer.node_id(dst as u32);
                assert!(layer.add_edge(src, dst, "connect").is_ok());
            }
        }

        // Second layer:
        // 10 semantic nodes, each parenting 10 coordinate nodes from the first layers
        // fully connected. around 5 semantic nodes father no visible coordinate nodes
        let mut nodes = Vec::new();
        const NUM_SEMANTIC_NODES: usize = NUM_COOR_NODES / 10;
        for id in 0..NUM_SEMANTIC_NODES {
            let node = sg.new_node(vec![Feature::new("name", format!("semantic {id}"))]);
            nodes.push(node);
        }
        let layer = sg.new_layer();
        layer.add_child(0)?;
        for node in nodes {
            layer.push_node(node)?;
        }
        for src in 0..NUM_SEMANTIC_NODES {
            for dst in 0..NUM_SEMANTIC_NODES {
                let src = layer.node_id((NUM_COOR_NODES + src) as u32);
                let dst = layer.node_id((NUM_COOR_NODES + dst) as u32);
                assert!(layer.add_edge(src, dst, "connect").is_ok());
            }
        }
        // eatch 10 nodes from the first layer under each semantic node
        for id in 0..NUM_COOR_NODES {
            let nestee = sg.layer(0)?.node_id(id as u32);
            let nester = sg.layer(1)?.node_id((NUM_COOR_NODES + id / 10) as u32);
            assert!(sg.nest(nestee).under(nester).is_ok());
        }

        // Third layer:
        // 1 node parenting all semantic nodes from the second layer
        let root_node = sg.new_node(vec![Feature::new("name", "root")]);
        let layer = sg.new_layer();
        layer.add_child(1)?;
        let root_id = layer.push_node(root_node)?;
        for id in 0..NUM_SEMANTIC_NODES {
            let nestee = sg.layer(1)?.node_id((NUM_COOR_NODES + id) as u32);
            assert!(sg.nest(nestee).under(root_id).is_ok());
        }

        // Query visible subgraph under root
        let cone = cone();
        let observed_sg = sg.visible_subgraph(&cone, root_id)?;

        // number of observed nodes in the first layer should be half of total
        let layer = observed_sg.layer(0)?;
        assert_eq!(layer.nodes.len(), NUM_COOR_NODES / 2);
        // only edges between visible nodes should be present
        let mut visible_node_ids: Vec<_> = layer.nodes.iter().map(|n| n.id).collect();
        visible_node_ids.sort();
        for src in &visible_node_ids {
            let mut edges = layer
                .node(*src)?
                .edges
                .iter()
                .map(|e| e.dst)
                .collect::<Vec<_>>();
            edges.sort();
            assert_eq!(&edges, &visible_node_ids);
        }

        // on the second layer, only semantic nodes parenting visible coordinate nodes should be present
        let pids = observed_sg
            .layer(0)?
            .nodes
            .iter()
            .filter_map(|n| n.pid)
            .collect::<HashSet<_>>();
        observed_sg.layer(1)?.nodes.iter().for_each(|n| {
            assert!(pids.contains(&n.id));
        });

        // some nodes from the second layer should be pruned
        assert!(pids.len() < NUM_SEMANTIC_NODES);
        // edges between remaining semantic nodes should be intact
        for src in &pids {
            let edges = observed_sg
                .layer(1)?
                .node(*src)?
                .edges
                .iter()
                .map(|e| e.dst)
                .collect::<HashSet<_>>();
            assert_eq!(&edges, &pids);
        }

        // on the third layer, the root node should be present
        let layer = observed_sg.layer(2)?;
        assert_eq!(layer.nodes.len(), 1);

        Ok(())
    }

    #[test]
    fn pos_query() -> Result<()> {
        use super::*;

        let mut sg = SceneGraph::default();

        // Create nodes
        let n1 = sg.new_coordinates(0.0, 0.0, 0.0, vec![]);
        let n2 = sg.new_coordinates(1.0, 1.0, 1.0, vec![]);
        let n3 = sg.new_coordinates(2.0, 2.0, 2.0, vec![]);

        // Create a layer and add nodes to it
        sg.new_indexed_layer();
        let n1_id = sg.layer_mut(0).unwrap().push_node(n1).unwrap();
        let n2_id = sg.layer_mut(0).unwrap().push_node(n2).unwrap();
        let n3_id = sg.layer_mut(0).unwrap().push_node(n3).unwrap();

        // create new semantic layer and nest all nodes under a new parent node
        let layer = sg.new_layer();
        layer.add_child(0).unwrap();

        // First test with indexing
        let parent_node = sg.new_indexed_node(vec![]);
        let pid = sg.top_layer_mut().unwrap().push_node(parent_node).unwrap();
        sg.nest(n1_id).under(pid).unwrap();
        sg.nest(n2_id).under(pid).unwrap();
        sg.nest(n3_id).under(pid).unwrap();

        let coord = Coordinate::new(0.6, 0.6, 0.6);
        let nearest_in_subtree = sg
            .nearest_in_subtree(&coord, pid, PosQueryCount::Count(2))
            .unwrap();
        assert_eq!(nearest_in_subtree.len(), 2);
        assert_eq!(
            sg.node(nearest_in_subtree[0].0)?.coordinates.unwrap(),
            Coordinate::new(1.0, 1.0, 1.0)
        );
        assert_eq!(
            sg.node(nearest_in_subtree[1].0)?.coordinates.unwrap(),
            Coordinate::new(0.0, 0.0, 0.0)
        );

        // Now test without indexing
        let parent_node = sg.new_node(vec![]);
        let pid = sg.top_layer_mut().unwrap().push_node(parent_node).unwrap();
        sg.nest(n1_id).under(pid).unwrap();
        sg.nest(n2_id).under(pid).unwrap();
        sg.nest(n3_id).under(pid).unwrap();

        let coord = Coordinate::new(0.6, 0.6, 0.6);
        let nearest_in_subtree = sg
            .nearest_in_subtree(&coord, pid, PosQueryCount::Count(2))
            .unwrap();
        assert_eq!(nearest_in_subtree.len(), 2);
        assert_eq!(
            sg.node(nearest_in_subtree[0].0)?.coordinates.unwrap(),
            Coordinate::new(1.0, 1.0, 1.0)
        );
        assert_eq!(
            sg.node(nearest_in_subtree[1].0)?.coordinates.unwrap(),
            Coordinate::new(0.0, 0.0, 0.0)
        );
        Ok(())
    }
    #[test]
    fn bounds() {
        // Scene Graph Construction
        let mut ygg = SceneGraph::default();
        let btm_lid = ygg.new_layer().id;
        let mid_lid = ygg.new_layer().id;
        let top_lid = ygg.new_layer().id;
        ygg.layer_mut(top_lid).unwrap().add_child(mid_lid).unwrap();
        ygg.layer_mut(mid_lid).unwrap().add_child(btm_lid).unwrap();

        let btm_node = ygg.new_coordinates(1_f32, 2_f32, 3_f32, Vec::new());
        let mid_node = ygg.new_node(Vec::new());
        let top_node = ygg.new_coordinates(4_f32, 5_f32, 6_f32, Vec::new());

        let btm_nid = ygg.layer_mut(btm_lid).unwrap().push_node(btm_node).unwrap();
        let mid_nid = ygg.layer_mut(mid_lid).unwrap().push_node(mid_node).unwrap();
        let top_nid = ygg.layer_mut(top_lid).unwrap().push_node(top_node).unwrap();

        assert_eq!(
            ygg.node(btm_nid).unwrap().bounds,
            Some(Bounds::bounding_box(1.0, 1.0, 2.0, 2.0, 3.0, 3.0))
        );
        assert_eq!(ygg.node(mid_nid).unwrap().bounds, None);
        assert_eq!(
            ygg.node(top_nid).unwrap().bounds,
            Some(Bounds::bounding_box(4.0, 4.0, 5.0, 5.0, 6.0, 6.0))
        );

        // Nesting would update the bounds of the parent node.
        ygg.nest(btm_nid).under(mid_nid).unwrap();
        // mid_node's bounds should still be None since bounds is disabled.
        assert_eq!(ygg.node(mid_nid).unwrap().bounds, None);
        // top_node's bounds should not change
        assert_eq!(
            ygg.node(top_nid).unwrap().bounds,
            Some(Bounds::bounding_box(4.0, 4.0, 5.0, 5.0, 6.0, 6.0))
        );

        // Nesting mid_node under top_node would further update the bounds
        // of top_node to encompass both top and mid node.
        ygg.nest(mid_nid).under(top_nid).unwrap();
        assert_eq!(
            ygg.node(top_nid).unwrap().bounds,
            Some(Bounds::bounding_box(1.0, 4.0, 2.0, 5.0, 3.0, 6.0))
        );

        // Detaching btm_node from mid_node would update top_nodes's bounds again.
        ygg.detach(mid_nid).from(top_nid).unwrap();
        assert_eq!(
            ygg.node(top_nid).unwrap().bounds,
            Some(Bounds::bounding_box(4.0, 4.0, 5.0, 5.0, 6.0, 6.0))
        );
    }
}
