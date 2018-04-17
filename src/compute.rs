use graph::Graph;
use graph::format::from_g6;
use graph::nauty::{canon_graph, orbits};
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

pub fn remove_add<I, V, C>(g: &Graph, k: usize, inv: &I, class: &C) -> Vec<(Graph, V)>
where
    I: Fn(&Graph) -> V,
    C: Fn(&Graph) -> bool,
    V: PartialOrd + Copy + ::std::fmt::Display,
{
    let mut res = vec![(g.clone(), Graph::new(g.order()))];
    //First, we remove k edges
    for _ in 0..k {
        res = res.iter()
            .flat_map(|&(ref g, ref v)| remove_edge_pair(&g, &v))
            .collect();
    }
    res.iter()
        .map(|&(ref g, ref v)| descent_add(&g, &v, &inv, class))
        .collect()
}

fn descent_add<I, V, C>(g: &Graph, v: &Graph, inv: &I, class: C) -> (Graph, V)
where
    I: Fn(&Graph) -> V,
    C: Fn(&Graph) -> bool,
    V: PartialOrd + Copy + ::std::fmt::Display,
{
    let mut res = add_edge_const(&g, &v);
    let mut stop = false;
    let mut pcand = g.clone();
    let mut pcval = inv(&pcand);
    if res.len() > 0 {
        pcand = res.iter()
            .max_by(|x, y| inv(x).partial_cmp(&inv(y)).unwrap())
            .unwrap()
            .clone();
        pcval = inv(&pcand);
        res = add_edge_const(&pcand, &v);
        let mut cand;
        let mut cval;
        while res.len() > 0 && !stop {
            cand = res.iter()
                .max_by(|x, y| inv(x).partial_cmp(&inv(y)).unwrap())
                .unwrap()
                .clone();
            cval = inv(&cand);
            if pcval > cval && class(&pcand) {
                stop = true;
            } else {
                pcand = cand.clone();
                pcval = cval;
                res = add_edge_const(&cand, &v);
            }
        }
    }
    (pcand, pcval)
}

pub fn remove_edge_pair(g: &Graph, v: &Graph) -> Vec<(Graph, Graph)> {
    let mut res = vec![];
    let mut fixed = Vec::with_capacity(1);
    for i in orbits(&g, &fixed) {
        fixed.push(i as u32);
        for &j in orbits(&g, &fixed)
            .iter()
            .filter(|&x| *x > i && g.is_edge(*x, i))
        {
            let mut ng = g.clone();
            let mut nv = v.clone();
            ng.remove_edge(i, j);
            nv.add_edge(i, j);
            res.push((ng, nv));
        }
        fixed.pop();
    }
    res
}

pub fn add_edge_const(g: &Graph, v: &Graph) -> Vec<Graph> {
    let mut res = vec![];
    let mut fixed = Vec::with_capacity(1);
    for i in orbits(&g, &fixed) {
        fixed.push(i as u32);
        for &j in orbits(&g, &fixed)
            .iter()
            .filter(|&x| *x > i && !g.is_edge(*x, i) && !v.is_edge(*x, i))
        {
            let mut ng = g.clone();
            ng.add_edge(i, j);
            res.push(ng);
        }
        fixed.pop();
    }
    res
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
    use remove_add;

    #[test]
    fn test_remove_add() {
        let g = from_g6(&"D?{".to_string()).unwrap();
        fn inv(x: &Graph) -> f64 {
            let lambda = 0.5;
            let i = match invariant::diameter(&x) {
                invariant::Distance::Val(i) => i as f64,
                invariant::Distance::Inf => 0 as f64,
            };
            i - lambda * (invariant::connected_components(&x).len() - 1) as f64
        };
        println!("{:?}", remove_add(&g, 2, &inv, &invariant::is_connected));
        assert!(false);
    }
}
