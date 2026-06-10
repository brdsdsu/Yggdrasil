# Basic Label Retrieval

## Retrieve nodes matching a specific feature (both key and value)

Running this query on the scene graph would return a vector of vector of nodes, each vector
of nodes representing the nodes matched in a specific layer. Layers with no matches would be
represented as empty vectors. In this case, we expect to find one node in the static object
layer, therefore the result would be a vector containing one vector with one node, and the
rest of the vectors would be empty.
```rust 
    // Retrieve all nodes with the feature "type" equal to "lamp".
    let lamp_nodes = ygg.nodes_matching(&[&Feature::new("type", "lamp")]);

    assert_eq!(lamp_nodes.iter().flatten().collect::<Vec<_>>().len(), 1);
    assert_eq!(lamp_nodes[1].len(), 1);
    assert_eq!(
        lamp_nodes.iter().flatten().collect::<Vec<_>>()[0].features,
        vec![Feature::new("type", "lamp")]
    );

```

The same query can be run on the layer level, which returns a vector of nodes, where the nodes are all matches in that
specific layer.
```rust 
    let lamp_nodes = ygg
        .layer(1)?
        .nodes_matching(&[&Feature::new("type", "lamp")]);

    assert_eq!(lamp_nodes.len(), 1);
    assert_eq!(lamp_nodes[0].features, vec![Feature::new("type", "lamp")]);
```

## Retrieve nodes having a feature with a specific key

Retrieve all nodes that have a feature with a specific key. Same as the previous query, this query
can also be run on both scene graph and layer level, and it would return all nodes that have
a feature with that specific key, regardless of the value. In this case, we expect to find 10 in total
nodes that have a feature with the key "type", 7 in the `static object` layer and 3 in the `dynamic object` layer.
```rust 
    // Scene Graph level query
    let nodes_with_type = ygg.nodes_having(&["type"]);

    assert_eq!(
        nodes_with_type.iter().flatten().collect::<Vec<_>>().len(),
        10
    );
    assert_eq!(nodes_with_type[0].len(), 0);
    assert_eq!(nodes_with_type[1].len(), 7);
    assert_eq!(nodes_with_type[2].len(), 3);
    assert_eq!(nodes_with_type[3].len(), 0);

    
    // Layer level query
    let static_nodes_with_type = ygg.layer(1)?.nodes_having(&["type"]);
    assert_eq!(static_nodes_with_type.len(), 7);

    let dynamic_nodes_with_type = ygg.layer(2)?.nodes_having(&["type"]);
    assert_eq!(dynamic_nodes_with_type.len(), 3);
```

## Retrieve edges matching a specific description
 Retrieve all edges matching the given pattern in the description. This query would
 return all matching edges in the scene graph, along with the source and destination nodes 
 of those edges. In this case, we expect to only find a single
 edge matching the "holding" pattern in the dynamic object layer, 
 connecting the "Human" node to the "Dog" node.

 Note that this query too can be run on both levels and the outputs have the same format.

```rust 

    let holding_edges = ygg.edges_matching("holding");
    assert_eq!(holding_edges.iter().flatten().collect::<Vec<_>>().len(), 1);
    assert_eq!(holding_edges[0].len(), 0);
    assert_eq!(holding_edges[1].len(), 0);
    assert_eq!(holding_edges[2].len(), 1);
    assert_eq!(holding_edges[3].len(), 0);

    // Retrieve Human and Dog nodes.
    let human_node_id = ygg
        .layer(2)?
        .nodes_matching(&[&Feature::new("type", "Human")])[0]
        .id;
    let dog_node_id = ygg
        .layer(2)?
        .nodes_matching(&[&Feature::new("type", "Dog")])[0]
        .id;
    assert_eq!(holding_edges[2][0].src, human_node_id);
    assert_eq!(holding_edges[2][0].dst, dog_node_id);
    assert_eq!(holding_edges[2][0].desc, "holding");
```

## Retrieve edges from/to a node
Retrieve all edges from, or to a certain node. For example, we can retrieve all edges that
are from/to the "Human" node. 

Note that this query can also be run on both levels. The only difference is that, unlike the
previous queries, when run on the scene graph level this query would return all edges in a
single vector, since edges are not associated with layers, and therefore
there is no need to separate the results by layer.
```rust 
    // edges from human
    let nodes_from_human = ygg.edges_from(human_node_id);

    assert_eq!(nodes_from_human.len(), 1);
    assert_eq!(nodes_from_human[0].src, human_node_id);
    assert_eq!(nodes_from_human[0].dst, dog_node_id);
    assert_eq!(nodes_from_human[0].desc, "holding");

    // edges to human
    let nodes_to_dog = ygg.edges_to(dog_node_id);

    assert_eq!(nodes_to_dog.len(), 1);
    assert_eq!(nodes_to_dog[0].src, human_node_id);
    assert_eq!(nodes_to_dog[0].dst, dog_node_id);
    assert_eq!(nodes_to_dog[0].desc, "holding");
```
