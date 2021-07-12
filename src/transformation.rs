use crate::errors::TransProofError;
use graph::transfos;
use graph::{transfo_result::GraphTransformation, GraphNauty};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::ops::{Add, AddAssign};
use std::sync::Arc;

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
                            $alias => Some(Box::new(transfos::$func) as Box<dyn Transformation>),
                        )*
                    )*
                    _ => None,
                }
            }
        };
}

transformations! {
    {add_edge,
        desc: "Adds an edge.",
        commands: ["add_edge"]
    },
    {add_isolated_vertex,
        desc: "Adds a isolated_vertex.",
        commands: ["add_vertex", "add_isolated_vertex"]
    },
    {remove_edge,
        desc: "Removes an edge.",
        commands: ["remove_edge"]
    },
    {remove_vertex,
        desc: "Removes a vertex.",
        commands: ["remove_vertex"]
    },
    {rotation,
        desc: "Rotates an edge.",
        commands: ["rotation"]
    },
    {slide,
        desc: "Rotation(a,b,c) where b and c are adjacent.",
        commands: ["slide"]
    },
    {move_distinct,
        desc: "Moves an edge ab between to distinct vertices c and d.",
        commands: ["move_distinct", "move"]
    },
    {two_opt,
        desc: "Switch two edges.",
        commands: ["two_opt"]
    },
    {shortcut,
        desc: "Adds a short cut between two vertices a and c at distance 2.",
        commands: ["short_cut"]
    },
    {detour,
        desc: "Replace an edge by a path of length 2.",
        commands: ["detour"]
    },
    {disco_twins,
        desc: "Disconnect twin vertices.",
        commands: ["disco_twins"]
    },
    {isolate,
        desc: "Isolate a vertex.",
        commands: ["isolate"]
    },
    {isolate_twin,
        desc: "Isolate a vertex having a twin.",
        commands: ["isolate_twin"]
    },
    {isolate_incl,
        desc: "Isolate a vertex having its neighborhood included in the neighborhood of another vertex.",
        commands: ["isolate_incl"]
    },
    {isolate_incl_adj,
        desc: "Isolate a vertex having its neighborhood included in the neighborhood of an adjacent vertex.",
        commands: ["isolate_incl_adj"]
    },
    {remove_incl_adj,
        desc: "Removes a vertex having its neighborhood included in the neighborhood of an adjacent vertex.",
        commands: ["remove_incl_adj"]
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
    fn apply(&self, input: &GraphNauty) -> Vec<GraphTransformation>;
}

impl<F> Transformation for F
where
    F: Fn(&GraphNauty) -> Vec<GraphTransformation> + Send + Sync,
{
    fn apply(&self, input: &GraphNauty) -> Vec<GraphTransformation> {
        self(input)
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
    fn apply(&self, input: &GraphNauty) -> Vec<GraphTransformation> {
        self.iter()
            .flat_map(|x| x.apply(input).into_iter())
            .collect()
    }
}

//#[derive(Clone)]
//pub enum Transformation<'a> {
//Single(Arc<dyn Fn(&GraphNauty) -> Vec<GraphTransformation> + Send + Sync + 'a>),
//Multiple(Vec<Transformation<'a>>),
//}

//impl<'a> Transformation<'a> {
//pub fn from_name(s: &str) -> Option<Transformation> {
//let s = s.trim().to_lowercase();
//TRANSFO_NAMES
//.get(s.as_str())
//.or_else(|| {
//TRANSFO_ALIASES
//.get(s.as_str())
//.and_then(|x| TRANSFO_NAMES.get(x))
//})
//.map(|x| x.0.clone())
//}

//pub fn apply(&self, g: &GraphNauty) -> Vec<GraphTransformation> {
//match *self {
//Transformation::Multiple(ref l) => {
//let mut res = Vec::new();
//for t in l {
//res.extend(t.apply(g));
//}
//res
//}
//Transformation::Single(ref f) => f(g),
//}
//}
//}

//impl<'a, F> From<F> for Transformation<'a>
//where
//F: Fn(&GraphNauty) -> Vec<GraphTransformation> + Send + Sync + 'a,
//{
//fn from(f: F) -> Self {
//Transformation::Single(Arc::new(f))
//}
//}

//impl<'a, T> Add<T> for Transformation<'a>
//where
//T: Into<Transformation<'a>> + 'a,
//{
//type Output = Transformation<'a>;

//fn add(self, other: T) -> Transformation<'a> {
//let mut trs = Vec::new();
//for obj in vec![self, other.into()] {
//match obj {
//Transformation::Single(_) => trs.push(obj),
//Transformation::Multiple(ref l) => trs.extend_from_slice(l),
//}
//}
//Transformation::Multiple(trs)
//}
//}

//fn add_other<'a>(ls: &mut Vec<Transformation<'a>>, other: Transformation<'a>) {
//match other {
//Transformation::Single(_) => ls.push(other),
//Transformation::Multiple(ref lo) => ls.extend_from_slice(lo),
//}
//}

//impl<'a, T> AddAssign<T> for Transformation<'a>
//where
//T: Into<Transformation<'a>> + 'a,
//{
//fn add_assign(&mut self, other: T) {
//let other = other.into();
//if let Transformation::Multiple(ref mut ls) = *self {
//add_other(ls, other);
//} else {
//let mut ls = vec![(*self).clone()];
//add_other(&mut ls, other);
//*self = Transformation::Multiple(ls);
//}
//}
//}
