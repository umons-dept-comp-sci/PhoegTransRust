use std::{collections::HashMap, hash::Hash};

use transproof::{
    parsing::PropertyGraphParser, property_graph::PropertyGraph, transformation::{
        souffle::{create_program_instance, generate_operation_trees}, transform_graph_ids, Operation, TransformGenerator
    }
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

fn get_transfos() -> HashMap<Operation, HashMap<Operation, Vec<Operation>>> {
    let mut res = HashMap::new();
    let mut tree = HashMap::new();
    tree.insert(Operation::AddVertex("vert".to_string()), vec![
        Operation::AddVertexLabel("vert".to_string(),"label".to_string()),
        Operation::AddVertexProperty("vert".to_string(),"prop".to_string(), "type".to_string())
    ]);
    tree.insert(Operation::AddVertexLabel("vert".to_string(),"label".to_string()), vec![
        Operation::AddVertexLabel("vert".to_string(),"label".to_string()),
        Operation::AddVertexProperty("vert".to_string(),"prop".to_string(), "type".to_string())
    ]);
    tree.insert(Operation::AddVertexProperty("vert".to_string(),"prop".to_string(), "type".to_string()), vec![
        Operation::AddVertex("othervert".to_string())
    ]);
    res.insert(Operation::AddVertex("vert".to_string()), tree);
    res
}

fn main() {
    let schema = get_schema();
    let trees = get_transfos();
    let iterator = TransformGenerator::new(get_transfos(), &schema);
    for gt in iterator {
        println!("{}", gt);
    }
}
