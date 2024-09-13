use crate::errors::*;
use crate::graph_transformation::GraphTransformation;
use crate::neo4j::write_graph_transformation;
use crate::property_graph::PropertyGraph;
use crate::similarity::property_graph_minhash;
use crate::transformation::*;
use crate::utils::plural;
use log::info;
use probminhash::jaccard::compute_probminhash_jaccard;
use rayon::prelude::*;
use std::collections::{BinaryHeap, HashSet};
use std::convert::From;
use std::fmt::{Debug, Display};
use std::fs::OpenOptions;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{stdout, BufWriter, Write};
use std::sync::mpsc::{Receiver, SendError, Sender, SyncSender};
use std::sync::Arc;
use std::time::Instant;

use self::souffle::{create_program_instance, Program};

const NUM_BEST: usize = 5;
const EPS: f64 = 1e-12;
pub struct SimGraph(f64, u64, GraphTransformation);

impl PartialEq for SimGraph {
    fn eq(&self, other: &Self) -> bool {
        (self.0 - other.0).abs() < EPS && self.1 == other.1
    }
}

impl PartialOrd for SimGraph {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl Eq for SimGraph {

}

impl Ord for SimGraph {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.0 - other.0 {
            x if x < -EPS => {
                std::cmp::Ordering::Greater
            },
            x if x > EPS => {
                std::cmp::Ordering::Less
            },
            _ => {
                self.1.cmp(&other.1)
            }
        }
    }
}

impl Debug for SimGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.2, f)
    }
}

impl Display for SimGraph{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.2, f)
    }
}

pub fn apply_filters<F>(g: &GraphTransformation, ftrs: Arc<F>) -> Result<String, ()>
where
    F: Fn(&GraphTransformation) -> Result<String, ()>,
{
    ftrs(g)
}

/// Should apply a set of transformations, filter the graphs and return the result
pub fn handle_graph<F>(
    program: Program,
    g: PropertyGraph,
    t: &mut SenderVariant<LogInfo>,
    trsf: &Vec<&str>,
    ftrs: Arc<F>,
    target_graph: &Option<PropertyGraph>,
) -> Result<(), TransProofError>
where
    F: Fn(&GraphTransformation) -> Result<String, ()>,
{
    let target_hash = target_graph.as_ref().map(|g| property_graph_minhash(&g));
    let r = apply_transformations(program, trsf, &g, target_graph);
    let mut bests = BinaryHeap::with_capacity(NUM_BEST+1);
    let mut stored = HashSet::with_capacity(NUM_BEST+1);
    for h in r {
        let s = apply_filters(&h, ftrs.clone());
        if let Ok(_res) = s {
            let mut hash = DefaultHasher::new();
            h.result.hash(&mut hash);
            let key = hash.finish();
            if let Some(target_hash) = target_hash.as_ref() {
                if !stored.contains(&key) {
                    stored.insert(key.clone());
                    let g_hash = property_graph_minhash(&h.result);
                    let sim = compute_probminhash_jaccard(&target_hash, &g_hash);
                    bests.push(SimGraph(sim,key,h));
                    if bests.len() > NUM_BEST {
                        let removed = bests.pop().unwrap();
                        stored.remove(&removed.1);
                    }
                }
            } else {
                t.send(LogInfo::Transfo(h, "".to_string()))?;
            }
        }
    }
    for transfo in bests {
        t.send(LogInfo::TransfoSim(transfo, "".to_string()))?;
    }
    Ok(())
}

/// Should apply a set of transformations, filter the graphs and return the result
pub fn handle_graphs<F>(
    program_name: &str,
    v: Vec<PropertyGraph>,
    t: SenderVariant<LogInfo>,
    trsf: &Vec<&str>,
    ftrs: Arc<F>,
    target_graph: Option<PropertyGraph>,
) -> Result<(), TransProofError>
where
    F: Fn(&GraphTransformation) -> Result<String, ()> + Send + Sync,
{
    let init = || {
        let t = t.clone();
        let prog = create_program_instance(program_name);
        (t, prog)
    };
    v.into_par_iter().try_for_each_init(init, |mut s, x| {
        handle_graph(s.1, x, &mut s.0, trsf, ftrs.clone(), &target_graph)
    })?;
    Ok(())
}

#[derive(Debug)]
pub enum LogInfo {
    Transfo(GraphTransformation, String),
    TransfoSim(SimGraph, String),
    IncorrectTransfo {
        result: GraphTransformation,
        before: f64,
        after: f64,
    },
    LocalExtremum(PropertyGraph),
}

fn store_property_graph(g: &PropertyGraph, db: &neo4rs::Graph, rt: &tokio::runtime::Runtime) {
    let tx = rt.block_on(db.start_txn()).unwrap();
}

pub fn output_neo4j(
    receiver: Receiver<LogInfo>, first_run: bool
) -> Result<(Option<f64>, Option<u64>), TransProofError> {
    //TODO remove the unwraps
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
    let neograph = runtime.block_on(neo4rs::Graph::new("localhost:7687", "", "")).unwrap();
    let mut best_key = None;
    let mut best_sim = None;
    let start = Instant::now();
    let mut i = 0;
    for log in receiver.iter() {
        match log {
            LogInfo::Transfo(t, _) => {
                i += 1;
                runtime.block_on(write_graph_transformation(&t, first_run, &neograph));
                // bufout.write_all(&format!("{}", t).into_bytes())?;
                // bufout.write_all(&s.into_bytes())?;
                // bufout.write_all(&['\n' as u8])?;
            }
            LogInfo::TransfoSim(t, _) => {
                i += 1;
                runtime.block_on(write_graph_transformation(&t.2, first_run, &neograph));
                if best_sim.map(|bsim| bsim < t.0).unwrap_or(true) {
                    best_sim = Some(t.0);
                    best_key = Some(t.1);
                }
                // bufout.write_all(&format!("{}", t).into_bytes())?;
                // bufout.write_all(&s.into_bytes())?;
                // bufout.write_all(&['\n' as u8])?;
            }
            LogInfo::IncorrectTransfo {
                result: _,
                before: _,
                after: _,
            } => {
                i += 1;
                // bufout.write_all(&format!("{}", g).into_bytes())?;
                // bufout.write_all(&format!(",{},{}\n", v1, v2).into_bytes())?;
            }
            LogInfo::LocalExtremum(g) => {
                // bufout.write_all(&format!("{:?}\n", g).into_bytes())?;
            }
        }
    }
    let duration = start.elapsed();
    info!("Done : {} transformation{}", i, plural(i));
    let secs = duration.as_secs() as usize;
    let millis = (duration.subsec_nanos() as usize) / (1e6 as usize);
    info!(
        "Took {} second{} and {} millisecond{}",
        secs,
        plural(secs),
        millis,
        plural(millis)
    );
    Ok((best_sim, best_key))
}

pub fn output(
    receiver: Receiver<LogInfo>,
    filename: String,
    buffer: usize,
    append: bool,
) -> Result<(Option<f64>, Option<u64>), TransProofError> {
    let mut bufout: Box<dyn Write> = match filename.as_str() {
        "-" => Box::new(BufWriter::with_capacity(buffer, stdout())),
        _ => Box::new(BufWriter::with_capacity(
            buffer,
            OpenOptions::new()
                .write(true)
                .append(append)
                .create(true)
                .open(filename)?,
        )),
    };
    let mut best_key = None;
    let mut best_sim = None;
    let start = Instant::now();
    let mut i = 0;
    for log in receiver.iter() {
        match log {
            LogInfo::Transfo(t, s) => {
                i += 1;
                bufout.write_all(&format!("{}", t).into_bytes())?;
                bufout.write_all(&s.into_bytes())?;
                bufout.write_all(&['\n' as u8])?;
            }
            LogInfo::TransfoSim(t, s) => {
                i += 1;
                bufout.write_all(&format!("{}", t).into_bytes())?;
                bufout.write_all(&s.into_bytes())?;
                bufout.write_all(&['\n' as u8])?;
                if best_sim.map(|bsim| bsim < t.0).unwrap_or(true) {
                    best_sim = Some(t.0);
                    best_key = Some(t.1);
                }
            }
            LogInfo::IncorrectTransfo {
                result: g,
                before: v1,
                after: v2,
            } => {
                i += 1;
                bufout.write_all(&format!("{}", g).into_bytes())?;
                bufout.write_all(&format!(",{},{}\n", v1, v2).into_bytes())?;
            }
            LogInfo::LocalExtremum(g) => {
                bufout.write_all(&format!("{:?}\n", g).into_bytes())?;
            }
        }
    }
    let duration = start.elapsed();
    info!("Done : {} transformation{}", i, plural(i));
    let secs = duration.as_secs() as usize;
    let millis = (duration.subsec_nanos() as usize) / (1e6 as usize);
    info!(
        "Took {} second{} and {} millisecond{}",
        secs,
        plural(secs),
        millis,
        plural(millis)
    );
    Ok((best_sim, best_key))
}

//#[derive(Clone)]
pub enum SenderVariant<T>
where
    T: Send,
{
    UnlimitedSender(Sender<T>),
    LimitedSender(SyncSender<T>),
}

impl<T> SenderVariant<T>
where
    T: Send,
{
    fn send(&self, t: T) -> Result<(), SendError<T>> {
        match self {
            SenderVariant::UnlimitedSender(s) => s.send(t),
            SenderVariant::LimitedSender(s) => s.send(t),
        }
    }
}

impl<T> From<Sender<T>> for SenderVariant<T>
where
    T: Send,
{
    fn from(sender: Sender<T>) -> Self {
        SenderVariant::UnlimitedSender(sender)
    }
}

impl<T> From<SyncSender<T>> for SenderVariant<T>
where
    T: Send,
{
    fn from(sender: SyncSender<T>) -> Self {
        SenderVariant::LimitedSender(sender)
    }
}

impl<T> Clone for SenderVariant<T>
where
    T: Send,
{
    fn clone(&self) -> Self {
        match self {
            SenderVariant::UnlimitedSender(s) => SenderVariant::UnlimitedSender(s.clone()),
            SenderVariant::LimitedSender(s) => SenderVariant::LimitedSender(s.clone()),
        }
    }
}
