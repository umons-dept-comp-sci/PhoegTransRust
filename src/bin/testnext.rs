use transproof::{parsing::PropertyGraphParser, transformation::{souffle::{create_program_instance, generate_operation_trees}, transform_graph}};

fn main() {
    let text = "CREATE GRAPH TYPE fraudGraphType {
( personType : Person { name STRING , birthday DATE }) ,
( customerType : Person & Customer { name STRING , since DATE }) ,
( suspiciousType : Suspicious { reason STRING }) ,
( : customerType )
-[ friendType : Knows & Likes {time INT} ] ->
( : customerType ),
( : suspiciousType )
-[ aliasType {frequency INT} ] ->
( : suspiciousType )
}
create graph type grantGraphType {
( donatorType : Person { name STRING } ),
( projectType { name STRING, budget INT, duration INT } ),
( grantType {duration STRING } ),
( leaderType : Person { name STRING } ),
( teamType { name STRING } ),
( : donatorType )
- [ donatesType : Credit { amount INT } ] ->
( : projectType ),
( : projectType )
- [ offersType { percent INT } ] ->
( : grantType ),
( : grantType )
- [ grantedType ] ->
( : teamType ),
( : leaderType )
- [ spendsType : Debit { amount INT, reason STRING } ] ->
( : grantType ),
( : leaderType )
- [ managesType ] ->
( : teamType )
}
";
    let parser = PropertyGraphParser;
    let mut results = parser.convert_text(text);
    let target = results.pop().unwrap();
    let source = results.pop().unwrap();
    let program = create_program_instance("add_vertex");
    let transfos = transform_graph(program, &vec!["AddVertexLabel"], &source, &Some(target));
    for transfo in transfos {
        println!("{}", transfo);
    }
}
