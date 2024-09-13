use std::io::Write;
use std::{
    collections::HashMap,
    fmt::format,
    hash::{DefaultHasher, Hash, Hasher},
    io::BufWriter,
};

use neo4rs::{query, Graph, Node, Path, Relation, Txn};

use crate::{
    graph_transformation::GraphTransformation,
    property_graph::{Properties, PropertyGraph},
};

const INTERNAL_LABEL: &str = "Internal";
const META_LABEL: &str = "Meta";
const INNER_LABEL: &str = "Inner";
pub const NEW_LABEL: &str = "New";
pub const SOURCE_LABEL: &str = "Source";
pub const TARGET_LABEL: &str = "Target";
const PATH_LABEL: &str = "Path";
const CREATED_PROP: &str = "created";
const KEY_PROP: &str = "key";
const NAME_PROP: &str = "_name";
pub const OPERATIONS_PROP: &str = "operations";

async fn get_or_create_metanode(
    key: u64,
    is_output: bool,
    is_source: bool,
    conn: &mut Txn,
) -> bool {
    let add_new = if is_output {
        format!(", n:{new}", new = NEW_LABEL)
    } else {
        "".to_string()
    };
    let add_source = if is_source {
        format!(", n:{source}", source = SOURCE_LABEL)
    } else {
        "".to_string()
    };
    let remove_new = if is_output {
        "".to_string()
    } else {
        format!("remove n:{new}", new = NEW_LABEL)
    };
    let query = query(&format!(
        "
call {{
with timestamp() as time
merge (n:{meta} {{{key}:$key}})
on create
set n.{created} = time {add_new} {add_source}
return n,n.{created} = time as created
}}
{remove_new}
return created
",
        add_new = add_new,
        add_source = add_source,
        remove_new = remove_new,
        key = KEY_PROP,
        created = CREATED_PROP,
        meta = META_LABEL
    ))
    .param("key", key as i64);
    let mut data = conn.execute(query).await.unwrap();
    let row = data.next(conn.handle()).await.unwrap().unwrap();
    row.get("created").unwrap()
}

fn format_data(
    out: &mut BufWriter<Vec<u8>>,
    labels: &Vec<&String>,
    props: &Properties,
    edge: bool,
) {
    write!(out, "{}", props.name);
    if edge && labels.is_empty() {
        write!(out, ":{}", INTERNAL_LABEL);
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
    write!(out, " {{ {name}:\"{}\"", props.name, name = NAME_PROP);
    for (key, typ) in props.map.iter() {
        write!(out, ", ");
        write!(out, "{}:\"{}\"", key, typ);
    }
    write!(out, " }}");
}

fn create_property_graph_query(g: &PropertyGraph) -> String {
    let mut out = BufWriter::new(Vec::new());
    write!(
        out,
        "MATCH (_meta:{meta} {{{key}:$key}}) CREATE ",
        meta = META_LABEL,
        key = KEY_PROP
    );
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
        write!(
            out,
            ", (_meta)-[:{inner}]->({name})",
            inner = INNER_LABEL,
            name = name
        );
    }
    write!(out, ";");
    String::from_utf8(out.into_inner().unwrap()).unwrap()
}

async fn write_property_graph(
    g: &PropertyGraph,
    is_output: bool,
    is_source: bool,
    conn: &Graph,
) -> u64 {
    let mut hash = DefaultHasher::new();
    g.hash(&mut hash);
    let key = hash.finish();
    let mut tx = conn.start_txn().await.unwrap();
    if get_or_create_metanode(key, is_output, is_source, &mut tx).await {
        let query = query(&create_property_graph_query(g)).param("key", key as i64);
        tx.run(query).await.unwrap();
    }
    tx.commit().await.unwrap();
    key
}

fn build_meta_edge_query() -> String {
    let start = format!(
        "
MATCH (n1: {meta} {{{key}:$first_key}}), (n2: {meta} {{{key}:$second_key}})
CREATE (n1) -[:{meta} {{{ops}:$operations}}]-> (n2);
",
        key = KEY_PROP,
        meta = META_LABEL,
        ops = OPERATIONS_PROP
    );
    start.to_string()
}

pub async fn write_graph_transformation(gt: &GraphTransformation, is_source: bool, conn: &Graph) {
    let first = &gt.init;
    let first_key = write_property_graph(first, false, is_source, conn).await;
    let second = &gt.result;
    let second_key = write_property_graph(second, true, false, conn).await;
    let query = query(&build_meta_edge_query())
        .param("first_key", first_key as i64)
        .param("second_key", second_key as i64)
        .param("operations", gt.operations.clone());
    conn.run(query).await.unwrap();
}

async fn get_source_graphs_async(label: &str, conn: &Graph) -> Vec<PropertyGraph> {
    let mut graphs = Vec::new();
    let query = query(&format!(
        "match (s:{selected})
return
  collect {{ match (s)-[:{inner}]->(n) return n }} as n,
  collect {{ match (s)-[:{inner}]->()-[e:!{inner}]->() return e }} as e;
",
        selected = label,
        inner = INNER_LABEL
    ));
    let mut res = conn.execute(query).await.unwrap();
    while let Ok(Some(row)) = res.next().await {
        let mut g = PropertyGraph::default();
        let mut ids = HashMap::new();
        let nodes: Vec<Node> = row.get("n").unwrap();
        for node in nodes {
            let mut props = HashMap::new();
            let mut name = None;
            for key in node.keys() {
                if key == NAME_PROP {
                    name = Some(node.get(key).unwrap());
                } else {
                    props.insert(key.to_string(), node.get(key).unwrap());
                }
            }
            let props = Properties {
                name: name.unwrap(),
                map: props,
            };
            let id = g.graph.add_node(props);
            for label in node.labels() {
                let lid = g.vertex_label.add_label(label.to_string());
                g.vertex_label.add_label_mapping(&id, lid).unwrap();
            }
            ids.insert(node.id(), id);
        }
        let edges: Vec<Relation> = row.get("e").unwrap();
        for edge in edges {
            let mut props = HashMap::new();
            let mut name = None;
            for key in edge.keys() {
                if key == NAME_PROP {
                    name = Some(edge.get(key).unwrap());
                } else {
                    props.insert(key.to_string(), edge.get(key).unwrap());
                }
            }
            let props = Properties {
                name: name.unwrap(),
                map: props,
            };
            let from_id = ids.get(&edge.start_node_id()).unwrap();
            let to_id = ids.get(&edge.end_node_id()).unwrap();
            let id = g.graph.add_edge(*from_id, *to_id, props);
            let label = edge.typ();
            if label != INTERNAL_LABEL {
                let lid = g.edge_label.add_label(label.to_string());
                g.edge_label.add_label_mapping(&id, lid).unwrap();
            }
        }
        graphs.push(g);
    }
    graphs
}

pub fn get_source_graphs(label: &str) -> Vec<PropertyGraph> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let neograph = runtime
        .block_on(neo4rs::Graph::new("localhost:7687", "", ""))
        .unwrap();
    runtime.block_on(get_source_graphs_async(label, &neograph))
}

async fn add_label_async(label: &str, key: u64, conn: &Graph) {
    let query_str = format!(
        "
match (n {{{key}:$key}})
set n:{label};
        ",
        key = KEY_PROP,
        label = label
    );
    let query = query(&query_str).param("key", key as i64);
    conn.run(query).await.unwrap();
}

pub fn add_label(label: &str, key: u64) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let neograph = runtime
        .block_on(neo4rs::Graph::new("localhost:7687", "", ""))
        .unwrap();
    runtime.block_on(add_label_async(label, key, &neograph))
}

async fn compute_paths_async(
    source_label: &str,
    target_label: &str,
    operations_name: &str,
    conn: &Graph,
) {
    let path_query = format!(
        "
match p=shortest 1 (s:{source})-[:{meta}]-*(t:{target})
return p;
    ",
        source = source_label,
        meta = META_LABEL,
        target = target_label
    );

    let add_edge_query = format!("
match (s {{{key}:$key_source}}), (t {{{key}:$key_target}})
create (s)-[:{path} {{{ops}:$ops}}]->(t);
    ",key=KEY_PROP, ops=operations_name, path=PATH_LABEL);
    
    let mut paths = conn.execute(query(&path_query)).await.unwrap();
    while let Some(row) = paths.next().await.unwrap() {
        let path: Path = row.get("p").unwrap();
        let nodes = path.nodes();
        let ops: Vec<String> = path
            .rels()
            .iter()
            .flat_map(|rel| {
                rel.get::<Vec<String>>(operations_name)
                    .unwrap()
                    .into_iter()
            })
            .collect();
        let first_key = nodes.first().map(|n| n.get::<i64>("key").unwrap()).unwrap();
        let last_key = nodes.last().map(|n| n.get::<i64>("key").unwrap()).unwrap();
        let query = query(&add_edge_query)
            .param("key_source", first_key)
            .param("key_target", last_key)
            .param("ops", ops);
        conn.run(query).await.unwrap();
    }
}

pub fn compute_paths(source_label: &str, target_label: &str, operations_name: &str) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let neograph = runtime
        .block_on(neo4rs::Graph::new("localhost:7687", "", ""))
        .unwrap();
    runtime.block_on(compute_paths_async(
        source_label,
        target_label,
        operations_name,
        &neograph,
    ))
}

#[cfg(test)]
mod tests {
    use crate::parsing::PropertyGraphParser;

    use super::*;

    #[test]
    fn get_graph_test() {
        get_source_graphs("Selected");
    }

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
