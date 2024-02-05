use ce_network_sim::{
    HasBytesSize as _, SimConfiguration, SimContext, SimId, SimSocketConfiguration,
};
use tokio::time::Instant;

const NET1: SimId = SimId::new("net1");
const NET2: SimId = SimId::new("net2");
const MSG: &str = "Hello World!";

#[tokio::main]
async fn main() {
    let configuration = SimConfiguration {};
    let context: SimContext<&'static str> = SimContext::new(configuration).await;

    let net1 = context
        .open(NET1, SimSocketConfiguration::default())
        .unwrap();
    let mut net2 = context
        .open(
            NET2,
            SimSocketConfiguration {
                bytes_per_sec: MSG.bytes_size(),
            },
        )
        .unwrap();

    net1.send_to(NET2, MSG).unwrap();

    let instant = Instant::now();
    let Some((from, msg)) = net2.recv().await else {
        panic!("expecting message from NET1")
    };
    let elapsed = instant.elapsed();

    assert_eq!(from, NET1);

    println!("{from} -> {NET2} ({}ms): {msg}", elapsed.as_millis());

    context.shutdown().await.unwrap();
}
