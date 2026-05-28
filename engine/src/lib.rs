use scene_graph::{
    Feature, Node, NodeId, Observer, Result as SceneGraphResult, SceneGraph, SceneGraphError,
};

mod consts;

pub struct Engine {
    scene_graph: SceneGraph,
    merge_queue: Vec<MergeTask>,
}

impl Engine {
    /// Executes a given request and returns the response.
    pub fn execute(&mut self, request: Request) -> Response {
        match request {
            Request::SubGraph { root, observer } => {
                let dag = match self.scene_graph.layers_dag(self.scene_graph.layer_of(root)) {
                    Ok(dag) => dag.into_iter().map(|l| l.id).collect::<Vec<_>>(),
                    // Only case of error should be when the root node doesn't exist, which is a
                    // client error.
                    Err(e) => return Response::Error(e),
                };
                self.sync_for(&dag);
                match observer {
                    Some(obs) => self.scene_graph.visible_subgraph(&obs, root),
                    None => self.scene_graph.subgraph(root),
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
                    true => self.scene_graph.new_indexed_layer(),
                    false => self.scene_graph.new_layer(),
                };
                layer.meta.insert(
                    String::from(consts::annotations::VERSION),
                    String::from("1"),
                );
                Response::LayerId(layer.id)
            }
            Request::NestLayer { parent, child } => {
                // syncing is not required here since nesting doesn't change layers' content.
                match self.scene_graph.layer_mut(parent) {
                    Ok(parent) => parent.add_child(child).into(),
                    Err(e) => Response::Error(e),
                }
            }
            Request::DeleteNode { id } => {
                let dag = match self.scene_graph.layers_dag(self.scene_graph.layer_of(id)) {
                    Ok(dag) => dag.into_iter().map(|l| l.id).collect::<Vec<_>>(),
                    // Only case of error should be when the node doesn't exist, which is a
                    // client error.
                    Err(e) => return Response::Error(e),
                };
                self.sync_for(&dag);
                self.scene_graph.del_node(id).into()
            }
            Request::NodesHaving { keys } => {
                self.sync_with(|t| !t.subgraph.nodes_having(&keys).is_empty());
                let nodes = self
                    .scene_graph
                    .nodes_having(&keys)
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                Response::Nodes(nodes)
            }
            Request::NodesMatching { features } => {
                self.sync_with(|t| !t.subgraph.nodes_matching(&features).is_empty());
                let nodes = self
                    .scene_graph
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
            match task.execute(&mut self.scene_graph) {
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
            if let Ok(old_layer) = self.scene_graph.layer(new_layer.id) {
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

struct MergeTask {
    subgraph: SceneGraph,
}

impl MergeTask {
    fn check_layer_exists(&self, lid: u32) -> bool {
        self.subgraph.layers().any(|layer| layer.id == lid)
    }
    fn check_layers_exist(&self, lids: &[u32]) -> bool {
        lids.iter().any(|lid| self.check_layer_exists(*lid))
    }
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

pub enum Request {
    SubGraph {
        root: NodeId,
        observer: Option<Observer>,
    },
    Merge {
        subgraph: SceneGraph,
    },
    AddLayer {
        indexed: bool,
    },
    NestLayer {
        parent: u32,
        child: u32,
    },
    DeleteNode {
        id: NodeId,
    },
    NodesHaving {
        keys: Vec<String>,
    },
    NodesMatching {
        features: Vec<Feature>,
    },
}

pub enum Response {
    SceneGraph(SceneGraph),
    Error(SceneGraphError),
    Reject(u32),
    LayerId(u32),
    Empty,
    Nodes(Vec<Node>),
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
