use self::souffle::extract_text;
use crate::errors::TransProofError;
use crate::property_graph::{LabelMap, Properties, PropertyGraph};
use crate::transformation::souffle::extract_number;
use crate::{graph_transformation::GraphTransformation, transformation::souffle::OutputTuple};
use lazy_static::lazy_static;
use log::error;
use petgraph::stable_graph::{EdgeIndex, NodeIndex};
use petgraph::visit::NodeIndexable;
use souffle::{generate_operation_trees, generate_operation_trees_ids};
use std::collections::{HashMap, HashSet, VecDeque};
use std::convert::TryFrom;
use std::fmt::format;
use std::hash::Hash;
use std::net::ToSocketAddrs;

use self::souffle::Program;

pub mod souffle;

#[derive(Clone, Copy)]
enum OperationName {
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

impl OperationName {
    fn symbol<'a>(&'a self) -> &'a str {
        match self {
            OperationName::AddEdge => "AddEdge",
            OperationName::AddVertexLabel => "AddVertexLabel",
            OperationName::RemoveVertexLabel => "RemoveVertexLabel",
            OperationName::AddEdgeLabel => "AddEdgeLabel",
            OperationName::RemoveEdgeLabel => "RemoveEdgeLabel",
            OperationName::AddVertex => "AddVertex",
            OperationName::RemoveVertex => "RemoveVertex",
            OperationName::RemoveEdge => "RemoveEdge",
            OperationName::AddVertexProperty => "AddVertexProperty",
            OperationName::RemoveVertexProperty => "RemoveVertexProperty",
            OperationName::AddEdgeProperty => "AddEdgeProperty",
            OperationName::RemoveEdgeProperty => "RemoveEdgeProperty",
            OperationName::RenameVertex => "RenameVertex",
            OperationName::RenameEdge => "RenameEdge",
            OperationName::MoveEdgeTarget => "MoveEdgeTarget",
            OperationName::MoveEdgeSource => "MoveEdgeSource",
        }
    }

    fn arity(&self) -> u32 {
        match self {
            OperationName::AddVertexLabel => 2,
            OperationName::RemoveVertexLabel => 2,
            OperationName::AddEdgeLabel => 2,
            OperationName::RemoveEdgeLabel => 2,
            OperationName::AddVertex => 1,
            OperationName::RemoveVertex => 1,
            OperationName::AddEdge => 3,
            OperationName::RemoveEdge => 1,
            OperationName::AddVertexProperty => 3,
            OperationName::RemoveVertexProperty => 2,
            OperationName::AddEdgeProperty => 3,
            OperationName::RemoveEdgeProperty => 2,
            OperationName::RenameVertex => 2,
            OperationName::RenameEdge => 2,
            OperationName::MoveEdgeTarget => 2,
            OperationName::MoveEdgeSource => 2,
        }
    }
}

static OPERATIONS: [OperationName; 16] = [
    OperationName::AddVertex,
    OperationName::AddVertexLabel,
    OperationName::AddVertexProperty,
    OperationName::AddEdge,
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

lazy_static! {
    static ref OPERATION_ORDER: Vec<OperationName> = {
        let mut names = vec![
            OperationName::AddVertex,
            OperationName::AddVertexLabel,
            OperationName::AddVertexProperty,
            OperationName::AddEdge,
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
        names.sort_by(|name1, name2| name1.symbol().cmp(name2.symbol()));
        names
    };
}

fn name_from_order(v: i32) -> Option<OperationName> {
    if 0 <= v && v < OPERATION_ORDER.len() as i32 {
        Some(OPERATION_ORDER[v as usize])
    } else {
        None
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Operation {
    AddVertexLabel(String, String),
    RemoveVertexLabel(String, String),
    AddEdgeLabel(String, String),
    RemoveEdgeLabel(String, String),
    AddVertex(String),
    RemoveVertex(String),
    AddEdge(String, String, String),
    RemoveEdge(String),
    AddVertexProperty(String, String, String),
    RemoveVertexProperty(String, String),
    AddEdgeProperty(String, String, String),
    RemoveEdgeProperty(String, String),
    RenameVertex(String, String),
    RenameEdge(String, String),
    MoveEdgeTarget(String, String),
    MoveEdgeSource(String, String),
}

#[derive(Debug)]
pub enum OperationWithIds {
    AddVertexLabel(u32, u32, String),
    RemoveVertexLabel(u32, u32),
    AddEdgeLabel(u32, u32, String),
    RemoveEdgeLabel(u32, u32),
    AddVertex(u32),
    RemoveVertex(u32),
    AddEdge(u32, u32, u32),
    RemoveEdge(u32),
    AddVertexProperty(u32, String, String),
    RemoveVertexProperty(u32, String),
    AddEdgeProperty(u32, String, String),
    RemoveEdgeProperty(u32, String),
    RenameVertex(u32, String),
    RenameEdge(u32, String),
    MoveEdgeTarget(u32, u32),
    MoveEdgeSource(u32, u32),
}

fn get_node_index(id: &u32, node_map: &HashMap<u32, NodeIndex<u32>>) -> NodeIndex<u32> {
    *node_map.get(&id).unwrap_or(&(*id).into())
}

fn get_edge_index(id: &u32, edge_map: &HashMap<u32, EdgeIndex<u32>>) -> EdgeIndex<u32> {
    *edge_map.get(&id).unwrap_or(&(*id).into())
}

fn get_node_label_index(id: &u32, node_label_map: &HashMap<u32, u32>) -> u32 {
    *node_label_map.get(id).unwrap_or(id)
}

fn get_edge_label_index(id: &u32, edge_label_map: &HashMap<u32, u32>) -> u32 {
    *edge_label_map.get(id).unwrap_or(id)
}

impl OperationWithIds {
    fn apply(
        &self,
        g: &mut GraphTransformation,
        node_map: &mut HashMap<u32, NodeIndex<u32>>,
        edge_map: &mut HashMap<u32, EdgeIndex<u32>>,
        node_label_map: &mut HashMap<u32, u32>,
        edge_label_map: &mut HashMap<u32, u32>,
    ) {
        match self {
            Self::AddVertexLabel(v, l, name) => {
                let index = get_node_index(v, node_map);
                let lid = if let Some(id) = g.result.vertex_label.get_id(name) {
                    *id
                } else {
                    let id = g.result.vertex_label.add_label(name.clone());
                    node_label_map.insert(*l, id);
                    id
                };
                node_label_map.insert(*l, lid);
                g.result
                    .vertex_label
                    .add_label_mapping(&index, lid)
                    .unwrap();
                let name = g.result.graph.node_weight(index).unwrap().name.clone();
                let label = g.result.vertex_label.get_label(lid).unwrap().clone();
                g.operations
                    .push(format!("AddVertexLabel({},{})", name, label));
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
                g.operations
                    .push(format!("RemoveVertexLabel({},{})", name, label));
            }
            Self::AddEdgeLabel(e, l, name) => {
                let index = get_edge_index(e, edge_map);
                let lid = if let Some(id) = g.result.edge_label.get_id(name) {
                    *id
                } else {
                    let id = g.result.edge_label.add_label(name.clone());
                    edge_label_map.insert(*l, id);
                    id
                };
                edge_label_map.insert(*l, lid);
                g.result.edge_label.add_label_mapping(&index, lid).unwrap();
                let name = g.result.graph.edge_weight(index).unwrap().name.clone();
                let label = g.result.edge_label.get_label(lid).unwrap().clone();
                g.operations
                    .push(format!("AddEdgeLabel({},{})", name, label));
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
                g.operations
                    .push(format!("RemoveEdgeLabel({},{})", name, label));
            }
            Self::AddVertex(v) => {
                let index = get_node_index(v, node_map);
                if g.result.graph.contains_node(index) {
                    // error!("Node {v} already exists.");
                    // panic!("Node {v} already exists.");
                } else {
                    //TODO Need a name when creating a node.
                    let real_index = g.result.graph.add_node(Properties {
                        name: "".to_string(),
                        map: HashMap::new(),
                    });
                    node_map.insert(*v, real_index);
                }
            }
            Self::RemoveVertex(v) => {
                let index = get_node_index(v, node_map);
                let name = g.result.graph.node_weight(index).unwrap().name.clone();
                g.result.vertex_label.remove_element(&index);
                g.result.graph.remove_node(index);
                node_map.remove(v);
                g.operations.push(format!("RemoveVertex({})", name));
            }
            Self::AddEdge(e, start, end) => {
                let index = get_edge_index(e, edge_map);
                if g.result.graph.edge_weight(index).is_some() {
                    // error!("Edge {e} already exists.");
                    // panic!("Edge {e} already exists.");
                } else {
                    //TODO Need a name when creating an edge.
                    let n1 = get_node_index(start, node_map);
                    let n2 = get_node_index(end, node_map);
                    let name1 = g.result.graph.node_weight(n1).unwrap().name.clone();
                    let name2 = g.result.graph.node_weight(n2).unwrap().name.clone();
                    let real_index = g.result.graph.add_edge(
                        n1,
                        n2,
                        Properties {
                            name: "".to_string(),
                            map: HashMap::new(),
                        },
                    );
                    edge_map.insert(*e, real_index);
                    g.operations.push(format!("AddEdge({},{})", name1, name2));
                }
            }
            Self::RemoveEdge(e) => {
                let index = get_edge_index(e, edge_map);
                let name = g.result.graph.edge_weight(index).unwrap().name.clone();
                g.result.edge_label.remove_element(&index);
                g.result.graph.remove_edge(index);
                edge_map.remove(e);
                g.operations.push(format!("RemoveEdge({})", name));
            }
            Self::AddVertexProperty(v, name, value) => {
                let prop = g
                    .result
                    .graph
                    .node_weight_mut(get_node_index(v, node_map))
                    .expect(&format!("Unknown vertex {v}"));
                prop.map.insert(name.to_string(), value.to_string());
                g.operations.push(format!(
                    "AddVertexProperty({},{},{})",
                    prop.name, name, value
                ));
            }
            Self::RemoveVertexProperty(v, name) => {
                let prop = g
                    .result
                    .graph
                    .node_weight_mut(get_node_index(v, node_map))
                    .expect(&format!("Unknown vertex {v}"));
                prop.map.remove(name);
                g.operations
                    .push(format!("RemoveVertexProperty({},{})", prop.name, name));
            }
            Self::AddEdgeProperty(e, name, value) => {
                let prop = g
                    .result
                    .graph
                    .edge_weight_mut(get_edge_index(e, edge_map))
                    .expect(&format!("Unknown edge {e}"));
                prop.map.insert(name.to_string(), value.to_string());
                g.operations
                    .push(format!("AddEdgeProperty({},{},{})", prop.name, name, value));
            }
            Self::RemoveEdgeProperty(e, name) => {
                let prop = g
                    .result
                    .graph
                    .edge_weight_mut(get_edge_index(e, edge_map))
                    .expect(&format!("Unknown edge {e}"));
                prop.map.remove(name);
                g.operations
                    .push(format!("RemoveEdgeProperty({},{})", prop.name, name));
            }
            Self::RenameVertex(v, name) => {
                let prop = g
                    .result
                    .graph
                    .node_weight_mut(get_node_index(v, node_map))
                    .expect(&format!("Unknown node {v}"));
                g.operations
                    .push(format!("RenameVertex({},{})", prop.name, name));
                prop.name = name.to_string();
            }
            Self::RenameEdge(e, name) => {
                let prop = g
                    .result
                    .graph
                    .edge_weight_mut(get_edge_index(e, edge_map))
                    .expect(&format!("Unknown edge {e}"));
                g.operations
                    .push(format!("RenameEdge({},{})", prop.name, name));
                prop.name = name.to_string();
            }
            Self::MoveEdgeTarget(e, t) => {
                let edgeindex = get_edge_index(e, edge_map);
                let src = g.result.graph.edge_endpoints(edgeindex).unwrap().0;
                let target = get_node_index(t, node_map);
                let w = g.result.graph.remove_edge(edgeindex).unwrap();
                let edgename = w.name.clone();
                let real_index = g.result.graph.add_edge(src, target, w);
                let labels: Vec<u32> = g
                    .result
                    .edge_label
                    .element_labels(&edgeindex)
                    .copied()
                    .collect();
                labels.into_iter().for_each(|l| {
                    g.result
                        .edge_label
                        .add_label_mapping(&real_index, l)
                        .unwrap()
                });
                g.result.edge_label.remove_element(&edgeindex);
                edge_map.insert(*e, real_index);
                g.operations.push(format!(
                    "MoveEdgeTarget({},{})",
                    edgename.clone(),
                    g.result.graph.node_weight(target).unwrap().name.clone()
                ));
            }
            Self::MoveEdgeSource(e, s) => {
                let edgeindex = get_edge_index(e, edge_map);
                let target = g.result.graph.edge_endpoints(edgeindex).unwrap().1;
                let src = get_node_index(s, node_map);
                let w = g.result.graph.remove_edge(edgeindex).unwrap();
                let edgename = w.name.clone();
                let real_index = g.result.graph.add_edge(src, target, w);
                let labels: Vec<u32> = g
                    .result
                    .edge_label
                    .element_labels(&edgeindex)
                    .copied()
                    .collect();
                labels.into_iter().for_each(|l| {
                    g.result
                        .edge_label
                        .add_label_mapping(&real_index, l)
                        .unwrap()
                });
                g.result.edge_label.remove_element(&edgeindex);
                edge_map.insert(*e, real_index);
                g.operations.push(format!(
                    "MoveEdgeSource({},{})",
                    edgename.clone(),
                    g.result.graph.node_weight(src).unwrap().name.clone()
                ));
            }
        }
    }
}

pub fn apply_single_transformation(
    program: Program,
    rel_name: &str,
    g: &PropertyGraph,
    target_graph: &Option<PropertyGraph>,
) -> Vec<GraphTransformation> {
    let mut res = vec![];
    let operations = souffle::generate_operations(program, rel_name, g, target_graph);
    for transfo in operations.values() {
        let mut ng: GraphTransformation = g.into();
        for operation in transfo {
            ng.apply_ids(operation);
        }
        if ng.result.check_unique_names() {
            res.push(ng);
        }
    }
    res
}

pub fn apply_transformations(
    program: Program,
    rel_names: &Vec<&str>,
    g: &PropertyGraph,
    target_graph: &Option<PropertyGraph>,
) -> Vec<GraphTransformation> {
    rel_names
        .iter()
        .flat_map(|name| apply_single_transformation(program, name, g, target_graph))
        .collect()
}

macro_rules! indentprintln {
    ($txt:literal,$depth:expr $(,$params:expr)*) => {
        log::debug!(concat!("{}",$txt),"  ".repeat($depth),$($params),*);
    };
}

fn print_and_test(depth: usize, op: &Operation, g: &mut GraphTransformation) -> bool {
    let r = g.apply(op);
    let text = if r.is_some() { "" } else { " ERROR" };
    indentprintln!("{:?}{}", depth, op, text);
    r.is_some()
}

fn transform_graph_from_tree(
    tree: &HashMap<Operation, Vec<Operation>>,
    current: &Operation,
    mut g: GraphTransformation,
    result: &mut Vec<GraphTransformation>,
    seen: &mut HashSet<Operation>,
    depth: usize,
) {
    if seen.contains(current) {
        indentprintln!("{:?} CYCLE", depth, current);
        result.push(g);
    } else if !tree.contains_key(current) {
        // seen.insert(*current);
        if print_and_test(depth, current, &mut g) {
            result.push(g);
        }
        indentprintln!("END OF BRANCH", depth);
    } else {
        seen.insert(current.clone());
        // println!("{}: added {:?}"," ".repeat(depth),current);
        let branches = tree.get(current).unwrap();
        if branches.is_empty() {
            if print_and_test(depth, current, &mut g) {
                result.push(g);
            }
        } else if branches.len() == 1 {
            if print_and_test(depth, current, &mut g) {
                transform_graph_from_tree(tree, &branches[0], g, result, seen, depth + 1);
            }
        } else if print_and_test(depth, current, &mut g) {
            for id in branches {
                let ng = g.clone();
                transform_graph_from_tree(tree, id, ng, result, seen, depth + 1);
            }
        }
        // println!("{}: removed {:?}"," ".repeat(depth),current);
        seen.remove(current);
    }
}

fn transform_graph_from_tree_ids(
    tree: &HashMap<i32, Vec<i32>>,
    current: &i32,
    mut g: GraphTransformation,
    result: &mut Vec<GraphTransformation>,
    seen: &mut HashSet<i32>,
    id_map: &HashMap<i32, OperationWithIds>,
    depth: usize,
) {
    if id_map.contains_key(current) {
        if seen.contains(current) {
            let op = id_map.get(current).unwrap();
            println!("CYCLE {}: {}{:?}", current, " ".repeat(depth), op);
            result.push(g);
        } else if !tree.contains_key(current) {
            // seen.insert(*current);
            let op = id_map.get(current).unwrap();
            println!("{}: {}{:?}", current, " ".repeat(depth), op);
            g.apply_ids(op);
            result.push(g);
        } else {
            seen.insert(*current);
            println!("{}: added {}", " ".repeat(depth), current);
            let branches = tree.get(current).unwrap();
            if branches.is_empty() {
                let op = id_map.get(current).unwrap();
                println!("{}: {}{:?}", current, " ".repeat(depth), op);
                g.apply_ids(op);
                result.push(g);
            } else if branches.len() == 1 {
                let op = id_map.get(current).unwrap();
                println!("{}: {}{:?}", current, " ".repeat(depth), op);
                g.apply_ids(op);
                transform_graph_from_tree_ids(
                    tree,
                    &branches[0],
                    g,
                    result,
                    seen,
                    id_map,
                    depth + 1,
                );
            } else {
                let op = id_map.get(current).unwrap();
                println!("{}: {}{:?}", current, " ".repeat(depth), op);
                g.apply_ids(op);
                for id in branches {
                    let ng = g.clone();
                    transform_graph_from_tree_ids(tree, id, ng, result, seen, id_map, depth + 1);
                }
            }
            println!("{}: removed {}", " ".repeat(depth), current);
            seen.remove(current);
        }
    }
}

pub struct TransformGenerator {
    trees: HashMap<Operation, HashMap<Operation, Vec<Operation>>>,
    roots: VecDeque<Operation>,
    current_tree: Option<HashMap<Operation, Vec<Operation>>>,
    list: VecDeque<(Operation, GraphTransformation, usize)>,
    g: GraphTransformation,
    seen: HashSet<Operation>,
    current_path: Vec<Operation>,
}

impl TransformGenerator {
    pub fn new(
        trees: HashMap<Operation, HashMap<Operation, Vec<Operation>>>,
        g: &PropertyGraph,
    ) -> Self {
        let roots = trees.keys().cloned().collect();
        TransformGenerator {
            trees,
            roots,
            current_tree: None,
            list: VecDeque::new(),
            g: g.into(),
            seen: HashSet::new(),
            current_path: Vec::new(),
        }
    }

    fn start_list(&mut self) -> bool {
        if self.list.is_empty() {
            if self.roots.is_empty() {
                false
            } else {
                let root = self.roots.pop_front().unwrap();
                self.current_tree = Some(self.trees.get(&root).unwrap().clone());
                self.list.push_back((root, self.g.clone(), 0));
                true
            }
        } else {
            true
        }
    }
}

impl Iterator for TransformGenerator {
    type Item = GraphTransformation;

    fn next(&mut self) -> Option<Self::Item> {
        while self.start_list() {
            let (current, mut g, depth) = self.list.pop_back().unwrap();
            for op in self.current_path.drain(depth..) {
                self.seen.remove(&op);
            }
            if self.seen.contains(&current) {
                return Some(self.g.clone());
            }
            let current_tree = self.current_tree.as_ref().unwrap();
            if current_tree.contains_key(&current) {
                if g.apply(&current).is_some() {
                    return Some(g);
                }
            } else {
                let branches = current_tree.get(&current).unwrap();
                if branches.is_empty() {
                    return Some(g);
                } else if g.apply(&current).is_some() {
                    self.seen.insert(current.clone());
                    self.current_path.push(current.clone());
                    if branches.len() == 1 {
                        g.apply(&current).unwrap();
                        self.list.push_back((branches[0].clone(), g, depth + 1));
                    } else {
                        g.apply(&current).unwrap();
                        for id in branches {
                            let ng = g.clone();
                            self.list.push_back((id.clone(), ng, depth + 1));
                        }
                    }
                }
            }

        }
        None
    }
}

pub fn transform_graph(
    program: Program,
    transformations: &Vec<&str>,
    g: &PropertyGraph,
    target_graph: &Option<PropertyGraph>,
) -> Option<TransformGenerator> {
    let transfos: HashSet<&str> = transformations.iter().copied().collect();
    // let mut res = Vec::new();
    // let mut seen = HashSet::new();
    if let Some(trees) = generate_operation_trees(program, &transfos, g, target_graph) {
        Some(TransformGenerator::new(trees, g))
        // for root in trees.keys() {
        //     let transfo = GraphTransformation::from(g);
        //     transform_graph_from_tree(
        //         trees.get(root).unwrap(),
        //         root,
        //         transfo,
        //         &mut res,
        //         &mut seen,
        //         0,
        //     )
        // }
    } else {
        None
    }
    // res
}

pub fn transform_graph_ids(
    program: Program,
    transformations: &Vec<&str>,
    g: &PropertyGraph,
    target_graph: &Option<PropertyGraph>,
) -> Vec<GraphTransformation> {
    let transfos: HashSet<&str> = transformations.iter().copied().collect();
    let mut res = Vec::new();
    let mut seen = HashSet::new();
    if let Some((trees, id_map)) = generate_operation_trees_ids(program, &transfos, g, target_graph)
    {
        for root in trees.keys() {
            let transfo = GraphTransformation::from(g);
            transform_graph_from_tree_ids(
                trees.get(root).unwrap(),
                root,
                transfo,
                &mut res,
                &mut seen,
                &id_map,
                0,
            )
        }
    }
    res
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
