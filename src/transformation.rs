use graph::Graph;
use graph::transfos;
use std::ops::{Add, AddAssign};
use std::sync::Arc;

#[derive(Clone)]
pub enum Transformation<'a> {
    Single(Arc<Fn(&Graph) -> Vec<Graph> + Send + Sync + 'a>),
    Multiple(Vec<Transformation<'a>>),
}

impl<'a> Transformation<'a> {
    pub fn from_name(s: &str) -> Option<Transformation> {
        match s.trim().to_lowercase().as_str() {
            "add_edge" => Some(Transformation::Single(Arc::new(transfos::add_edge))),
            "remove_edge" => Some(Transformation::Single(Arc::new(transfos::remove_edge))),
            "rotation" => Some(Transformation::Single(Arc::new(transfos::rotation))),
            "slide" => Some(Transformation::Single(Arc::new(transfos::slide))),
            "move_distinct" => Some(Transformation::Single(Arc::new(transfos::move_distinct))),
            "two_opt" => Some(Transformation::Single(Arc::new(transfos::two_opt))),
            "shortcut" => Some(Transformation::Single(Arc::new(transfos::shortcut))),
            "detour" => Some(Transformation::Single(Arc::new(transfos::detour))),
            _ => None,
        }
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
        let mut ls: Vec<Transformation<'a>>;
        if let Transformation::Multiple(ref mut ls) = *self {
            add_other(ls, other);
        } else {
            let mut ls = vec![(*self).clone()];
            add_other(&mut ls, other);
            *self = Transformation::Multiple(ls);
        }
    }
}
