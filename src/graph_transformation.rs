use std::{collections::HashMap, fmt::Display};

use petgraph::graph::{EdgeIndex, NodeIndex};

use crate::property_graph::PropertyGraph;

#[derive(Debug)]
pub struct GraphTransformation {
    pub init : PropertyGraph,
    pub result: PropertyGraph,
    pub mapping_init_vertex: HashMap<NodeIndex, NodeIndex>,
    pub mapping_init_edge: HashMap<EdgeIndex, EdgeIndex>,
    pub mapping_result_vertex: HashMap<NodeIndex, NodeIndex>,
    pub mapping_result_edge: HashMap<EdgeIndex, EdgeIndex>,
}

impl From<&PropertyGraph> for GraphTransformation {
    fn from(g: &PropertyGraph) -> Self {
        let mut res = GraphTransformation {
            init : g.clone(),
            result : g.clone(),
            mapping_init_edge : HashMap::with_capacity(g.graph.edge_count()),
            mapping_init_vertex : HashMap::with_capacity(g.graph.node_count()),
            mapping_result_edge : HashMap::with_capacity(g.graph.edge_count()),
            mapping_result_vertex : HashMap::with_capacity(g.graph.node_count())
        };
        for vertex in g.graph.node_indices() {
            res.mapping_init_vertex.insert(vertex, vertex);
            res.mapping_result_vertex.insert(vertex, vertex);
        }
        for edge in g.graph.edge_indices() {
            res.mapping_init_edge.insert(edge, edge);
            res.mapping_result_edge.insert(edge, edge);
        }
        res
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