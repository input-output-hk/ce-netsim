use netsim_core::{
    Latency,
    network::{Network, Packet},
};
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    let mut network = Network::<&'static str>::new();

    let sender = network
        .new_node()
        .set_upload_bandwidth("10mbps".parse()?)
        .build();
    let receiver = network
        .new_node()
        .set_download_bandwidth("100mbps".parse()?)
        .build();

    // Connect the two nodes with a 50ms latency link.
    network
        .configure_link(sender, receiver)
        .set_latency(Latency::new(Duration::from_millis(50)))
        .set_bandwidth("100mbps".parse()?)
        .apply();

    let packet = Packet::builder(network.packet_id_generator())
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

    // Packet should not arrive yet — still within the 50ms link latency.
    network.advance_with(Duration::from_millis(10), |_| {
        panic!("packet arrived too early")
    });

    // After enough time the packet is delivered.
    network.advance_with(Duration::from_millis(100), print);

    Ok(())
}
