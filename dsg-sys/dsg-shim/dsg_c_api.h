#pragma once
#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Opaque handle to a DynamicSceneGraph
typedef struct dsg_graph_t dsg_graph_t;

typedef uint64_t dsg_node_id_t;
typedef uint16_t dsg_layer_id_t;

typedef struct {
  double x, y, z;
} dsg_vec3_t;

// Lifecycle
dsg_graph_t *dsg_create(void);
void dsg_free(dsg_graph_t *g);

// Layers
bool dsg_add_layer(dsg_graph_t *g, dsg_layer_id_t layer_id);

// // Nodes
bool dsg_add_node(dsg_graph_t *g, dsg_layer_id_t layer_id,
                  dsg_node_id_t node_id, dsg_vec3_t position);

bool dsg_set_node_label(dsg_graph_t *g, dsg_layer_id_t layer_id,
                        dsg_node_id_t node_id, const char *label);

// Edges
bool dsg_add_edge(dsg_graph_t *g, dsg_node_id_t a_id, dsg_node_id_t b_id);
bool dsg_nest(dsg_graph_t *g, dsg_node_id_t nester, dsg_node_id_t nestee);

// Serialize to JSON
bool dsg_save_json(const dsg_graph_t *g, const char *path);

#ifdef __cplusplus
}
#endif
