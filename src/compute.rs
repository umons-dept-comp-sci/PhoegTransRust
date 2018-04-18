use graph::Graph;
use graph::format::from_g6;
use graph::nauty::canon_graph;
use graph::transfos::{add_edge_const, remove_k_edges};
use std::fs::File;
use std::io::{stdout, BufRead, BufWriter, Write};
use rayon::prelude::*;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Instant;
use std::sync::Arc;
use utils::*;

pub fn apply_filters<F>(g: &Graph, ftrs: Arc<F>) -> Result<String, ()>
where
    F: Fn(&Graph) -> Result<String, ()>,
{
    ftrs(&g)
}

/// Applying transformations to the graph g.
pub fn apply_transfos<F>(g: &Graph, trs: Arc<F>) -> Vec<Graph>
where
    F: Fn(&Graph) -> Vec<Graph>,
{
    trs(&g).iter().map(|x| canon_graph(x).0).collect()
}

/// Should apply a set of transfomation, filter the graphs and return the result
pub fn handle_graph<F, T>(g: Graph, t: &mut Sender<String>, trsf: Arc<F>, ftrs: Arc<T>)
where
    F: Fn(&Graph) -> Vec<Graph>,
    T: Fn(&Graph) -> Result<String, ()>,
{
    let r = apply_transfos(&g, trsf);
    for h in r {
        let s = apply_filters(&h, ftrs.clone());
        if s.is_ok() {
            t.send(format!("{},{}\n", g, s.unwrap())).unwrap();
        }
    }
}

/// Should apply a set of transfomation, filter the graphs and return the result
pub fn handle_graphs<F, T>(v: Vec<Graph>, t: Sender<String>, trsf: Arc<F>, ftrs: Arc<T>)
where
    F: Fn(&Graph) -> Vec<Graph> + Send + Sync,
    T: Fn(&Graph) -> Result<String, ()> + Send + Sync,
{
    v.into_par_iter()
        .for_each_with(t, |s, x| handle_graph(x, s, trsf.clone(), ftrs.clone()));
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
                    eprintln!("Wrong input : {}", e);
                }
            },
            Err(e) => {
                eprintln!("{}", e);
            }
        }
    }
    t
}

pub fn search_transfo_all<I, V, C>(
    v: Vec<Graph>,
    inv: Arc<I>,
    class: Arc<C>,
    t: Sender<(Option<usize>, String)>,
) where
    I: Fn(&Graph) -> V + Send + Sync,
    C: Fn(&Graph) -> bool + Send + Sync,
    V: PartialOrd + Copy + ::std::fmt::Display + ::std::fmt::Debug,
{
    v.into_par_iter()
        .for_each_with(t, |s, x| search_transfo(&x, inv.clone(), class.clone(), s));
}

///Maximizes the invariant
pub fn search_transfo<I, V, C>(
    g: &Graph,
    inv: Arc<I>,
    class: Arc<C>,
    s: &mut Sender<(Option<usize>, String)>,
) where
    I: Fn(&Graph) -> V,
    C: Fn(&Graph) -> bool,
    V: PartialOrd + Copy + ::std::fmt::Display + ::std::fmt::Debug,
{
    let ginv = inv(&g);
    let mut k = 0;
    let mut r = remove_add(&g, k, inv.clone(), class.clone());
    //TODO we use three times the same condition on the lenght of r. It's ugly
    if r.len() > 0 {
        let mut ninv = r.iter()
            .max_by(|x, y| x.3.partial_cmp(&y.3).unwrap())
            .unwrap()
            .3;
        while r.len() > 0 && ninv <= ginv {
            k += 1;
            r = remove_add(&g, k, inv.clone(), class.clone());
            if r.len() > 0 {
                ninv = r.iter()
                    .max_by(|x, y| x.3.partial_cmp(&y.3).unwrap())
                    .unwrap()
                    .3;
            }
        }
        if r.len() > 0 {
            for t in r {
                s.send((
                    Some(t.1.size()),
                    format!(
                        "{} -> {} | r : {} a : {} i : {}\n",
                        g,
                        t.0,
                        t.1.size(),
                        t.2.size(),
                        t.3
                    ),
                )).unwrap();
            }
        } else {
            s.send((None, format!("{}\n", g)));
        }
    } else {
        s.send((None, format!("{}\n", g)));
    }
}

pub fn remove_add<I, V, C>(
    g: &Graph,
    k: usize,
    inv: Arc<I>,
    class: Arc<C>,
) -> Vec<(Graph, Graph, Graph, V)>
where
    I: Fn(&Graph) -> V,
    C: Fn(&Graph) -> bool,
    V: PartialOrd + Copy + ::std::fmt::Display,
{
    //First, we remove k edges
    let res = remove_k_edges(&g, k, |_, _, _| true);
    //Then we add enough to increase the invariant or just reach a graph in the class that cannot
    //be improved
    res.iter()
        .map(|&(ref g, ref v)| descent_add(&g, &v, inv(&g), inv.clone(), class.clone()))
        .collect()
}

fn descent_add<I, V, C>(
    g: &Graph,
    v: &Graph,
    gval: V,
    inv: Arc<I>,
    class: Arc<C>,
) -> (Graph, Graph, Graph, V)
where
    I: Fn(&Graph) -> V,
    C: Fn(&Graph) -> bool,
    V: PartialOrd + Copy + ::std::fmt::Display,
{
    let mut res = add_edge_const(&g, &v, &Graph::new(g.order()));
    let mut stop = false;
    let mut pcand = (g.clone(), Graph::new(g.order()));
    let mut pcval = inv(&pcand.0);
    let mut cand;
    let mut cval;
    while res.len() > 0 && !stop {
        if gval < pcval && class(&pcand.0) {
            stop = true;
        } else {
            cand = res.iter()
                .max_by(|x, y| inv(&x.0).partial_cmp(&inv(&y.0)).unwrap())
                .unwrap()
                .clone();
            cval = inv(&cand.0);
            if pcval > cval && class(&pcand.0) {
                stop = true;
            } else {
                pcand = cand.clone();
                pcval = cval;
                res = add_edge_const(&cand.0, &v, &cand.1);
            }
        }
    }
    (pcand.0, v.clone(), pcand.1, pcval)
}

pub fn output_search(receiver: Receiver<(Option<usize>, String)>, filename: String, buffer: usize) {
    let mut bufout: Box<Write> = match filename.as_str() {
        "-" => Box::new(BufWriter::with_capacity(buffer, stdout())),
        _ => Box::new(BufWriter::with_capacity(
            buffer,
            File::open(filename).expect("Could not open file"),
        )),
    };
    let start = Instant::now();
    let mut i = 0;
    let mut mk = 0;
    let mut nok = vec![];
    for (k, t) in receiver.iter() {
        if let Some(k) = k {
            i += 1;
            if mk < k {
                mk = k;
            }
            bufout.write(&t.into_bytes()).unwrap();
        } else {
            nok.push(t);
        }
    }
    bufout.flush();
    let duration = start.elapsed();
    eprintln!(
        "Done : {} improvement{}. Maximum k was {}",
        i,
        plural(i),
        mk
    );
    if nok.len() > 0 {
        eprintln!("{} remaining graph{} : ", nok.len(), plural(nok.len()));
        for t in nok {
            bufout.write(&t.into_bytes()).unwrap();
        }
    }
    bufout.flush();
    let secs = duration.as_secs() as usize;
    let millis = (duration.subsec_nanos() as usize) / (1e6 as usize);
    eprintln!(
        "Took {} second{} and {} millisecond{}",
        secs,
        plural(secs),
        millis,
        plural(millis)
    );
}

pub fn output(receiver: Receiver<String>, filename: String, buffer: usize) {
    let mut bufout: Box<Write> = match filename.as_str() {
        "-" => Box::new(BufWriter::with_capacity(buffer, stdout())),
        _ => Box::new(BufWriter::with_capacity(
            buffer,
            File::open(filename).expect("Could not open file"),
        )),
    };
    let start = Instant::now();
    let mut i = 0;
    for t in receiver.iter() {
        i += 1;
        bufout.write(&t.into_bytes()).unwrap();
    }
    let duration = start.elapsed();
    eprintln!("Done : {} transformation{}", i, plural(i));
    let secs = duration.as_secs() as usize;
    let millis = (duration.subsec_nanos() as usize) / (1e6 as usize);
    eprintln!(
        "Took {} second{} and {} millisecond{}",
        secs,
        plural(secs),
        millis,
        plural(millis)
    );
}

#[cfg(test)]
mod test {
    use graph::format::from_g6;
    use graph::Graph;
    use graph::invariant;
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::Arc;
    use search_transfo_all;

    #[test]
    fn test_remove_add() {
        let g = vec![from_g6(&"D?{".to_string()).unwrap()];
        //let g = from_g6(&"C^".to_string()).unwrap();
        //TODO add the lambda and compensation in the search_transfo
        fn inv(x: &Graph) -> f64 {
            let lambda = 0.5;
            let i = match invariant::diameter(&x) {
                invariant::Distance::Val(i) => i as f64,
                invariant::Distance::Inf => 0 as f64,
            };
            i - lambda * (invariant::connected_components(&x).len() - 1) as f64
        };
        let (sender, receiver): (Sender<String>, Receiver<String>) = channel();
        println!(
            "{:?}",
            search_transfo_all(
                g,
                Arc::new(inv),
                Arc::new(invariant::is_connected),
                sender.clone()
            )
        );
        assert!(false);
    }
}
