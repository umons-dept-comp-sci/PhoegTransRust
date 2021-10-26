pub mod compute;
mod errors;
mod transformation;
mod utils;

use graph::transfo_result::GraphTransformation;
use graph::GraphNauty;
// use graph::invariant;
use docopt::Docopt;
use log::{debug, info, warn, error};
use serde::Deserialize;
use std::fs::File;
use std::io::{stdin, BufRead, BufReader};
use std::sync::mpsc::{channel, sync_channel};
use std::sync::Arc;
use std::thread;
use std::convert::TryInto;

use compute::*;
use errors::*;
use transformation::*;
use utils::*;

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
    transrust [options] remove <e>
    transrust [options] <transformations>...
    transrust (-h | --help)
    transrust --transfos

Options:
    -h, --help             Show this message.
    -v, --verbose          Shows more information.
    --transfos             Shows a list of available transformations.
    -i, --input <input>    File containing the graph6 signatures. Uses the standard input if '-'.
                           [default: -]
    -o, --output <output>  File where to write the result. Uses the standard output if '-'.
                           [default: -]
    -b, --batch <batch>    Batch size [default: 1000000]
    -s, --buffer <buffer>  Size of the buffer [default: 2000000000]
    -t <threads>           Number of threads to be used for computation. A value of 0 means using
                           as many threads cores on the machine. [default: 0]
    -c <channel>           Size of the buffer to use for each threads (in number of messages). If
                           the size is 0, the buffer is unlimited. Use this if you have memory
                           issues even while setting a smaller output buffer and batch size.
                           [default: 0]
    -a, --append           Does not overwrite output file but appends results instead.
    -f, --filter           Only outputs incorrect transfos.
    --postgres             Format as a csv ready to import in a postgresql table.
    ";

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
    flag_c: usize,
    flag_append: bool,
    cmd_remove: bool,
    arg_e: Option<u64>,
    flag_f: bool,
    flag_postgres: bool,
}

fn init_transfo(lst: &[String]) -> TransfoVec {
    lst.iter().map(|x| x.as_str().try_into()).inspect(|res| {
        if let Err(e) = res {
            warn!("{}", e);
        }
    })
    .filter_map(Result::ok)
    .collect()
    //if lst.is_empty() {
        //return Vec::new();
    //}
    //let mut transfo = Transformation::from_name(&lst[0]);
    //let mut i = 1;
    //while transfo.is_none() && i < lst.len() {
        //warn!("Unknown transformation : {}.", lst[i - 1]);
        //transfo = Transformation::from_name(&lst[i]);
        //i += 1;
    //}
    //if transfo.is_some() {
        //let mut ttrs;
        //while i < lst.len() {
            //ttrs = Transformation::from_name(&lst[i]);
            //if let Some(ttrs_val) = ttrs {
                //match transfo.as_mut() {
                    //Some(t) => *t += ttrs_val,
                    //None => panic!("Should not happen."),
                //}
            //} else {
                //warn!("Unknown transformation : {}", lst[i]);
            //}
            //i += 1;
        //}
    //}
    //transfo
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
    let channel_size = args.flag_c;
    let append = args.flag_append;
    let cmd_remove = args.cmd_remove;
    let arg_e = args.arg_e;
    let flag_f = args.flag_f;
    let flag_postgres = args.flag_postgres;

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
    let sender;
    let receiver;
    if channel_size == 0 {
        let chan = channel::<LogInfo>();
        sender = SenderVariant::from(chan.0);
        receiver = chan.1;
    } else {
        let chan = sync_channel::<LogInfo>(channel_size);
        sender = SenderVariant::from(chan.0);
        receiver = chan.1;
    }
    let builder = thread::Builder::new();
    let whandle = builder.spawn(move || output(receiver, outfilename, buffer, append))?;

    // Init transformations
    let trs: TransfoVec = if !cmd_remove {
        let trs = init_transfo(&transfos);
        if trs.is_empty() {
            error!("No transformation found.");
            panic!("No transformation found.");
        }
        trs
    } else {
        let mut res: TransfoVec = Vec::new();
        res.push(Box::new(move |g: &GraphNauty| graph::transfos::remove_num_edges(g, arg_e.unwrap())));
        res
    };

    let red_client = redis::Client::open("redis://127.0.0.1/").expect("Could not connect to redis.");

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
            res = handle_graphs(v, sender.clone(), &trs, deftest.clone(), flag_f, &red_client, flag_postgres);
            if res.is_err() {
                break;
            }
            info!("Finished a batch of size {} ({} so far)", s, total);
        }
    }
    drop(sender);
    whandle.join().map_err(|x| TransProofError::Thread(x))??;
    res?;
    Ok(())
}
