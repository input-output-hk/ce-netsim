use criterion::{Criterion, black_box, criterion_group, criterion_main};
use netsim_core::{
    data::Data,
    link::Link,
    measure::{Bandwidth, CongestionChannel, Gauge, Latency, Upload},
    network::{Packet, PacketIdGenerator, Round, Route},
    node::{Node, NodeId},
};
use std::{sync::Arc, time::Duration};

#[allow(clippy::declare_interior_mutable_const)]
const BD_1KBPS: Bandwidth = Bandwidth::new(1_024, Duration::from_secs(1));
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
    let congestion_channel = CongestionChannel::new(BD_1KBPS);

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
    let channel = Arc::new(CongestionChannel::new(BD_1KBPS));
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
    let sender: Node<TestData> = Node::new(NodeId::ZERO);
    let link = Link::new(Latency::ZERO, Arc::new(CongestionChannel::new(BD_1KBPS)));
    let recipient: Node<TestData> = Node::new(NodeId::ONE);
    let data = Packet::builder(&PacketIdGenerator::new())
        .from(sender.id())
        .to(recipient.id())
        .data(TestData(u64::MAX))
        .build()
        .unwrap();
    let mut round = Round::ZERO;

    let mut transit = Route::builder()
        .upload(&sender)
        .link(&link)
        .download(&recipient)
        .build()
        .unwrap()
        .transit(data)
        .unwrap();

    c.bench_function("transit", |b| {
        b.iter(|| {
            round = round.next();
            transit.advance(round, Duration::from_secs(1));
        })
    });
}

criterion_group!(benches, gauge, congestion_channel, upload, transit);
criterion_main!(benches);
