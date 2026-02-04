use transproof::{constants::IDEMPOTENCE, parsing::PropertyGraphParser, property_graph::PropertyGraph, similarity::jaccard_index, transformation::Operation, transformation_automaton::{contract_graph, TransformGeneratorGraph, TransformationAutomaton}};

fn get_schema() -> PropertyGraph {
    let text = "
create graph type persondata {
(personType: Person {name string, address string}),
(vertexType: {prop0 string})
}
";
    let parser = PropertyGraphParser;
    let mut results = parser.convert_text(text);
    results.pop().unwrap()
}

fn get_target() -> PropertyGraph {
    let text = "
create graph type persondata {
(personType: Person {name string, address string}),
(vertexType: {prop0 string, prop1 string, prop2 string, prop3 string, prop4 string})
}
";
    let parser = PropertyGraphParser;
    let mut results = parser.convert_text(text);
    results.pop().unwrap()
}

fn get_operations() -> TransformationAutomaton {
    let mut res = TransformationAutomaton::default();
    let mut root = Operation::AddVertex("vertexType".to_string());
    let mut root_v =  res.add_operation(&root, &root, Some(0), true);
    let mut vertices = Vec::new();
    for i in 0..5 {
        let op = Operation::AddVertexProperty("vertexType".to_string(), format!("prop{}", i), "string".to_string());
        let op_v = res.add_operation(&op, &root, Some(0), false);
        res.graph.add_edge(root_v, op_v, None);
        for prev in vertices.iter() {
            res.graph.add_edge(*prev, op_v, None);
        }
        vertices.push(op_v);
    }
    // let mut root_v =  res.add_operation(&root, &root, Some(1), true);
    // for i in 0..5 {
    //     let op = Operation::AddVertexProperty("vertexType".to_string(), format!("prop{}", i), "string".to_string());
    //     let op_v = res.add_operation(&op, &root, Some(1), false);
    //     res.graph.add_edge(root_v, op_v, None);
    //     for prev in vertices.iter() {
    //         res.graph.add_edge(*prev, op_v, None);
    //     }
    //     vertices.push(op_v);
    // }
    contract_graph(&mut res);
    res
}

fn main() {
    IDEMPOTENCE.set(true);
    let g = get_schema();
    let transfos = get_operations();
    let gen = TransformGeneratorGraph::new(transfos, &g);
    let target = get_target();
    for transfo in gen {
        let sim = jaccard_index(&target, &transfo.result);
        println!("{}", transfo);
        println!("sim: {}", sim);
    }
}
