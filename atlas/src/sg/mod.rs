mod fov;
mod layer;
mod node;
pub mod parse;
mod sg;

pub use fov::Observer;
pub use layer::Layer;
pub use node::{Coordinate, Edge, Feature, Node};
pub use sg::SceneGraph;

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use super::*;
    use crate::error::Result;

    #[test]
    fn api() -> Result<()> {
        let mut sg = SceneGraph::default();

        // create nodes
        let node1 = sg.new_node(vec![Feature::new("name", "Node 1")]);
        let node2 = sg.new_node(vec![Feature::new("name", "Node 2")]);
        let node3 = sg.new_node(vec![Feature::new("name", "Node 3")]);
        let id2 = node2.id;
        let id3 = node3.id;
        let id1 = node1.id;

        // create a semantic layer and add nodes to layers
        let semantic_layer = sg.new_layer();
        semantic_layer.push_node(node2);
        semantic_layer.push_node(node3);
        // create the second semantic layer
        let semantic_layer = sg.new_layer();
        semantic_layer.push_node(node1);

        // nesting
        assert!(sg.nest(id2).under(id1).is_ok());
        assert!(sg.nest(id3).under(id1).is_ok());
        assert_eq!(sg.node(id1)?.children, vec![id2, id3]);
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
        assert_eq!(sg.node(id1)?.children, vec![id3]);

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
        let chair_id = chair.id;
        let table_id = table.id;
        let wall_id = wall.id;
        let clock_id = clock.id;

        // create layers and add nodes to layers
        let l = sg.new_layer();
        l.push_node(table);
        l.push_node(wall);
        l.push_node(chair);
        l.push_node(clock);

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
            layer.push_node(node);
        }

        for src in 0..NUM_COOR_NODES {
            for dst in 0..NUM_COOR_NODES {
                assert!(layer.add_edge(src, dst, "connect").is_ok());
            }
        }

        // Second layer:
        // 10 semantic nodes, each parenting 10 coordinate nodes from the first layers
        // fully connected. around 5 semantic nodes father no visible coordinate nodes
        let mut nodes = Vec::new();
        const NUM_SEMANTIC_NODES: usize = NUM_COOR_NODES / 10;
        for id in 0..NUM_SEMANTIC_NODES {
            let node = sg.new_node(vec![Feature::new("name", &format!("semantic {}", id))]);
            nodes.push(node);
        }
        let layer = sg.new_layer();
        for node in nodes {
            layer.push_node(node);
        }
        for src in 0..NUM_SEMANTIC_NODES {
            for dst in 0..NUM_SEMANTIC_NODES {
                assert!(
                    layer
                        .add_edge(NUM_COOR_NODES + src, NUM_COOR_NODES + dst, "connect")
                        .is_ok()
                );
            }
        }
        // eatch 10 nodes from the first layer under each semantic node
        for id in 0..NUM_COOR_NODES {
            assert!(sg.nest(id).under(NUM_COOR_NODES + id / 10).is_ok());
        }

        // Third layer:
        // 1 node parenting all semantic nodes from the second layer
        let root_node = sg.new_node(vec![Feature::new("name", "root")]);
        let root_id = root_node.id;
        let layer = sg.new_layer();
        layer.push_node(root_node);
        for id in 0..NUM_SEMANTIC_NODES {
            assert!(sg.nest(NUM_COOR_NODES + id).under(root_id).is_ok());
        }

        // Query visible subgraph under root
        let cone = cone();
        let observed_sg = sg.visible_subgraph(cone, root_id)?;

        // number of observed nodes in the first layer should be half of total
        let layer = observed_sg.layer(0)?;
        assert_eq!(layer.nodes.len(), NUM_COOR_NODES / 2);
        // only edges between visible nodes should be present
        let mut visible_node_ids: Vec<usize> = layer.nodes.iter().map(|n| n.id).collect();
        visible_node_ids.sort();
        for src in &visible_node_ids {
            let mut edges = layer
                .node(*src)?
                .edges
                .iter()
                .map(|e| e.dst)
                .collect::<Vec<usize>>();
            edges.sort();
            assert_eq!(&edges, &visible_node_ids);
        }

        // on the second layer, only semantic nodes parenting visible coordinate nodes should be present
        let pids = observed_sg
            .layer(0)?
            .nodes
            .iter()
            .filter_map(|n| n.pid)
            .collect::<HashSet<usize>>();
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
                .collect::<HashSet<usize>>();
            assert_eq!(&edges, &pids);
        }

        // on the third layer, the root node should be present
        let layer = observed_sg.layer(2)?;
        assert_eq!(layer.nodes.len(), 1);

        Ok(())
    }
}
