use crate::errors::TransProofError;
use crate::property_graph::{LabelMap, Properties, PropertyGraph};
use crate::souffle::extract_text;
use crate::transformation::souffle::extract_number;
use crate::{graph_transformation::GraphTransformation, transformation::souffle::OutputTuple};
use lazy_static::lazy_static;
use log::error;
use petgraph::stable_graph::{NodeIndex, EdgeIndex};
use petgraph::visit::NodeIndexable;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::format;
use std::net::ToSocketAddrs;

use self::souffle::Program;

pub mod souffle;

static OPERATIONS : [OperationName; 18] = [
    OperationName::AddVertex,
    OperationName::CreateVertexLabel,
    OperationName::AddVertexLabel,
    OperationName::AddVertexProperty,
    OperationName::AddEdge,
    OperationName::CreateEdgeLabel,
    OperationName::AddEdgeLabel,
    OperationName::AddEdgeProperty,
    OperationName::MoveEdgeTarget,
    OperationName::MoveEdgeSource,
    OperationName::RenameVertex,
    OperationName::RenameEdge,
    OperationName::RemoveEdgeProperty,
    OperationName::RemoveEdgeLabel,
    OperationName::RemoveEdge,
    OperationName::RemoveVertexProperty,
    OperationName::RemoveVertexLabel,
    OperationName::RemoveVertex,
];

pub enum Operation {
    AddVertexLabel(u32, u32),
    CreateVertexLabel(u32, String),
    RemoveVertexLabel(u32,u32),
    CreateEdgeLabel(u32, String),
    AddEdgeLabel(u32, u32),
    RemoveEdgeLabel(u32,u32),
    AddVertex(u32),
    RemoveVertex(u32),
    AddEdge(u32,u32,u32),
    RemoveEdge(u32),
    AddVertexProperty(u32, String, String),
    RemoveVertexProperty(u32,String),
    AddEdgeProperty(u32, String, String),
    RemoveEdgeProperty(u32,String),
    RenameVertex(u32,String),
    RenameEdge(u32,String),
    MoveEdgeTarget(u32,u32),
    MoveEdgeSource(u32,u32),
}

fn get_node_index(id : &u32, node_map: &HashMap<u32, NodeIndex<u32>>) -> NodeIndex<u32> {
    *node_map.get(&id).unwrap_or(&(*id).into())
}

fn get_edge_index(id : &u32, edge_map: &HashMap<u32, EdgeIndex<u32>>) -> EdgeIndex<u32> {
    *edge_map.get(&id).unwrap_or(&(*id).into())
}

fn get_node_label_index(id: &u32, node_label_map: &HashMap<u32, u32>) -> u32 {
    *node_label_map.get(id).unwrap_or(id)
}

fn get_edge_label_index(id: &u32, edge_label_map: &HashMap<u32, u32>) -> u32 {
    *edge_label_map.get(id).unwrap_or(id)
}

impl Operation {
    fn apply(&self, g: &mut GraphTransformation, node_map: &mut HashMap<u32, NodeIndex<u32>>, edge_map: &mut HashMap<u32, EdgeIndex<u32>>, node_label_map: &mut HashMap<u32, u32>, edge_label_map: &mut HashMap<u32, u32>) {
        match self {
            Self::AddVertexLabel(v, l) => {
                let index = get_node_index(v, node_map);
                let lid = get_node_label_index(l, node_label_map);
                g.result
                    .vertex_label
                    .add_label_mapping(&index, lid)
                    .unwrap();
                let name = g.result.graph.node_weight(index).unwrap().name.clone();
                let label = g.result.vertex_label.get_label(lid).unwrap().clone();
                g.operations.push(format!("AddVertexLabel({},{})", name, label));
            },
            Self::CreateVertexLabel(l, name) => {
                //FIXME what if the name already exists ? Or the id ?
                let index = g.result.vertex_label.add_label(name.clone());
                node_label_map.insert(*l, index);
                g.operations.push(format!("CreateVertexLabel({})", name));
            }
            Self::RemoveVertexLabel(v, l) => {
                let index = get_node_index(v, node_map);
                let lid = get_node_label_index(l, node_label_map);
                g.result
                    .vertex_label
                    .remove_label_mapping(&index, lid)
                    .unwrap();
                let name = g.result.graph.node_weight(index).unwrap().name.clone();
                let label = g.result.vertex_label.get_label(lid).unwrap().clone();
                g.operations.push(format!("RemoveVertexLabel({},{})", name, label));
            },
            Self::AddEdgeLabel(e, l) => {
                let index = get_edge_index(e, edge_map);
                let lid = get_edge_label_index(l, edge_label_map);
                g.result
                    .edge_label
                    .add_label_mapping(&index, lid)
                    .unwrap();
                let name = g.result.graph.edge_weight(index).unwrap().name.clone();
                let label = g.result.edge_label.get_label(lid).unwrap().clone();
                g.operations.push(format!("AddEdgeLabel({},{})", name, label));
            },
            Self::CreateEdgeLabel(l, name) => {
                //FIXME what if the name already exists ? Or the id ?
                let index = g.result.edge_label.add_label(name.clone());
                edge_label_map.insert(*l, index);
                g.operations.push(format!("CreateEdgeLabel({})", name));
            }
            Self::RemoveEdgeLabel(e, l) => {
                let index = get_edge_index(e, edge_map);
                let lid = get_edge_label_index(l, edge_label_map);
                g.result
                    .edge_label
                    .remove_label_mapping(&index, lid)
                    .unwrap();
                let name = g.result.graph.edge_weight(index).unwrap().name.clone();
                let label = g.result.edge_label.get_label(lid).unwrap().clone();
                g.operations.push(format!("RemoveEdgeLabel({},{})", name, label));
            },
            Self::AddVertex(v) => {
                let index = get_node_index(v, node_map);
                if g.result.graph.contains_node(index) {
                    error!("Node {v} already exists.");
                    panic!("Node {v} already exists.");
                } else {
                    //TODO Need a name when creating a node.
                    let real_index = g.result.graph.add_node(Properties {
                        name : "".to_string(),
                        map : HashMap::new()
                    });
                    node_map.insert(*v, real_index);
                }
            },
            Self::RemoveVertex(v) => {
                let index = get_node_index(v, node_map);
                let name = g.result.graph.node_weight(index).unwrap().name.clone();
                g.result.vertex_label.remove_element(&index);
                g.result.graph.remove_node(index);
                node_map.remove(v);
                g.operations.push(format!("RemoveVertex({})", name));
            },
            Self::AddEdge(e, start, end) => {
                let index = get_edge_index(e, edge_map);
                if g.result.graph.edge_weight(index).is_some() {
                    error!("Edge {e} already exists.");
                    panic!("Edge {e} already exists.");
                } else {
                    //TODO Need a name when creating an edge.
                    let n1 = get_node_index(start, node_map);
                    let n2 = get_node_index(end, node_map);
                    let name1 = g.result.graph.node_weight(n1).unwrap().name.clone();
                    let name2 = g.result.graph.node_weight(n2).unwrap().name.clone();
                    let real_index = g.result.graph.add_edge(n1, n2, Properties {
                        name : "".to_string(),
                        map : HashMap::new()
                    });
                    edge_map.insert(*e, real_index);
                    g.operations.push(format!("AddEdge({},{})", name1, name2));
                }
            },
            Self::RemoveEdge(e) => {
                let index = get_edge_index(e, edge_map);
                let name = g.result.graph.edge_weight(index).unwrap().name.clone();
                g.result.edge_label.remove_element(&index);
                g.result.graph.remove_edge(index);
                edge_map.remove(e);
                g.operations.push(format!("RemoveEdge({})", name));
            },
            Self::AddVertexProperty(v, name, value) => {
                let prop = g.result.graph.node_weight_mut(get_node_index(v, node_map)).expect(&format!("Unknown vertex {v}"));
                prop.map.insert(name.to_string(), value.to_string());
                g.operations.push(format!("AddVertexProperty({},{},{})", prop.name, name, value));
            },
            Self::RemoveVertexProperty(v, name) => {
                let prop = g.result.graph.node_weight_mut(get_node_index(v, node_map)).expect(&format!("Unknown vertex {v}"));
                prop.map.remove(name);
                g.operations.push(format!("RemoveVertexProperty({},{})", prop.name, name));
            },
            Self::AddEdgeProperty(e, name, value) => {
                let prop = g.result.graph.edge_weight_mut(get_edge_index(e, edge_map)).expect(&format!("Unknown edge {e}"));
                prop.map.insert(name.to_string(), value.to_string());
                g.operations.push(format!("AddEdgeProperty({},{},{})", prop.name, name, value));
            },
            Self::RemoveEdgeProperty(e, name) => {
                let prop = g.result.graph.edge_weight_mut(get_edge_index(e, edge_map)).expect(&format!("Unknown edge {e}"));
                prop.map.remove(name);
                g.operations.push(format!("RemoveEdgeProperty({},{})", prop.name, name));
            },
            Self::RenameVertex(v, name) => {
                let prop = g.result.graph.node_weight_mut(get_node_index(v, node_map)).expect(&format!("Unknown node {v}"));
                g.operations.push(format!("RenameVertex({},{})", prop.name, name));
                prop.name = name.to_string();
            },
            Self::RenameEdge(e, name) => {
                let prop = g.result.graph.edge_weight_mut(get_edge_index(e, edge_map)).expect(&format!("Unknown edge {e}"));
                g.operations.push(format!("RenameEdge({},{})", prop.name, name));
                prop.name = name.to_string();
            },
            Self::MoveEdgeTarget(e,t) => {
                let edgeindex = get_edge_index(e, edge_map);
                let src = g.result.graph.edge_endpoints(edgeindex).unwrap().0;
                let target = get_node_index(t, node_map);
                let w = g.result.graph.remove_edge(edgeindex).unwrap();
                let edgename = w.name.clone();
                let real_index = g.result.graph.add_edge(src, target, w);
                let labels: Vec<u32> = g.result.edge_label.element_labels(&edgeindex).copied().collect();
                labels.into_iter().for_each(|l| g.result.edge_label.add_label_mapping(&real_index, l).unwrap());
                g.result.edge_label.remove_element(&edgeindex);
                edge_map.insert(*e, real_index);
                g.operations.push(format!("MoveEdgeTarget({},{})", edgename.clone(), g.result.graph.node_weight(target).unwrap().name.clone()));
            },
            Self::MoveEdgeSource(e,s) => {
                let edgeindex = get_edge_index(e, edge_map);
                let target = g.result.graph.edge_endpoints(edgeindex).unwrap().1;
                let src = get_node_index(s, node_map);
                let w = g.result.graph.remove_edge(edgeindex).unwrap();
                let edgename = w.name.clone();
                let real_index = g.result.graph.add_edge(src, target, w);
                let labels: Vec<u32> = g.result.edge_label.element_labels(&edgeindex).copied().collect();
                labels.into_iter().for_each(|l| g.result.edge_label.add_label_mapping(&real_index, l).unwrap());
                g.result.edge_label.remove_element(&edgeindex);
                edge_map.insert(*e, real_index);
                g.operations.push(format!("MoveEdgeSource({},{})", edgename.clone(), g.result.graph.node_weight(src).unwrap().name.clone()));
            },
        }
    }
}

enum OperationName {
    CreateVertexLabel,
    CreateEdgeLabel,
    AddVertexLabel,
    RemoveVertexLabel,
    AddEdgeLabel,
    RemoveEdgeLabel,
    AddVertex,
    RemoveVertex,
    AddEdge,
    RemoveEdge,
    AddVertexProperty,
    RemoveVertexProperty,
    AddEdgeProperty,
    RemoveEdgeProperty,
    RenameVertex,
    RenameEdge,
    MoveEdgeTarget,
    MoveEdgeSource,
}

impl OperationName {
    fn get_relation(&self) -> &str {
        match self {
            Self::CreateVertexLabel => "CreateVertexLabel_",
            Self::CreateEdgeLabel => "CreateEdgeLabel_",
            Self::AddVertexLabel => "AddVertexLabel_",
            Self::RemoveVertexLabel => "RemoveVertexLabel_",
            Self::AddEdgeLabel => "AddEdgeLabel_",
            Self::RemoveEdgeLabel => "RemoveEdgeLabel_",
            Self::AddVertex => "AddVertex_",
            Self::RemoveVertex => "RemoveVertex_",
            Self::AddEdge => "AddEdge_",
            Self::RemoveEdge => "RemoveEdge_",
            Self::AddVertexProperty => "AddVertexProperty_",
            Self::RemoveVertexProperty => "RemoveVertexProperty_",
            Self::AddEdgeProperty => "AddEdgeProperty_",
            Self::RemoveEdgeProperty => "RemoveEdgeProperty_",
            Self::RenameVertex => "RenameVertex_",
            Self::RenameEdge => "RenameEdge_",
            Self::MoveEdgeTarget => "MoveEdgeTarget_",
            Self::MoveEdgeSource => "MoveEdgeSource_",
        }
    }
}

pub fn apply_single_transformation(program: Program, rel_name: &str, g: &PropertyGraph, target_graph: &Option<PropertyGraph>) -> Vec<GraphTransformation> {
    let mut res = vec![];
    let operations = souffle::generate_operations(program, rel_name, g, target_graph);
    for transfo in operations.values() {
        let mut ng : GraphTransformation = g.into();
        let mut node_map = HashMap::new();
        let mut edge_map = HashMap::new();
        let mut node_label_map = HashMap::new();
        let mut edge_label_map = HashMap::new();
        for operation in transfo {
            operation.apply(&mut ng, &mut node_map, &mut edge_map, &mut node_label_map, &mut edge_label_map);
        }
        if ng.result.check_unique_names() {
            res.push(ng);
        }
    }
    res
}

pub fn apply_transformations(program: Program, rel_names: &Vec<&str>, g: &PropertyGraph, target_graph: &Option<PropertyGraph>) -> Vec<GraphTransformation> {
    rel_names.iter().flat_map(|name| apply_single_transformation(program, name, g, target_graph)).collect()
}

/*
pub fn relabel_vertex_souffle(program: Program, g: &PropertyGraph) -> Vec<GraphTransformation> {
    fn extract_data(tuple: OutputTuple) -> (u32, u32, u32) {
        (
            extract_number(tuple),
            extract_number(tuple),
            extract_number(tuple),
        )
    }
    fn relabel(g: &PropertyGraph, operation: (u32, u32, u32)) -> GraphTransformation {
        let mut res: GraphTransformation = g.into();
        res.result
            .vertex_label
            .remove_label_mapping(&(operation.0.into()), operation.1)
            .unwrap();
        res.result
            .vertex_label
            .add_label_mapping(&(operation.0.into()), operation.2)
            .unwrap();
        res
    }
    apply_transformation(program, "RelabelVertex", extract_data, relabel, g)
}

pub fn remove_edge(program: Program, g: &PropertyGraph) -> Vec<GraphTransformation> {
    fn extract_data(tuple: OutputTuple) -> u32 {
        extract_number(tuple)
    }
    fn remove(g: &PropertyGraph, operation: u32) -> GraphTransformation {
        let mut res: GraphTransformation = g.into();
        let index = operation.into();
        res.result.graph.remove_edge(index);
        res.result.edge_label.remove_element(&index);
        res
    }
    apply_transformation(program, "RemoveEdge", extract_data, remove, g)
}

pub fn remove_vertex_property(program: Program, g: &PropertyGraph) -> Vec<GraphTransformation> {
    fn extract_data(tuple: OutputTuple) -> (u32, std::string::String) {
        (extract_number(tuple), extract_text(tuple))
    }
    fn remove(g: &PropertyGraph, operation: (u32, std::string::String)) -> GraphTransformation {
        let mut res: GraphTransformation = g.into();
        let index = operation.0.into();
        res.result
            .graph
            .node_weight_mut(index)
            .unwrap()
            .map
            .remove(&operation.1);
        res
    }
    apply_transformation(program, "RemoveProperty", extract_data, remove, g)
}

*/
