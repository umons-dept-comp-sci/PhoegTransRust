use crate::transformation::souffle::extract_number;
use crate::{graph_transformation::GraphTransformation, transformation::souffle::OutputTuple};
use crate::property_graph::PropertyGraph;
use crate::errors::TransProofError;
use lazy_static::lazy_static;
use petgraph::visit::NodeIndexable;
use std::collections::HashMap;
use std::convert::TryFrom;

use self::souffle::{apply_transformation, SouffleProgram};

pub mod souffle;

macro_rules! transformations {
    ($( {$func:ident, desc : $desc:expr, commands: [$( $alias:expr ),+]} ),+)
        => {
            lazy_static!
            {
                pub static ref TRANSFO_NAMES: HashMap<&'static str, usize> = {
                    let mut m = HashMap::new();
                    let mut id = 0;
                    $(
                        $(
                            m.insert($alias, id);
                        )*
                        id += 1;
                    )*
                        m
                };
                pub static ref TRANSFO_DESCS: Vec<&'static str> = {
                    let mut v = Vec::new();
                    $(
                        v.push($desc);
                    )*
                        v
                };
            }
            pub fn get_transformation_from_name(name: &str) -> Option<Box<dyn Transformation>> {
                match name {
                    $(
                        $(
                            $alias => Some(Box::new($func) as Box<dyn Transformation>),
                        )*
                    )*
                    _ => None,
                }
            }
        };
}

pub fn relabel_vertex(g : &PropertyGraph) -> Vec<GraphTransformation> {
    let mut res = Vec::new();
    let labels : Vec<_> = g.vertex_label.labels().collect();
    for old_label in labels.iter() {
        for new_label in labels.iter() {
            for vertex in g.vertex_label.label_elements(**old_label) {
                if !g.vertex_label.has_label(vertex, **new_label) {
                    let mut trs : GraphTransformation = g.into();
                    trs.result.vertex_label.remove_label_mapping(vertex, **old_label).unwrap();
                    trs.result.vertex_label.add_label_mapping(vertex, **new_label).unwrap();
                    res.push(trs);
                }
            } 
        }
    }
    res
}

pub fn relabel_vertex_souffle(program: SouffleProgram, g : &PropertyGraph) -> Vec<GraphTransformation> {
    fn extract_data(tuple : OutputTuple) -> (u32, u32, u32) {
        (extract_number(tuple), extract_number(tuple), extract_number(tuple))
    }
    fn relabel(g : &PropertyGraph, operation : (u32, u32, u32)) -> GraphTransformation {
        let mut res: GraphTransformation = g.into();
        res.result.vertex_label.remove_label_mapping(&(operation.0.into()), operation.1).unwrap();
        res.result.vertex_label.add_label_mapping(&(operation.0.into()), operation.2).unwrap();
        res
    }
    apply_transformation(program, "RelabelVertex", extract_data, relabel, g)
}

pub fn remove_edge(program: SouffleProgram, g : &PropertyGraph) -> Vec<GraphTransformation> {
    fn extract_data(tuple : OutputTuple) -> u32 {
        extract_number(tuple)
    }
    fn remove(g : &PropertyGraph, operation : u32) -> GraphTransformation {
        let mut res: GraphTransformation = g.into();
        let index = operation.into();
        res.result.graph.remove_edge(index);
        res.result.edge_label.remove_element(&index);
        res
    }
    apply_transformation(program, "RemoveEdge", extract_data, remove, g)
}

transformations! {
    {
        relabel_vertex_souffle,
        desc: "Tries every relabelling of a vertex using each existing label.",
        commands: ["relabel_vertex"]
    },
    {
        remove_edge,
        desc: "Tries every edge removal.",
        commands: ["remove_edge"]
    }
}

pub fn print_transfos() {
    let mut names = vec![Vec::new(); TRANSFO_DESCS.len()];
    for (name, &id) in TRANSFO_NAMES.iter() {
        names[id].push(name);
    }
    for f_id in 0..names.len() {
        // We know there is at least one by definition of the macro.
        print!("{}", names[f_id][0]);
        for alias in names[f_id].iter().skip(1) {
            print!(", {}", alias);
        }
        println!(" :\n    {}", TRANSFO_DESCS[f_id]);
    }
}

pub trait Transformation: Send + Sync {
    fn apply(&self, program: SouffleProgram, input: &PropertyGraph) -> Vec<GraphTransformation>;
}

impl<F> Transformation for F
where
    F: Fn(SouffleProgram, &PropertyGraph) -> Vec<GraphTransformation> + Send + Sync,
{
    fn apply(&self, program: SouffleProgram, input: &PropertyGraph) -> Vec<GraphTransformation> {
        self(program, input)
    }
}

impl TryFrom<&str> for Box<dyn Transformation> {
    type Error = TransProofError;

    fn try_from(input: &str) -> Result<Self, Self::Error> {
        let s = input.trim().to_lowercase();
        get_transformation_from_name(s.as_str()).ok_or(TransProofError::UnknownTransformation(s.to_string()))
    }
}

pub type TransfoVec = Vec<Box<dyn Transformation>>;

impl Transformation for TransfoVec {
    fn apply(&self, program: SouffleProgram, input: &PropertyGraph) -> Vec<GraphTransformation> {
        self.iter()
            .flat_map(|x| x.apply(program, input).into_iter())
            .collect()
    }
}