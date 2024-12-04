use netsim_core::network::{Network, Packet};
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    let mut network = Network::<&'static str>::new();

    let sender = network
        .new_node()
        .set_upload_bandwidth("1bps".parse()?)
        .build();
    let receiver = network
        .new_node()
        .set_download_bandwidth("100mbps".parse()?)
        .build();

    let packet = Packet::builder()
        .from(sender)
        .to(receiver)
        .data("Hello World!")
        .build()?;
    let id = packet.id();
    network.send(packet)?;

    let print = |packet: Packet<&str>| {
        assert_eq!(id, packet.id());
        println!(
            "[{id}]{from}->{to}: {msg}",
            id = packet.id(),
            from = packet.from(),
            to = packet.to(),
            msg = packet.into_inner()
        );
    };

    network.advance_with(Duration::from_secs(10), |_| panic!());
    network.advance_with(Duration::from_secs(1), |_| panic!());

    network.advance_with(Duration::from_millis(1000), print);

    Ok(())
}
