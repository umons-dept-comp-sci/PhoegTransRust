extern crate docopt;
extern crate graph;
extern crate rayon;
extern crate env_logger;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;

mod utils;
mod compute;
mod errors;
mod transformation;

use graph::Graph;
use graph::format::to_g6;
use graph::transfos;
use graph::invariant;
use std::fs::File;
use std::io::{stdin, BufRead, BufReader};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::sync::Arc;
use docopt::Docopt;

use utils::*;
use compute::*;
use errors::*;
use transformation::*;

const USAGE: &str =
    "
    Transrust is a tool to compute the results of different transformations on a given set \
     of
    graphs. These graphs have to be given in graph6 format from the input (one signature \
     per line)
    and the result is outputed in csv format.

    Usage:
        transrust [-v] \
     [-i <input>] [-o <output>] [-b <batch>] [-s <buffer>] -t <transformation>... -f <filter>...
        \
     transrust --help

    Options:
        -h, --help             Show this message.
        -v, \
     --verbose          Shows more information.
        -i, --input <input>    File containing \
     the graph6 signatures. Uses the standard input if '-'.
                               \
     [default: -]
        -o, --output <output>  File where to write the result. Uses the \
     standard output if '-'
                               [default: -]
        -b, --batch \
     <batch>    Batch size [default: 1000000]
        -s, --buffer <buffer>  Size of the buffer \
     [default: 2000000000]
        -t <transformation>    The transformations to computes for the \
     graphs.
        -f <filter>            The filters to apply to the results of the \
     transformations.
";

#[derive(Debug, Deserialize, Clone)]
struct Args {
    flag_v: bool,
    flag_i: String,
    flag_o: String,
    flag_b: usize,
    flag_s: usize,
    flag_t: Vec<String>,
    flag_f: Vec<String>,
}

fn get_transfo(s: &String) -> Result<Box<Fn(&Graph) -> Vec<Graph>>, String> {
    match s.as_str() {
        "rotation" => Ok(Box::new(move |ref x| transfos::rotation(&x))),
        "add_edge" => Ok(Box::new(move |ref x| transfos::add_edge(&x))),
        "remove_edge" => Ok(Box::new(move |ref x| transfos::remove_edge(&x))),
        _ => Err(format!("Transformation '{}' not defined.", s)),
    }
}

fn transfotmp(g: &Graph) -> Vec<Graph> {
    let mut res = Vec::new();
    res.append(&mut transfos::remove_edge(g));
    res.append(&mut transfos::add_edge(g));
    res.append(&mut transfos::rotation(g));
    res.append(&mut transfos::move_distinct(g));
    res.append(&mut transfos::detour(g));
    res.append(&mut transfos::shortcut(g));
    res.append(&mut transfos::two_opt(g));
    res
}

fn run() -> Result<(), TransProofError> {
    // Parsing args
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());
    let filename = args.flag_i;
    let outfilename = args.flag_o;
    let batch = args.flag_b;
    let buffer = args.flag_s;
    let verbose = args.flag_v;

    // Init logger
    let debug_level = if verbose { "debug" } else { "info" };
    let env = env_logger::Env::default().filter_or("RUST_LOG", debug_level);
    let mut builder = env_logger::Builder::from_env(env);
    if !verbose {
        builder.default_format_module_path(false);
    }
    builder.init();

    // Init filters
    let contest =
        |ref x: &Graph| -> Result<String, ()> { as_filter(invariant::is_connected, to_g6)(&x) };
    let ftrs =
        Arc::new(|ref x: &Graph| -> Result<String, ()> { combine_filters(&contest, trash_node)(&x) });

    // Init input
    let mut buf: Box<BufRead> = match filename.as_str() {
        "-" => Box::new(BufReader::new(stdin())),
        _ => Box::new(BufReader::new(File::open(filename)?)),
    };

    // Init comunications with sink thread
    let (sender, receiver): (Sender<String>, Receiver<String>) = channel();
    let builder = thread::Builder::new();
    let whandle = builder.spawn(move || output(receiver, outfilename, buffer))?;

    let mut trsf = Transformation::from(|x: &Graph| transfos::move_distinct(x));

    let mut s = 1;
    let mut total = 0;
    let mut v;
    let mut res = Ok(());
    while s > 0 {
        v = read_graphs(&mut buf, batch);
        s = v.len();
        total += s;
        if s > 0 {
            info!("Loaded a batch of size {}", s);
            res = handle_graphs(v, sender.clone(), &trsf, ftrs.clone());
            if res.is_err() {
                break;
            }
            info!("Finished a batch of size {} ({} so far)", s, total);
        }
    }
    drop(sender);
    whandle.join()??;
    res?;
    Ok(())
}

fn main() {
    match run() {
        Ok(_) => (),
        Err(e) => error!("{}", e),
    }
}
