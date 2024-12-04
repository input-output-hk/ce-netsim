use netsim::SimContext;
use netsim_core::{LinkId, EdgePolicy, Latency};
use std::time::{Duration, Instant};

const MSG: &str = "Hello World!";

fn main() {
    let mut context: SimContext<&'static str> = SimContext::default();

    let net1 = context.open().unwrap();
    let mut net2 = context.open().unwrap();

    context
        .set_edge_policy(
            LinkId::new((net1.id(), net2.id())),
            EdgePolicy {
                latency: Latency::new(Duration::from_secs(1)),
                ..Default::default()
            },
        )
        .unwrap();

    net1.send_to(net2.id(), MSG).unwrap();

    let instant = Instant::now();
    let Some((from, msg)) = net2.recv() else {
        panic!("expecting message from NET1")
    };
    let elapsed = instant.elapsed();

    assert_eq!(from, net1.id());

    println!(
        "{from} -> {net2} ({}ms): {msg}",
        elapsed.as_millis(),
        net2 = net2.id()
    );

    context.shutdown().unwrap();
}
