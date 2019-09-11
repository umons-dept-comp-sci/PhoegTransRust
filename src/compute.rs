use errors::*;
use graph::Graph;
use graph::format::from_g6;
use graph::transfo_result::GraphTransformation;
use graph::Graph;
use rayon::prelude::*;
use std::convert::From;
use std::fs::OpenOptions;
use std::io::{stdout, BufRead, BufWriter, Write};
use std::sync::mpsc::{Receiver, SendError, Sender, SyncSender};
use std::sync::Arc;
use std::time::Instant;
use transformation::*;
use utils::*;

pub fn apply_filters<F>(g: &GraphTransformation, ftrs: Arc<F>) -> Result<String, ()>
where
    F: Fn(&GraphTransformation) -> Result<String, ()>,
{
    ftrs(g)
}

/// Applying transformations to the graph g.
pub fn apply_transfos(g: &Graph, trs: &Transformation) -> Vec<GraphTransformation> {
    let mut r = trs.apply(&g);
    for rg in r.iter_mut() {
        rg.canon();
    }
    r
}

/// Should apply a set of transformations, filter the graphs and return the result
pub fn handle_graph<T>(
    g: Graph,
    t: &mut SenderVariant<String>,
    trsf: &Transformation,
    ftrs: Arc<T>,
) -> Result<(), TransProofError>
where
    T: Fn(&GraphTransformation) -> Result<String, ()>,
{
    let r = apply_transfos(&g, trsf);
    for h in r {
        let s = apply_filters(&h, ftrs.clone());
        if s.is_ok() {
            t.send(format!("{}\n", s.unwrap()))?;
        }
    }
    Ok(())
}

/// Should apply a set of transformations, filter the graphs and return the result
pub fn handle_graphs<T>(
    v: Vec<Graph>,
    t: SenderVariant<String>,
    trsf: &Transformation,
    ftrs: Arc<T>,
) -> Result<(), TransProofError>
where
    T: Fn(&GraphTransformation) -> Result<String, ()> + Send + Sync,
{
    v.into_par_iter()
        .try_for_each_with(t, |s, x| handle_graph(x, s, &trsf, ftrs.clone()))?;
    Ok(())
}

/// Read files of graphs
/// (file of sigs)
pub fn read_graphs<F>(rdr: &mut F, batchsize: usize) -> Vec<Graph>
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

pub fn output(
    receiver: Receiver<String>,
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
    for t in receiver.iter() {
        i += 1;
        bufout.write(&t.into_bytes())?;
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

#[derive(Clone)]
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
