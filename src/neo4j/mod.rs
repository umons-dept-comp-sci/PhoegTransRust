use std::borrow::Cow;

use petgraph::stable_graph::NodeIndex;

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

#[cfg(test)]
mod test {
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
    }
}
