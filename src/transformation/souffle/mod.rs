use std::{collections::HashMap, ptr::{null, null_mut}};

use cxx::{let_cxx_string, CxxString, UniquePtr};
use petgraph::visit::{EdgeRef, IntoEdgeReferences, IntoNodeReferences, NodeRef};

use crate::{graph_transformation::GraphTransformation, property_graph::PropertyGraph};

use log::{error, info};

use self::souffle_ffi::getNumber;

use super::{Operation, OperationName, OPERATIONS};

mod souffle_ffi;

pub type Program = *mut souffle_ffi::SouffleProgram;
type Relation = *mut souffle_ffi::Relation;
type InputTuple = UniquePtr<souffle_ffi::tuple>;
pub type OutputTuple = *const souffle_ffi::tuple;

const INPUT_RELATION_NAMES: [&'static str; 12] = [
    "VertexLabel",
    "VertexLabelName",
    "Vertex",
    "VertexName",
    "VertexHasLabel",
    "VertexProperty",
    "EdgeLabel",
    "EdgeLabelName",
    "Edge",
    "EdgeName",
    "EdgeProperty",
    "EdgeHasLabel",
];

const TARGET_RELATION_NAMES: [&'static str; 12] = [
    "TargetVertexLabel",
    "TargetVertexLabelName",
    "TargetVertex",
    "TargetVertexName",
    "TargetVertexHasLabel",
    "TargetVertexProperty",
    "TargetEdgeLabel",
    "TargetEdgeLabelName",
    "TargetEdge",
    "TargetEdgeName",
    "TargetEdgeProperty",
    "TargetEdgeHasLabel",
];

pub fn create_program_instance(name: &str) -> Program {
    let_cxx_string!(cname = name);
    souffle_ffi::newInstance(&cname)
}

pub fn get_transfos(prog: Program) -> Option<Vec<String>> {
    unsafe {
        souffle_ffi::runProgram(prog);
        if let Some(rel_transfo) = get_relation(prog, "Transformation") {
            let mut names = vec![];
            let mut iter = souffle_ffi::createTupleIterator(rel_transfo);
            while souffle_ffi::hasNext(&iter) {
                let tup = souffle_ffi::getNext(&mut iter);
                names.push(extract_text(tup));
            }
            Some(names)
        } else {
            None
        }
    }
}

pub fn free_program(prog: Program) {
    unsafe {
        souffle_ffi::freeProgram(prog);
    }
}

pub fn has_relation(prog: Program, name: &str) -> bool {
    get_relation(prog, name).is_some()
}

fn get_relation(program: Program, name: &str) -> Option<Relation> {
    let_cxx_string!(cname = name);
    unsafe {
        let relation = souffle_ffi::getRelation(program, &cname);
        if relation.is_null() {
            None
        } else {
            Some(relation)
        }
    }
}

fn fill_relation<E, I, F>(program: Program, relation_name: &str, elements: I, to_tuple: F)
where
    I: Iterator<Item = E>,
    F: Fn(&InputTuple, E),
{
    if let Some(relation) = get_relation(program, relation_name) {
        for element in elements {
            unsafe {
                let tuple = souffle_ffi::createTuple(relation);
                to_tuple(&tuple, element);
                souffle_ffi::insertTuple(relation, tuple);
            }
        }
    }
}

fn encode_graph(program: Program, graph: &PropertyGraph, relation_names: &[&str; 12]) {
    fill_relation(
        program,
        relation_names[0],
        graph.vertex_label.labels(),
        |tup, id| {
            souffle_ffi::insertNumber(tup, *id);
        },
    );
    fill_relation(
        program,
        relation_names[1],
        graph.vertex_label.labels().map(|id| (id, graph.vertex_label.get_label(*id).unwrap())),
        |tup, (id, name)| {
            souffle_ffi::insertNumber(tup, *id);
            let_cxx_string!(cname=name);
            souffle_ffi::insertText(tup, &cname);
        },
    );
    fill_relation(
        program,
        relation_names[2],
        graph.graph.node_references(),
        |tup, node| {
            souffle_ffi::insertNumber(tup, node.id().index() as u32);
        },
    );
    fill_relation(
        program,
        relation_names[3],
        graph.graph.node_references(),
        |tup, node| {
            souffle_ffi::insertNumber(tup, node.id().index() as u32);
            let name = &node.weight().name;
            let_cxx_string!(cname = name);
            souffle_ffi::insertText(tup, &cname);
        },
    );
    fill_relation(
        program,
        relation_names[4],
        graph
            .graph
            .node_indices()
            .flat_map(|id| std::iter::repeat(id).zip(graph.vertex_label.element_labels(&id))),
        |tup, (vertex, label)| {
            souffle_ffi::insertNumber(tup, vertex.index() as u32);
            souffle_ffi::insertNumber(tup, *label);
        },
    );
    fill_relation(
        program,
        relation_names[5],
        graph.graph.node_indices().flat_map(|n| {
            let weight = graph.graph.node_weight(n).unwrap();
            std::iter::repeat(n)
                .zip(weight.map.iter())
                .map(|(n, pair)| (n, pair.0, pair.1))
        }),
        |tup, data| {
            souffle_ffi::insertNumber(tup, data.0.id().index() as u32);
            let_cxx_string!(name = data.1);
            souffle_ffi::insertText(tup, &name);
            let_cxx_string!(value = data.2);
            souffle_ffi::insertText(tup, &value);
        },
    );
    fill_relation(
        program,
        relation_names[6],
        graph.edge_label.labels(),
        |tup, id| {
            souffle_ffi::insertNumber(tup, *id);
        },
    );
    fill_relation(
        program,
        relation_names[7],
        graph.edge_label.labels().map(|id| (id, graph.edge_label.get_label(*id).unwrap())),
        |tup, (id, name)| {
            souffle_ffi::insertNumber(tup, *id);
            let_cxx_string!(cname=name);
            souffle_ffi::insertText(tup, &cname);
        },
    );
    fill_relation(
        program,
        relation_names[8],
        graph.graph.edge_references(),
        |tup, edge| {
            souffle_ffi::insertNumber(tup, edge.id().index() as u32);
            souffle_ffi::insertNumber(tup, edge.source().index() as u32);
            souffle_ffi::insertNumber(tup, edge.target().index() as u32);
        },
    );
    fill_relation(
        program,
        relation_names[9],
        graph.graph.edge_references(),
        |tup, edge| {
            souffle_ffi::insertNumber(tup, edge.id().index() as u32);
            let name = &edge.weight().name;
            let_cxx_string!(cname = name);
            souffle_ffi::insertText(tup, &cname);
        },
    );
    fill_relation(
        program,
        relation_names[10],
        graph.graph.edge_indices().flat_map(|n| {
            let weight = graph.graph.edge_weight(n).unwrap();
            std::iter::repeat(n)
                .zip(weight.map.iter())
                .map(|(n, pair)| (n, pair.0, pair.1))
        }),
        |tup, data| {
            souffle_ffi::insertNumber(tup, data.0.index() as u32);
            let_cxx_string!(name = data.1);
            souffle_ffi::insertText(tup, &name);
            let_cxx_string!(value = data.2);
            souffle_ffi::insertText(tup, &value);
        },
    );
    fill_relation(
        program,
        relation_names[11],
        graph
            .graph
            .edge_indices()
            .flat_map(|id| std::iter::repeat(id).zip(graph.edge_label.element_labels(&id))),
        |tup, (edge, label)| {
            souffle_ffi::insertNumber(tup, edge.index() as u32);
            souffle_ffi::insertNumber(tup, *label);
        },
    );
}

pub fn encode_input_graph(program: Program, graph: &PropertyGraph) {
    encode_graph(program, graph, &INPUT_RELATION_NAMES);
}

pub fn encode_target_graph(program: Program, graph: &PropertyGraph) {
    encode_graph(program, graph, &TARGET_RELATION_NAMES);
}

pub fn extract_number(tuple: OutputTuple) -> u32 {
    unsafe { souffle_ffi::getNumber(tuple) }
}

pub fn extract_signed(tuple: OutputTuple) -> i32 {
    unsafe { souffle_ffi::getSigned(tuple) }
}

pub fn extract_text(tuple: OutputTuple) -> std::string::String {
    unsafe {
        let str = souffle_ffi::getText(tuple);
        str.to_str().expect("Error with utf8.").to_string()
    }
}

impl OperationName {
    fn construct(&self, t : OutputTuple) -> Operation {
        unsafe {
        match self {
                Self::CreateVertexLabel => {
                    let label = extract_number(t);
                    let name = extract_text(t);
                    Operation::CreateVertexLabel(label, name)
                },
                Self::CreateEdgeLabel => {
                    let label = extract_number(t);
                    let name = extract_text(t);
                    Operation::CreateEdgeLabel(label, name)
                },
                Self::AddVertexLabel => {
                    let vertex = extract_number(t);
                    let label = extract_number(t);
                    Operation::AddVertexLabel(vertex, label)
                },
                Self::RemoveVertexLabel => {
                    let vertex = extract_number(t);
                    let label = extract_number(t);
                    Operation::RemoveVertexLabel(vertex, label)

                },
                Self::AddEdgeLabel => {
                    let edge = extract_number(t);
                    let label = extract_number(t);
                    Operation::AddEdgeLabel(edge, label)
                },
                Self::RemoveEdgeLabel => {
                    let edge = extract_number(t);
                    let label = extract_number(t);
                    Operation::RemoveEdgeLabel(edge, label)
                },
                Self::AddVertex => {
                    let vertex = extract_number(t);
                    Operation::AddVertex(vertex)
                },
                Self::RemoveVertex => {
                    let vertex = extract_number(t);
                    Operation::RemoveVertex(vertex)

                },
                Self::AddEdge => {
                    let edge = extract_number(t);
                    let from = extract_number(t);
                    let to = extract_number(t);
                    Operation::AddEdge(edge, from, to)
                },
                Self::RemoveEdge => {
                    let edge = extract_number(t);
                    Operation::RemoveEdge(edge)
                },
                Self::AddVertexProperty => {
                    let vertex = extract_number(t);
                    let name = extract_text(t);
                    let value = extract_text(t);
                    Operation::AddVertexProperty(vertex, name, value)
                },
                Self::RemoveVertexProperty => {
                    let vertex = extract_number(t);
                    let name = extract_text(t);
                    Operation::RemoveVertexProperty(vertex, name)

                },
                Self::AddEdgeProperty => {
                    let edge = extract_number(t);
                    let name = extract_text(t);
                    let value = extract_text(t);
                    Operation::AddEdgeProperty(edge, name, value)
                },
                Self::RemoveEdgeProperty => {
                    let edge = extract_number(t);
                    let name = extract_text(t);
                    Operation::RemoveEdgeProperty(edge, name)
                },
                Self::RenameVertex => {
                    let vertex = extract_number(t);
                    let name = extract_text(t);
                    Operation::RenameVertex(vertex, name)
                },
                Self::RenameEdge => {
                    let edge = extract_number(t);
                    let name = extract_text(t);
                    Operation::RenameEdge(edge, name)
                },
                Self::MoveEdgeTarget => {
                    let edge = extract_number(t);
                    let target = extract_number(t);
                    Operation::MoveEdgeTarget(edge, target)
                },
                Self::MoveEdgeSource => {
                    let edge = extract_number(t);
                    let source = extract_number(t);
                    Operation::MoveEdgeSource(edge, source)
                },
            }
        }
    }
}

pub fn generate_operations(program: Program, relation_name: &str, g: &PropertyGraph, target_graph: &Option<PropertyGraph>) -> HashMap<i32, Vec<Operation>> {
    encode_input_graph(program, g);
    if let Some(target) = target_graph {
        encode_target_graph(program, target);
    }
    unsafe {
        souffle_ffi::runProgram(program);
        let out_relation = get_relation(program, relation_name)
            .expect("No relation for the transformations.");
        let mut iter = souffle_ffi::createTupleIterator(out_relation);
        let mut ids = vec![];
        while souffle_ffi::hasNext(&iter) {
            let id = extract_signed(souffle_ffi::getNext(&mut iter));
            ids.push(id);
        }
        let mut operations : HashMap<i32, Vec<Operation>> = HashMap::new();
        for operation in OPERATIONS.iter() {
            if let Some(out_relation) = get_relation(program, operation.get_relation()) {
                let mut iter = souffle_ffi::createTupleIterator(out_relation);
                while souffle_ffi::hasNext(&iter) {
                    let t = souffle_ffi::getNext(&mut iter);
                    let name = extract_text(t);
                    if name == relation_name {
                        let id = extract_signed(t);
                        let op = operation.construct(t);
                        operations.entry(id).or_default().push(op);
                    }
                }

            }
        }
        souffle_ffi::purgeProgram(program);
        operations
    }
}
