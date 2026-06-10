# Bootstrap a Scene Graph

Lets start getting familiar with Yggdrasil Scene Graph and its API by creating a new scene graph and running some queries on it.

The following function would create a new demo scene graph which we can use to run some queries on:
```rust 
use scene_graph as yggdrasil;
use yggdrasil::Feature;

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
```
Let's go through each line one by one to better understand how `Yggdrasil` concepts combine to create this scene graph.

## New Scene Graph
```rust 
    let mut ygg = yggdrasil::SceneGraph::default();
```
This line creates an empty scene graph for us to manipulate

## Allocating layers
```rust
    let point_cloud_layer_id = ygg.new_layer().id;
    let static_objects_layer_id = ygg.new_layer().id;
    let dynamic_objects_layer_id = ygg.new_layer().id;
    let root_layer_id = ygg.new_layer().id;
```

In this snippet, we are allocating four different layers for the scene graph. Layers are an important segmentation logic of the scene graph which help with performance and better modeling logic of the environment.
In `yggdrasil`, you can think of each layer as an isolated graph of nodes and edges. Layer isolation is enforced at the scene graph layers, meaning crossing different layers is not allowed for certain operations. For example, an edge cannot be added between two nodes that belong to different layers.

In this example, we allocate four layers:
1. point cloud layer: stores the actual position of the points in our scene.
2. static object layer: stores semantic data for each static object in the scene.
3. dynamic object layer: stores semantic data for each moving object in the scene.
4. root layer: holds the root node, which contains the entire scene underneath itself.

## Setting Up The Hierarchy
```rust 
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
```
In `yggdrasil`, layers have hierarchy and form a DAG in the scene graph. Hierarchy is used to enforce constraints on nodes and edges in the scene graph. For example, the parent-child relationship between two nodes is allowed if and only if their layers have the same (parent-child) relationship.

Using the `SceneGraph::layer` and `SceneGraph::layer_mut` functions, we can retrieve a layer using its `id`. We use `Layer::add_child` function to declare one `Layer` object the child of another. Note that layers form a DAG, meaning one layer can have multiple direct parent layers. For example, in our demo `point_cloud_layer` is a child of both `static_objects_layer` and `dynamic_objects_layer`.

Layer ids follow a topological ordering, meaning, a layer with id `i` can be a child of layer `j` if and only if `i` < `j`

## Root Layer
```rust 
    // Create a root node with a name feature
    // This node will serve as the parent for all other nodes in the scene graph.
    let root_node = ygg.new_node(vec![Feature::new("name", "root")]);
    let root_node_id = ygg.layer_mut(root_layer_id)?.push_node(root_node);

```
We create a new root node and push it to root layer. The `SceneGraph::new_node` function will take a list of `SceneGraph::Feature`s which is essentially a key-value pair and creates a new node that is ready to push. Note that the output of `SceneGraph::new_node` is a `AnonNode` demonstrating the node's incompleteness. This node **IS NOT** a part of the scene graph yet and as a result, has no id associated to it. a `NodeId` is only associated to a node when it is pushed to a layer since part of its id is determined by the layer it resides in. the output of `Layer::push_node` is the `NodeId` of the newly added node.

## Point Cloud Layer
```rust 
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

```
We create 100 new nodes, each representing a random spatial point in the scene. The `SceneGraph::new_coordinates` function creates a new node and assigns it 3d coordinates. This Node is the same as the node that is created via `SceneGraph::new_node` with its position initialized. We can use `AnonNode::Coordinates` and `Node::Cordinates` to directly access the position of a node and change it later.

## Static Object Layer
```rust 
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
```
The above snippet creates 7 new nodes in the `static object` layer, each representing a new static object. Since these nodes represent actual objects in the scene, we nest all their points under their semantic node:

```rust 
    // Nest all the point cloud nodes under this object's semantic node.
    for point_cloud_node_id in &point_cluod_node_ids[i * 10..(i + 1) * 10] {
        ygg.nest(*point_cloud_node_id).under(object_node_id)?;
    }
```

Using the `ygg.nest(nestee_id).under(nester_id)` pattern, we can nest the nestee node `under` the nester node. Note that `nester` can nest `nestee` if and only if `nestee`'s layer is a child of `nester`'s layer.

Each node in the scene graph can have at most one nester or parent. If `under` is called on a node that already has a parent, the old parent-child relationship is overwritten.

In this code snippet, we also make sure to mark the object node as a child of the root node
```rust 
    // Nest this object's semantic node under the root node.
    ygg.nest(object_node_id).under(root_node_id)?;
```

## Dynamic Object Layer

```rust 
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
```
Similarly to the static layer, we create 3 dynamic objects to represent living entities, e.g., humans, in the scene.

```rust 
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

```
`yggdrasil` supports semantic edges between two nodes of the same layer. Here, we create two semantic edges representing interactions between the "Human", the "Dog", and the "Cat". These edges can have any arbitrary label. The labels can be used to filter nodes and edges.

Using the `Layer::add_edge` one can add an edge between two nodes by passing their ids. Note that this function will fail if the nodes are not from this layer.
