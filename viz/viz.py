# Visualization of Dynamic Scene Graphs (DSGs) 
# This script assumes the graph data is available in 'graph.json' file.

import spark_dsg as dsg
from spark_dsg.visualization import plot_scene_graph

G = dsg.DynamicSceneGraph.load("graph.json")
fig = plot_scene_graph(G)
fig.show()

