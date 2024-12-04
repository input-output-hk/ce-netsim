use criterion::{
    black_box, criterion_group, criterion_main, measurement::WallTime, BenchmarkGroup, Criterion,
    Throughput,
};
use netsim_core::{
    data::Data,
    measure::Bandwidth,
    network::{Network, PacketBuilder},
};
use std::time::Duration;

const MESSAGE_SIZE: u64 = 100_000_000;
const BANDWIDTH: Bandwidth = Bandwidth::new(10 * 1_024, Duration::from_secs(1));

struct TestData;
impl Data for TestData {
    fn bytes_size(&self) -> u64 {
        MESSAGE_SIZE
    }
}

fn send(c: &mut Criterion) {
    let mut network: Network<TestData> = Network::new();
    let node1 = network.new_node().build();
    let node2 = network.new_node().build();

    c.bench_function("send", |b| {
        b.iter(|| {
            let packet = PacketBuilder::new()
                .from(node1)
                .to(node2)
                .data(TestData)
                .build()
                .unwrap();
            network.send(black_box(packet)).unwrap()
        })
    });
}

fn bench_advance_size(group: &mut BenchmarkGroup<'_, WallTime>, size: usize) {
    let mut network: Network<TestData> = Network::new();

    // initialise all the nodes of the network
    let mut nodes = Vec::with_capacity(size);
    for _ in 0..size {
        nodes.push(
            network
                .new_node()
                .set_upload_bandwidth(BANDWIDTH)
                .set_download_bandwidth(BANDWIDTH)
                .build(),
        );
    }

    // have every nodes send 1 message to each other nodes
    let mut num_msg = 0u64;

    for sender in nodes.iter().copied() {
        for receiver in nodes.iter().copied().filter(|id| id != &sender) {
            let packet = PacketBuilder::new()
                .from(sender)
                .to(receiver)
                .data(TestData)
                .build()
                .unwrap();
            network.send(packet).unwrap();
            num_msg += 1;
        }
    }

    group.throughput(Throughput::Elements(num_msg));
    group.bench_function(format!("{num_msg} messages"), |b| {
        b.iter(|| {
            network.advance_with(Duration::from_millis(200), |_| {
                num_msg -= 1;
            })
        })
    });
}

fn advance(c: &mut Criterion) {
    let mut group = c.benchmark_group("advance");

    for size in [100, 200, 400, 600, 800, 1000] {
        bench_advance_size(&mut group, size);
    }

    group.finish();
}

criterion_group!(benches, send, advance);
criterion_main!(benches);
