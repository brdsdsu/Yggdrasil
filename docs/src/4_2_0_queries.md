# Run Queries On The Scene Graph

A scene graph is only as useful as its queries! We will explore some of the queries that `yggdrasil` supports and get a glimpse of what each query does.

Let's start by using the function we created in the Bootstrapping section to generate our demo scene graph:
```rust 
use scene_graph as yggdrasil;
use yggdrasil::Feature;

fn main() -> yggdrasil::Result<()> {
    // Create a demo scene graph with the following structure:
    //
    // root_layer (root node with name "root")
    // ├────────── dynamic_objects_layer    (3 dynamic nodes with type "Human", "Dog", 
    // │                            │       "Cat", and edges:
    // │                            │                    "Human" -holding-> "Dog",
    // │                            │                     "Dog" -looking-> "Cat"  )
    // │                            │
    // └─ static_objects_layer      │       (7 static nodes with type "car", "tree", 
    //          │                   │        "building", "bicycle", "bench", "sign",
    //          │                   │           "lamp")
    //          │                   │
    //          └──── point_cloud_layer (100 point cloud nodes)
    //
    let ygg = create_demo_graph()?;

    .
    .
    .

    Ok(())
}
```

