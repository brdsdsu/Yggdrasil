# Nearest Neighbors Queries

A **Nearest Neighbor** query is when given a coordinate, you want to find the closest point (or `m` closest points) from a set of coordinates to the given query point. `yggdrasil` supports a variety of nearest neighbor queries on the scene graph. There are three different ways one can run a nearest neighbor query on the scene graph, on scene graph level, on layer level, and on node level:

1. On the layer level, the node with closest coordinate within that layer is returned.
2. On the scene graph level, the closest node is chosen among all the nodes in all the layers.
3. On the node level, the closest node within the subtree of the given node is returened. The subtree of a node `r` is defined as a scene graph in which all the nodes are decendant of `r`. you can think of the subtree of `r` as a sub-scene-graph from the original scene graph where the root node is `r`.

With that explained, lets look at how we can run different nearest neighbor queries in `yggdrasil`


## Nearest Node
First lets start by retrieving the 5 nearest nodes ot coordinates (5, 5, 5) in the entire scene graph.
```rust 
    use yggdrasil::PosQueryCount;

    let neighbors = ygg.nearest_nodes(&[5.0, 5.0, 5.0].into(), PosQueryCount::Count(5));
    assert_eq!(neighbors.len(), 5);
```
Since in our graph creation phase, we randomly assigned static objects points 
from the (0, 0, 0) to (10, 10, 10) region, and assigned dynamic objects points 
from the (10, 10, 10) to (20, 20, 20) region, we should expect to find that all 
the returned points are from the static objects. we can assert that by checking
the layer id of their respective parent in the scene graph.
```rust 
    // Check that all the neighbors are from the static object layer.
    assert!(
        neighbors
            .iter()
            .filter_map(|(n, _)| n.pid)
            .all(|pid| ygg.layer_of(pid) == 1)
    );
```

Next, we can repeat the same query on the layer level. Knowing that only the point cloud layer 
has coordinates assigned to its node, we can run the exact same query on the point cloud layer
and expect the exact same outputs.
```rust 
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
```

Lastly, we can run the same query on a specific node. In this example, we choose the Human node 
and try to find the 5 closest nodes in its subtree to the query point (5, 5, 5). Note that since 
Human is a dynamic object, the result should be different from the previous queries.
```rust 
    // Retrieve Human node's id
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
```
The `Nearest_in_subtree` function will search in all layers in the subtree and returns 
the first `m` closest nodes regardless of their layer in the subtree.

In all the above queries, if the number of existing nodes is less than the requested count (`m`), 
the returning vector would not be of size `m`
```rust 
    // Asking for 20 neighbors when the Human subtree only has 10 nodes with coordinates should
    // return all the 10 nodes without any error.
    let neighbors_in_human_subtree = ygg.nearest_in_subtree(
        &[5.0, 5.0, 5.0].into(),
        human_node_id,
        PosQueryCount::Count(20),
    )?;
    assert_eq!(neighbors_in_human_subtree.len(), 10);
```

## Nodes within 
Another type of positional query supported by `yggdrasil` is the `nodes_within` query 
which returns nodes that are within a certain radious of a given point. much like the `nearest_node` query,
this query can also be run on all the tree levels and the semantic and differences are exactly the same.

```rust 
    // Retrieving all nodes within a radius of 5 from the (5, 5, 5) coordinates. The results should
    // contain less than or equal to 70 nodes, since this is the number of point cloud nodes that
    // are located in the (0, 0, 0) to (10, 10, 10) region.
    let nodes_within = ygg.nodes_within(&[5.0, 5.0, 5.0].into(), 5.0, PosQueryCount::All);
    assert!(nodes_within.len() <= 70);

    // Expanding the radious to 26 should return all the point cloud nodes, since now both the
    // static and dynamic nodes are within the radius.
    let nodes_within = ygg.nodes_within(&[5.0, 5.0, 5.0].into(), 26.0, PosQueryCount::All);
    assert_eq!(nodes_within.len(), 100);

    // Retrieving exactly 1 node within a radius of 5 from the (5, 5, 5) coordinates should return
    // the first node retured by the first nodes_within query.
    let nodes_within = ygg.nodes_within(&[5.0, 5.0, 5.0].into(), 5.0, PosQueryCount::Count(1));
    assert_eq!(nodes_within.len(), 1);
    assert_eq!(nodes_within[0].id, neighbors[0].0.id);

```

