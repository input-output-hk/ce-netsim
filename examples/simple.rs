use ce_network_sim::{SimContext, SimId};

const NET1: SimId = SimId::new("net1");
const NET2: SimId = SimId::new("net2");

#[tokio::main]
async fn main() {
    let context: SimContext<&'static str> = SimContext::new().await;

    let net1 = context.open(NET1).unwrap();
    let mut net2 = context.open(NET2).unwrap();

    net1.send_to(NET2, "Hello World!").unwrap();
    let Some((from, msg)) = net2.recv().await else {
        panic!("expecting message from NET1")
    };

    assert_eq!(from, NET1);

    println!("{msg}")
}
