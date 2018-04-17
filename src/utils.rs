use graph::Graph;
use graph::transfos;

pub fn plural(i: usize) -> String {
    if i != 1 {
        String::from("s")
    } else {
        String::new()
    }
}

pub fn as_filter<'a, F, S>(filter: F, name: S) -> Box<Fn(&Graph) -> Result<String, ()> + 'a>
where
    F: Fn(&Graph) -> bool + 'a,
    S: Fn(&Graph) -> String + 'a,
{
    Box::new(move |x| if filter(x) { Ok(name(x)) } else { Err(()) })
}

pub fn combine_filters<'a, F, G>(f: F, g: G) -> Box<Fn(&Graph) -> Result<String, ()> + 'a>
where
    F: Fn(&Graph) -> Result<String, ()> + 'a,
    G: Fn(&Graph) -> Result<String, ()> + 'a,
{
    Box::new(move |x| match f(x) {
        Err(_) => g(x),
        Ok(s) => Ok(s),
    })
}

pub fn combine_transfos<'a, F, G>(f: F, g: G) -> Box<Fn(&Graph) -> Vec<Graph> + 'a>
where
    F: Fn(&Graph) -> Vec<Graph> + 'a,
    G: Fn(&Graph) -> Vec<Graph> + 'a,
{
    Box::new(move |x| {
        let mut t = f(x);
        let mut v = g(x);
        t.append(&mut v);
        t
    })
}

pub fn trash_node(_: &Graph) -> Result<String, ()> {
    Ok("TRASH".to_string())
}

//TODO use macros ?
pub enum Transfo {
    ROTATION,
    ADD_EDGE,
    REMOVE_EDGE,
}

impl Transfo {
    pub fn new(s: String) -> Transfo {
        //TODO use regexes
        let s = s.trim().to_lowercase().replace("-", "").replace("_", "");
        match s.as_str() {
            "rotation" => Transfo::ROTATION,
            "addedge" => Transfo::ADD_EDGE,
            "removeedge" => Transfo::REMOVE_EDGE,
            _ => panic!(format!("Unknown transformation : \"{}\"", s)),
        }
    }

    pub fn apply(&self, g: &Graph) -> Vec<Graph> {
        match self {
            ROTATION => transfos::rotation(&g),
            ADD_EDGE => transfos::add_edge(&g),
            REMOVE_EDGE => transfos::remove_edge(&g),
            _ => vec![],
        }
    }
}
