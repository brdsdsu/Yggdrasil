#include "dsg_c_api.h"
#include <Eigen/Core>
#include <memory>
#include <spark_dsg/dynamic_scene_graph.h>
#include <spark_dsg/edge_attributes.h>
#include <spark_dsg/node_attributes.h>
#include <string>

//! Shim structure holding the actual DynamicSceneGraph
struct dsg_graph_t {
  std::unique_ptr<spark_dsg::DynamicSceneGraph> impl;
};

extern "C" {

dsg_graph_t *dsg_create(void) {
  try {
    auto g = new dsg_graph_t();
    g->impl = std::make_unique<spark_dsg::DynamicSceneGraph>();
    return g;
  } catch (...) {
    return nullptr;
  }
}

void dsg_free(dsg_graph_t *g) { delete g; }

bool dsg_add_layer(dsg_graph_t *g, dsg_layer_id_t layer_id) {
  try {
    g->impl->addLayer(static_cast<spark_dsg::LayerId>(layer_id));
    return true;
  } catch (...) {
    return false;
  }
}

//! All nodes are created with RoomNodeAttributes since that's the only type
//! that currently supports setting labels.
bool dsg_add_node(dsg_graph_t *g, dsg_layer_id_t layer_id,
                  dsg_node_id_t node_id, dsg_vec3_t p) {
  try {
    auto attrs = std::make_unique<spark_dsg::RoomNodeAttributes>();
    attrs->position = Eigen::Vector3d(p.x, p.y, p.z);
    g->impl->emplaceNode(static_cast<spark_dsg::LayerId>(layer_id),
                         static_cast<spark_dsg::NodeId>(node_id),
                         std::move(attrs));
    return true;
  } catch (...) {
    return false;
  }
}

bool dsg_set_node_label(dsg_graph_t *g, dsg_layer_id_t layer_id,
                        dsg_node_id_t node_id, const char *label) {
  try {
    auto &node = g->impl->getNode(static_cast<spark_dsg::NodeId>(node_id));
    // We only support setting room labels for now
    node.tryAttributes<spark_dsg::RoomNodeAttributes>()
        ->semantic_class_probabilities[std::string(label)] = 1.0;
    return true;
  } catch (...) {
    return false;
  }
}

bool dsg_add_edge(dsg_graph_t *g, dsg_node_id_t a_id, dsg_node_id_t b_id) {
  try {
    g->impl->insertEdge(static_cast<spark_dsg::NodeId>(a_id),
                        static_cast<spark_dsg::NodeId>(b_id), nullptr, false);
    return true;
  } catch (...) {
    return false;
  }
}

bool dsg_nest(dsg_graph_t *g, dsg_node_id_t nester, dsg_node_id_t nestee) {
  try {
    g->impl->insertEdge(static_cast<spark_dsg::NodeId>(nester),
                        static_cast<spark_dsg::NodeId>(nestee), nullptr, true);
    return true;
  } catch (...) {
    return false;
  }
}

bool dsg_save_json(const dsg_graph_t *g, const char *path) {
  try {
    const std::string out = path ? path : "graph.json";
    g->impl->save(out);
    return true;
  } catch (...) {
    return false;
  }
}

} // extern "C"
