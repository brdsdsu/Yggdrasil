# Bounding Box

A common practice in scene graph usage is to keep track of bounding boxes for different objects in the scene. A Bounding Box is an axis-aligned Box that represents the bounds of the coordinates of an object. It can be used for efficient spatial estimation and optimization.

`yggdrasil` supports this functionality through `bounds`. `bounds` is an optional feature that we can activate for any arbitrary node. By default, all nodes that are created via call to `SceneGraph::new_coordinates` or `SceneGraph::new_indexed_node` (which we will talk about in the next chapter, a talk on performance) have theirs `bounds` enabled.
```rust 
// Scene Graph Construction
let mut ygg = yggdrasil::SceneGraph::default();
let btm_lid = ygg.new_layer().id;
let mid_lid = ygg.new_layer().id;
let top_lid = ygg.new_layer().id;
ygg.layer_mut(top_lid)?.add_child(mid_lid)?;
ygg.layer_mut(mid_lid)?.add_child(btm_lid)?;

let btm_node = ygg.new_coordinates(1_f32, 2_f32, 3_f32, Vec::new());
let mid_node = ygg.new_node(Vec::new());
let top_node = ygg.new_coordinates(4_f32, 5_f32, 6_f32, Vec::new());

println!("{:?}", btm_node.bounds); // Some(BoundingBox { x: (1.0, 1.0), y: (2.0, 2.0), z: (3.0, 3.0) })
println!("{:?}", mid_node.bounds); // None
println!("{:?}", top_node.bounds); // Some(BoundingBox { x: (4.0, 4.0), y: (5.0, 5.0), z: (6.0, 6.0) })

let btm_nid = ygg.layer_mut(btm_lid)?.push_node(btm_node);
let mid_nid = ygg.layer_mut(mid_lid)?.push_node(mid_node);
let top_nid = ygg.layer_mut(top_lid)?.push_node(top_node);
```

```
Some(Bounded(BoundingBox { x: (1.0, 1.0), y: (2.0, 2.0), z: (3.0, 3.0) }))
None 
Some(Bounded(BoundingBox { x: (4.0, 4.0), y: (5.0, 5.0), z: (6.0, 6.0) }))
```

A node's `bounds` represents the bounds of all the coordinates in its subtree. `yggdrasil` keep tracks of bounds of every node in the scene graph and will update it upon deletion or addition of a new coordinates in the subtree.

```rust 
// Nesting would update the bounds of the parent node.
ygg.nest(btm_nid).under(mid_nid)?;
// mid_node's bounds should still be None since bounds is disabled. 
println!("{:?}", ygg.node(mid_nid)?.bounds); // None 
// top_node's bounds should not change
println!("{:?}", ygg.node(top_nid)?.bounds); // Some(BoundingBox { x: (4.0, 4.0), y: (5.0, 5.0), z: (6.0, 6.0) })

// Nesting mid_node under top_node would further update the bounds of top_node to encompass both top and mid node.
ygg.nest(mid_nid).under(top_nid)?;
println!("{:?}", ygg.node(top_nid)?.bounds); // Some(BoundingBox { x: (1.0, 4.0), y: (2.0, 5.0), z: (3.0, 6.0) })
```

```
None
Some(Bounded(BoundingBox { x: (4.0, 4.0), y: (5.0, 5.0), z: (6.0, 6.0) }))
Some(Bounded(BoundingBox { x: (1.0, 4.0), y: (2.0, 5.0), z: (3.0, 6.0) }))
```

Same as addition, node removal would also affect the bounds
```rust 
// Detaching btm_node from mid_node would update top_nodes's bounds again.
ygg.detach(mid_nid).from(top_nid)?;
println!("{:?}", ygg.node(top_nid)?.bounds); // Some(BoundingBox { x: (4.0, 4.0), y: (5.0, 5.0), z: (6.0, 6.0) })
```

```
Some(Bounded(BoundingBox { x: (4.0, 4.0), y: (5.0, 5.0), z: (6.0, 6.0) }))
```
