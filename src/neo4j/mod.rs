use std::{collections::HashMap, hash::{DefaultHasher, Hash, Hasher}, io::BufWriter};
use std::io::Write;

use neo4rs::{query, Graph, Txn};

use crate::{graph_transformation::GraphTransformation, property_graph::{Properties, PropertyGraph}};

async fn get_or_create_metanode(key: u64, conn: &mut Txn) -> bool {
    let query = query(
"
with timestamp() as time
merge (n:Meta {key:$key})
on create
set n.created = time
return time = n.created as created
").param("key", key as i64);
    let mut data = conn.execute(query).await.unwrap();
    let row = data.next(conn.handle()).await.unwrap().unwrap();
    row.get("created").unwrap()
}

fn format_data(
    out: &mut BufWriter<Vec<u8>>,
    labels: &Vec<&String>,
    props: &Properties,
    edge: bool
) {
    write!(out, "{}", props.name);
    if edge && labels.is_empty() {
        write!(out, ":Internal");
    } else {
        let mut start = true;
        for label in labels {
            if !start && edge {
                write!(out, "_");
            } else {
                write!(out, ":");
                start = false;
            }
            write!(out, "{}", label);
        }
    }
    write!(out, " {{ _name:\"{}\"", props.name);
    for (key, typ) in props.map.iter() {
        write!(out, ", ");
        write!(out, "{}:\"{}\"", key, typ);
    }
    write!(out, " }}");
}

fn create_property_graph_query(g: &PropertyGraph) -> String {
    let mut out = BufWriter::new(Vec::new());
    write!(out, "MATCH (_meta:Meta {{key:$key}}) CREATE ");
    let mut names = HashMap::new();
    let mut start = true;
    for vertex in g.graph.node_indices() {
        if start {
           start = false;
        } else {
            write!(out, ", ");
        }
        let props = g.graph.node_weight(vertex).unwrap();
        names.insert(vertex, props.name.clone());
        let labels = g
            .vertex_label
            .element_labels(&vertex)
            .map(|id| g.vertex_label.get_label(*id).unwrap())
            .collect();
        write!(out, "( ");
        format_data(&mut out, &labels, props, false);
        write!(out, " )");
    }
    for edge in g.graph.edge_indices() {
        let (from, to) = g.graph.edge_endpoints(edge).unwrap();
        let props = g.graph.edge_weight(edge).unwrap();
        let labels = g
            .edge_label
            .element_labels(&edge)
            .map(|id| g.edge_label.get_label(*id).unwrap())
            .collect();
        write!(out, ", ({})", names.get(&from).unwrap());
        write!(out, "  -[");
        format_data(&mut out, &labels, props, true);
        write!(out, " ]->");
        write!(out, "({})", names.get(&to).unwrap());
    }
    for name in names.values() {
        write!(out, ", (_meta)-[:Inner]->({})", name);
    }
    write!(out, ";");
    String::from_utf8(out.into_inner().unwrap()).unwrap()
}

async fn write_property_graph(g: &PropertyGraph, conn: &Graph) -> u64 {
    let mut hash = DefaultHasher::new();
    g.hash(&mut hash);
    let key = hash.finish();
    let mut tx = conn.start_txn().await.unwrap();
    if get_or_create_metanode(key, &mut tx).await {
        let query = query(&create_property_graph_query(g)).param("key", key as i64);
        tx.run(query).await.unwrap();
    }
    tx.commit().await.unwrap();
    key
}

fn build_meta_edge_query(first_key : u64, second_key : u64, gt: &GraphTransformation) -> String {
    let start =
"
MATCH (n1: Meta {key:$first_key}), (n2: Meta {key:$second_key})
CREATE (n1) -[:Meta]-> (n2);
";
    start.to_string()
}

pub async fn write_graph_transformation(gt: &GraphTransformation, conn: &Graph) {
    let first = &gt.init;
    let first_key = write_property_graph(first, conn).await;
    let second = &gt.result;
    let second_key = write_property_graph(second, conn).await;
    let query = query(&build_meta_edge_query(first_key, second_key, gt)).param("first_key", first_key as i64).param("second_key", second_key as i64);
    conn.run(query).await.unwrap();
}

#[cfg(test)]
mod tests {
    use crate::parsing::PropertyGraphParser;

    use super::*;

    #[test]
    fn test_name() {
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
        panic!()
    }
}
