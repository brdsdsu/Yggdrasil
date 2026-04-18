use scene_graph as yggdrasil;
use yggdrasil::Feature;

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

    // Create 100 point cloud nodes with random coordinates between 0 and 10.
    // Each node will have a feature "point_cloud_id" with a unique integer value.
    // The nodes will be added to the point cloud layer.
    let point_cloud = (0..100)
        .map(|_| {
            let x = rand::random::<f32>() * 10.0;
            let y = rand::random::<f32>() * 10.0;
            let z = rand::random::<f32>() * 10.0;
            (x, y, z)
        })
        .collect::<Vec<_>>();

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
