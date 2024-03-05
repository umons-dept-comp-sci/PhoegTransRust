use crate::graph_transformation::GraphTransformation;

/// Returns "s" if i is different from 1 and an empty string otherwise.
pub fn plural(i: usize) -> String {
    if i != 1 {
        String::from("s")
    } else {
        String::new()
    }
}

pub fn as_filter<'a, F, S>(filter: F, name: S) -> Box<dyn Fn(&GraphTransformation) -> Result<String, ()> + 'a>
    where F: Fn(&GraphTransformation) -> bool + 'a,
          S: Fn(&GraphTransformation) -> String + 'a
{
    Box::new(move |x| if filter(x) { Ok(name(x)) } else { Err(()) })
}

pub fn combine_filters<'a, F, G>(f: F, g: G) -> Box<dyn Fn(&GraphTransformation) -> Result<String, ()> + 'a>
    where F: Fn(&GraphTransformation) -> Result<String, ()> + 'a,
          G: Fn(&GraphTransformation) -> Result<String, ()> + 'a
{
    Box::new(move |x| match f(x) {
        Err(_) => g(x),
        Ok(s) => Ok(s),
    })
}

pub fn trash_node(_: &GraphTransformation) -> Result<String, ()> {
    Ok("TRASH".to_string())
}
