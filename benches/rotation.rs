use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use graph::transfo_result::GraphTransformation;
use graph::transfos::rotation;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::mpsc::sync_channel;
use std::sync::Arc;
use std::thread;
use transrust_lib::compute::*;
use transrust_lib::utils::as_filter;

pub fn rotation_bench(c: &mut Criterion) {
    let mut buf = BufReader::new(File::open("benches/g8.g6").unwrap());
    let nthreads = rayon::current_num_threads();
    let v = read_graphs(&mut buf, 10000);
    let red_client =
        redis::Client::open("redis://127.0.0.1/").expect("Could not connect to redis.");
    let deftest = |ref x: &GraphTransformation| -> Result<String, ()> {
        as_filter(|_| true, |x| x.tocsv())(&x)
    };
    let mut group = c.benchmark_group("group");
    group.sample_size(10).measurement_time(std::time::Duration::from_secs(40));
    group.bench_function("rotation", |b| {
        b.iter_batched(
            || v.clone(),
            |v| {
                let (snd, rcv) = sync_channel::<LogInfo>(2 * nthreads);
                let whandle = thread::spawn(move || {
                    output(rcv, "/dev/null".to_string(), 2000000, false, false)
                });
                handle_graphs(
                    v,
                    SenderVariant::LimitedSender(snd.clone()),
                    &rotation,
                    Arc::new(deftest),
                    false,
                    &red_client,
                    false,
                )
                .unwrap();
                drop(snd);
                whandle.join().unwrap().unwrap();
            },
            BatchSize::SmallInput,
        )
    });
    group.finish();
}

criterion_group!(transfos, rotation_bench);
criterion_main!(transfos);
