use std::{collections::HashMap, fmt::Display};

use log::error;
use petgraph::graph::{EdgeIndex, NodeIndex};

use crate::{
    property_graph::{Properties, PropertyGraph},
    transformation::Operation,
};

#[derive(Debug)]
pub struct GraphTransformation {
    pub init: PropertyGraph,
    pub result: PropertyGraph,
    pub operations: Vec<String>,
    node_map: HashMap<u32, NodeIndex<u32>>,
    edge_map: HashMap<u32, EdgeIndex<u32>>,
    node_label_map: HashMap<u32, u32>,
    edge_label_map: HashMap<u32, u32>,
}

impl From<&PropertyGraph> for GraphTransformation {
    fn from(g: &PropertyGraph) -> Self {
        GraphTransformation {
            init: g.clone(),
            result: g.clone(),
            operations: Vec::new(),
            node_map: HashMap::new(),
            edge_map: HashMap::new(),
            node_label_map: HashMap::new(),
            edge_label_map: HashMap::new(),
        }
    }
}

impl Display for GraphTransformation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "===")?;
        write!(f, "{}", self.init)?;
        writeln!(f, "---")?;
        write!(f, "{}", self.result)?;
        writeln!(f, "===")
    }
}

impl Clone for GraphTransformation {
    fn clone(&self) -> Self {
        Self {
            init: self.init.clone(),
            result: self.result.clone(),
            operations: self.operations.clone(),
            node_map: self.node_map.clone(),
            edge_map: self.edge_map.clone(),
            node_label_map: self.node_label_map.clone(),
            edge_label_map: self.edge_label_map.clone(),
        }
    }
}

impl GraphTransformation {
    fn get_node_index(&self, id: &u32) -> NodeIndex<u32> {
        *self.node_map.get(&id).unwrap_or(&(*id).into())
    }

    fn get_edge_index(&self, id: &u32) -> EdgeIndex<u32> {
        *self.edge_map.get(&id).unwrap_or(&(*id).into())
    }

    fn get_node_label_index(&self, id: &u32) -> u32 {
        *self.node_label_map.get(id).unwrap_or(id)
    }

    fn get_edge_label_index(&self, id: &u32) -> u32 {
        *self.edge_label_map.get(id).unwrap_or(id)
    }
    pub fn apply(&mut self, op: &Operation) {
        match op {
            Operation::AddVertexLabel(v, l) => {
                let index = self.get_node_index(v);
                let lid = self.get_node_label_index(l);
                self.result
                    .vertex_label
                    .add_label_mapping(&index, lid)
                    .unwrap();
                let name = self.result.graph.node_weight(index).unwrap().name.clone();
                let label = self.result.vertex_label.get_label(lid).unwrap().clone();
                self.operations
                    .push(format!("AddVertexLabel({},{})", name, label));
            }
            Operation::CreateVertexLabel(l, name) => {
                //FIXME what if the name already exists ? Or the id ?
                let index = self.result.vertex_label.add_label(name.clone());
                self.node_label_map.insert(*l, index);
                self.operations.push(format!("CreateVertexLabel({})", name));
            }
            Operation::RemoveVertexLabel(v, l) => {
                let index = self.get_node_index(v);
                let lid = self.get_node_label_index(l);
                self.result
                    .vertex_label
                    .remove_label_mapping(&index, lid)
                    .unwrap();
                let name = self.result.graph.node_weight(index).unwrap().name.clone();
                let label = self.result.vertex_label.get_label(lid).unwrap().clone();
                self.operations
                    .push(format!("RemoveVertexLabel({},{})", name, label));
            }
            Operation::AddEdgeLabel(e, l) => {
                let index = self.get_edge_index(e);
                let lid = self.get_edge_label_index(l);
                self.result
                    .edge_label
                    .add_label_mapping(&index, lid)
                    .unwrap();
                let name = self.result.graph.edge_weight(index).unwrap().name.clone();
                let label = self.result.edge_label.get_label(lid).unwrap().clone();
                self.operations
                    .push(format!("AddEdgeLabel({},{})", name, label));
            }
            Operation::CreateEdgeLabel(l, name) => {
                //FIXME what if the name already exists ? Or the id ?
                let index = self.result.edge_label.add_label(name.clone());
                self.edge_label_map.insert(*l, index);
                self.operations.push(format!("CreateEdgeLabel({})", name));
            }
            Operation::RemoveEdgeLabel(e, l) => {
                let index = self.get_edge_index(e);
                let lid = self.get_edge_label_index(l);
                self.result
                    .edge_label
                    .remove_label_mapping(&index, lid)
                    .unwrap();
                let name = self.result.graph.edge_weight(index).unwrap().name.clone();
                let label = self.result.edge_label.get_label(lid).unwrap().clone();
                self.operations
                    .push(format!("RemoveEdgeLabel({},{})", name, label));
            }
            Operation::AddVertex(v) => {
                let index = self.get_node_index(v);
                if self.result.graph.contains_node(index) {
                    error!("Node {v} already exists.");
                    panic!("Node {v} already exists.");
                } else {
                    //TODO Need a name when creating a node.
                    let real_index = self.result.graph.add_node(Properties {
                        name: "".to_string(),
                        map: HashMap::new(),
                    });
                    self.node_map.insert(*v, real_index);
                }
            }
            Operation::RemoveVertex(v) => {
                let index = self.get_node_index(v);
                let name = self.result.graph.node_weight(index).unwrap().name.clone();
                self.result.vertex_label.remove_element(&index);
                self.result.graph.remove_node(index);
                self.node_map.remove(v);
                self.operations.push(format!("RemoveVertex({})", name));
            }
            Operation::AddEdge(e, start, end) => {
                let index = self.get_edge_index(e);
                if self.result.graph.edge_weight(index).is_some() {
                    error!("Edge {e} already exists.");
                    panic!("Edge {e} already exists.");
                } else {
                    //TODO Need a name when creating an edge.
                    let n1 = self.get_node_index(start);
                    let n2 = self.get_node_index(end);
                    let name1 = self.result.graph.node_weight(n1).unwrap().name.clone();
                    let name2 = self.result.graph.node_weight(n2).unwrap().name.clone();
                    let real_index = self.result.graph.add_edge(
                        n1,
                        n2,
                        Properties {
                            name: "".to_string(),
                            map: HashMap::new(),
                        },
                    );
                    self.edge_map.insert(*e, real_index);
                    self.operations
                        .push(format!("AddEdge({},{})", name1, name2));
                }
            }
            Operation::RemoveEdge(e) => {
                let index = self.get_edge_index(e);
                let name = self.result.graph.edge_weight(index).unwrap().name.clone();
                self.result.edge_label.remove_element(&index);
                self.result.graph.remove_edge(index);
                self.edge_map.remove(e);
                self.operations.push(format!("RemoveEdge({})", name));
            }
            Operation::AddVertexProperty(v, name, value) => {
                let prop = self
                    .result
                    .graph
                    .node_weight_mut(self.get_node_index(v))
                    .expect(&format!("Unknown vertex {v}"));
                prop.map.insert(name.to_string(), value.to_string());
                self.operations.push(format!(
                    "AddVertexProperty({},{},{})",
                    prop.name, name, value
                ));
            }
            Operation::RemoveVertexProperty(v, name) => {
                let prop = self
                    .result
                    .graph
                    .node_weight_mut(self.get_node_index(v))
                    .expect(&format!("Unknown vertex {v}"));
                prop.map.remove(name);
                self.operations
                    .push(format!("RemoveVertexProperty({},{})", prop.name, name));
            }
            Operation::AddEdgeProperty(e, name, value) => {
                let prop = self
                    .result
                    .graph
                    .edge_weight_mut(self.get_edge_index(e))
                    .expect(&format!("Unknown edge {e}"));
                prop.map.insert(name.to_string(), value.to_string());
                self.operations
                    .push(format!("AddEdgeProperty({},{},{})", prop.name, name, value));
            }
            Operation::RemoveEdgeProperty(e, name) => {
                let prop = self
                    .result
                    .graph
                    .edge_weight_mut(self.get_edge_index(e))
                    .expect(&format!("Unknown edge {e}"));
                prop.map.remove(name);
                self.operations
                    .push(format!("RemoveEdgeProperty({},{})", prop.name, name));
            }
            Operation::RenameVertex(v, name) => {
                let prop = self
                    .result
                    .graph
                    .node_weight_mut(self.get_node_index(v))
                    .expect(&format!("Unknown node {v}"));
                self.operations
                    .push(format!("RenameVertex({},{})", prop.name, name));
                prop.name = name.to_string();
            }
            Operation::RenameEdge(e, name) => {
                let prop = self
                    .result
                    .graph
                    .edge_weight_mut(self.get_edge_index(e))
                    .expect(&format!("Unknown edge {e}"));
                self.operations
                    .push(format!("RenameEdge({},{})", prop.name, name));
                prop.name = name.to_string();
            }
            Operation::MoveEdgeTarget(e, t) => {
                let edgeindex = self.get_edge_index(e);
                let src = self.result.graph.edge_endpoints(edgeindex).unwrap().0;
                let target = self.get_node_index(t);
                let w = self.result.graph.remove_edge(edgeindex).unwrap();
                let edgename = w.name.clone();
                let real_index = self.result.graph.add_edge(src, target, w);
                let labels: Vec<u32> = self
                    .result
                    .edge_label
                    .element_labels(&edgeindex)
                    .copied()
                    .collect();
                labels.into_iter().for_each(|l| {
                    self.result
                        .edge_label
                        .add_label_mapping(&real_index, l)
                        .unwrap()
                });
                self.result.edge_label.remove_element(&edgeindex);
                self.edge_map.insert(*e, real_index);
                self.operations.push(format!(
                    "MoveEdgeTarget({},{})",
                    edgename.clone(),
                    self.result.graph.node_weight(target).unwrap().name.clone()
                ));
            }
            Operation::MoveEdgeSource(e, s) => {
                let edgeindex = self.get_edge_index(e);
                let target = self.result.graph.edge_endpoints(edgeindex).unwrap().1;
                let src = self.get_node_index(s);
                let w = self.result.graph.remove_edge(edgeindex).unwrap();
                let edgename = w.name.clone();
                let real_index = self.result.graph.add_edge(src, target, w);
                let labels: Vec<u32> = self
                    .result
                    .edge_label
                    .element_labels(&edgeindex)
                    .copied()
                    .collect();
                labels.into_iter().for_each(|l| {
                    self.result
                        .edge_label
                        .add_label_mapping(&real_index, l)
                        .unwrap()
                });
                self.result.edge_label.remove_element(&edgeindex);
                self.edge_map.insert(*e, real_index);
                self.operations.push(format!(
                    "MoveEdgeSource({},{})",
                    edgename.clone(),
                    self.result.graph.node_weight(src).unwrap().name.clone()
                ));
            }
        }
    }
}
