use std::{borrow::Cow, hash::Hash};

use petgraph::{graph::EdgeIndex, stable_graph::NodeIndex, visit::EdgeRef};

use crate::property_graph::PropertyGraph;

pub fn generate_key(p: &PropertyGraph) -> String {
    let mut node_names : Vec<(NodeIndex,Cow<str>)> = p.graph.node_indices().map(|n| (n,Cow::from(&p.graph.node_weight(n).unwrap().name))).collect();
    node_names.sort_by(|(_, name1), (_, name2)| name1.cmp(name2));
    //TODO check for duplicates
    let key = node_names.into_iter().fold(String::new(), |mut buff, (node_id,node_name)| {
        buff += node_name.as_ref();
        let mut edges : Vec<Cow<str>> = p.graph.edges_directed(node_id, petgraph::EdgeDirection::Outgoing).map(|e| Cow::from(&e.weight().name)).collect();
        if !edges.is_empty() {
            buff += ":";
            edges.sort();
            buff += &edges.join(",");
        }
        buff += ";";
        buff
    });
    key
}

fn hash_edge<H: std::hash::Hasher>(edge_name: Cow<str>, edge_id: EdgeIndex, g: &PropertyGraph, state: &mut H) {
    edge_name.hash(state);
    let mut props : Vec<(Cow<str>, Cow<str>)> = g.graph.edge_weight(edge_id).unwrap().map.iter().map(|(k,v)| (Cow::from(k), Cow::from(v))).collect();
    props.sort();
    props.into_iter().for_each(|(k,v)| {k.hash(state); v.hash(state)} );
    let mut labels : Vec<Cow<str>> = g.edge_label.element_labels(&edge_id).map(|id| Cow::from(g.edge_label.get_label(*id).unwrap())).collect();
    labels.sort();
    labels.into_iter().for_each(|l| l.hash(state) );
}

fn hash_node<H: std::hash::Hasher>(node_name: Cow<str>, node_id: NodeIndex, g: &PropertyGraph, state: &mut H) {
    node_name.hash(state);
    let mut props : Vec<(Cow<str>, Cow<str>)> = g.graph.node_weight(node_id).unwrap().map.iter().map(|(k,v)| (Cow::from(k), Cow::from(v))).collect();
    props.sort();
    props.into_iter().for_each(|(k,v)| {k.hash(state); v.hash(state)} );
    let mut labels : Vec<Cow<str>> = g.vertex_label.element_labels(&node_id).map(|id| Cow::from(g.vertex_label.get_label(*id).unwrap())).collect();
    labels.sort();
    labels.into_iter().for_each(|l| l.hash(state) );
    let mut edges : Vec<(EdgeIndex,Cow<str>)> = g.graph.edges_directed(node_id, petgraph::EdgeDirection::Outgoing).map(|e| (e.id(), Cow::from(&e.weight().name))).collect();
    edges.sort_by(|(_, name1), (_, name2)| name1.cmp(name2));
    for (edge_id, edge_name) in edges.into_iter() {
        hash_edge(edge_name, edge_id, g, state);
    }
}

impl Hash for PropertyGraph {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let mut node_names : Vec<(NodeIndex,Cow<str>)> = self.graph.node_indices().map(|n| (n,Cow::from(&self.graph.node_weight(n).unwrap().name))).collect();
        node_names.sort_by(|(_, name1), (_, name2)| name1.cmp(name2));
        //TODO check for duplicates
        for (node_id, node_name) in node_names.into_iter() {
            hash_node(node_name, node_id, &self, state);
        }
    }
}

#[cfg(test)]
mod test {
    use std::hash::{DefaultHasher, Hash, Hasher};

    use crate::parsing::PropertyGraphParser;

    use super::generate_key;

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
        let g = results.get(0).unwrap();
        let key = generate_key(g);
        let expected = "customerType:aliasType,friendType;personType;suspiciousType;";
        assert_eq!(key, expected);
        let mut h = DefaultHasher::new();
        g.hash(&mut h);
        println!("{}", h.finish());
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
        let g = results.get(0).unwrap();
        let mut h = DefaultHasher::new();
        g.hash(&mut h);
        println!("{}", h.finish());
        panic!()
    }
}
