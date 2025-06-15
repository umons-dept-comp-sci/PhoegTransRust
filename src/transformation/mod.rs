use crate::property_graph::PropertyGraph;
use crate::transformation::souffle::extract_number;
use crate::{graph_transformation::GraphTransformation, transformation::souffle::OutputTuple};
use lazy_static::lazy_static;
use log::error;
use petgraph::stable_graph::{EdgeIndex, NodeIndex};
use petgraph::visit::NodeIndexable;
use souffle::{generate_operation_trees};
use std::collections::{HashMap, HashSet, VecDeque};
use std::convert::TryFrom;
use std::fmt::format;
use std::hash::Hash;
use std::net::ToSocketAddrs;

use self::souffle::Program;

pub mod souffle;

#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub enum OperationName {
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

impl From<Operation> for OperationName {
    fn from(value: Operation) -> Self {
        match value {
            Operation::AddVertexLabel(_, _) => Self::AddVertexLabel,
            Operation::RemoveVertexLabel(_, _) => Self::RemoveVertexLabel,
            Operation::AddEdgeLabel(_, _) => Self::AddEdgeLabel,
            Operation::RemoveEdgeLabel(_, _) => Self::RemoveEdgeLabel,
            Operation::AddVertex(_) => Self::AddVertex,
            Operation::RemoveVertex(_) => Self::RemoveVertex,
            Operation::AddEdge(_, _, _) => Self::AddEdge,
            Operation::RemoveEdge(_) => Self::RemoveEdge,
            Operation::AddVertexProperty(_, _, _) => Self::AddVertexProperty,
            Operation::RemoveVertexProperty(_, _) => Self::RemoveVertexProperty,
            Operation::AddEdgeProperty(_, _, _) => Self::AddEdgeProperty,
            Operation::RemoveEdgeProperty(_, _) => Self::RemoveEdgeProperty,
            Operation::RenameVertex(_, _) => Self::RenameVertex,
            Operation::RenameEdge(_, _) => Self::RenameEdge,
            Operation::MoveEdgeTarget(_, _) => Self::MoveEdgeTarget,
            Operation::MoveEdgeSource(_, _) => Self::MoveEdgeSource,
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
            // dbg!(&current);
            // dbg!(depth);
            for op in self.current_path.drain(depth..) {
                self.seen.remove(&op);
            }
            if self.seen.contains(&current) {
                return Some(g);
            }
            let current_tree = self.current_tree.as_ref().unwrap();
            if !current_tree.contains_key(&current) {
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
                        self.list.push_back((branches[0].clone(), g, depth + 1));
                    } else {
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
    if let Some(trees) = generate_operation_trees(program, &transfos, g, target_graph) {
        Some(TransformGenerator::new(trees, g))
    } else {
        None
    }
}

