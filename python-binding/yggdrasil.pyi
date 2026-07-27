# yggdrasil/__init__.pyi
from __future__ import annotations

from typing import List, Optional, Tuple

Coordinate = Tuple[float, float, float]
QueryResult = List[Tuple["Node", float]]


class NodeId:
    """
    Unique identifier for a node in the scene graph.

    Packs the layer id (upper 32 bits) and the in-layer node id (lower 32 bits)
    into a single value, so a node's layer is always recoverable from its id.
    """
    def layer_id(self) -> int:
        """The id of the layer this node belongs to."""
        ...
    def node_id(self) -> int:
        """The in-layer id of this node."""
        ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...


class Feature:
    """
    A feature associated with a node, represented as a key-value pair.
    """
    key: str
    value: str

    def __init__(self, key: str, value: str) -> None: ...


class Edge:
    """
    An edge connecting two nodes in the same layer.
    """
    src: NodeId
    dst: NodeId
    desc: str


class PosQueryCount:
    """
    Specifies the number of nodes to return in position-based queries.
    """
    @staticmethod
    def All() -> PosQueryCount:
        """Return all matching nodes."""
        ...
    @staticmethod
    def Count(count: int) -> PosQueryCount:
        """Return a specific number of matching nodes."""
        ...


class Observer:
    """
    A field-of-view cone frustum in 3D space, defined by a position, a viewing
    direction, a half-angle, and near/far distances.
    """

    def __init__(
        self,
        position: Coordinate,
        direction: Coordinate,
        half_angle: float,
        near: float,
        far: float,
    ) -> None:
        """
        Build an observer at `position` looking along `direction`.

        `direction` is an (x, y, z) vector in the scene's own frame; only its axis
        matters, so it need not be a unit vector. Raises `ValueError` if it has zero
        length or is not finite. `half_angle` is in radians, measured out from the
        direction, so the cone's full opening is twice it.
        """
        ...


class __Node:
    """
    Internal node value used when constructing nodes before inserting them
    into layers.
    """

    @property
    def pid(self) -> Optional[NodeId]:
        """Parent node ID, if this node is nested under another node."""
        ...

    @property
    def features(self) -> List[Feature]:
        """Features associated with the node."""
        ...

    @property
    def coordinates(self) -> Optional[Coordinate]:
        """Optional 3D coordinates of the node."""
        ...

    @coordinates.setter
    def coordinates(self, coords: Optional[Coordinate]) -> None: ...

    def has_feature(self, key: str) -> bool:
        """Check if the node has a feature with the specified key."""
        ...
    def match_feature(self, f: Feature) -> bool:
        """Check if the node has the exact key-value pair as a feature."""
        ...
    def feature(self, key: str) -> str:
        """Get the value of a feature by its key."""
        ...
    def set_feature(self, feature: Feature) -> None:
        """Set or update a feature for the node."""
        ...


class Node:
    """
    A handle/reference to a node in the scene graph.
    """
    @property
    def id(self) -> NodeId:
        """Unique identifier for the node."""
        ...

    @property
    def pid(self) -> Optional[NodeId]:
        """Parent node ID, if this node is nested under another node."""
        ...

    @property
    def children(self) -> List[NodeId]:
        """Child node IDs from the lower layer, if this node has nested nodes."""
        ...

    @property
    def edges(self) -> List[Edge]:
        """Edges to other nodes in the same layer."""
        ...

    @property
    def features(self) -> List[Feature]:
        """Features associated with the node."""
        ...

    @property
    def coordinates(self) -> Optional[Coordinate]:
        """Optional 3D coordinates of the node."""
        ...

    @coordinates.setter
    def coordinates(self, coords: Optional[Coordinate]) -> None: ...

    def has_feature(self, key: str) -> bool:
        """Check if the node has a feature with the specified key."""
        ...
    def match_feature(self, f: Feature) -> bool:
        """Check if the node has the exact key-value pair as a feature."""
        ...
    def feature(self, key: str) -> str:
        """Get the value of a feature by its key."""
        ...
    def set_feature(self, feature: Feature) -> None:
        """Set or update a feature for the node."""
        ...


class Layer:
    """
    A layer within the scene graph.
    """

    @property
    def lid(self) -> int:
        """The unique identifier for this layer."""
        ...

    def node(self, id: NodeId) -> Node:
        """Get a reference to a node by its ID."""
        ...

    def push_node(self, node: __Node) -> NodeId:
        """Add a new node (created via SceneGraph) to this layer."""
        ...

    def add_edge(self, src: NodeId, dst: NodeId, desc: str) -> None:
        """Add an edge from source node to destination node with a description."""
        ...

    def add_child(self, child_id: int) -> None:
        """Add a child layer by its ID."""
        ...

    def del_edge(self, src: NodeId, dst: NodeId) -> None:
        """Delete an edge from source node to destination node."""
        ...

    def nearest_nodes(
        self,
        coord: Coordinate,
        count: PosQueryCount,
    ) -> List[Tuple[Node, float]]:
        """Retrieve up to `count` nearest nodes to the given position in this layer."""
        ...

    def nodes_within(
        self,
        coord: Coordinate,
        radius: float,
        count: PosQueryCount,
    ) -> List[Tuple[Node, float]]:
        """Retrieve up to `count` nodes within `radius` of the given position."""
        ...

    def nodes(self) -> List[Node]:
        """Get list of all nodes in the layer."""
        ...

    def nodes_having(self, keys: List[str]) -> List[Node]:
        """Get list of all nodes having all the specified feature keys."""
        ...

    def nodes_matching(self, features: List[Feature]) -> List[Node]:
        """Get list of all nodes matching the specified feature set."""
        ...

    def edges_matching(self, desc: str) -> List[Edge]:
        """Get list of all edges matching a specific description."""
        ...
    def edges_from(self, src: NodeId) -> List[Edge]:
        """Get list of all edges from a specific source node."""
        ...
    def edges_to(self, dst: NodeId) -> List[Edge]:
        """Get list of all edges to a specific destination node."""
        ...


class SceneGraph:
    """
    A hierarchical representation of objects and their relationships in a 3D environment.
    """
    def __init__(self) -> None: ...

    def new_layer(self) -> Layer:
        """Create a new layer without position indexing and add it to the scene graph."""
        ...
    def new_indexed_layer(self) -> Layer:
        """Create a new layer with position indexing enabled and add it to the scene graph."""
        ...
    def merge(self, m: SceneGraph) -> None:
        """Merge another SceneGraph into this one."""
        ...
    def top_layer(self) -> Layer:
        """Get a reference to the top layer."""
        ...
    def layer(self, id: int) -> Layer:
        """Get a reference to a layer by its id."""
        ...
    def layer_of(self, nid: NodeId) -> int:
        """Get the layer index of a node by its ID."""
        ...
    def node(self, nid: NodeId) -> Node:
        """Get a reference to a node by its ID."""
        ...

    def new_coordinates(
        self,
        x: float,
        y: float,
        z: float,
        features: List[Feature],
    ) -> __Node:
        """Create a new metric node with coordinates and features."""
        ...

    def new_indexed_node(self, features: List[Feature]) -> __Node:
        """Create a new indexed semantic node with the specified features."""
        ...

    def new_node(self, features: List[Feature]) -> __Node:
        """Create a new semantic node with the given features."""
        ...

    def del_node(self, nid: NodeId) -> None:
        """Delete a node (and its subtree) by its ID from the scene graph."""
        ...
    def nest(self, nestee: NodeId, nester: NodeId) -> None:
        """Nest a node under another node, establishing a parent-child relationship."""
        ...

    def nodes_having(self, keys: List[str]) -> List[List[Node]]:
        """Get list of all nodes having a specific set of feature keys, grouped by layer."""
        ...
    def nodes_matching(self, features: List[Feature]) -> List[List[Node]]:
        """Get list of all nodes matching a specific set of features, grouped by layer."""
        ...

    def visible_subgraph(self, observer: Observer, root_node_id: NodeId) -> SceneGraph:
        """Get the subgraph of nodes within the observer's field of view under the given root."""
        ...

    def nearest_nodes(
        self,
        coord: Coordinate,
        count: PosQueryCount,
    ) -> QueryResult:
        """Retrieve up to `count` nearest nodes to the given position."""
        ...

    def nodes_within(
        self,
        coord: Coordinate,
        radius: float,
        count: PosQueryCount,
    ) -> QueryResult:
        """Retrieve up to `count` nodes within `radius` of the given position."""
        ...

    def nearest_in_subtree(
        self,
        coord: Coordinate,
        root: NodeId,
        count: PosQueryCount,
    ) -> QueryResult:
        """Retrieve up to `count` nearest nodes within the subtree rooted at `root`."""
        ...

    def nearest_in_subtree_for_each(
        self,
        points: List[Coordinate],
        root: NodeId,
        count: PosQueryCount,
    ) -> List[QueryResult]:
        """Same as `nearest_in_subtree` but for multiple points."""
        ...

    def nearest_nodes_for_each(
        self,
        points: List[Coordinate],
        count: PosQueryCount,
    ) -> List[QueryResult]:
        """Same as `nearest_nodes` but for multiple points."""
        ...

    def nodes_within_for_each(
        self,
        points: List[Coordinate],
        radius: float,
        count: PosQueryCount,
    ) -> List[QueryResult]:
        """Same as `nodes_within` but for multiple points."""
        ...

    def edges_matching(self, desc: str) -> List[List[Edge]]:
        """Get list of all edges matching a specific description, grouped by layer."""
        ...
    def edges_from(self, src: NodeId) -> List[Edge]:
        """Get list of all edges from a specific source node."""
        ...
    def edges_to(self, dst: NodeId) -> List[Edge]:
        """Get list of all edges to a specific destination node."""
        ...


__all__ = [
    "SceneGraph",
    "Layer",
    "Node",
    "__Node",
    "NodeId",
    "Edge",
    "Feature",
    "Observer",
    "PosQueryCount",
    "Coordinate",
]
