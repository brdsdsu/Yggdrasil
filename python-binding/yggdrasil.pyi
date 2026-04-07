# yggdrasil/__init__.pyi
from __future__ import annotations

from typing import List, Optional, Tuple

Coordinate = Tuple[float, float, float]
QueryResult = List[Tuple[Node, float]]


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
    roll, half-angle, near and far distances. [cite: 56]
    """
    def __init__(self, *args, **kwargs) -> None: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...


class __Node:
    """
    Internal node value used when constructing nodes before inserting them
    into layers. [cite: 57]
    """

    @property
    def pid(self) -> Optional[int]:
        """Parent node ID, if this node is nested under another node."""
        ...

    @property
    def children(self) -> List[int]:
        """Child node IDs from the lower layer, if this node has nested nodes. [cite: 60]"""
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
    def coordinates(self) -> List[Coordinate]:
        """List of 3D coordinates associated with the node. [cite: 61]"""
        ...

    @coordinates.setter
    def coordinates(self, coords: List[Coordinate]) -> None: ...

    def has_feature(self, key: str) -> bool: ...
    def match_feature(self, feature: Feature) -> bool: ...
    def feature(self, key: str) -> str: ...
    def set_feature(self, feature: Feature) -> None: ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...


class Node:
    """
    A handle/reference to a node in the scene graph. [cite: 62]
    """
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
        """Child node IDs from the lower layer, if this node has nested nodes. [cite: 65]"""
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
    def coordinates(self) -> List[Coordinate]:
        """List of 3D coordinates associated with the node. [cite: 66]"""
        ...

    @coordinates.setter
    def coordinates(self, coords: List[Coordinate]) -> None: ...

    def has_feature(self, key: str) -> bool: ...
    def match_feature(self, feature: Feature) -> bool: ...
    def feature(self, key: str) -> str: ...
    def set_feature(self, feature: Feature) -> None: ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...


class Layer:
    """
    A layer within the scene graph. [cite: 67]
    """

    @property
    def lid(self) -> int:
        """
        The unique identifier for this layer.
        """
        ...
    def node(self, id: int) -> Node:
        """Get a reference to a node by its ID (raises on error)."""
        ...

    def push_node(self, node: __Node) -> int:
        """Add a new node (created via SceneGraph) to this layer."""
        ...

    def add_edge(self, src: int, dst: int, desc: str) -> None:
        """Add an edge from source node to destination node with a description. [cite: 69]"""
        ...

    def add_child(self, child_id: int) -> None:
        """Add a child layer by its ID."""
        ...

    def del_edge(self, src: int, dst: int) -> None:
        """Delete an edge from source node to destination node. [cite: 71]"""
        ...

    def nodes(self) -> List[Node]:
        """Get List of all nodes in the layer. [cite: 72]"""
        ...

    def nodes_having(self, keys: List[str]) -> List[Node]:
        """Get list of all nodes having all the specified feature keys. [cite: 73]"""
        ...

    def nodes_matching(self, features: List[Feature]) -> List[Node]:
        """Get list of all nodes matching the specified feature set. [cite: 74]"""
        ...

    def nearest_nodes(
        self,
        coord: Coordinate,
        count: PosQueryCount,
    ) -> List[Tuple[Node, float]]:
        """Retrieve up to `count` nearest nodes to the given position in this layer. [cite: 75]"""
        ...

    def nodes_within(
        self,
        coord: Coordinate,
        radius: float,
        count: PosQueryCount,
    ) -> List[Tuple[Node, float]]:
        """Retrieve up to `count` nodes within `radius` of the given position. [cite: 77]"""
        ...

    def edges_matching(self, desc: str) -> List[Edge]: ...
    def edges_from(self, src: int) -> List[Edge]: ...
    def edges_to(self, dst: int) -> List[Edge]: ...


class SceneGraph:
    """
    A hierarchical representation of objects and their relationships in a 3D environment. [cite: 79]
    """
    def __init__(self) -> None: ...

    def new_layer(self) -> Layer: ...
    def new_indexed_layer(self) -> Layer: ...
    def merge(self, other: SceneGraph) -> None: ...
    def top_layer(self) -> Layer: ...
    def layer(self, index: int) -> Layer: ...
    def layer_of(self, node_id: int) -> int: ...
    def node(self, node_id: int) -> Node: ...

    def new_coordinates(
        self,
        x: float,
        y: float,
        z: float,
        features: List[Feature],
    ) -> __Node:
        """Create a new metric node with coordinates and features. [cite: 85]"""
        ...

    def new_indexed_node(self, features: List[Feature]) -> __Node:
        """Create a new Indexed Semantic Node with specified features. [cite: 88]"""
        ...

    def new_node(self, features: List[Feature]) -> __Node:
        """Create a new semantic node with the given features. [cite: 91]"""
        ...

    def del_node(self, node_id: int) -> None: ...
    def nest(self, nestee: int, nester: int) -> None: ...

    def nodes_having(self, keys: List[str]) -> List[List[Node]]: ...
    def nodes_matching(self, features: List[Feature]) -> List[List[Node]]: ...

    def visible_subgraph(self, observer: Observer, root_node_id: int) -> SceneGraph: ...

    def nearest_nodes(
        self,
        coord: Coordinate,
        count: PosQueryCount,
    ) -> QueryResult: ...

    def nodes_within(
        self,
        coord: Coordinate,
        radius: float,
        count: PosQueryCount,
    ) -> QueryResult: ...

    def nearest_in_subtree(
        self,
        coord: Coordinate,
        root: int,
        count: PosQueryCount,
    ) -> QueryResult:
        """Retrieve up to `count` nearest nodes within the subtree rooted at `root`. """
        ...

    def nearest_in_subtree_for_each(
        self, 
        points: List[Coordinate], 
        root: int, 
        count: PosQueryCount
    ) -> List[QueryResult]: ...

    def nearest_nodes_for_each(
        self, 
        points: List[Coordinate], 
        count: PosQueryCount
    ) -> List[QueryResult]: ...

    def nodes_within_for_each(
        self, 
        points: List[Coordinate], 
        radius: float, 
        count: PosQueryCount
    ) -> List[QueryResult]: ...

    def edges_matching(self, desc: str) -> List[List[Edge]]: ...
    def edges_from(self, src: int) -> List[Edge]: ...
    def edges_to(self, dst: int) -> List[Edge]: ...


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
