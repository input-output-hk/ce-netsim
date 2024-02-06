use ce_netsim_async::{HasBytesSize as _, SimConfiguration, SimContext, SimSocketConfiguration};
use tokio::time::Instant;

const MSG: &str = "Hello World!";

#[tokio::main]
async fn main() {
    let configuration = SimConfiguration {};
    let mut context: SimContext<&'static str> = SimContext::new(configuration).await;

    let net1 = context.open(SimSocketConfiguration::default()).unwrap();
    let mut net2 = context
        .open(SimSocketConfiguration {
            download_bytes_per_sec: MSG.bytes_size(),
            ..Default::default()
        })
        .unwrap();

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