# Graph Queries
`yggdrasil` has support for common graph queries as well as the queries we saw thus far.


## Node by id
Starting with the basics, besides the labing matching, a node in `yggdrasil` can also be retrieved via its id.
```rust 
    // Here we first retrieve the root node by matching for its features, and then use its id to
    // retrieve the same node using its id.
    let root_node_id = ygg
        .layer(3)?
        .nodes_matching(&[&Feature::new("name", "root")])[0]
        .id;
```
For immutable access:
```rust 
    let root_node = ygg.node(root_node_id)?;
    assert_eq!(root_node.features, vec![Feature::new("name", "root")]);
```

And for muttable access:
```rust 
   let mut root_node = ygg.node_mut(root_node_id)?;
   root_node.features.push(Feature::new("is_root", "true"));
```

## Layer by id
As shown many times throughout this demo, a layer can also be retrieved by its id
```rust 
    let root_layer = ygg.layer(3)?;
    assert_eq!(root_layer.id, ygg.layer_of(root_node_id));
```

## All Nodes
Using the retrieved layer, one can iterate over all nodes in the layer
```rust 
    let layer = ygg.layer(0)?
    for node in layer.all_nodes(){
        println!("Node: {:?}",node.id)
    }
```


## Sub Graph
`yggdrasil` allows you to get a sub graph of the scene, given a reference root node. This is specially useful when dealing with agents with limited computational resources that might not have a need for the entire scene and just a sub set of the scene would be enough for them.

```rust 
    // Subgraph retrieval: retrieve the subtree under the "car" node. This query would return a new
    // scene graph containing only the nodes in the subtree under the "car" node, along with the
    // edges between those nodes.
    let car_node_id = ygg
        .layer(1)?
        .nodes_matching(&[&Feature::new("type", "car")])[0]
        .id;

    let car_subgraph = ygg.subgraph(car_node_id)?;

    // We expect the subgraph to contain the "car" node and its 10 point cloud nodes, and no other
    // nodes.
    assert_eq!(car_subgraph.layer(0)?.all_nodes().len(), 10);
    assert_eq!(car_subgraph.layer(1)?.all_nodes().len(), 1);
    assert_eq!(car_subgraph.layer(2)?.all_nodes().len(), 0);
    assert_eq!(car_subgraph.layer(3)?.all_nodes().len(), 0);
```

## Merge
Updates in `yggdrasil` can be in the form of sub graph merge requests, that is, merging a newer version of a sub graph of the original scene graph into the scene graph. This is usefull in scenarios where a global scene graph is maintained somewhere and agents can send their local updates to it.

`yggdrasil` will do the following during the merge:
1. Nodes with the same ID will be merged, meaning the node in the original scene graph gets updated.
2. New nodes in the subgraph will be added to the original scene graph.
3. Node deletion is not supported in this operation.
4. Edges from the original scene graph from `src` to `dst` where both `src` and `dst` are present in the subgraph but the edge itself is not, are removed from the scene graph.
5. New edges in the subgraph will be added to the original scene graph.
```rust 
    ygg.merge(car_subgraph)?;
```
