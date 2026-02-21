//! Congestion example: saturate a link and observe packet drops.
//!
//! This example creates a link with a small upload buffer and floods it with
//! more data than the buffer can hold. The drop counter on the sender's socket
//! rises as the buffer saturates.
//!
//! Run with:
//!   cargo run --example congestion -p netsim

use anyhow::Result;
use netsim::{Data, Latency, SimContext};
use std::time::Duration;

// 1 KB payload
struct Payload(#[allow(dead_code)] [u8; 1024]);

impl Data for Payload {
    fn bytes_size(&self) -> u64 {
        1024
    }
}

fn main() -> Result<()> {
    let mut network = SimContext::<Payload>::new()?;

    // Sender with a very small upload buffer (32 KB)
    let mut sender = network
        .open()
        .set_upload_buffer(32 * 1_024)
        .build()?;

    let receiver = network.open().build()?;

    // Very narrow link: 10 Kbps and 100ms latency
    // This ensures the buffer fills quickly
    network
        .configure_link(sender.id(), receiver.id())
        .set_latency(Latency::new(Duration::from_millis(100)))
        .set_bandwidth("10kbps".parse()?)
        .apply()?;

    println!("Flooding a 10 Kbps / 100ms link with 1KB packets...");

    // Try to send 200 packets. Many will be dropped because the buffer is tiny.
    let mut sent = 0usize;
    for _ in 0..200 {
        if sender.send_to(receiver.id(), Payload([0u8; 1024])).is_ok() {
            sent += 1;
        }
    }

    let dropped = sender.packets_dropped();
    println!("Sent to multiplexer: {sent}");
    println!("Dropped (buffer full): {dropped}");
    println!("In-flight: {}", sent - dropped as usize);

    network.shutdown()
}
