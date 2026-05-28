use scene_graph::{
    Feature, Node, NodeId, Observer, Result as SceneGraphResult, SceneGraph, SceneGraphError,
};

mod consts;

/// Engine is the main struct that maintains the state of the globabl scene graph and handles all
/// incomming requests and guarantees consistency and correctness while optimizing for performance
/// by batching and lazyly applying updates.
///
/// Internally, The engine uses the assumption that merges on a specific layer can be deffered
/// until a read operation on that layer is requested. This allows for batching multiple merges
/// together, running merge operations in the background, and avoiding unnecessary latency for
/// write operations.
pub struct Engine {
    /// The actual global scene graph.
    global_graph: SceneGraph,
    /// A queue of pending merge operations.
    merge_queue: Vec<MergeTask>,
}

impl Engine {
    /// Executes a given request and returns the response.
    pub fn execute(&mut self, request: Request) -> Response {
        match request {
            Request::SubGraph { root, observer } => {
                let dag = match self.global_graph.layers_dag(self.global_graph.layer_of(root)) {
                    Ok(dag) => dag.into_iter().map(|l| l.id).collect::<Vec<_>>(),
                    // Only case of error should be when the root node doesn't exist, which is a
                    // client error.
                    Err(e) => return Response::Error(e),
                };
                self.sync_for(&dag);
                match observer {
                    Some(obs) => self.global_graph.visible_subgraph(&obs, root),
                    None => self.global_graph.subgraph(root),
                }
                .into()
            }
            Request::Merge { subgraph } => {
                // Reject if there are any conflicts (granuarity: layer)
                if let Some(conflicting_layer_id) = self.conflicts(&subgraph) {
                    return Response::Reject(conflicting_layer_id); // TODO: error code
                }
                self.merge_queue.push(MergeTask { subgraph });
                Response::Empty
            }
            Request::AddLayer { indexed } => {
                let layer = match indexed {
                    true => self.global_graph.new_indexed_layer(),
                    false => self.global_graph.new_layer(),
                };
                layer.meta.insert(
                    String::from(consts::annotations::VERSION),
                    String::from("1"),
                );
                Response::LayerId(layer.id)
            }
            Request::NestLayer { parent, child } => {
                // syncing is not required here since nesting doesn't change layers' content.
                match self.global_graph.layer_mut(parent) {
                    Ok(parent) => parent.add_child(child).into(),
                    Err(e) => Response::Error(e),
                }
            }
            Request::DeleteNode { id } => {
                let dag = match self.global_graph.layers_dag(self.global_graph.layer_of(id)) {
                    Ok(dag) => dag.into_iter().map(|l| l.id).collect::<Vec<_>>(),
                    // Only case of error should be when the node doesn't exist, which is a
                    // client error.
                    Err(e) => return Response::Error(e),
                };
                self.sync_for(&dag);
                self.global_graph.del_node(id).into()
            }
            Request::NodesHaving { keys } => {
                self.sync_with(|t| !t.subgraph.nodes_having(&keys).is_empty());
                let nodes = self
                    .global_graph
                    .nodes_having(&keys)
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                Response::Nodes(nodes)
            }
            Request::NodesMatching { features } => {
                self.sync_with(|t| !t.subgraph.nodes_matching(&features).is_empty());
                let nodes = self
                    .global_graph
                    .nodes_matching(&features)
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                Response::Nodes(nodes)
            }
        }
    }

    /// Applies all pending merges that involve any of the given layer ids. This should always be
    /// called before any subgraph query that involves those layers.
    fn sync_for(&mut self, layer_ids: &[u32]) {
        self.sync_with(|t| t.check_layers_exist(layer_ids));
    }

    /// Applies all pending merges that satisfy the given predicate.
    fn sync_with<F>(&mut self, p: F)
    where
        F: Fn(&MergeTask) -> bool,
    {
        let to_merge = self.merge_queue.extract_if(.., |t| p(t));
        for task in to_merge {
            match task.execute(&mut self.global_graph) {
                Ok(_) => {}
                Err(e) => unreachable!("didn't expect merge failure: {e}"),
            }
        }
    }

    /// Checks if there are any version missmatches between layers in the subgraph and layers with
    /// the same id in the global scene graph. Returns the id of the first conflicting layer, if
    /// any.
    fn conflicts(&self, subgraph: &SceneGraph) -> Option<u32> {
        for new_layer in subgraph.layers() {
            if let Ok(old_layer) = self.global_graph.layer(new_layer.id) {
                let old_version = old_layer
                    .meta
                    .get(consts::annotations::VERSION)
                    .map(|s| s.parse::<u32>().unwrap_or(0))
                    .unwrap_or(0);
                let new_version = new_layer
                    .meta
                    .get(consts::annotations::VERSION)
                    .map(|s| s.parse::<u32>().unwrap_or(0))
                    .unwrap_or(0);
                if new_version != old_version || new_version == 0 {
                    return Some(new_layer.id);
                }
            }
        }
        None
    }
}

/// MergeTask represents a pending merge operation. It contains the subgraph to be merged and
/// provides fast membership checks for layers to determine if the merge is required before
/// executing another operation.
struct MergeTask {
    subgraph: SceneGraph,
}

impl MergeTask {
    /// Checks if the given layer id exists in the subgraph.
    fn check_layer_exists(&self, lid: u32) -> bool {
        self.subgraph.layers().any(|layer| layer.id == lid)
    }
    /// Checks if any of the given layer ids exist in the subgraph.
    fn check_layers_exist(&self, lids: &[u32]) -> bool {
        lids.iter().any(|lid| self.check_layer_exists(*lid))
    }
    /// Execute the merge and merge the subgraph into the given scene graph.
    /// This will also increment the version annotation for all merged layers to reflect the
    /// update.
    fn execute(self, scene_graph: &mut SceneGraph) -> SceneGraphResult<()> {
        let target_layers = self.subgraph.layers().map(|l| l.id).collect::<Vec<_>>();
        scene_graph.merge(self.subgraph)?;
        // Increment version annotation for all merged layers to reflect the update.
        for lid in target_layers {
            if let Ok(layer) = scene_graph.layer_mut(lid) {
                let version = layer
                    .meta
                    .get(consts::annotations::VERSION)
                    .map(|s| s.parse::<u32>().unwrap_or(0))
                    .unwrap_or(0)
                    + 1;
                layer.meta.insert(
                    String::from(consts::annotations::VERSION),
                    version.to_string(),
                );
            }
        }
        Ok(())
    }
}

/// Requests that can be sent to the `Engine`.
pub enum Request {
    /// Requesting a subgraph is represented by the `SubGraph` variant. The `root` parameter
    /// specifies the id of the root node and the optional `observer` parameter allows for
    /// field-of-view queries to take place.
    SubGraph {
        root: NodeId,
        observer: Option<Observer>,
    },
    /// Indicates a request to merge the given subgraph into the global scene graph.
    Merge { subgraph: SceneGraph },
    /// Request to add a new layer, The `indexed` parameter indicates whether the new layer should
    /// be indexed of not.
    AddLayer { indexed: bool },

    /// Request to nest a layer inside another.
    NestLayer { parent: u32, child: u32 },
    /// Request to delete a node with the given id.
    DeleteNode { id: NodeId },
    /// Request to run the `SceneGraph::nodes_having` query with the given keys.
    NodesHaving { keys: Vec<String> },
    /// Request to run the `SceneGraph::nodes_matching` query with the given features.
    NodesMatching { features: Vec<Feature> },
}

/// Response to a Query or an Update request. The `Error` variant is used for both client and
/// server errors, while the `Empty` variant represents a sucessful operation with no result to
/// return. The `Reject` variant is a special case of error where the operation is rejected by the
/// server, mostly due to conflicts and outdated versions.
pub enum Response {
    /// All responses that return a form of `SceneGraph`
    SceneGraph(SceneGraph),
    /// Client and server side errors.
    Error(SceneGraphError),
    /// Rejection of an Operation, Mostly, Merge updates due to version conflicts.
    /// The `u32` parameter represents the id of the conflicting layer.
    Reject(u32),
    /// Id of a requested Layer.
    LayerId(u32),
    /// List of nodes matching a query.
    Nodes(Vec<Node>),
    /// Generic success response with no data to return.
    Empty,
}

impl From<SceneGraphResult<SceneGraph>> for Response {
    fn from(result: SceneGraphResult<SceneGraph>) -> Self {
        match result {
            Ok(subgraph) => Response::SceneGraph(subgraph),
            Err(e) => Response::Error(e),
        }
    }
}
impl From<SceneGraphResult<Vec<Node>>> for Response {
    fn from(result: SceneGraphResult<Vec<Node>>) -> Self {
        match result {
            Ok(nodes) => Response::Nodes(nodes),
            Err(e) => Response::Error(e),
        }
    }
}

impl From<SceneGraphResult<()>> for Response {
    fn from(result: SceneGraphResult<()>) -> Self {
        match result {
            Ok(()) => Response::Empty,
            Err(e) => Response::Error(e),
        }
    }
}
