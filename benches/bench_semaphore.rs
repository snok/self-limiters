use criterion::black_box;
use criterion::Criterion;
use criterion::{criterion_group, criterion_main};
use redis::Client;
use self_limiters::_utils::send_shared_state;
use self_limiters::semaphore::create_and_acquire_semaphore;
use self_limiters::semaphore::ThreadState;
use tokio::runtime::Builder;
use tokio::runtime::Runtime;

async fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("semaphore_aenter", |b| {
        let runtime = Builder::new_current_thread().build().unwrap();
        let receiver = send_shared_state(ThreadState {
            client: Client::open("redis://127.0.0.1:6389").unwrap(),
            name: "bench-semaphore".parse().unwrap(),
            capacity: 1,
            max_sleep: 0.0,
        })
        .unwrap();
        b.to_async(runtime)
            .iter(|| create_and_acquire_semaphore(black_box(receiver)))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
