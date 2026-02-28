use criterion::{Criterion, black_box, criterion_group, criterion_main};
use netsim_core::{
    data::Data,
    measure::{Bandwidth, CongestionChannel, Gauge, Latency, Upload},
    network::{Network, Packet, Round},
};
use std::{sync::Arc, time::Duration};

#[allow(clippy::declare_interior_mutable_const)]
const BD_8KBPS: Bandwidth = Bandwidth::new(8_192);
const RESERVE_SIZE: u64 = 0xF7;

fn gauge(c: &mut Criterion) {
    let gauge = Gauge::new();

    c.bench_function("reserve", |b| {
        b.iter(|| gauge.reserve(black_box(RESERVE_SIZE)))
    });

    gauge.reserve(u64::MAX);

    c.bench_function("free", |b| b.iter(|| gauge.free(black_box(RESERVE_SIZE))));
}

fn congestion_channel(c: &mut Criterion) {
    let congestion_channel = CongestionChannel::new(BD_8KBPS);

    let mut round = Round::ZERO;

    c.bench_function("no_update_capacity", |b| {
        b.iter(|| {
            congestion_channel
                .update_capacity(black_box(round), black_box(Duration::from_millis(100)))
        })
    });
    c.bench_function("update_capacity", |b| {
        b.iter(|| {
            round = round.next();
            congestion_channel
                .update_capacity(black_box(round), black_box(Duration::from_millis(100)))
        })
    });
}

fn upload(c: &mut Criterion) {
    let gauge = Arc::new(Gauge::new());
    let channel = Arc::new(CongestionChannel::new(BD_8KBPS));
    let mut upload = Upload::new(gauge, channel);

    let mut round = Round::ZERO;

    assert!(upload.send(u64::MAX));

    c.bench_function("upload_process", |b| {
        b.iter(|| {
            round = round.next();
            upload.update_capacity(black_box(round), black_box(Duration::from_millis(100)));

            upload.process()
        })
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TestData(u64);

impl Data for TestData {
    fn bytes_size(&self) -> u64 {
        self.0
    }
}

fn transit(c: &mut Criterion) {
    let mut network: Network<TestData> = Network::new();
    let sender = network.new_node().build();
    let recipient = network.new_node().build();
    network
        .configure_link(sender, recipient)
        .set_bandwidth(BD_8KBPS)
        .set_latency(Latency::ZERO)
        .apply();

    let data = Packet::builder(network.packet_id_generator())
        .from(sender)
        .to(recipient)
        .data(TestData(u64::MAX))
        .build()
        .unwrap();
    let mut round = Round::ZERO;

    let route = network.route(sender, recipient).unwrap();
    let mut transit = route.transit(data).unwrap();

    c.bench_function("transit", |b| {
        b.iter(|| {
            round = round.next();
            transit.advance(round, Duration::from_secs(1));
        })
    });
}

criterion_group!(benches, gauge, congestion_channel, upload, transit);
criterion_main!(benches);
