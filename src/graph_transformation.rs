use std::fmt::Display;

use crate::property_graph::PropertyGraph;

#[derive(Debug)]
pub struct GraphTransformation {
    pub init: PropertyGraph,
    pub result: PropertyGraph,
    pub operations: Vec<String>,
}

impl From<&PropertyGraph> for GraphTransformation {
    fn from(g: &PropertyGraph) -> Self {
        GraphTransformation {
            init: g.clone(),
            result: g.clone(),
            operations: Vec::new(),
        }
    }
}

impl Display for GraphTransformation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "===")?;
        write!(f, "{}", self.init)?;
        writeln!(f, "---")?;
        write!(f, "{}", self.result)?;
        writeln!(f, "===")
    }
}
