use std::ptr::{null, null_mut};

use cxx::{let_cxx_string, CxxString, UniquePtr};
use petgraph::visit::{EdgeRef, IntoEdgeReferences, IntoNodeReferences, NodeRef};

use crate::{graph_transformation::GraphTransformation, property_graph::PropertyGraph};

mod souffle_ffi;

pub type SouffleProgram = *mut souffle_ffi::SouffleProgram;
type Relation = *mut souffle_ffi::Relation;
type InputTuple = UniquePtr<souffle_ffi::tuple>;
pub type OutputTuple = *const souffle_ffi::tuple;

pub fn create_program_instance(name: &str) -> SouffleProgram {
    let_cxx_string!(cname = name);
    souffle_ffi::newInstance(&cname)
}

fn get_relation(program: SouffleProgram, name: &str) -> Option<Relation> {
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

fn fill_relation<E, I, F>(
    program: *mut souffle_ffi::SouffleProgram,
    relation_name: &str,
    elements: I,
    to_tuple: F,
) where
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

fn encode_graph(program: SouffleProgram, graph: &PropertyGraph) {
    fill_relation(
        program,
        "VertexLabel",
        graph.vertex_label.labels(),
        |tup, id| {
            souffle_ffi::insertNumber(tup, *id);
        },
    );
    fill_relation(program, "Vertex", graph.graph.node_references(), |tup, node| {
        souffle_ffi::insertNumber(tup, node.id().index() as u32);
    });
    fill_relation(
        program,
        "VertexHasLabel",
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
        "VertexProperty",
        graph.graph.node_indices().flat_map(|n| {
            let weight = graph.graph.node_weight(n).unwrap();
            std::iter::repeat(n).zip(weight.map.iter()).map(|(n,pair)| (n, pair.0, pair.1))
        }),
        |tup, data| {
            souffle_ffi::insertNumber(tup, data.0.id().index() as u32);
            let_cxx_string!(name = data.1);
            souffle_ffi::insertText(tup, &name);
            let_cxx_string!(value = data.2);
            souffle_ffi::insertText(tup, &value);
    });
    fill_relation(
        program,
        "EdgeLabel",
        graph.edge_label.labels(),
        |tup, id| {
            souffle_ffi::insertNumber(tup, *id);
        },
    );
    fill_relation(program, "Edge", graph.graph.edge_references(), |tup, edge| {
        souffle_ffi::insertNumber(tup, edge.id().index() as u32);
        souffle_ffi::insertNumber(tup, edge.source().index() as u32);
        souffle_ffi::insertNumber(tup, edge.target().index() as u32);
    });
    fill_relation(
        program,
        "EdgeProperty",
        graph.graph.edge_indices().flat_map(|n| {
            let weight = graph.graph.edge_weight(n).unwrap();
            std::iter::repeat(n).zip(weight.map.iter()).map(|(n,pair)| (n, pair.0, pair.1))
        }),
        |tup, data| {
            souffle_ffi::insertNumber(tup, data.0.index() as u32);
            let_cxx_string!(name = data.1);
            souffle_ffi::insertText(tup, &name);
            let_cxx_string!(value = data.2);
            souffle_ffi::insertText(tup, &value);
    });
    fill_relation(
        program,
        "EdgeHasLabel",
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

pub fn extract_number(tuple : OutputTuple) -> u32 {
    unsafe {
        souffle_ffi::getNumber(tuple)
    }
}

pub fn extract_text(tuple : OutputTuple) -> std::string::String {
    unsafe {
        let str = souffle_ffi::getText(tuple);
        str.to_str().expect("Error with utf8.").to_string()
    }
}

pub fn apply_transformation<P, Ex, Tr>(program : SouffleProgram, output_relation_name : &str, extract_data : Ex, apply_transfo : Tr, g : &PropertyGraph) -> Vec<GraphTransformation> 
where
    Ex : Fn(OutputTuple) -> P,
    Tr : Fn(&PropertyGraph, P) -> GraphTransformation
{ 
    let mut res = Vec::new();
    encode_graph(program, g);
    unsafe {
        souffle_ffi::runProgram(program);
        let out_relation = get_relation(program, output_relation_name).expect("No relation for the transformations.");
        let mut iter = souffle_ffi::createTupleIterator(out_relation);
        while souffle_ffi::hasNext(&iter) {
            let params = extract_data(souffle_ffi::getNext(&mut iter));
            res.push(apply_transfo(g, params));
        }
        souffle_ffi::purgeProgram(program);
    }
    res
}