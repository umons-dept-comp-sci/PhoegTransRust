use std::{
    collections::{HashMap, HashSet, VecDeque},
    ptr::{null, null_mut},
};

use cxx::{let_cxx_string, CxxString, UniquePtr};
use petgraph::visit::{EdgeRef, IntoEdgeReferences, IntoNodeReferences, NodeRef};
use souffle_ffi::{
    decode_symbol, getRecordTable, getSymbolTable, unpack_record, RecordTable, SymbolTable,
};

use crate::{graph_transformation::GraphTransformation, property_graph::PropertyGraph};

use log::{debug, error, info};

use self::souffle_ffi::getNumber;

use super::{
    name_from_order, Operation, OperationName, OperationWithIds, OPERATIONS, OPERATION_ORDER,
};

mod souffle_ffi;

pub type Program = *mut souffle_ffi::SouffleProgram;
type Relation = *mut souffle_ffi::Relation;
type InputTuple = UniquePtr<souffle_ffi::tuple>;
pub type OutputTuple = *const souffle_ffi::tuple;

pub struct RelationNames<'a> {
    pub vertex_label: &'a str,
    pub vertex: &'a str,
    pub vertex_has_label: &'a str,
    pub vertex_property: &'a str,
    pub edge_label: &'a str,
    pub edge: &'a str,
    pub edge_has_label: &'a str,
    pub edge_property: &'a str,
}

pub const INPUT_RELATION_NAMES: RelationNames<'static> = RelationNames {
    vertex_label: "VertexLabel",
    vertex: "Vertex",
    vertex_has_label: "VertexHasLabel",
    vertex_property: "VertexProperty",
    edge_label: "EdgeLabel",
    edge: "Edge",
    edge_has_label: "EdgeHasLabel",
    edge_property: "EdgeProperty",
};

// same with target relation names
pub const TARGET_RELATION_NAMES: RelationNames<'static> = RelationNames {
    vertex_label: "TargetVertexLabel",
    vertex: "TargetVertex",
    vertex_has_label: "TargetVertexHasLabel",
    vertex_property: "TargetVertexProperty",
    edge_label: "TargetEdgeLabel",
    edge: "TargetEdge",
    edge_has_label: "TargetEdgeHasLabel",
    edge_property: "TargetEdgeProperty",
};

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
            // print!("{}(",relation_name);
            unsafe {
                let tuple = souffle_ffi::createTuple(relation);
                to_tuple(&tuple, element);
                souffle_ffi::insertTuple(relation, tuple);
            }
            // println!(").");
        }
    }
}

fn encode_graph(program: Program, graph: &PropertyGraph, relation_names: &RelationNames<'static>) {
    let vid_to_name: HashMap<u32, &str> = graph
        .graph
        .node_references()
        .map(|(index, props)| (index.id().index() as u32, props.name.as_str()))
        .collect();
    let lvid_to_label: HashMap<u32, &str> = graph
        .vertex_label
        .labels()
        .map(|&id| (id, graph.vertex_label.get_label(id).unwrap().as_str()))
        .collect();
    let eid_to_name: HashMap<u32, &str> = graph
        .graph
        .edge_references()
        .map(|eref| (eref.id().index() as u32, eref.weight().name.as_str()))
        .collect();
    let leid_to_label: HashMap<u32, &str> = graph
        .edge_label
        .labels()
        .map(|&id| (id, graph.edge_label.get_label(id).unwrap().as_str()))
        .collect();
    fill_relation(
        program,
        relation_names.vertex_label,
        lvid_to_label.values(),
        |tup, name| {
            // print!("{}",id);
            let_cxx_string!(cname = name);
            souffle_ffi::insertText(tup, &cname);
        },
    );
    fill_relation(
        program,
        relation_names.vertex,
        graph.graph.node_references(),
        |tup, (_, prop)| {
            // print!("{}",node.id().index());
            let_cxx_string!(name = &prop.name);
            souffle_ffi::insertText(tup, &name);
        },
    );
    fill_relation(
        program,
        relation_names.vertex_has_label,
        graph.graph.node_indices().flat_map(|id| {
            std::iter::repeat(vid_to_name.get(&(id.index() as u32)).unwrap()).zip(
                graph
                    .vertex_label
                    .element_labels(&id)
                    .map(|&id| lvid_to_label.get(&id).unwrap()),
            )
        }),
        |tup, (vertex, label)| {
            // print!("{}, {}",vertex.index(),label);
            let_cxx_string!(vname = vertex);
            let_cxx_string!(lname = label);
            souffle_ffi::insertText(tup, &vname);
            souffle_ffi::insertText(tup, &lname);
        },
    );
    fill_relation(
        program,
        relation_names.vertex_property,
        graph.graph.node_indices().flat_map(|n| {
            let weight = graph.graph.node_weight(n).unwrap();
            std::iter::repeat(vid_to_name.get(&(n.index() as u32)).unwrap())
                .zip(weight.map.iter())
                .map(|(n, pair)| (n, pair.0, pair.1))
        }),
        |tup, data| {
            // print!("{}, \"{}\", \"{}\"",data.0.id().index(),data.1,data.2);
            let_cxx_string!(vname = data.0);
            souffle_ffi::insertText(tup, &vname);
            let_cxx_string!(name = data.1);
            souffle_ffi::insertText(tup, &name);
            let_cxx_string!(value = data.2);
            souffle_ffi::insertText(tup, &value);
        },
    );
    fill_relation(
        program,
        relation_names.edge_label,
        leid_to_label.values(),
        |tup, name| {
            // print!("{}",id);
            let_cxx_string!(cname = name);
            souffle_ffi::insertText(tup, &cname);
        },
    );
    fill_relation(
        program,
        relation_names.edge,
        graph.graph.edge_references().map(|eref| {
            (
                eid_to_name.get(&(eref.id().index() as u32)).unwrap(),
                vid_to_name.get(&(eref.source().index() as u32)).unwrap(),
                vid_to_name.get(&(eref.target().index() as u32)).unwrap(),
            )
        }),
        |tup, (edge, source, target)| {
            // print!("{}, {}, {}",edge.id().index(),edge.source().index(),edge.target().index());
            let_cxx_string!(cedge = edge);
            souffle_ffi::insertText(tup, &cedge);
            let_cxx_string!(csource = source);
            souffle_ffi::insertText(tup, &csource);
            let_cxx_string!(ctarget = target);
            souffle_ffi::insertText(tup, &ctarget);
        },
    );
    fill_relation(
        program,
        relation_names.edge_has_label,
        graph.graph.edge_indices().flat_map(|id| {
            std::iter::repeat(eid_to_name.get(&(id.index() as u32)).unwrap()).zip(
                graph
                    .edge_label
                    .element_labels(&id)
                    .map(|id| leid_to_label.get(id).unwrap()),
            )
        }),
        |tup, (edge, label)| {
            let_cxx_string!(cedge = edge);
            souffle_ffi::insertText(tup, &cedge);
            let_cxx_string!(clabel = label);
            souffle_ffi::insertText(tup, &clabel);
            // print!("{}, {}",edge.index(),label);
        },
    );
    fill_relation(
        program,
        relation_names.edge_property,
        graph.graph.edge_indices().flat_map(|e| {
            let weight = graph.graph.edge_weight(e).unwrap();
            std::iter::repeat(eid_to_name.get(&(e.index() as u32)).unwrap())
                .zip(weight.map.iter())
                .map(|(n, pair)| (n, pair.0, pair.1))
        }),
        |tup, data| {
            let_cxx_string!(ename = data.0);
            souffle_ffi::insertText(tup, &ename);
            let_cxx_string!(name = data.1);
            souffle_ffi::insertText(tup, &name);
            let_cxx_string!(value = data.2);
            souffle_ffi::insertText(tup, &value);
            // print!("{}, \"{}\", \"{}\"",data.0.index(),data.1,data.2);
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

impl Operation {
    fn from_record_index(
        index: i32,
        record: &RecordTable,
        symbol: &SymbolTable,
    ) -> Option<Operation> {
        // Souffle stores data in its record table. A first record of arity 2 contains the
        // operation id (from 0 and ordered alphabetically) and the second value depends on the
        // operation arity.
        // If arity is 1, the second value is the argument.
        // If arity is > 1, the second value is an index to another record.
        let (name, mut args) = unsafe {
            let values = unpack_record(record, index, 2);
            let name = name_from_order(values[0])?;
            let args: VecDeque<String> = match name.arity() {
                1 => {
                    let arg = decode_symbol(symbol, values[1]);
                    VecDeque::from([arg])
                }
                x if x > 1 => unpack_record(record, values[1], name.arity())
                    .into_iter()
                    .map(|id| decode_symbol(symbol, id))
                    .collect(),
                _ => panic!("Arity should be at least 1"),
            };
            (name, args)
        };
        match name {
            OperationName::AddVertexLabel => Some(Operation::AddVertexLabel(
                args.pop_front()?,
                args.pop_front()?,
            )),
            OperationName::RemoveVertexLabel => Some(Operation::RemoveVertexLabel(
                args.pop_front()?,
                args.pop_front()?,
            )),
            OperationName::AddEdgeLabel => Some(Operation::AddEdgeLabel(
                args.pop_front()?,
                args.pop_front()?,
            )),
            OperationName::RemoveEdgeLabel => Some(Operation::RemoveEdgeLabel(
                args.pop_front()?,
                args.pop_front()?,
            )),
            OperationName::AddVertex => Some(Operation::AddVertex(args.pop_front()?)),
            OperationName::RemoveVertex => Some(Operation::RemoveVertex(args.pop_front()?)),
            OperationName::AddEdge => Some(Operation::AddEdge(
                args.pop_front()?,
                args.pop_front()?,
                args.pop_front()?,
            )),
            OperationName::RemoveEdge => Some(Operation::RemoveEdge(args.pop_front()?)),
            OperationName::AddVertexProperty => Some(Operation::AddVertexProperty(
                args.pop_front()?,
                args.pop_front()?,
                args.pop_front()?,
            )),
            OperationName::RemoveVertexProperty => Some(Operation::RemoveVertexProperty(
                args.pop_front()?,
                args.pop_front()?,
            )),
            OperationName::AddEdgeProperty => Some(Operation::AddEdgeProperty(
                args.pop_front()?,
                args.pop_front()?,
                args.pop_front()?,
            )),
            OperationName::RemoveEdgeProperty => Some(Operation::RemoveEdgeProperty(
                args.pop_front()?,
                args.pop_front()?,
            )),
            OperationName::RenameVertex => Some(Operation::RenameVertex(
                args.pop_front()?,
                args.pop_front()?,
            )),
            OperationName::RenameEdge => {
                Some(Operation::RenameEdge(args.pop_front()?, args.pop_front()?))
            }
            OperationName::MoveEdgeTarget => Some(Operation::MoveEdgeTarget(
                args.pop_front()?,
                args.pop_front()?,
            )),
            OperationName::MoveEdgeSource => Some(Operation::MoveEdgeSource(
                args.pop_front()?,
                args.pop_front()?,
            )),
        }
    }
}

impl OperationName {
    fn construct(&self, t: OutputTuple) -> OperationWithIds {
        unsafe {
            match self {
                Self::AddVertexLabel => {
                    let vertex = extract_number(t);
                    let label = extract_number(t);
                    let labelname = extract_text(t);
                    OperationWithIds::AddVertexLabel(vertex, label, labelname)
                }
                Self::RemoveVertexLabel => {
                    let vertex = extract_number(t);
                    let label = extract_number(t);
                    OperationWithIds::RemoveVertexLabel(vertex, label)
                }
                Self::AddEdgeLabel => {
                    let edge = extract_number(t);
                    let label = extract_number(t);
                    let labelname = extract_text(t);
                    OperationWithIds::AddEdgeLabel(edge, label, labelname)
                }
                Self::RemoveEdgeLabel => {
                    let edge = extract_number(t);
                    let label = extract_number(t);
                    OperationWithIds::RemoveEdgeLabel(edge, label)
                }
                Self::AddVertex => {
                    let vertex = extract_number(t);
                    OperationWithIds::AddVertex(vertex)
                }
                Self::RemoveVertex => {
                    let vertex = extract_number(t);
                    OperationWithIds::RemoveVertex(vertex)
                }
                Self::AddEdge => {
                    let edge = extract_number(t);
                    let from = extract_number(t);
                    let to = extract_number(t);
                    OperationWithIds::AddEdge(edge, from, to)
                }
                Self::RemoveEdge => {
                    let edge = extract_number(t);
                    OperationWithIds::RemoveEdge(edge)
                }
                Self::AddVertexProperty => {
                    let vertex = extract_number(t);
                    let name = extract_text(t);
                    let value = extract_text(t);
                    OperationWithIds::AddVertexProperty(vertex, name, value)
                }
                Self::RemoveVertexProperty => {
                    let vertex = extract_number(t);
                    let name = extract_text(t);
                    OperationWithIds::RemoveVertexProperty(vertex, name)
                }
                Self::AddEdgeProperty => {
                    let edge = extract_number(t);
                    let name = extract_text(t);
                    let value = extract_text(t);
                    OperationWithIds::AddEdgeProperty(edge, name, value)
                }
                Self::RemoveEdgeProperty => {
                    let edge = extract_number(t);
                    let name = extract_text(t);
                    OperationWithIds::RemoveEdgeProperty(edge, name)
                }
                Self::RenameVertex => {
                    let vertex = extract_number(t);
                    let name = extract_text(t);
                    OperationWithIds::RenameVertex(vertex, name)
                }
                Self::RenameEdge => {
                    let edge = extract_number(t);
                    let name = extract_text(t);
                    OperationWithIds::RenameEdge(edge, name)
                }
                Self::MoveEdgeTarget => {
                    let edge = extract_number(t);
                    let target = extract_number(t);
                    OperationWithIds::MoveEdgeTarget(edge, target)
                }
                Self::MoveEdgeSource => {
                    let edge = extract_number(t);
                    let source = extract_number(t);
                    OperationWithIds::MoveEdgeSource(edge, source)
                }
            }
        }
    }
}

#[deprecated]
pub fn generate_operations(
    program: Program,
    relation_name: &str,
    g: &PropertyGraph,
    target_graph: &Option<PropertyGraph>,
) -> HashMap<i32, Vec<OperationWithIds>> {
    encode_input_graph(program, g);
    if let Some(target) = target_graph {
        encode_target_graph(program, target);
    }
    unsafe {
        souffle_ffi::runProgram(program);
        let out_relation =
            get_relation(program, relation_name).expect("No relation for the transformations.");
        let mut iter = souffle_ffi::createTupleIterator(out_relation);
        let mut ids = vec![];
        while souffle_ffi::hasNext(&iter) {
            let id = extract_signed(souffle_ffi::getNext(&mut iter));
            ids.push(id);
        }
        let mut operations: HashMap<i32, Vec<OperationWithIds>> = HashMap::new();
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

pub type TransfoTrees = HashMap<Operation, HashMap<Operation, Vec<Operation>>>;

pub fn generate_operation_trees(
    program: Program,
    transformations: &HashSet<&str>,
    g: &PropertyGraph,
    target_graph: &Option<PropertyGraph>,
) -> Option<TransfoTrees> {
    encode_input_graph(program, g);
    if let Some(target) = target_graph {
        encode_target_graph(program, target);
    }
    unsafe {
        souffle_ffi::runProgram(program);
        let trees = generate_trees(program);
        souffle_ffi::purgeProgram(program);
        trees
    }
}

pub type TransfoTreesIds = HashMap<i32, HashMap<i32, Vec<i32>>>;

pub fn generate_operation_trees_ids(
    program: Program,
    transformations: &HashSet<&str>,
    g: &PropertyGraph,
    target_graph: &Option<PropertyGraph>,
) -> Option<(TransfoTreesIds, HashMap<i32, OperationWithIds>)> {
    encode_input_graph(program, g);
    if let Some(target) = target_graph {
        encode_target_graph(program, target);
    }
    unsafe {
        souffle_ffi::runProgram(program);
        if let Some((trees, ids)) = generate_trees_ids(program) {
            let id_map = extract_ids(program, transformations, ids);
            souffle_ffi::purgeProgram(program);
            return Some((trees, id_map));
        }
        souffle_ffi::purgeProgram(program);
    }
    None
}

unsafe fn extract_ids(
    program: Program,
    transformations: &HashSet<&str>,
    ids: HashSet<i32>,
) -> HashMap<i32, OperationWithIds> {
    let mut ops: HashMap<i32, OperationWithIds> = HashMap::new();
    for operation in OPERATIONS.iter() {
        if let Some(out_relation) = get_relation(program, operation.get_relation()) {
            let mut iter = souffle_ffi::createTupleIterator(out_relation);
            while souffle_ffi::hasNext(&iter) {
                let t = souffle_ffi::getNext(&mut iter);
                let name = extract_text(t);
                if transformations.contains(name.as_str()) {
                    let id = extract_signed(t);
                    if ids.contains(&id) {
                        let op = operation.construct(t);
                        ops.insert(id, op);
                    }
                }
            }
        }
    }
    ops
}
unsafe fn generate_trees(program: Program) -> Option<TransfoTrees> {
    let record = getRecordTable(&program);
    let symbol = getSymbolTable(&program);
    let next_relation = get_relation(program, "Next");
    if let Some(next_relation) = next_relation {
        let mut trees = HashMap::new();
        let mut iter = souffle_ffi::createTupleIterator(next_relation);
        while souffle_ffi::hasNext(&iter) {
            let t = souffle_ffi::getNext(&mut iter);
            let root = Operation::from_record_index(extract_signed(t), record, symbol)?;
            let prev = Operation::from_record_index(extract_signed(t), record, symbol)?;
            let next = Operation::from_record_index(extract_signed(t), record, symbol)?;
            trees
                .entry(root)
                .or_insert_with(HashMap::new)
                .entry(prev)
                .or_insert_with(Vec::new)
                .push(next);
        }
        Some(trees)
    } else {
        None
    }
}

unsafe fn generate_trees_ids(program: Program) -> Option<(TransfoTreesIds, HashSet<i32>)> {
    let next_relation = get_relation(program, "Next_");
    if let Some(next_relation) = next_relation {
        let mut trees = HashMap::new();
        let mut ids = HashSet::new();
        let mut iter = souffle_ffi::createTupleIterator(next_relation);
        while souffle_ffi::hasNext(&iter) {
            let t = souffle_ffi::getNext(&mut iter);
            let root = extract_signed(t);
            let prev = extract_signed(t);
            let next = extract_signed(t);
            println!("Next_({},{},{}).", root, prev, next);
            trees
                .entry(root)
                .or_insert_with(HashMap::new)
                .entry(prev)
                .or_insert_with(Vec::new)
                .push(next);
            ids.insert(root);
            ids.insert(prev);
            ids.insert(next);
        }
        Some((trees, ids))
    } else {
        None
    }
}
