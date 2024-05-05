use crate::errors::TransProofError;
use crate::property_graph::PropertyGraph;
use crate::souffle::extract_text;
use crate::transformation::souffle::extract_number;
use crate::{graph_transformation::GraphTransformation, transformation::souffle::OutputTuple};
use lazy_static::lazy_static;
use petgraph::visit::NodeIndexable;
use std::collections::HashMap;
use std::convert::TryFrom;

use self::souffle::Program;

pub mod souffle;

static OPERATIONS : [OperationName; 2] = [OperationName::RemoveVertexLabel, OperationName::AddVertexLabel];

pub enum Operation {
    AddVertexLabel(u32, u32),
    RemoveVertexLabel(u32,u32),
    AddEdgeLabel(u32, u32),
    RemoveEdgeLabel(u32,u32),
}

impl Operation {
    fn apply(&self, g: &mut GraphTransformation) {
        match self {
            Self::AddVertexLabel(v, l) => {
                g.result
                    .vertex_label
                    .add_label_mapping(&((*v).into()), *l)
                    .unwrap();
            },
            Self::RemoveVertexLabel(v, l) => {
                g.result
                    .vertex_label
                    .remove_label_mapping(&((*v).into()), *l)
                    .unwrap();
            },
            Self::AddEdgeLabel(v, l) => {
                g.result
                    .edge_label
                    .add_label_mapping(&((*v).into()), *l)
                    .unwrap();
            },
            Self::RemoveEdgeLabel(v, l) => {
                g.result
                    .edge_label
                    .remove_label_mapping(&((*v).into()), *l)
                    .unwrap();
            }
        }
    }
}

enum OperationName {
    AddVertexLabel,
    RemoveVertexLabel,
    AddEdgeLabel,
    RemoveEdgeLabel,
}

impl OperationName {
    fn get_relation(&self) -> &str {
        match self {
            Self::AddVertexLabel => "AddVertexLabel_",
            Self::RemoveVertexLabel => "RemoveVertexLabel_",
            Self::AddEdgeLabel => "AddEdgeLabel_",
            Self::RemoveEdgeLabel => "RemoveEdgeLabel_",
        }
    }
}

pub fn apply_single_transformation(program: Program, rel_name: &str, g: &PropertyGraph) -> Vec<GraphTransformation> {
    let mut res = vec![];
    let operations = souffle::generate_operations(program, rel_name, g);
    for transfo in operations.values() {
        let mut ng : GraphTransformation = g.into();
        for operation in transfo {
            operation.apply(&mut ng);
        }
        res.push(ng);
    }
    res
}

pub fn apply_transformations(program: Program, rel_names: &Vec<&str>, g: &PropertyGraph) -> Vec<GraphTransformation> {
    rel_names.iter().flat_map(|name| apply_single_transformation(program, name, g)).collect()
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
