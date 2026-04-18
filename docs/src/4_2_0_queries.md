# Run Queries On The Scene Graph

The whole point of having a scene graph is to run some complex queries on it! We will start going through most of the queries that `yggdrasil` provides and get a glimps of what each query does.

Lets start by using the function from the previous section to generate our demo scene graph
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

