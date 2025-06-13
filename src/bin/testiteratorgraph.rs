use std::{
    collections::{HashMap, HashSet},
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
    parsing::PropertyGraphParser,
    property_graph::PropertyGraph,
    transformation::{
        souffle::{
            create_program_instance, generate_operation_trees, AutomatonNode,
            TransformationAutomaton,
        },
        transform_graph_ids, Operation, OperationName, TransformGenerator,
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
    let operations = (1..=5).map(|i| {
        Operation::AddVertexProperty("vert".to_string(), format!("prop{}", i), "type".to_string())
    }).collect::<Vec<_>>();
    let mut graph = TransformationAutomaton::new();
    let add_node_n = graph.add_operation(&add_node, &add_node, true);
    let nodes = operations.iter().map(|op| graph.add_operation(op, &add_node, false)).collect::<Vec<_>>();
    for (i, node) in nodes.iter().enumerate() {
        graph.graph.add_edge(add_node_n, *node, None);
        for (j, node2) in nodes.iter().enumerate().filter(|(j,_)| i != *j) {
            graph.graph.add_edge(*node, *node2, None);
        }
    }
    graph
}

fn get_transfos() -> HashMap<Operation, HashMap<Operation, Vec<Operation>>> {
    let mut res = HashMap::new();
    let mut tree = HashMap::new();
    tree.insert(
        Operation::AddVertex("vert".to_string()),
        vec![
            Operation::AddVertexLabel("vert".to_string(), "label".to_string()),
            Operation::AddVertexProperty(
                "vert".to_string(),
                "prop".to_string(),
                "type".to_string(),
            ),
        ],
    );
    tree.insert(
        Operation::AddVertexLabel("vert".to_string(), "label".to_string()),
        vec![
            Operation::AddVertexLabel("vert".to_string(), "label".to_string()),
            Operation::AddVertexProperty(
                "vert".to_string(),
                "prop".to_string(),
                "type".to_string(),
            ),
        ],
    );
    tree.insert(
        Operation::AddVertexProperty("vert".to_string(), "prop".to_string(), "type".to_string()),
        vec![Operation::AddVertex("othervert".to_string())],
    );
    res.insert(Operation::AddVertex("vert".to_string()), tree);
    res
}

fn to_undirected(
    g: &TransformationAutomaton,
) -> (
    StableGraph<(Operation, Operation), (), Undirected>,
    HashMap<NodeIndex, NodeIndex>,
) {
    let mut new_graph: StableGraph<(Operation, Operation), (), Undirected> = StableGraph::default();
    let mut node_map: HashMap<NodeIndex, NodeIndex> = HashMap::new();
    for node in g.graph.node_indices() {
        node_map.insert(
            node,
            new_graph.add_node((g.graph[node].root.clone(), g.graph[node].op.clone())),
        );
    }
    for e in g.graph.edge_references() {
        let v1 = e.source();
        let v2 = e.target();
        let t1: OperationName = g.graph[v1].op.clone().into();
        let t2: OperationName = g.graph[v2].op.clone().into();
        let nv1 = node_map.get(&v1).unwrap();
        let nv2 = node_map.get(&v2).unwrap();
        if !new_graph.contains_edge(*nv1, *nv2) && t1 == t2 && g.graph.contains_edge(v2, v1) {
            new_graph.add_edge(*nv1, *nv2, ());
        }
    }
    (new_graph, node_map)
}

fn contract_graph(g: &mut TransformationAutomaton) {
    let (undirected, node_map) = to_undirected(g);
    let cliques = petgraph::algo::maximal_cliques(&undirected);
    let mut handled = HashSet::new();
    for set in cliques.into_iter().filter(|s| s.len() > 1) {
        let node1 = set.iter().next().unwrap();
        let (root, op) = &undirected[*node1];
        let mut new_node = AutomatonNode {
            root: root.clone(),
            op: op.clone(),
            group: None,
        };
        let new_node_ref = g.graph.add_node(new_node);
        let mut group = vec![];
        for und_v in set.iter() {
            if !handled.contains(und_v) {
                let (root, op) = &undirected[*und_v];
                group.push(op.clone());
                let v = g.node_set.get(&root).unwrap().get(&op).unwrap();
                let neighbors_incoming = g.graph.neighbors_directed(*v, Incoming).detach();
                let neighbors_outgoing = g.graph.neighbors_directed(*v, Outgoing).detach();
                for (mut neighbor, incoming) in
                    [(neighbors_incoming, true), (neighbors_outgoing, false)]
                {
                    while let Some(u) = neighbor.next_node(&g.graph) {
                        if !set.contains(node_map.get(&u).unwrap()) {
                            if incoming && !g.graph.contains_edge(u, new_node_ref) {
                                g.graph.add_edge(u, new_node_ref, Some(op.clone()));
                            } else if !incoming && !g.graph.contains_edge(new_node_ref, u) {
                                g.graph.add_edge(new_node_ref, u, Some(op.clone()));
                            }
                        }
                    }
                }
                g.graph.remove_node(*v);
                g.node_set.get_mut(&root).unwrap().insert(op.clone(), new_node_ref);
                handled.insert(*und_v);
            }
        }
        g.graph[new_node_ref].group = Some(group);
    }
}

struct SubsetGenerator<'a, T>
where T: Clone
{
    list: &'a Vec<T>,
    current: Vec<T>,
    indices: Vec<usize>,
    index: usize
}

impl<'a, T> SubsetGenerator<'a, T>
where T: Clone
{
    fn new(list: &'a Vec<T>) -> Self {
        let mut indices = vec![0; list.len()];
        SubsetGenerator {
            list,
            current: Vec::new(),
            indices,
            index: 0,
        }
    }
}

impl <'a, T> Iterator for SubsetGenerator<'a, T>
where T:Clone
{
    type Item = Vec<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.list.is_empty() {
            None
        } else if self.current.is_empty() {
            self.current.push(self.list[0].clone());
            Some(self.current.clone())
        } else {
            if self.indices[0] == self.list.len() - 1 {
                None
            } else {
                if self.indices[self.index] == self.list.len() - 1 {
                    self.current.pop();
                    self.index -= 1;
                    self.indices[self.index] += 1;
                    self.current[self.index] = self.list[self.indices[self.index]].clone();
                    Some(self.current.clone())
                } else {
                    self.indices[self.index + 1] = self.indices[self.index] + 1;
                    self.index += 1;
                    self.current.push(self.list[self.indices[self.index]].clone());
                    Some(self.current.clone())
                }
            }
        }
    }
}

fn main() {
    let schema = get_schema();
    let mut g = get_graph();
    contract_graph(&mut g);
    for arc in g.graph.edge_references() {
        let src = arc.source();
        let dst = arc.target();
        println!(
            "{:?} -> {:?}: {:?} ({:?} -> {:?})",
            g.graph[src].op,
            g.graph[dst].op,
            arc.weight(),
            g.graph[src].group,
            g.graph[dst].group
        );
    }

    let list = vec![1, 2, 3, 4, 5];
    for v in SubsetGenerator::new(&list) {
        println!("{:?}", v);
    }
    // let iterator = TransformGenerator::new(get_transfos(), &schema);
    // for gt in iterator {
    //     println!("{}", gt);
    // }
}
