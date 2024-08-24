use std::{collections::HashMap, fmt::format};

use petgraph::graph::{EdgeIndex, NodeIndex};
use probminhash::probminhasher::ProbMinHash3aSha;

use crate::property_graph::PropertyGraph;

pub fn node_base_features(g: &PropertyGraph, n: &NodeIndex) -> Vec<String> {
    let mut features = Vec::new();
    let weight = g.graph.node_weight(*n).unwrap();
    features.push(format!("node:name:{}",weight.name));
    for prop in weight.map.iter() {
        features.push(format!("node:prop:{}:{}",prop.0,prop.1));
    }
    for label in g.vertex_label.element_labels(n).map(|id| g.vertex_label.get_label(*id).unwrap()) {
        features.push(format!("node:label:{}",label));
    }
    features
}

pub fn edge_base_features(g: &PropertyGraph, e: &EdgeIndex) -> Vec<String> {
    let mut features = Vec::new();
    let weight = g.graph.edge_weight(*e).unwrap();
    features.push(format!("edge:name:{}",weight.name));
    for prop in weight.map.iter() {
        features.push(format!("edge:prop:{}:{}",prop.0,prop.1));
    }
    for label in g.edge_label.element_labels(e).map(|id| g.edge_label.get_label(*id).unwrap()) {
        features.push(format!("edge:label:{}",label));
    }
    features
}

pub fn inner_features(base_features: &[String]) -> Vec<String> {
    let mut res: Vec<String> = base_features.iter().cloned().collect();
    for i in 0..base_features.len() {
        for j in 0..base_features.len() {
            if i != j {
                res.push(format!("inner:{};{}", base_features[i], base_features[j]));
            }
        }
    }
    res
}

pub fn pair_features(first_features: &[String], second_features: &[String], prefix: &str) -> Vec<String> {
    let mut res = Vec::new();
    for f1 in first_features.iter() {
        for f2 in second_features.iter() {
            res.push(format!("{}{};{}",prefix,f1,f2));
        }
    }
    res
}

pub fn adj_features(from_features: &[String], to_features: &[String], edge_features: &[String]) -> Vec<String> {
    pair_features(from_features, to_features, "adj:").into_iter()
        .chain(pair_features(from_features, edge_features, "").into_iter())
        .chain(pair_features(edge_features, to_features, "").into_iter())
        .collect()
}

pub fn property_graph_features(g: &PropertyGraph) -> Vec<String> {
    let node_features: HashMap<NodeIndex, Vec<String>> = g.graph.node_indices().map(|id| (id,node_base_features(g, &id))).collect();
    g.graph.node_indices().flat_map(|id| inner_features(node_features.get(&id).unwrap()).into_iter())
        .chain(g.graph.edge_indices().flat_map(|id| {
            let ef = edge_base_features(g, &id);
            let (from,to) = g.graph.edge_endpoints(id).unwrap();
            let ff = node_features.get(&from).unwrap();
            let tf = node_features.get(&to).unwrap();
            inner_features(&ef).into_iter().chain(adj_features(&ff, &tf, &ef).into_iter())
        }))
        .collect()
}

pub fn property_graph_minhash(g: &PropertyGraph) -> Vec<String> {
    let features = property_graph_features(g).into_iter().fold(HashMap::new(), |mut map, feature| {
        *map.entry(feature).or_insert(0) += 1;
        map
    });
    let mut minhash = ProbMinHash3aSha::new(200, "".to_string());
    minhash.hash_weigthed_hashmap(&features);
    minhash.get_signature().to_vec()
}

#[cfg(test)]
mod sim_test {
    use probminhash::jaccard::compute_probminhash_jaccard;

    use crate::parsing::PropertyGraphParser;

    use super::*;

    #[test]
    fn smoke_test() {
        let text = "CREATE GRAPH TYPE fraudGraphType {
( personType : Person { name STRING , birthday DATE }) ,
( customerType : Person & Customer { name STRING , since DATE }) ,
( suspiciousType : Suspicious { reason STRING }) ,
( : customerType )
-[ friendType : Knows & Likes {time INT} ] ->
( : customerType ),
( : customerType )
-[ aliasType {frequency INT} ] ->
( : suspiciousType )
}";
        let parser = PropertyGraphParser;
        let results = parser.convert_text(text);
        let g1 = results.get(0).unwrap();

        let hash1 = property_graph_minhash(g1);
        println!("{:?}", hash1);

        let text = "CREATE GRAPH TYPE fraudGraphType {
( personType : Person { name STRING , birthday DATE }) ,
( customerType : Person & Customer { name STRING , since DATE }) ,
( suspiciousType : Suspicious { reason STRING }) ,
( : customerType )
-[ friendType : Knows & Likes {time INT} ] ->
( : customerType ),
( : customerType )
-[ aliasType {frequency INT} ] ->
( : suspiciousType )
}";
        let parser = PropertyGraphParser;
        let results = parser.convert_text(text);
        let g2 = results.get(0).unwrap();

        let hash2 = property_graph_minhash(g2);
        println!("{:?}", hash2);

        println!("dist: {}", compute_probminhash_jaccard(&hash1, &hash2));
    }
}
