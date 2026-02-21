//! Basic example: two nodes exchange a message with a custom link configuration.
//!
//! Run with:
//!   cargo run --example basic -p netsim

use anyhow::Result;
use netsim::{Data, Latency, PacketLoss, SimContext};
use std::time::Duration;

#[derive(Debug, Clone)]
struct Msg(String);

impl Data for Msg {
    fn bytes_size(&self) -> u64 {
        self.0.len() as u64
    }
}

fn main() -> Result<()> {
    let mut network = SimContext::<Msg>::new()?;

    // Create two nodes with explicit bandwidth limits
    let mut alice = network
        .open()
        .set_upload_bandwidth("100mbps".parse()?)
        .set_download_bandwidth("100mbps".parse()?)
        .build()?;

    let mut bob = network
        .open()
        .set_upload_bandwidth("100mbps".parse()?)
        .set_download_bandwidth("100mbps".parse()?)
        .build()?;

    // Configure the link between alice and bob:
    //   - 20ms latency (simulating a cross-continent hop)
    //   - 50 Mbps link bandwidth
    //   - 1% packet loss
    network
        .configure_link(alice.id(), bob.id())
        .set_latency(Latency::new(Duration::from_millis(20)))
        .set_bandwidth("50mbps".parse()?)
        .set_packet_loss(PacketLoss::Rate(0.01))
        .apply()?;

    println!("Alice (id={}) -> Bob (id={})", alice.id(), bob.id());
    println!("Link: 20ms latency, 50 Mbps, 1% packet loss");
    println!();

    // Send a message from alice to bob
    let packet_id = alice.send_to(bob.id(), Msg("Hello, Bob!".to_string()))?;
    println!("Sent packet {} from alice", packet_id);

    // Receive the message at bob (blocks until delivered)
    let packet = bob.recv_packet().expect("Should receive the packet");
    println!(
        "Bob received packet {} from alice: {:?}",
        packet.id(),
        packet.into_inner().0
    );

    network.shutdown()
}
