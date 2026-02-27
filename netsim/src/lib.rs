/*!
# `netsim` — Network Simulator

`netsim` is a **thread-driven** network simulator that runs in real time.
Send messages between simulated nodes over configurable links (latency,
bandwidth, packet loss) and receive them through a blocking or non-blocking
socket-like API — all without writing your own event loop.

For deterministic, user-controlled time see the lower-level
[`netsim_core`] crate that `netsim` is built on.

## Quick start

```rust,no_run
use netsim::{SimContext, Latency, Bandwidth, Data};
use std::time::Duration;

// 1. Define your message type and implement Data.
#[derive(Debug)]
struct Ping(u64);

impl Data for Ping {
    fn bytes_size(&self) -> u64 { 8 }
}

fn main() -> anyhow::Result<()> {
    // 2. Create a SimContext — this starts the background multiplexer thread.
    let mut sim = SimContext::<Ping>::new()?;

    // 3. Open sockets for each simulated node.
    let mut client = sim.open().build()?;
    let mut server = sim.open().build()?;

    // 4. Connect the nodes. No link = no delivery.
    sim.configure_link(client.id(), server.id())
        .set_latency(Latency::new(Duration::from_millis(20)))
        .set_bandwidth("100mbps".parse()?)
        .apply()?;

    // 5. Send a message.
    let id = client.send_to(server.id(), Ping(1))?;

    // 6. Block until the packet arrives (at least 20 ms later).
    let packet = server.recv_packet().expect("server should receive");
    assert_eq!(packet.id(), id);

    let msg = packet.into_inner();
    println!("received Ping({})", msg.0);

    // 7. Clean shutdown.
    sim.shutdown()
}
```

## Architecture

```text
                      upload buffer
client.send_to() ──►  [ sender node ] ──► link (latency + bandwidth) ──► [ receiver node ] ──► socket.recv_packet()
                                                                               download buffer
```

The **multiplexer** is a background thread that advances simulated time in
~200 µs steps (at real-time pace) and delivers packets to receiver sockets
via a bounded channel.

### Nodes and sockets

Every call to [`SimContext::open`] creates a **node** in the simulation and
returns a [`SimSocket`] handle. The socket owns:

- An upload buffer — packets wait here until bandwidth allows them to be sent.
- A download channel — the multiplexer delivers arrived packets here.

Each node gets a unique [`NodeId`]. Store these IDs to configure links and
address packets.

### Links

Nodes are **not** connected by default. You must call
[`SimContext::configure_link`] before packets can flow between two nodes.
A link is **bidirectional and shared**: traffic in both directions competes
for the same bandwidth budget.

```rust,no_run
use netsim::{SimContext, Latency, Bandwidth, PacketLoss, Data};
use std::time::Duration;
// 20 ms latency, 1 Gbps shared bandwidth, 1% packet loss
# struct MyMsg;
# impl Data for MyMsg { fn bytes_size(&self) -> u64 { 0 } }
# fn example() -> anyhow::Result<()> {
# let mut sim = SimContext::<MyMsg>::new()?;
# let a = sim.open().build()?;
# let b = sim.open().build()?;
sim.configure_link(a.id(), b.id())
    .set_latency(Latency::new(Duration::from_millis(20)))
    .set_bandwidth("1gbps".parse()?)
    .set_packet_loss(PacketLoss::Rate(0.01))
    .apply()?;
# Ok(()) }
```

### Packet loss (UDP semantics)

`netsim` uses **UDP semantics**: dropped packets are silent. A packet can be
lost at three points:

1. **Upload buffer full** — the sender node's buffer overflows; the packet is
   dropped and [`SimSocket::packets_dropped`] is incremented.
2. **Probabilistic link loss** — the link's `packet_loss` rate fires; the
   packet is silently discarded in-flight.
3. **Receiver channel full** — the bounded receive channel fills up; the
   packet is dropped at delivery.

### Sending and receiving

| Method | Description |
|--------|-------------|
| [`SimSocket::send_to`] | Build and send a packet to a `NodeId` in one step |
| [`SimSocket::send_packet`] | Send a pre-built [`Packet`] |
| [`SimSocket::recv_packet`] | Block until a packet arrives (or the sim shuts down) |
| [`SimSocket::try_recv_packet`] | Non-blocking receive; returns immediately |

### Shutdown

Always call [`SimContext::shutdown`] to join the background thread and
propagate any errors. If `SimContext` is dropped without calling `shutdown`,
the background thread will eventually exit on its own (when the channel
disconnects), but errors will be lost.

```rust,no_run
use netsim::{SimContext, Data};
# struct MyMsg;
# impl Data for MyMsg { fn bytes_size(&self) -> u64 { 0 } }
# fn example() -> anyhow::Result<()> {
let sim = SimContext::<MyMsg>::new()?;
// ... use the sim ...
sim.shutdown()?;   // joins the background thread; returns any multiplexer error
# Ok(()) }
```

## Monitoring

[`SimContext::stats`] returns a [`SimStats`] snapshot with per-node buffer
usage and per-link bytes-in-transit, useful for debugging congestion:

```rust,no_run
use netsim::{SimContext, Data};
# struct MyMsg;
# impl Data for MyMsg { fn bytes_size(&self) -> u64 { 0 } }
# fn example() -> anyhow::Result<()> {
let mut sim = SimContext::<MyMsg>::new()?;
let stats = sim.stats()?;
for node in &stats.nodes {
    println!(
        "node {}: upload {}/{} bytes",
        node.inner.id,
        node.inner.upload_buffer_used,
        node.inner.upload_buffer_max
    );
}
# Ok(()) }
```
*/

mod multiplexer;
mod socket;
pub mod stats;

// convenient re-export of `netsim_core` core objects
pub use netsim_core::{
    Bandwidth, Latency, LinkId, NodeId, Packet, PacketBuilder, PacketId, PacketLoss, data::Data,
};

pub use self::{
    multiplexer::{SimContext, SimLinkBuilder},
    socket::{RecvError, SendError, SendToError, SimSocket, TryRecvError},
    stats::{NodeStats, SimStats},
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ops::Deref, time::Instant};

    #[derive(Debug)]
    struct Msg(Instant);
    impl Deref for Msg {
        type Target = Instant;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl Msg {
        pub fn new() -> Self {
            Self(Instant::now())
        }
    }
    impl Data for Msg {
        fn bytes_size(&self) -> u64 {
            0
        }
    }

    #[test]
    fn simple() {
        let mut network = SimContext::<Msg>::new().unwrap();
        let mut n1 = network.open().build().unwrap();
        let mut n2 = network.open().build().unwrap();

        // Nodes must be connected with configure_link before packets can be sent.
        network.configure_link(n1.id(), n2.id()).apply().unwrap();

        let packet_id = n1.send_to(n2.id(), Msg::new()).unwrap();

        let packet = n2
            .recv_packet()
            .expect("Should receive packets before disconnecting...");
        let id = packet.id();
        let msg = packet.into_inner();
        let elapsed = msg.elapsed();

        assert_eq!(id, packet_id);
        // The default link latency is 5ms. Wall-clock time will always be >= 5ms
        // because the multiplexer drives simulation time at real-time pace.
        // We use a loose upper bound to avoid flakiness on slow machines.
        assert!(
            elapsed.as_micros() >= 5000,
            "elapsed {elapsed:?} should be >= 5ms (default latency)"
        );
        assert!(
            elapsed.as_millis() < 1000,
            "elapsed {elapsed:?} should arrive in under 1s"
        );
    }
}
