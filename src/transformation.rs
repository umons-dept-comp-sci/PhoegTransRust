use graph::transfos;
use graph::{transfo_result::GraphTransformation, GraphNauty};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::ops::{Add, AddAssign};
use std::sync::Arc;

macro_rules! addtransfo {
    ( $m:ident, $n:expr, $f:ident, $d:expr ) => {
        $m.insert($n, (Transformation::Single(Arc::new(transfos::$f)), $d))
    };
}

macro_rules! addalias {
    ( $m:ident, $n:expr, $a:expr ) => {
        assert!(TRANSFO_NAMES.get($a).is_some());
        $m.insert($n, $a);
    };
}

lazy_static! {
    pub static ref TRANSFO_NAMES: HashMap<&'static str, (Transformation<'static>, &'static str)> = {
        let mut m = HashMap::with_capacity(8);
        addtransfo!(m, "add_edge", add_edge, "Adds an edge");
        addtransfo!(m, "remove_edge", remove_edge, "Removes an edge");
        addtransfo!(m, "remove_vertex", remove_vertex, "Removes a vertex");
        addtransfo!(m, "rotation", rotation, "Rotates an edge");
        addtransfo!(
            m,
            "slide",
            slide,
            "Rotation(a,b,c) where b and c are adjacent."
        );
        addtransfo!(
            m,
            "move_distinct",
            move_distinct,
            "Moves an edge ab between to distict vertices c and d."
        );
        addtransfo!(m, "two_opt", two_opt, "Switch two edges.");
        addtransfo!(
            m,
            "short_cut",
            shortcut,
            "Adds a sort cut between two vertices a and c at distance 2."
        );
        addtransfo!(
            m,
            "detour",
            detour,
            "Replace an edge by a path of length 2."
        );
        addtransfo!(m, "disco_twins", disco_twins, "Disconnect twin vertices");
        addtransfo!(
            m,
            "isolate_twin",
            isolate_twin,
            "Isolate a vertex having a twin"
        );
        addtransfo!(m, "isolate_incl",isolate_incl,"Isolate a vertex having its neighborhood included in the neighborhoord of another vertex");
        m
    };
    pub static ref TRANSFO_ALIASES: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::with_capacity(1);
        addalias!(m, "move", "move_distinct");
        m
    };
}

pub fn print_transfos() {
    let mut aliases: HashMap<&str, Vec<&str>> = HashMap::new();
    for (k, v) in TRANSFO_ALIASES.iter() {
        if !aliases.contains_key(v) {
            aliases.insert(v, Vec::new());
        }
        aliases.get_mut(v).unwrap().push(k);
    }
    for (transfo, data) in TRANSFO_NAMES.iter() {
        let a = aliases.get(transfo)
            .map_or("".to_owned(), |x| {
                if x.len() > 1 {
                    "[aliases : ".to_owned()
                } else {
                    "[alias : ".to_owned()
                }
            } + &x.join(", ") + "]");
        println!("{} : {} {}", transfo, data.1, a);
    }
}

#[derive(Clone)]
pub enum Transformation<'a> {
    Single(Arc<dyn Fn(&GraphNauty) -> Vec<GraphTransformation> + Send + Sync + 'a>),
    Multiple(Vec<Transformation<'a>>),
}

impl<'a> Transformation<'a> {
    pub fn from_name(s: &str) -> Option<Transformation> {
        let s = s.trim().to_lowercase();
        TRANSFO_NAMES
            .get(s.as_str())
            .or_else(|| {
                TRANSFO_ALIASES
                    .get(s.as_str())
                    .and_then(|x| TRANSFO_NAMES.get(x))
            })
            .map(|x| x.0.clone())
    }

    pub fn apply(&self, g: &GraphNauty) -> Vec<GraphTransformation> {
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
where
    F: Fn(&GraphNauty) -> Vec<GraphTransformation> + Send + Sync + 'a,
{
    fn from(f: F) -> Self {
        Transformation::Single(Arc::new(f))
    }
}

impl<'a, T> Add<T> for Transformation<'a>
where
    T: Into<Transformation<'a>> + 'a,
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
where
    T: Into<Transformation<'a>> + 'a,
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
