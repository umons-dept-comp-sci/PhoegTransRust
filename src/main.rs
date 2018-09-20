extern crate docopt;
extern crate graph;
extern crate rayon;
extern crate env_logger;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;

mod utils;
mod compute;
mod errors;
mod transformation;

use graph::transfos::TransfoResult;
// use graph::invariant;
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

// (-f <filter>)...
// -f <filter>            The filters \
// to apply to the results of the transformations.
// t <transformation>    The transformations to computes for the \
// graphs.
const USAGE: &str =
    "
Transrust is a tool to compute the results of different transformations on a
given set of \
     graphs. These graphs have to be given in graph6 format from the
input (one signature per \
     line) and the result is outputed in csv format.

Usage:
    transrust [-v | --verbose] [-i \
     <input>] [-o <output>] [-b <batch>] [-s <buffer>] [-t <threads> | -m] <transformations>...
    \
     transrust (-h | --help)
    transrust --transfos

Options:
    -h, --help             Show \
     this message.
    -v, --verbose          Shows more information.
    --transfos             \
     Shows a list of available transformations.
    -i, --input <input>    File containing the \
     graph6 signatures. Uses the standard input if '-'. [default: -]
    -o, --output <output>  \
     File where to write the result. Uses the standard output if '-' [default: -]
    -b, --batch \
     <batch>    Batch size [default: 1000000]
    -s, --buffer <buffer>  Size of the buffer \
     [default: 2000000000]
    -t <threads>           Number of threads to be used for \
     computation. A value of 0 means using as many threads cores on the machine. [default: 0]";

#[derive(Debug, Deserialize, Clone)]
struct Args {
    flag_v: bool,
    flag_transfos: bool,
    flag_i: String,
    flag_o: String,
    flag_b: usize,
    flag_s: usize,
    arg_transformations: Vec<String>,
    flag_t: usize,
    flag_m: bool,
}

fn init_transfo(lst: &Vec<String>) -> Option<Transformation> {
    if lst.is_empty() {
        return None;
    }
    let mut transfo = Transformation::from_name(&lst[0]);
    let mut i = 1;
    while transfo.is_none() && i < lst.len() {
        warn!("Unknown transformation : {}.", lst[i - 1]);
        transfo = Transformation::from_name(&lst[i]);
        i += 1;
    }
    if transfo.is_some() {
        let mut ttrs;
        while i < lst.len() {
            ttrs = Transformation::from_name(&lst[i]);
            if ttrs.is_some() {
                match transfo.as_mut() {
                    Some(t) => *t += ttrs.unwrap(),
                    None => panic!("Should not happen."),
                }
            } else {
                warn!("Unknown transformation : {}", lst[i]);
            }
            i += 1;
        }
    }
    transfo
}

fn main() -> Result<(), TransProofError> {
    // Parsing args
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());
    let verbose = args.flag_v;

    if args.flag_transfos {
        print_transfos();
        std::process::exit(0);
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
    let batch = args.flag_b;
    let buffer = args.flag_s;
    let transfos = args.arg_transformations;
    let num_threads = args.flag_t;

    // Init filters
    let deftest = |ref x: &TransfoResult| -> Result<String, ()> { as_filter(|_| true, |x| format!("{}",x))(&x) };
    let ftrs =
        Arc::new(|ref x: &TransfoResult| -> Result<String, ()> { combine_filters(&deftest, trash_node)(&x) });

    // Init input
    let mut buf: Box<BufRead> = match filename.as_str() {
        "-" => Box::new(BufReader::new(stdin())),
        _ => Box::new(BufReader::new(File::open(filename)?)),
    };

    // Init thread pool
    rayon::ThreadPoolBuilder::new().num_threads(num_threads).build_global()?;

    // Init comunications with sink thread
    let (sender, receiver): (Sender<String>, Receiver<String>) = channel();
    let builder = thread::Builder::new();
    let whandle = builder.spawn(move || output(receiver, outfilename, buffer))?;

    // Init transformations
    let trs = init_transfo(&transfos);
    if trs.is_none() {
        error!("No transformation found.");
        panic!("No transformation found.");
    }
    let trs = trs.unwrap();

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
            res = handle_graphs(v, sender.clone(), &trs, ftrs.clone());
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
