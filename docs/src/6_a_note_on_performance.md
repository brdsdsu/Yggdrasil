# A Note On Performance

Although `yggdrasil` has support for a variety of functionalities, they don't come free! these functionalities incur performance penalties either at query time, i.e. when the query is being executed, or at graph construction time, i.e. when graph is first created of updated. Understanding this performance tradeoff and the performance tuning options that `yggdrasil` provides can help you customize the scene graph for your specific usecase and guarantee certain performance and latency limitations.

## Indexing
`yggdrasil` provides support for indexing the coordinates in the scene graph. This indexing can be enabled on both the layer level and the node level. By having indexing enabled, `yggdrasil` will keep track of every coordinate add/update/removal on a layer, in case of layer indexing, or on a node's subtree in case of node indexing. This would result in faster positional queries such as `nearest neighbor` or `field of view`.

In `yggdrasil`, by calling `SceneGraph::new_indexed_layer` instead of `SceneGraph::new_layer` you can create a layer with indexing enabled

```rust 
let mut ygg = yggdrasil::SceneGraph::default();

let no_index_layer = ygg.new_layer();
let indexed_layer  = ygg.new_indexed_layer();
```

Following the same pattern, `SceneGraph::new_indexed_node` will create a new node with indexing enabled.
```rust 
let mut ygg = yggdrasil::SceneGraph::default();

let no_index_node = ygg.new_node(Vec::new());
let indexed_node  = ygg.new_inedexed_node(Vec::new());
```

Although indexing would result in a better performance on query execution time, it would incur some performance penalties on graph creation/update time since now every change in layers with indexing enabled and every change in the subtree of nodes with indexing enabled would result in an update propagation on these indexes.

One way to mitigate this problem for layer indexing, is to have two seperate layers for static and moving objects. If you know there is a subset of objects in the scene that don't move or rarely change position and you want fast positional query runtime on those, its better to keep them in a seperate layer. This way, there would be no updates on the layer's index and the performance penalty would only ocur on the dynamic objects layer's index.


## Bounding Box
Bounding boxes can be thought of as some sort of spacial estimation and just like indexing, 
they come at the cost of scene graph creation/update perfromance loss. 
When a node with coordinates is added to/removed from the scene graph, all the ancestors of that node 
are affected and if any of those have `bounds` enabled, their bounding box will go through an update process. 

But unlike indexing, bounding box performance penalty has different charactristics that 
makes it less likely to be performance bottle neck.

Usually, addition/deletion of a single coordinate will not cause a big update propagation throughout the ancestory path of a node since most of the time, the newly added/deleted coordinate is already fully encompassed by the bounding box of the parent node and thus, no further update is required. So its safe to say that performance penalty of coordinate change is negligible when it comes to bounding boxes.
