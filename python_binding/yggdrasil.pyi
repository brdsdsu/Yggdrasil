# yggdrasil/__init__.pyi
from __future__ import annotations

from typing import List, Optional, Tuple

Coordinate = Tuple[float, float, float]


class Feature:
    """
    A feature associated with a node, represented as a key-value pair.
    """

    key: str
    value: str

    def __init__(self, key: str, value: str) -> None: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...


class Edge:
    """
    An edge connecting two nodes in the same layer.
    """

    src: int
    dst: int
    desc: str

    def __init__(self, src: int, dst: int, desc: str) -> None: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...


class PosQueryCount:
    """
    Specifies the number of nodes to return in position-based queries.
    """

    @staticmethod
    def All() -> PosQueryCount: ...
    @staticmethod
    def Count(count: int) -> PosQueryCount: ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...


class Observer:
    """
    A field-of-view cone frustum in 3D space, defined by position, yaw, pitch,
    roll, half-angle, near and far distances.
    """

    # Constructor details are implemented in Rust; this is intentionally minimal.
    def __init__(self, *args, **kwargs) -> None: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...


class __Node:
    """
    Internal node value used when constructing nodes before inserting them
    into layers.

    Instances of this class are typically created via SceneGraph.new_node()
    or SceneGraph.new_coordinates() and then passed to Layer.push_node().
    """

    # ---- Read-only properties taken directly from the underlying Node ----

    @property
    def id(self) -> int:
        """Unique identifier for the node."""
        ...

    @property
    def pid(self) -> Optional[int]:
        """Parent node ID, if this node is nested under another node."""
        ...

    @property
    def children(self) -> List[int]:
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

    # ---- Feature operations (direct, no graph access) ----

    def has_feature(self, key: str) -> bool: ...
    def match_feature(self, feature: Feature) -> bool: ...
    def feature(self, key: str) -> str: ...
    def set_feature(self, feature: Feature) -> None: ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...


class Node:
    """
    A handle/reference to a node in the scene graph.

    Each Node belongs to a specific layer and can have features, coordinates,
    and edges to other nodes in the same layer, as well as parent/child
    relationships across layers.
    """

    # ---- Read-only properties via graph access ----

    @property
    def id(self) -> int:
        """Unique identifier for the node."""
        ...

    @property
    def pid(self) -> Optional[int]:
        """Parent node ID, if this node is nested under another node."""
        ...

    @property
    def children(self) -> List[int]:
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

    # ---- Feature operations ----

    def has_feature(self, key: str) -> bool: ...
    def match_feature(self, feature: Feature) -> bool: ...
    def feature(self, key: str) -> str: ...
    def set_feature(self, feature: Feature) -> None: ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...


class Layer:
    """
    A layer within the scene graph.

    Provides node access and modification within that layer, as well as
    edge, feature-based, and spatial queries.
    """

    # ---- Node access / modification ----

    def node(self, id: int) -> Node:
        """Get a reference to a node by its ID (raises on error)."""
        ...

    def push_node(self, node: __Node) -> None:
        """Add a new node (created via SceneGraph) to this layer."""
        ...

    def add_edge(self, src: int, dst: int, desc: str) -> None:
        """
        Add an edge from source node to destination node with a description.
        Ensures both nodes exist in the layer.
        """
        ...

    def del_edge(self, src: int, dst: int) -> None:
        """
        Delete an edge from source node to destination node.
        Raises an exception if the edge does not exist.
        """
        ...

    # ---- Feature-based node queries ----

    def nodes(self) -> List[Node]:
        """
        Get List of all nodes in the layer.
        """
        ...


    def nodes_having(self, keys: List[str]) -> List[Node]:
        """
        Get list of all nodes having all the specified feature keys.
        """
        ...

    def nodes_matching(self, features: List[Feature]) -> List[Node]:
        """
        Get list of all nodes matching the specified feature set.
        """
        ...

    # ---- Spatial queries (per-layer) ----

    def nearest_nodes(
        self,
        coord: Coordinate,
        count: PosQueryCount,
    ) -> List[Tuple[Node, float]]:
        """
        Retrieve up to `count` nearest nodes to the given position in this layer.

        Returns a list of (node, distance) sorted from nearest to farthest.
        """
        ...

    def nodes_within(
        self,
        coord: Coordinate,
        radius: float,
        count: PosQueryCount,
    ) -> List[Tuple[Node, float]]:
        """
        Retrieve up to `count` nodes within `radius` of the given position
        in this layer.

        Returns a list of (node, distance).
        """
        ...

    # ---- Edge queries ----

    def edges_matching(self, desc: str) -> List[Edge]:
        """Get list of all edges matching a specific description."""
        ...

    def edges_from(self, src: int) -> List[Edge]:
        """Get list of all edges from a specific source node."""
        ...

    def edges_to(self, dst: int) -> List[Edge]:
        """Get list of all edges to a specific destination node."""
        ...


class SceneGraph:
    """
    A hierarchical representation of objects and their relationships in a 3D environment.

    Organized into layers, with nodes representing objects and supporting:
    - parent/child relationships between nodes on adjacent layers
    - edges between nodes in the same layer
    - feature-based and spatial queries
    """

    def __init__(self) -> None: ...

    # ---- Layer management ----

    def new_layer(self) -> Layer:
        """
        Create a new layer without position indexing and add it to the scene graph.
        """
        ...

    def new_indexed_layer(self) -> Layer:
        """
        Create a new layer with position indexing enabled for spatial queries.
        """
        ...

    def merge(self, other: SceneGraph) -> None:
        """
        Merge another SceneGraph into this one.

        This does not delete nodes or edges, but applies changes in features/edges
        for nodes that exist in both graphs.
        """
        ...

    def top_layer(self) -> Layer:
        """Get a reference to the top (last) layer."""
        ...

    def layer(self, index: int) -> Layer:
        """Get a reference to a layer by its index."""
        ...

    def layer_of(self, node_id: int) -> int:
        """Get the layer index of a node by its ID."""
        ...

    def node(self, node_id: int) -> Node:
        """Get a reference to a node by its ID."""
        ...

    # ---- Node creation / deletion ----

    def new_coordinates(
        self,
        x: float,
        y: float,
        z: float,
        features: List[Feature],
    ) -> __Node:
        """
        Create a new metric node with coordinates and features.

        The returned __Node should be inserted into a Layer via push_node().
        """
        ...


    def new_indexed_node(self, features: List[Feature]) -> __Node:
        """
        Create a new Indexed Semantic Node with specified features.
        This node will have position indexing enabled and will track of the coordinates of its
        sub-tree for efficient spatial queries.

        The returned __Node should be inserted into a Layer via push_node().
        """
        ...

    def new_node(self, features: List[Feature]) -> __Node:
        """
        Create a new semantic node with the given features.

        The returned __Node should be inserted into a Layer via push_node().
        """
        ...

    def del_node(self, node_id: int) -> None:
        """
        Delete a node by its ID (recursively deleting all child nodes as well).
        """
        ...

    def nest(self, nestee: int, nester: int) -> None:
        """
        Make the node `nestee` a child of `nester`.

        Both must exist; `nester` must be on the layer immediately above `nestee`.
        """
        ...

    # ---- Feature-based queries across layers ----

    def nodes_having(self, keys: List[str]) -> List[List[Node]]:
        """
        Get a list of per-layer lists of nodes having all specified feature keys.
        """
        ...

    def nodes_matching(self, features: List[Feature]) -> List[List[Node]]:
        """
        Get a list of per-layer lists of nodes matching the specified feature set.
        """
        ...

    # ---- Visibility / spatial queries across graph ----

    def visible_subgraph(self, observer: Observer, root_node_id: int) -> SceneGraph:
        """
        Get a subgraph containing nodes within the field of view of an observer
        and descending from the specified root node.
        """
        ...

    def nearest_nodes(
        self,
        coord: Coordinate,
        count: PosQueryCount,
    ) -> List[Tuple[Node, float]]:
        """
        Retrieve up to `count` nearest nodes to the given position in the graph.

        Returns a list of (node, distance) sorted from nearest to farthest.
        """
        ...

    def nodes_within(
        self,
        coord: Coordinate,
        radius: float,
        count: PosQueryCount,
    ) -> List[Tuple[Node, float]]:
        """
        Retrieve up to `count` nodes within `radius` of the given position
        in the graph.

        Returns a list of (node, distance).
        """
        ...

    def nearest_in_subtree(
        self,
        coord: Coordinate,
        root: int,
        count: PosQueryCount,
    ) -> List[Tuple[Node, float]]:
        """
        Retrieve up to `count` nearest nodes to the given position within
        the subtree rooted at `root`.

        Returns a list of (node, distance).
        """
        ...

    # ---- Edge queries across graph ----

    def edges_matching(self, desc: str) -> List[List[Edge]]:
        """
        Get a list of per-layer lists of edges matching the given description.
        """
        ...

    def edges_from(self, src: int) -> List[Edge]:
        """Get list of all edges from a specific source node."""
        ...

    def edges_to(self, dst: int) -> List[Edge]:
        """Get list of all edges to a specific destination node."""
        ...


__all__ = [
    "SceneGraph",
    "Layer",
    "Node",
    "__Node",
    "Edge",
    "Feature",
    "Observer",
    "PosQueryCount",
    "Coordinate",
]

