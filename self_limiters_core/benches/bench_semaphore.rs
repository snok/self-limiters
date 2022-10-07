use std::sync::mpsc::{channel, Receiver};

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use redis::Client;
use tokio::runtime::Runtime;

use self_limiters_core::semaphore::{create_and_acquire_semaphore, release_semaphore, ThreadState};
use self_limiters_core::utils::SLResult;

async fn t(r1: Receiver<ThreadState>, r2: Receiver<ThreadState>) -> SLResult<()> {
    create_and_acquire_semaphore(black_box(r1)).await?;
    release_semaphore(black_box(r2)).await?;
    Ok(())
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("semaphore_aenter", |b| {
        let rt = Runtime::new().unwrap();

        b.to_async(rt).iter(|| {
            let ts = ThreadState {
                client: Client::open("redis://127.0.0.1:6389").unwrap(),
                name: "bench-semaphore-2".parse().unwrap(),
                capacity: 10,
                max_sleep: 0.0,
            };
            let (s1, r1) = channel();
            s1.send(ts.to_owned()).unwrap();
            let (s2, r2) = channel();
            s2.send(ts).unwrap();

            // Test semaphore
            t(r1, r2)
        })
    });
}

criterion_group! {
  name = benches;
  config = Criterion::default().sample_size(10).nresamples(50);
  targets = criterion_benchmark
}
criterion_main!(benches);
