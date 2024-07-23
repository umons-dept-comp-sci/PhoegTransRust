pub mod compute;
mod errors;
mod graph_transformation;
mod parsing;
mod property_graph;
mod transformation;
mod utils;
mod neo4j;

use docopt::Docopt;
use log::{debug, error, warn};
use serde::Deserialize;
use std::convert::TryInto;
use std::fs::File;
use std::io::{stdin, BufRead, BufReader, Read};
use std::sync::mpsc::{channel, sync_channel};
use std::sync::Arc;
use std::thread;

use compute::*;
use errors::*;
use transformation::*;
use utils::*;

use crate::graph_transformation::GraphTransformation;
use crate::parsing::PropertyGraphParser;
use crate::property_graph::PropertyGraph;

// (-f <filter>)...
// -f <filter>            The filters \
// to apply to the results of the transformations.
// t <transformation>    The transformations to computes for the \
// graphs.
const USAGE: &str = "
Transrust is a tool to compute the results of different transformations on a given set of graphs.
These graphs have to be given in graph6 format from the input (one signature per line) and the
result is outputed in csv format.

Usage:
    transrust [options] <program> <transformations>...
    transrust (-h | --help)
    transrust <program> --transfos

Options:
    -h, --help             Show this message.
    -v, --verbose          Shows more information.
    --transfos             Shows a list of available transformations.
    -i, --input <input>    File containing the input schemas. Uses the standard input if '-'.
                           [default: -]
    -o, --output <output>  File where to write the result. Uses the standard output if '-'.
                           [default: -]
    -s, --buffer <buffer>  Size of the buffer [default: 2000000000]
    -t <threads>           Number of threads to be used for computation. A value of 0 means using
                           as many threads cores on the machine. [default: 0]
    -c <channel>           Size of the buffer to use for each threads (in number of messages). If
                           the size is 0, the buffer is unlimited. Use this if you have memory
                           issues even while setting a smaller output buffer and batch size.
                           [default: 0]
    -a, --append           Does not overwrite output file but appends results instead.
    --neo4j                Writes the output in a Neo4j database. Incompatible with -o.
    --target <target>      File containing the target schema.
    ";

#[derive(Debug, Deserialize, Clone)]
struct Args {
    flag_v: bool,
    flag_transfos: bool,
    flag_i: String,
    flag_o: String,
    flag_s: usize,
    arg_program: String,
    arg_transformations: Vec<String>,
    flag_t: usize,
    flag_c: usize,
    flag_append: bool,
    flag_neo4j : bool,
    flag_target: Option<String>,
}


fn main() -> Result<(), TransProofError> {
    // Parsing args
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());
    let verbose = args.flag_v;

    let prog = souffle::create_program_instance(&args.arg_program);
    let mut transfos : Vec<&str> = vec![];
    if prog.is_null() {
        error!("Unknown program: {}", args.arg_program);
        panic!("Unknown program: {}", args.arg_program);
    } else {
        if args.flag_transfos {
            if let Some(transfos) = souffle::get_transfos(prog) {
                for transfo in transfos {
                    println!("{}", transfo);
                }
                std::process::exit(0);
            } else {
                error!("No relation Transformation found.");
                std::process::exit(1);
            }
        } else {
            for transfo in args.arg_transformations.iter() {
                if souffle::has_relation(prog, transfo) {
                    transfos.push(transfo);
                } else {
                    warn!("No relation named {}.", transfo);
                }
            }
        }
        souffle::free_program(prog);
    }

    if transfos.is_empty() {
        error!("No transformation found.");
        panic!("No transformation found.");
    }

    // Init logger
    let debug_level = if verbose { "debug" } else { "info" };
    let env = env_logger::Env::default().filter_or("RUST_LOG", debug_level);
    let mut builder = env_logger::Builder::from_env(env);
    if !verbose {
        builder.default_format_module_path(false);
    }
    builder.init();
    debug!("{:?}", args);

    let filename = args.flag_i;
    let outfilename = args.flag_o;
    let buffer = args.flag_s;
    let num_threads = args.flag_t;
    let channel_size = args.flag_c;
    let append = args.flag_append;
    let program = args.arg_program;
    let neo4j = args.flag_neo4j;
    let target_graph: Option<PropertyGraph> = args.flag_target.map(|fname| -> Result<PropertyGraph, std::io::Error> {
        let mut buf = BufReader::new(File::open(fname)?);
        let mut text = String::new();
        buf.read_to_string(&mut text)?;
        let parser = PropertyGraphParser;
        let mut v = parser.convert_text(&text);
        if v.len() != 1 {
            error!("Only one target schema is supported. Found {}.", v.len());
            panic!("Only one target schema is supported. Found {}.", v.len());
        }
        let target = v.drain(0..1).next().unwrap();
        Ok(target)
    }).transpose().unwrap();

    if (outfilename != "-" || append) && neo4j {
        error!("Option --neo4j is not compatible with -o or -a.");
        panic!("Option --neo4j is not compatible with -o or -a.");
    }

    // Init filters
    let deftest = Arc::new(|ref x: &GraphTransformation| -> Result<String, ()> {
        as_filter(|_| true, |_| "".to_string())(&x)
    });
    //let ftrs = Arc::new(|ref x: &GraphTransformation| -> Result<String, ()> {
    //combine_filters(&deftest, trash_node)(&x)
    //});

    // Init input
    let mut buf: Box<dyn BufRead> = match filename.as_str() {
        "-" => Box::new(BufReader::new(stdin())),
        _ => Box::new(BufReader::new(File::open(filename)?)),
    };

    // Init thread pool
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build_global()?;

    // Init comunications with sink thread
    let result_sender;
    let result_receiver;
    if channel_size == 0 {
        let result_chan = channel::<LogInfo>();
        result_sender = SenderVariant::from(result_chan.0);
        result_receiver = result_chan.1;
    } else {
        let result_chan = sync_channel::<LogInfo>(channel_size);
        result_sender = SenderVariant::from(result_chan.0);
        result_receiver = result_chan.1;
    }
    let builder = thread::Builder::new();
    let whandle;
    if neo4j {
        whandle = builder.spawn(move || output_neo4j(result_receiver))?;
    } else {
        whandle = builder.spawn(move || output(result_receiver, outfilename, buffer, append))?;
    }

    let v;
    let parser = PropertyGraphParser;
    let mut text = String::new();
    buf.read_to_string(&mut text)?;
    v = parser.convert_text(&text);
    if !v.is_empty() {
        handle_graphs(&program, v, result_sender.clone(), &transfos, deftest.clone())?;
    }
    drop(result_sender);
    whandle.join().map_err(|x| TransProofError::Thread(x))??;
    Ok(())
}
