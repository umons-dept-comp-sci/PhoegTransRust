use crate::errors::*;
use crate::transformation::*;
use crate::utils::plural;
use graph::format::from_g6;
use graph::nauty::canon_graph;
use graph::transfo_result::GraphTransformation;
use graph::GraphNauty;
use log::{info, warn};
use rayon::prelude::*;
use redis::Commands;
use std::convert::From;
use std::fs::OpenOptions;
use std::io::{stdout, BufRead, BufWriter, Write};
use std::sync::mpsc::{Receiver, SendError, Sender, SyncSender};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

pub fn apply_filters<F>(g: &GraphTransformation, ftrs: Arc<F>) -> Result<String, ()>
where
    F: Fn(&GraphTransformation) -> Result<String, ()>,
{
    ftrs(g)
}

/// Applying transformations to the graph g.
pub fn apply_transfos<T>(g: &GraphNauty, trs: &T) -> Vec<GraphTransformation>
where
    T: Transformation,
{
    let mut r = trs.apply(&g);
    for rg in r.iter_mut() {
        rg.canon();
    }
    r
}

/// Should apply a set of transformations, filter the graphs and return the result
pub fn handle_graph<T, F>(
    g: GraphNauty,
    t: &mut SenderVariant<LogInfo>,
    trsf: &T,
    ftrs: Arc<F>,
    filter: bool,
    red_con: &mut Arc<Mutex<redis::Connection>>,
) -> Result<(), TransProofError>
where
    T: Transformation,
    F: Fn(&GraphTransformation) -> Result<String, ()>,
{
    let mut r = apply_transfos(&g, trsf);
    if filter {
        if !r.is_empty() {
            let mut sig = format!("{}", g);
            let tot_trans = r.len();
            let mut pipe = redis::pipe();
            pipe.hget(&sig[&sig.len() - 2..], &sig);
            let mut fg;
            for res in r.iter_mut() {
                fg = canon_graph(&res.final_graph());
                sig = format!("{}", fg.0);
                pipe.hget(&sig[&sig.len() - 2..], &sig);
            }
            let vals: Vec<f64> = pipe.query(&mut *red_con.lock().unwrap()).unwrap();
            let filtered = r
                .iter()
                .enumerate()
                .filter(|(id, _)| vals[0] <= vals[id + 1])
                .collect::<Vec<_>>();
            if filtered.len() == tot_trans {
                t.send(LogInfo::LocalExtremum(g))?;
            } else {
                for (id, g) in filtered {
                    t.send(LogInfo::IncorrectTransfo {
                        result: g.clone(),
                        before: vals[0],
                        after: vals[id + 1],
                    })?
                }
            }
        }
    } else {
        for h in r {
            let s = apply_filters(&h, ftrs.clone());
            if let Ok(res) = s {
                t.send(LogInfo::Transfo(h))?;
            }
        }
    }
    Ok(())
}

/// Should apply a set of transformations, filter the graphs and return the result
pub fn handle_graphs<T, F>(
    v: Vec<GraphNauty>,
    t: SenderVariant<LogInfo>,
    trsf: &T,
    ftrs: Arc<F>,
    filter: bool,
    red_client: &redis::Client,
) -> Result<(), TransProofError>
where
    T: Transformation,
    F: Fn(&GraphTransformation) -> Result<String, ()> + Send + Sync,
{
    let red_con = Arc::new(Mutex::new(red_client.get_connection().unwrap()));
    v.into_par_iter().try_for_each_with((t, red_con), |s, x| {
        handle_graph(x, &mut s.0, trsf, ftrs.clone(), filter, &mut s.1)
    })?;
    Ok(())
}

/// Read files of graphs
/// (file of sigs)
pub fn read_graphs<F>(rdr: &mut F, batchsize: usize) -> Vec<GraphNauty>
where
    F: BufRead,
{
    let mut t = Vec::with_capacity(batchsize);
    for l in rdr.lines().by_ref().take(batchsize) {
        match l {
            Ok(sig) => match from_g6(&sig) {
                Ok(g) => {
                    t.push(g);
                }
                Err(e) => {
                    warn!("Wrong input : {}", e);
                }
            },
            Err(e) => {
                warn!("{}", e);
            }
        }
    }
    t
}

#[derive(Debug)]
pub enum LogInfo {
    Transfo(GraphTransformation),
    IncorrectTransfo {
        result: GraphTransformation,
        before: f64,
        after: f64,
    },
    LocalExtremum(GraphNauty),
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
            LogInfo::Transfo(t) => {
                i += 1;
                bufout.write_all(&format!("{}\n", t.tocsv()).into_bytes())?;
            }
            LogInfo::IncorrectTransfo {
                result: g,
                before: v1,
                after: v2,
            } => {
                i += 1;
                bufout.write_all(&format!("{}", g.tocsv()).into_bytes())?;
                bufout.write_all(&format!(",{},{}\n",v1,v2).into_bytes())?;
            }
            LogInfo::LocalExtremum(g) => {
                bufout.write_all(&format!("{}\n", g).into_bytes())?;
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
