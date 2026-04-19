use scene_graph as yggdrasil;
use yggdrasil::{Coordinate, Feature, Observer, PosQueryCount};

fn main() -> yggdrasil::Result<()> {
    // Create a demo scene graph with the following structure:
    //
    // root_layer (root node with name "root")
    // ├────────── dynamic_objects_layer    (3 dynamic nodes with type "Human", "Dog", "Cat", and
    // │                            │           edges "Human" -holding-> "Dog" -looking-> "Cat")
    // │                            │
    // │                            │
    // │                            │
    // └─ static_objects_layer      │       (7 static nodes with type "car", "tree", "building", "bicycle", "bench", "sign", "lamp")
    //          │                   │
    //          └──── point_cloud_layer (100 point cloud nodes)
    //
    let ygg = create_demo_graph()?;

    // Queries that work with node features and edge descriptions.
    run_basic_label_retrieval_queries(&ygg)?;

    // Queries that work with coordinates and spatial relationships.
    run_nearest_neighbors_queris(&ygg)?;

    // Queries for field of view retrieval.
    run_field_of_view_queries(&ygg)?;

    Ok(())
}

fn run_field_of_view_queries(ygg: &scene_graph::SceneGraph) -> yggdrasil::Result<()> {
    // Create a cone observer which can view all the positive space, i.e. the space where none of the
    // x, y, z coordinates are negative. The observer is located at the origin and is looking
    // towards the (1, 1, 1) direction, with a field of view of 90 degrees and a far reach of 6
    // units.
    let pos = Coordinate::ZERO;
    let yaw = 45_f32.to_radians();
    let pitch = -35.26439_f32.to_radians();
    let roll = 0.0;
    let half_angle = (1.0 / 3.0_f32.sqrt()).acos() + 0.001;

    let near = 0.001;
    let far = 6_f32;

    let observer = Observer::from_ypr(pos, yaw, pitch, roll, half_angle, near, far);

    // Retrieve the root node of the scene graph.
    let root_node_id = ygg
        .layer(3)?
        .nodes_matching(&[&Feature::new("name", "root")])[0]
        .id;

    // Observable subgraph of this observer would contain some of the static objects and none of
    // the dynamic ones, since they are all located in the (10, 10, 10) to (20, 20, 20) region,
    // which is outside the observer's view frustum's far range.
    // We use the root node as the reference node for this query to check the visibility of all
    // nodes in the scene graph.
    let subgraph = ygg.visible_subgraph(&observer, root_node_id)?;

    // We expect to see some of the point cloud nodes in the subgraph
    assert!(subgraph.layer(0)?.all_nodes().len() > 0);

    // For each observed point cloud node, we expect its parent static object to also apear in the
    // subgraph.
    let uniq_static_objects = subgraph
        .layer(0)?
        .all_nodes()
        .filter_map(|n| n.pid)
        .collect::<std::collections::HashSet<_>>()
        .len();
    assert_eq!(subgraph.layer(1)?.all_nodes().len(), uniq_static_objects);

    // We expect to see none of the dynamic objects in the subgraph.
    assert_eq!(subgraph.layer(2)?.all_nodes().len(), 0);

    // Running the same field of view query with a dynamic object node as the reference node should
    // return an empty subgraph, since none of the dynamic objects are within the field of view of
    // the observer.
    let human_node_id = ygg
        .layer(2)?
        .nodes_matching(&[&Feature::new("type", "Human")])[0]
        .id;
    let subgraph = ygg.visible_subgraph(&observer, human_node_id)?;
    assert_eq!(subgraph.layer(0)?.all_nodes().len(),0);
    assert_eq!(subgraph.layer(1)?.all_nodes().len(),0);
    assert_eq!(subgraph.layer(2)?.all_nodes().len(),0);

    // Expanding the `far` reach of the observer to 35 should include all the dynamic objects as
    // well as the static ones, meaning that the subgraph should be the same as the original scene
    // graph.
    let observer = Observer::from_ypr(pos, yaw, pitch, roll, half_angle, near, 35.0);
    let subgraph = ygg.visible_subgraph(&observer, root_node_id)?;
    assert_eq!(subgraph.layer(0)?.all_nodes().len(),ygg.layer(0)?.all_nodes().len());
    assert_eq!(subgraph.layer(1)?.all_nodes().len(),ygg.layer(1)?.all_nodes().len());
    assert_eq!(subgraph.layer(2)?.all_nodes().len(),ygg.layer(2)?.all_nodes().len());

    Ok(())
}

fn run_nearest_neighbors_queris(ygg: &scene_graph::SceneGraph) -> yggdrasil::Result<()> {
    // Retrieve the 5 nearest nodes to the (5, 5, 5) coordinates. Nodes returned by this query
    // should all be points of static objects, since the dynamic objects are all located in the
    // (10, 10, 10) to (20, 20, 20) region.
    let neighbors = ygg.nearest_nodes(&[5.0, 5.0, 5.0].into(), PosQueryCount::Count(5));
    assert_eq!(neighbors.len(), 5);
    // Check that all the neighbors are from the static object layer.
    assert!(
        neighbors
            .iter()
            .filter_map(|(n, _)| n.pid)
            .all(|pid| ygg.layer_of(pid) == 1)
    );

    // Retrieve the 5 nearest nodes to the (5, 5, 5) coordinates from the point cload layer. The
    // results of this query should be the same as the previous one.
    let neighbors_in_pc_layer = ygg
        .layer(0)?
        .nearest_nodes(&[5.0, 5.0, 5.0].into(), PosQueryCount::Count(5));

    assert_eq!(neighbors_in_pc_layer.len(), 5);
    // Check that all the neighbors are from the static object layer.
    assert!(
        neighbors_in_pc_layer
            .iter()
            .filter_map(|(n, _)| n.pid)
            .all(|pid| ygg.layer_of(pid) == 1)
    );
    // Check that the results of this query are the same as the previous one.
    assert!(
        neighbors_in_pc_layer
            .iter()
            .map(|(n, _)| n.id)
            .eq(neighbors.iter().map(|(n, _)| n.id))
    );

    // Retrieve the 5 nearest nodes to the (5, 5, 5) coordinates from the subtree of the "Human" node.
    // Since the "Human" node is located in the (10, 10, 10) to (20, 20, 20) region, the nearest
    // nodes should be different from the previous queries.
    let human_node_id = ygg
        .layer(2)?
        .nodes_matching(&[&Feature::new("type", "Human")])[0]
        .id;
    let neighbors_in_human_subtree = ygg.nearest_in_subtree(
        &[5.0, 5.0, 5.0].into(),
        human_node_id,
        PosQueryCount::Count(5),
    )?;
    assert_eq!(neighbors_in_human_subtree.len(), 5);
    // Check that all the neighbors are from the Human subtree. Since all the nodes with
    // coordinates are from layer 0 and their immediate parents are in layer 1 or 2, we can simply
    // check for parent id instead of traversing the whole subtree.
    assert!(
        neighbors_in_human_subtree
            .iter()
            .filter_map(|(n, _)| n.pid)
            .all(|pid| pid == human_node_id)
    );
    // Check that the results of this query are different from the previous ones.
    assert!(
        !neighbors_in_human_subtree
            .iter()
            .map(|(n, _)| n.id)
            .eq(neighbors.iter().map(|(n, _)| n.id))
    );

    // Asking for 20 neighbors when the Human subtree only has 10 nodes with coordinates should
    // return all the 10 nodes without any error.
    let neighbors_in_human_subtree = ygg.nearest_in_subtree(
        &[5.0, 5.0, 5.0].into(),
        human_node_id,
        PosQueryCount::Count(20),
    )?;
    assert_eq!(neighbors_in_human_subtree.len(), 10);

    // Retrieving all nodes within a radius of 5 from the (5, 5, 5) coordinates. The results should
    // contain less than or equal to 70 nodes, since this is the number of point cloud nodes that
    // are located in the (0, 0, 0) to (10, 10, 10) region.
    let nodes_within_5 = ygg.nodes_within(&[5.0, 5.0, 5.0].into(), 5.0, PosQueryCount::All);
    assert!(nodes_within_5.len() <= 70);

    // Expanding the radious to 26 should return all the point cloud nodes, since now both the
    // static and dynamic nodes are within the radius.
    let nodes_within_26 = ygg.nodes_within(&[5.0, 5.0, 5.0].into(), 26.0, PosQueryCount::All);
    assert_eq!(nodes_within_26.len(), 100);

    // Retrieving exactly 1 node within a radius of 5 from the (5, 5, 5) coordinates should return
    // the first node retured by the first nodes_within query.
    let nodes_within = ygg.nodes_within(&[5.0, 5.0, 5.0].into(), 5.0, PosQueryCount::Count(1));
    assert_eq!(nodes_within.len(), 1);
    assert_eq!(nodes_within[0].0.id, nodes_within_5[0].0.id);

    Ok(())
}

fn run_basic_label_retrieval_queries(ygg: &scene_graph::SceneGraph) -> yggdrasil::Result<()> {
    // Retrieve all nodes with the feature "type" equal to "lamp".

    // Running this query on the scene graph would return a vector of vector of nodes, each vector
    // of nodes representing the nodes matched in a specific layer. Layers with no matches would be
    // represented as empty vectors. In this case, we expect to find one node in the static object
    // layer, therefore the result would be a vector containing one vector with one node, and the
    // rest of the vectors would be empty.
    let lamp_nodes = ygg.nodes_matching(&[&Feature::new("type", "lamp")]);
    assert_eq!(lamp_nodes.iter().flatten().collect::<Vec<_>>().len(), 1);
    assert_eq!(lamp_nodes[1].len(), 1);
    assert_eq!(
        lamp_nodes.iter().flatten().collect::<Vec<_>>()[0].features,
        vec![Feature::new("type", "lamp")]
    );

    // The same query can be run on the layer level, which would only return the matches in that
    // specific node. This query would run faster since there are less nodes to go through, and it
    // would return a single vector containing the matched nodes.
    let lamp_nodes = ygg
        .layer(1)?
        .nodes_matching(&[&Feature::new("type", "lamp")]);
    assert_eq!(lamp_nodes.len(), 1);
    assert_eq!(lamp_nodes[0].features, vec![Feature::new("type", "lamp")]);

    // Retrieve all nodes that have a feature called "type". Same as the previous query, this query
    // can also be run on both scene graph and layer level, and it would return all nodes that have
    // a feature with the key "type", regardless of the value. In this case, we expect to find 10
    // nodes in total, 7 in the static object layer and 3 in the dynamic object layer.
    let nodes_with_type = ygg.nodes_having(&["type"]);
    assert_eq!(
        nodes_with_type.iter().flatten().collect::<Vec<_>>().len(),
        10
    );
    assert_eq!(nodes_with_type[0].len(), 0);
    assert_eq!(nodes_with_type[1].len(), 7);
    assert_eq!(nodes_with_type[2].len(), 3);
    assert_eq!(nodes_with_type[3].len(), 0);

    let static_nodes_with_type = ygg.layer(1)?.nodes_having(&["type"]);
    assert_eq!(static_nodes_with_type.len(), 7);

    let dynamic_nodes_with_type = ygg.layer(2)?.nodes_having(&["type"]);
    assert_eq!(dynamic_nodes_with_type.len(), 3);

    // Retrieve Human and Dog nodes.
    let human_node_id = ygg
        .layer(2)?
        .nodes_matching(&[&Feature::new("type", "Human")])[0]
        .id;
    let dog_node_id = ygg
        .layer(2)?
        .nodes_matching(&[&Feature::new("type", "Dog")])[0]
        .id;

    // Retrieve all relationships matching the pattern "holding" in the description. This query would
    // return all edges in the scene graph that have the "holding" description, along with the
    // source and destination nodes of those edges. In this case, we expect to only find a single
    // edge in the dynamic object layer, connecting the "Human" node to the "Dog" node.
    //
    // Note that this query too can be run on both levels and the outputs have the same format.

    let holding_edges = ygg.edges_matching("holding");
    assert_eq!(holding_edges.iter().flatten().collect::<Vec<_>>().len(), 1);
    assert_eq!(holding_edges[0].len(), 0);
    assert_eq!(holding_edges[1].len(), 0);
    assert_eq!(holding_edges[2].len(), 1);
    assert_eq!(holding_edges[3].len(), 0);

    assert_eq!(holding_edges[2][0].src, human_node_id);
    assert_eq!(holding_edges[2][0].dst, dog_node_id);
    assert_eq!(holding_edges[2][0].desc, "holding");

    // Retrieve all edges from, or to a certain node. For example, we can retrieve all edges that
    // are from/or to the "Human" node.
    //
    // Note that this query can also be run on both levels. The only difference is that unlike the
    // previous queries, when run on the scene graph level, this query would return all edges in a
    // single vector, since it is impossible for a node to be on multiple layers, and therefore
    // there is no need to separate the results by layer.
    let nodes_from_human = ygg.edges_from(human_node_id);
    assert_eq!(nodes_from_human.len(), 1);
    assert_eq!(nodes_from_human[0].src, human_node_id);
    assert_eq!(nodes_from_human[0].dst, dog_node_id);
    assert_eq!(nodes_from_human[0].desc, "holding");

    let nodes_to_dog = ygg.edges_to(dog_node_id);
    assert_eq!(nodes_to_dog.len(), 1);
    assert_eq!(nodes_to_dog[0].src, human_node_id);
    assert_eq!(nodes_to_dog[0].dst, dog_node_id);
    assert_eq!(nodes_to_dog[0].desc, "holding");

    Ok(())
}

fn create_demo_graph() -> yggdrasil::Result<yggdrasil::SceneGraph> {
    let mut ygg = yggdrasil::SceneGraph::default();

    let point_cloud_layer_id = ygg.new_layer().id;
    let static_objects_layer_id = ygg.new_layer().id;
    let dynamic_objects_layer_id = ygg.new_layer().id;
    let root_layer_id = ygg.new_layer().id;

    // Define the hierarchy of layers
    // We try to create the following DAG structure:
    //
    // root_layer
    // ├────────── dynamic_objects_layer
    // │                            │
    // │                            │
    // │                            │
    // └─ static_objects_layer      │
    //          │                   │
    //          └──── point_cloud_layer
    //
    ygg.layer_mut(root_layer_id)?
        .add_child(dynamic_objects_layer_id)?;
    ygg.layer_mut(root_layer_id)?
        .add_child(static_objects_layer_id)?;
    ygg.layer_mut(dynamic_objects_layer_id)?
        .add_child(point_cloud_layer_id)?;
    ygg.layer_mut(static_objects_layer_id)?
        .add_child(point_cloud_layer_id)?;

    // Create a root node with a name feature
    // This node will serve as the parent for all other nodes in the scene graph.
    let root_node = ygg.new_node(vec![Feature::new("name", "root")]);
    let root_node_id = ygg.layer_mut(root_layer_id)?.push_node(root_node);

    // Create 100 point cloud nodes with random coordinates. The first 70 nodes will have
    // coordinates between 0 and 10, and the next 30 will have coordinates between 10 and 20.
    // Each node will have a feature "point_cloud_id" with a unique integer value.
    // The nodes will be added to the point cloud layer.
    let mut point_cloud = (0..70)
        .map(|_| {
            let x = rand::random::<f32>() * 10.0;
            let y = rand::random::<f32>() * 10.0;
            let z = rand::random::<f32>() * 10.0;
            (x, y, z)
        })
        .collect::<Vec<_>>();
    point_cloud.extend((0..30).map(|_| {
        let x = rand::random::<f32>() * 10.0 + 10.0;
        let y = rand::random::<f32>() * 10.0 + 10.0;
        let z = rand::random::<f32>() * 10.0 + 10.0;
        (x, y, z)
    }));

    let mut point_cluod_node_ids = Vec::new();
    for (i, (x, y, z)) in point_cloud.iter().enumerate() {
        let point = ygg.new_coordinates(*x, *y, *z, vec![Feature::new("point_cloud_id", i)]);
        let point_cloud_node_id = ygg.layer_mut(point_cloud_layer_id)?.push_node(point);
        point_cluod_node_ids.push(point_cloud_node_id);
    }

    // Create 7 semantic nodes, representing different object in the scene
    // Each node will have a "type" feature with a value like "car", "tree", etc.
    // The nodes will be added to the static layer and will be connected to the root.
    let mut static_object_node_ids = Vec::new();
    let static_object_types = [
        "car", "tree", "building", "bicycle", "bench", "sign", "lamp",
    ];
    for i in 0..static_object_types.len() {
        let object_node = ygg.new_node(vec![Feature::new("type", static_object_types[i])]);
        let object_node_id = ygg
            .layer_mut(static_objects_layer_id)?
            .push_node(object_node);
        static_object_node_ids.push(object_node_id);

        // Nest all the point cloud nodes under this object's semantic node.
        for point_cloud_node_id in &point_cluod_node_ids[i * 10..(i + 1) * 10] {
            ygg.nest(*point_cloud_node_id).under(object_node_id)?;
        }

        // Nest this object's semantic node under the root node.
        ygg.nest(object_node_id).under(root_node_id)?;
    }

    // Create 3 dynamic nodes representing moving objects in the scene.
    // Each node will have a "type" feature with a value like "Humand", "Dog", and "Cat".
    // The nodes will be added to the dynamic layer and will be connected to the root.
    let mut dynamic_node_ids = Vec::new();
    let dynamic_object_types = ["Human", "Dog", "Cat"];
    for i in 0..dynamic_object_types.len() {
        let dynamic_node = ygg.new_node(vec![Feature::new("type", dynamic_object_types[i])]);
        let dynamic_node_id = ygg
            .layer_mut(dynamic_objects_layer_id)?
            .push_node(dynamic_node);
        dynamic_node_ids.push(dynamic_node_id);

        // Nest all the point cloud nodes under this object's semantic node.
        let offset = static_object_types.len() * 10;
        for point_cloud_node_id in &point_cluod_node_ids[offset + i * 10..offset + (i + 1) * 10] {
            ygg.nest(*point_cloud_node_id).under(dynamic_node_id)?;
        }

        // Nest this dynamic node under the root node.
        ygg.nest(dynamic_node_id).under(root_node_id)?;
    }

    // Create semantic relationships between dynamic nodes.
    // For example, we can say that:
    // 1. the "Human" node is "holding" the "Dog" node.
    // 2. the "Dog" node is "looking" the "Cat" node.
    ygg.layer_mut(dynamic_objects_layer_id)?.add_edge(
        dynamic_node_ids[0],
        dynamic_node_ids[1],
        "holding",
    )?;
    ygg.layer_mut(dynamic_objects_layer_id)?.add_edge(
        dynamic_node_ids[1],
        dynamic_node_ids[2],
        "looking",
    )?;
    Ok(ygg)
}
