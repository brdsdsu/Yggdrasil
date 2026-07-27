# Field of View Queries

The Field of View query is used when we want to limit the scene graph to a sub set of the scene that is
observable by the agent. This has many applications for path planning and navigation as it allows the agent
to have a prior perception of what is to be expected as it moves in the scene.

`yggdrasil` uses [frustum-check](https://en.wikipedia.org/wiki/Viewing_frustum) to filter out all the objects
that are within the field of view of the given camera.

## Visible Subgraph
In order to query the scene graph for objects within the field of view, we first have to create an observer.
Observers in `yggdrasil` can be thought of as a cone frustum. Cone frustums have three main parameters: 
1. Location
2. Direction
3. Far and near reach

The direction is a plain vector in whatever frame your own coordinates use, so nothing here assumes a
particular "up" axis. Only its axis matters, since a cone is symmetric about the direction it points
along; a direction of zero length or one carrying a `NaN` is rejected, which is why the constructor
returns a `Result`.
> For more detailed documentation on how to setup an observer, you can read the API doc for the `yggdrasil::Observer` struct.

In the following snippet, we create a cone frustum observer that is located at (0, 0, 0) and can view all the positive space.
We limit this observers view by near and far of (0.001,6.0) to make sure it can only observe the static objects for now.
```rust 
use scene_graph as yggdrasil;
use yggdrasil::{Coordinate, Observer};
```
```rust 

    // Create a cone observer which can view all the positive space, i.e. the space where none of the
    // x, y, z coordinates are negative. The observer is located at the origin and is looking
    // towards the (1, 1, 1) direction, with a field of view of 90 degrees and a far reach of 6
    // units.
    let pos = Coordinate::ZERO;
    let direction = Coordinate::ONE;
    // Widen the cone just past the angle from the (1, 1, 1) diagonal to each axis, so
    // the whole positive octant falls inside it.
    let half_angle = (1.0 / 3.0_f32.sqrt()).acos() + 0.001;

    let near = 0.001;
    let far = 6_f32;

    let observer = Observer::new_looking_along(pos, direction, half_angle, near, far)?;
```

In order to successfully run the `visible_subgraph` query, we also need to provide a reference node. The reference node acts 
as a constraining factor, limiting the resulting subgraph to the nodes that are within the subtree of the reference node.
In this example, since we have no constraint, we use the root node as the reference.
```rust
    // Retrieve the root node of the scene graph.
    let root_node_id = ygg
        .layer(3)?
        .nodes_matching(&[&Feature::new("name", "root")])[0]
        .id;
```


Now running the query would return a new scene graph, which is a visible subgraph of the original scene graph.
Since we limited the far reach of the observer to 6, we should be able to observe some of the static objects and none of the 
dynamic ones. Since this query returns a subgraph, the entire ancestry of each visible point cloud node should also apear in the 
subgraph.
```rust
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
```

Running the same query with the "Human" node as the reference should return an empty subgraph since none of the Human's 
point cloud is visible by the observer.
```rust
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
```

Expanding the far reach of the observer to 35 (≈`sqrt(3*20^2)`) should allow the observer to view dynamic objects as well as 
the static ones.

```rust
    // Expanding the `far` reach of the observer to 35 should include all the dynamic objects as
    // well as the static ones, meaning that the subgraph should be the same as the original scene
    // graph.
    let observer = Observer::looking_along(pos, direction, half_angle, near, 35.0)?;
    let subgraph = ygg.visible_subgraph(&observer, root_node_id)?;
    assert_eq!(subgraph.layer(0)?.all_nodes().len(),ygg.layer(0)?.all_nodes().len());
    assert_eq!(subgraph.layer(1)?.all_nodes().len(),ygg.layer(1)?.all_nodes().len());
    assert_eq!(subgraph.layer(2)?.all_nodes().len(),ygg.layer(2)?.all_nodes().len());
```
