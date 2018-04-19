extern crate docopt;
extern crate graph;
extern crate rayon;
#[macro_use]
extern crate serde_derive;

mod utils;
mod compute;

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

const USAGE: &'static str = "
    Transrust is a tool to compute the results of different transformations on a given set of
    graphs. These graphs have to be given in graph6 format from the input (one signature per line)
    and the result is outputed in csv format.

    Usage:
        transrust [-i <input>] [-o <output>] [-b <batch>] [-s <buffer>] [-k <k>] [-c] [-n]
        transrust --help

    Options:
        -h, --help             Show this message.
        -i, --input <input>    File containing the graph6 signatures. Uses the standard input if '-'.
                               [default: -]
        -o, --output <output>  File where to write the result. Uses the standard output if '-'
                               [default: -]
        -b, --batch <batch>    Batch size [default: 1000000]
        -s, --buffer <buffer>  Size of the buffer [default: 2000000000]
        -k <k>                 Maximal number of edge removal. -1 if no bound. [default: -1]
        -c                     Disables the output of improvements.
        -n                     Disables the output of graphs that were not improved.
";
//transrust [-i <input>] [-o <output>] [-b <batch>] [-s <buffer>] -t <transformation>... -f <filter>...
//-t <transformation>    The transformations to computes for the graphs.
//-f <filter>            The filters to apply to the results of the transformations.

#[derive(Debug, Deserialize)]
struct Args {
    flag_i: String,
    flag_o: String,
    flag_b: usize,
    flag_s: usize,
    flag_k: isize,
    flag_c: bool,
    flag_n: bool,
    //flag_t: Vec<String>,
    //flag_f: Vec<String>,
}

fn get_transfo(s: &String) -> Result<Box<Fn(&Graph) -> Vec<Graph>>, String> {
    match s.as_str() {
        "rotation" => Ok(Box::new(move |ref x| transfos::rotation(&x))),
        "add_edge" => Ok(Box::new(move |ref x| transfos::add_edge(&x))),
        "remove_edge" => Ok(Box::new(move |ref x| transfos::remove_edge(&x))),
        _ => Err(format!("Transformation '{}' not defined.", s)),
    }
}

#[allow(dead_code)]
fn mindiam(x: &Graph) -> f64 {
    let lambda = 0.5;
    let i = match invariant::diameter(&x) {
        invariant::Distance::Val(i) => -(i as f64),
        invariant::Distance::Inf => -(x.order() as f64),
    };
    i - lambda * (invariant::connected_components(&x).len() - 1) as f64
}

fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());
    println!("{:?}", args);
    let filename = args.flag_i;
    let outfilename = args.flag_o;
    let batch = args.flag_b;
    let buffer = args.flag_s;
    let k = args.flag_k;
    let c = args.flag_c;
    let n = args.flag_n;

    let mut buf: Box<BufRead> = match filename.as_str() {
        "-" => Box::new(BufReader::new(stdin())),
        _ => Box::new(BufReader::new(
            File::open(filename).expect("Could not open file"),
        )),
    };
    let (sender, receiver) = channel();
    let whandle = thread::spawn(move || output_search(receiver, outfilename, buffer, !c, !n));

    fn maxirreg2(x: &Graph) -> Box<Fn(&Graph) -> f64> {
        let m = x.size() as f64;
        Box::new(move |g: &Graph| -> f64 {
            let lambda = g.order() as f64;
            let i = invariant::irregularity(&g) as f64;
            i - lambda * ((m - g.size() as f64).abs()) as f64
        })
    };

    fn maxirreg(x: &Graph) -> f64 {
        let m = x.size() as f64;
        let lambda = x.order() as f64;
        let i = invariant::irregularity(&x) as f64;
        i - lambda * ((m - x.size() as f64).abs()) as f64
    };

    fn maxirregclass2(x: &Graph) -> Box<Fn(&Graph) -> bool> {
        let m = x.size();
        Box::new(move |g: &Graph| -> bool { g.size() == m })
    }

    fn maxirregclass(x: &Graph) -> bool {
        x.size() == 20
    }

    let mut s = 1;
    let mut total = 0;
    let mut v;

    while s > 0 {
        v = read_graphs(&mut buf, batch);
        s = v.len();
        total += s;
        if s > 0 {
            eprintln!("Loaded a batch of size {}", s);
            eprintln!("num threads : {}", rayon::current_num_threads());
            //handle_graphs(v, sender.clone(), trsf.clone(), ftrs.clone());
            search_transfo_all(
                v,
                Arc::new(maxirreg2),
                Arc::new(maxirregclass2),
                sender.clone(),
                k,
            );
            eprintln!("Finished a batch of size {} ({} so far)", s, total);
        }
    }
    drop(sender);
    whandle.join().expect("Could not join thread");
}

fn main2() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());
    println!("{:?}", args);
    let filename = args.flag_i;
    let outfilename = args.flag_o;
    let batch = args.flag_b;
    let buffer = args.flag_s;
    //let mut trsf = get_transfo(&args.flag_t[0]).unwrap_or_else(|x| panic!(x));
    //for t in args.flag_t.iter().skip(1) {
    //trsf = combine_transfos(*trsf, *get_transfo(t).unwrap_or_else(|x| panic!(x)));
    //}
    let trsf = Arc::new(|ref x: &Graph| -> Vec<Graph> {
        combine_transfos(transfos::add_edge, transfos::remove_edge)(&x)
    });
    let contest =
        |ref x: &Graph| -> Result<String, ()> { as_filter(invariant::is_connected, to_g6)(&x) };
    let ftrs = Arc::new(|ref x: &Graph| -> Result<String, ()> {
        combine_filters(&contest, trash_node)(&x)
    });

    let mut buf: Box<BufRead> = match filename.as_str() {
        "-" => Box::new(BufReader::new(stdin())),
        _ => Box::new(BufReader::new(
            File::open(filename).expect("Could not open file"),
        )),
    };
    let (sender, receiver): (Sender<String>, Receiver<String>) = channel();
    let whandle = thread::spawn(move || output(receiver, outfilename, buffer));

    let mut s = 1;
    let mut total = 0;
    let mut v;

    while s > 0 {
        v = read_graphs(&mut buf, batch);
        s = v.len();
        total += s;
        if s > 0 {
            eprintln!("Loaded a batch of size {}", s);
            handle_graphs(v, sender.clone(), trsf.clone(), ftrs.clone());
            eprintln!("Finished a batch of size {} ({} so far)", s, total);
        }
    }
    drop(sender);
    whandle.join().expect("Could not join thread");
}
