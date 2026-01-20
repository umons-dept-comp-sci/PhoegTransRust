use std::{
    collections::{HashMap, HashSet, VecDeque},
    hash::Hash,
};

use petgraph::{
    graph::{NodeIndex, WalkNeighbors},
    prelude::{StableGraph, StableUnGraph},
    visit::{EdgeRef, IntoEdgeReferences},
    Direction::{Incoming, Outgoing},
    Undirected,
};
use transproof::{
    graph_transformation::GraphTransformation,
    parsing::PropertyGraphParser,
    property_graph::PropertyGraph,
    transformation::{
        souffle::{create_program_instance, generate_operation_trees},
        Operation, OperationName, TransformGenerator,
    },
    transformation_automaton::{
        contract_graph, AutomatonNode, SubsetGenerator, TransformGeneratorGraph,
        TransformationAutomaton,
    },
};

fn get_schema() -> PropertyGraph {
    let text = "
create graph type persondata {
(personType: Person {name string, address string}),
(addressType: Address {occ string, city string}),
(placeType: Place {occ string, zip string})
}
";
    let parser = PropertyGraphParser;
    let mut results = parser.convert_text(text);
    results.pop().unwrap()
}

fn get_graph() -> TransformationAutomaton {
    let add_node = Operation::AddVertex("vert".to_string());
    let add_label = Operation::AddVertexLabel("vert".to_string(), "label".to_string());
    let operations = (1..=5)
        .map(|i| {
            Operation::AddVertexProperty(
                "vert".to_string(),
                format!("prop{}", i),
                "type".to_string(),
            )
        })
        .collect::<Vec<_>>();
    let mut graph = TransformationAutomaton::new();
    let add_node_n = graph.add_operation(&add_node, &add_node, None, true);
    let add_label_n = graph.add_operation(&add_label, &add_node, None, true);
    let nodes = operations
        .iter()
        .map(|op| graph.add_operation(op, &add_node, None, false))
        .collect::<Vec<_>>();
    for (i, node) in nodes.iter().enumerate() {
        graph.graph.add_edge(add_node_n, *node, None);
        graph.graph.add_edge(*node, add_label_n, None);
        for (j, node2) in nodes.iter().enumerate().filter(|(j, _)| i != *j) {
            graph.graph.add_edge(*node, *node2, None);
        }
    }
    graph
}

fn get_transfos() -> HashMap<Operation, HashMap<Operation, Vec<Operation>>> {
    let mut res = HashMap::new();
    let mut tree = HashMap::new();
    let add_node = Operation::AddVertex("vert".to_string());
    let operations = (1..=5)
        .map(|i| {
            Operation::AddVertexProperty(
                "vert".to_string(),
                format!("prop{}", i),
                "type".to_string(),
            )
        })
        .collect::<Vec<_>>();
    tree.insert(add_node.clone(), operations.clone());
    for op in operations.iter() {
        let new_list = operations
            .iter()
            .filter(|o| o != &op)
            .cloned()
            .collect::<Vec<_>>();
        tree.insert(op.clone(), new_list);
    }
    res.insert(add_node, tree);
    res
}

fn main2() {
    let schema = get_schema();
    // for schema in TransformGenerator::new(get_transfos(), &schema) {
    //     println!("{}", schema);
    // }
    let mut g = get_graph();
    contract_graph(&mut g);
    for schema in TransformGeneratorGraph::new(g, &schema) {
        println!("{}", schema);
    }
}

fn main() {
    let schema = get_schema();
    // let mut g = get_graph();
    // contract_graph(&mut g);
    // for arc in g.graph.edge_references() {
    //     let src = arc.source();
    //     let dst = arc.target();
    //     println!(
    //         "{:?} -> {:?}: {:?} ({:?} -> {:?})",
    //         g.graph[src].op,
    //         g.graph[dst].op,
    //         arc.weight(),
    //         g.graph[src].group,
    //         g.graph[dst].group
    //     );
    // }

    let list = (1..=3)
        .map(|i| {
            Operation::AddVertexProperty(
                "personType".to_string(),
                format!("prop{}", i),
                "type".to_string(),
            )
        })
        .collect::<Vec<_>>();
    let pg: GraphTransformation = (&schema).into();
    for v in SubsetGenerator::new(list, pg) {
        println!("{}", v);
    }
    // let iterator = TransformGenerator::new(get_transfos(), &schema);
    // for gt in iterator {
    //     println!("{}", gt);
    // }
}
