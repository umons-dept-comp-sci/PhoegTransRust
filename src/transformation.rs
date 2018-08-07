use graph::Graph;
use graph::transfos;
use std::ops::{Add, AddAssign};
use std::sync::Arc;
use std::collections::HashMap;

macro_rules! addtransfo {
    ( $m:ident, $n:expr, $f:ident, $d:expr ) => {
        $m.insert($n,(Transformation::Single(Arc::new(transfos::$f)), $d))
    }
}

lazy_static! {
    pub static ref NAMES: HashMap<&'static str, (Transformation<'static>, &'static str)> = {
        let mut m = HashMap::new();
        addtransfo!(m, "add_edge",add_edge,"Adds an edge");
        addtransfo!(m, "remove_edge",remove_edge,"Removes an edge");
        addtransfo!(m, "rotation",rotation,"Rotates an edge");
        addtransfo!(m, "slide",slide,"Rotation(a,b,c) where b and c are adjacent.");
        addtransfo!(m, "move_distinct",move_distinct,"Moves an edge ab between to distict vertices c and d.");
        addtransfo!(m, "two_opt",two_opt,"Switch two edges.");
        addtransfo!(m, "short_cut",shortcut,"Adds a sort cut between two vertices a and c at distance 2.");
        addtransfo!(m, "detour",detour,"Replace an edge by a path of lenght 2.");
        m
    };
}

#[derive(Clone)]
pub enum Transformation<'a> {
    Single(Arc<Fn(&Graph) -> Vec<Graph> + Send + Sync + 'a>),
    Multiple(Vec<Transformation<'a>>),
}

impl<'a> Transformation<'a> {
    pub fn from_name(s: &str) -> Option<Transformation> {
        NAMES.get(s.trim().to_lowercase().as_str()).map(|x| x.0.clone())
    }

    pub fn apply(&self, g: &Graph) -> Vec<Graph> {
        match *self {
            Transformation::Multiple(ref l) => {
                let mut res = Vec::new();
                for t in l {
                    res.extend(t.apply(g));
                }
                res
            }
            Transformation::Single(ref f) => f(g),
        }
    }
}

impl<'a, F> From<F> for Transformation<'a>
    where F: Fn(&Graph) -> Vec<Graph> + Send + Sync + 'a
{
    fn from(f: F) -> Self {
        Transformation::Single(Arc::new(f))
    }
}

impl<'a, T> Add<T> for Transformation<'a>
    where T: Into<Transformation<'a>> + 'a
{
    type Output = Transformation<'a>;

    fn add(self, other: T) -> Transformation<'a> {
        let mut trs = Vec::new();
        for obj in vec![self, other.into()] {
            match obj {
                Transformation::Single(_) => trs.push(obj),
                Transformation::Multiple(ref l) => trs.extend_from_slice(l),
            }
        }
        Transformation::Multiple(trs)
    }
}

fn add_other<'a>(ls: &mut Vec<Transformation<'a>>, other: Transformation<'a>) {
    match other {
        Transformation::Single(_) => ls.push(other),
        Transformation::Multiple(ref lo) => ls.extend_from_slice(lo),
    }
}

impl<'a, T> AddAssign<T> for Transformation<'a>
    where T: Into<Transformation<'a>> + 'a
{
    fn add_assign(&mut self, other: T) {
        let other = other.into();
        if let Transformation::Multiple(ref mut ls) = *self {
            add_other(ls, other);
        } else {
            let mut ls = vec![(*self).clone()];
            add_other(&mut ls, other);
            *self = Transformation::Multiple(ls);
        }
    }
}
