use std::fs::read_to_string;

use docopt::Docopt;
use serde::Deserialize;
use transproof::{parsing::PropertyGraphParser, similarity::jaccard_index};

const USAGE: &str = "
Converts from datalog to pgschema

Usage:
    apply_transfos <schema1> <schema2>
";

#[derive(Debug, Deserialize, Clone)]
struct Args {
    arg_schema1: String,
    arg_schema2: String,
}

fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    let parser = PropertyGraphParser;
    let s1 = parser.convert_text(&read_to_string(args.arg_schema1).unwrap()).pop().unwrap();
    let s2 = parser.convert_text(&read_to_string(args.arg_schema2).unwrap()).pop().unwrap();
    let sim = jaccard_index(&s1, &s2);
    println!("{sim}");
}
