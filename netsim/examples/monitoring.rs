//! Monitoring example: use SimContext::stats() to inspect the network state.
//!
//! Creates a multi-node network, sends some packets, and prints a live
//! snapshot of node and link statistics.
//!
//! Run with:
//!   cargo run --example monitoring -p netsim

use anyhow::Result;
use netsim::{Data, Latency, PacketLoss, SimContext};
use std::{thread, time::Duration};

struct Msg(#[allow(dead_code)] Vec<u8>);

impl Data for Msg {
    fn bytes_size(&self) -> u64 {
        self.0.len() as u64
    }
}

fn main() -> Result<()> {
    let mut network = SimContext::<Msg>::new()?;

    let mut n1 = network
        .open()
        .set_upload_bandwidth("1mbps".parse()?)
        .build()?;
    let mut n2 = network.open().build()?;
    let mut n3 = network.open().build()?;

    // Configure links with different properties
    network
        .configure_link(n1.id(), n2.id())
        .set_latency(Latency::new(Duration::from_millis(10)))
        .set_bandwidth("1mbps".parse()?)
        .apply()?;

    network
        .configure_link(n1.id(), n3.id())
        .set_latency(Latency::new(Duration::from_millis(50)))
        .set_bandwidth("512kbps".parse()?)
        .set_packet_loss(PacketLoss::rate(0.05)?)
        .apply()?;

    // Send packets in the background
    let n2_id = n2.id();
    let n3_id = n3.id();
    thread::spawn(move || {
        for i in 0..20u8 {
            let data = vec![i; 1024];
            let _ = n1.send_to(n2_id, Msg(data.clone()));
            let _ = n1.send_to(n3_id, Msg(data));
        }
    });

    // Let packets flow for a bit
    thread::sleep(Duration::from_millis(20));

    // Take a snapshot
    let stats = network.stats()?;

    println!("=== Network Snapshot ===");
    println!();
    println!("Nodes ({}):", stats.nodes.len());
    for node in &stats.nodes {
        println!(
            "  Node {}: upload buf {}/{} bytes | download buf {}/{} bytes | dropped {}",
            node.inner.id,
            node.inner.upload_buffer_used,
            node.inner.upload_buffer_max,
            node.inner.download_buffer_used,
            node.inner.download_buffer_max,
            node.packets_dropped,
        );
    }

    println!();
    println!("Links ({}):", stats.links.len());
    for link in &stats.links {
        println!(
            "  Link {:?}: latency={} | bandwidth={} | loss={:?} | in-transit={} bytes",
            link.id, link.latency, link.bandwidth, link.packet_loss, link.bytes_in_transit,
        );
    }

    // Drain receivers so shutdown is clean
    thread::spawn(move || while n2.try_recv_packet().is_ok() {});
    thread::spawn(move || while n3.try_recv_packet().is_ok() {});

    // Small sleep to let remaining packets through
    thread::sleep(Duration::from_millis(200));

    network.shutdown()
}
