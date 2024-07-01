use crate::errors::*;
use crate::graph_transformation::GraphTransformation;
use crate::neo4j::write_graph_transformation;
use crate::property_graph::PropertyGraph;
use crate::transformation::*;
use crate::utils::plural;
use log::info;
use rayon::prelude::*;
use std::convert::From;
use std::fs::OpenOptions;
use std::io::{stdout, BufWriter, Write};
use std::sync::mpsc::{Receiver, SendError, Sender, SyncSender};
use std::sync::Arc;
use std::time::Instant;

use self::souffle::{create_program_instance, Program};

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
) -> Result<(), TransProofError>
where
    F: Fn(&GraphTransformation) -> Result<String, ()>,
{
    let r = apply_transformations(program, trsf, &g);
    for h in r {
        let s = apply_filters(&h, ftrs.clone());
        if let Ok(_res) = s {
            t.send(LogInfo::Transfo(h, "".to_string()))?;
        }
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
        handle_graph(s.1, x, &mut s.0, trsf, ftrs.clone())
    })?;
    Ok(())
}

#[derive(Debug)]
pub enum LogInfo {
    Transfo(GraphTransformation, String),
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
    receiver: Receiver<LogInfo>,
) -> Result<(), TransProofError> {
    //TODO remove the unwraps
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
    let mut neograph = runtime.block_on(neo4rs::Graph::new("localhost:7687", "", "")).unwrap();
    let start = Instant::now();
    let mut i = 0;
    for log in receiver.iter() {
        match log {
            LogInfo::Transfo(t, s) => {
                i += 1;
                runtime.block_on(write_graph_transformation(&t, &neograph));
                // bufout.write_all(&format!("{}", t).into_bytes())?;
                // bufout.write_all(&s.into_bytes())?;
                // bufout.write_all(&['\n' as u8])?;
            }
            LogInfo::IncorrectTransfo {
                result: g,
                before: v1,
                after: v2,
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
    Ok(())
}

pub fn output(
    receiver: Receiver<LogInfo>,
    filename: String,
    buffer: usize,
    append: bool,
) -> Result<(), TransProofError> {
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
    Ok(())
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
