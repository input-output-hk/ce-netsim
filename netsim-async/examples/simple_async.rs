use std::time::Duration;

use ce_netsim_async::{SimConfiguration, SimContext};
use ce_netsim_core::{Edge, EdgePolicy, Latency};
use tokio::time::Instant;

const MSG: &str = "Hello World!";

#[tokio::main]
async fn main() {
    let configuration = SimConfiguration::default();
    let mut context: SimContext<&'static str> = SimContext::new(configuration).await;

    let net1 = context.open().unwrap();
    let mut net2 = context.open().unwrap();

    context.set_edge_policy(
        Edge::new((net1.id(), net2.id())),
        EdgePolicy {
            latency: Latency::new(Duration::from_secs(1)),
            ..Default::default()
        },
    );

    net1.send_to(net2.id(), MSG).unwrap();

    let instant = Instant::now();
    let Some((from, msg)) = net2.recv().await else {
        panic!("expecting message from NET1")
    };
    let elapsed = instant.elapsed();

    assert_eq!(from, net1.id());

    println!(
        "{from} -> {net2} ({}ms): {msg}",
        elapsed.as_millis(),
        net2 = net2.id()
    );

    context.shutdown().await.unwrap();
}
