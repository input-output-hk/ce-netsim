/*!
# `netsim-core` — Low-level Network Simulation Primitives

`netsim-core` is the building block for network protocol simulations. It
provides the data structures and algorithms for modelling nodes, links, and
in-flight packets, but deliberately does **not** manage wall-clock time or
threads. You control when time advances, which makes it easy to run
simulations faster than real time, pause them for inspection, or replay them
deterministically.

For a batteries-included, thread-driven experience with real-time pacing,
use the `netsim` crate instead.

## UDP semantics

This crate models **UDP-style** message passing:

- Packets are sent **without acknowledgement**. There are no retries.
- Dropped packets (due to buffer overflow or packet loss) are **silent**. The
  network returns `Ok(())` for a send even when a packet is later dropped.
- If the receiver's buffer fills up, arriving bytes are silently discarded.

## Mental model

```text
Network::send()
      │
      ▼ upload buffer (bytes wait here until bandwidth allows)
 [ Sender Node ]
      │
      │ outbound channel (upload bandwidth limit)
      ▼
  [ Link ] ─── latency ──► delivers after N ms of simulated time
      │          └─ bandwidth_forward (A→B independent channel)
      │          └─ bandwidth_reverse (B→A independent channel)
      │          └─ packet_loss (probabilistic drop rate)
      ▼
 [ Recipient Node ]
      │ inbound channel (download bandwidth limit)
      ▼ download buffer (bytes wait here until advance_with delivers them)
Network::advance_with() closure receives the packet
```

A **link** is **full-duplex**: each direction (A→B and B→A) has its own
independent bandwidth channel. Saturating the link in one direction does
not affect throughput in the opposite direction.

## Your message type and the `Data` trait

Any type `T` you want to send through the network must implement the
[`Data`](crate::data::Data) trait, which has a single method:

```
# use netsim_core::data::Data;
struct MyMessage {
    payload: Vec<u8>,
}

impl Data for MyMessage {
    fn bytes_size(&self) -> u64 {
        self.payload.len() as u64
    }
}
```

The byte size is used to account for bandwidth and buffer capacity. If your
message has no meaningful byte size (e.g. in unit tests), return `0` — the
packet will still transit the network and respect latency, it just won't
consume any bandwidth.

The `Data` trait is already implemented for common types: `()`, `u8`,
`String`, `Vec<u8>`, `Box<[u8]>`, `[u8; N]`, and `&'static str`.

# Building a [`Network`]

The main component of this crate is the [`Network`] object. It allows
to build the network: create [`Node`]s, configure their [`Bandwidth`] for
upload or download, configure their buffers size or the [`Bandwidth`]
and [`Latency`] of the connection between two [`Node`]s.

```
# type Data = ();
use netsim_core::network::Network;

# fn f() -> anyhow::Result<()> {
// create a new Simulated Network
let mut network: Network<Data> = Network::new();

// add a new node with the default policies
let n1 = network.new_node().build();
// add a new node with custom policies
let n2 = network
  .new_node()
  .set_upload_buffer(500 * 1_024 * 1_024) // 500MB of upload buffer
  .set_upload_bandwidth("800mbps".parse()?) // 500 Mb/s upload
  .set_download_buffer(1_024 * 1_024 * 1_024) // 500MB of upload buffer
  .set_download_bandwidth("1gbps".parse()?) // 1 Gb/s upload
  .build();
# Ok(()) }; f().unwrap();
```

# Packets

The kind of messages that travel in the network are [`Packet`].
They are _"envelop"_ structure around the actual message and
contains metadata information that are necessary for the [`Network`]
to process the transit of the message: (sender, recipient, size and
what to do of the message if the packet is dropped before reception).

```
# use netsim_core::{node::NodeId, network::Network};
use netsim_core::network::Packet;
# fn f() -> anyhow::Result<()> {
# let mut network: Network<&'static str> = Network::new();
# let n1 = NodeId::ZERO;
# let n2 = NodeId::ONE;

let packet = Packet::builder(network.packet_id_generator())
  .from(n1)
  .to(n2)
  .data("message")
  .build()?;
# Ok(()) }; f().unwrap();
```

# Connecting nodes

Nodes are **not** connected by default. Before sending any packet between two nodes,
you must configure a link between them using [`Network::configure_link`]. Without a
link, [`Network::send`] will return a [`RouteError::LinkNotFound`] error.

```
# use netsim_core::network::Network;
# fn f() -> anyhow::Result<()> {
# let mut network: Network<()> = Network::new();
# let n1 = network.new_node().build();
# let n2 = network.new_node().build();
// Connect n1 and n2 with default latency and bandwidth
network.configure_link(n1, n2).apply();
# Ok(()) }; f().unwrap();
```

You can also set specific latency and bandwidth for the link:

```
# use netsim_core::{network::Network, Latency, Bandwidth};
# use std::time::Duration;
# fn f() -> anyhow::Result<()> {
# let mut network: Network<()> = Network::new();
# let n1 = network.new_node().build();
# let n2 = network.new_node().build();
network
  .configure_link(n1, n2)
  .set_latency(Latency::new(Duration::from_millis(20)))
  .set_bandwidth("100mbps".parse()?)
  .apply();
# Ok(()) }; f().unwrap();
```

# Sending packets

Now sending a packet is easy: just call [`Network::send`]

```
# type Data = ();
# use netsim_core::network::{Network, Packet};
# fn f() -> anyhow::Result<()> {
# let mut network: Network<Data> = Network::new();
# let n1 = network.new_node().build();
# let n2 = network.new_node().build();
# network.configure_link(n1, n2).apply();
# let packet = Packet::builder(network.packet_id_generator()).from(n1).to(n2).data(()).build().unwrap();
let packet_id = network.send(packet)?;
# Ok(()) }; f().unwrap();
```

And you are nearly there now. The only things missing is to **advance**
the network. This is because this is the core library and it allows
you to control the clock of the network. You decide how fast time
moves.

```
# type Data = ();
# use netsim_core::network::{Network, Packet};
# use std::time::Duration;
# fn f() -> anyhow::Result<()> {
# let mut network: Network<Data> = Network::new();
# let n1 = network.new_node().build();
# let n2 = network.new_node().build();
# network.configure_link(n1, n2).apply();
# let packet = Packet::builder(network.packet_id_generator()).from(n1).to(n2).data(()).build().unwrap();
# let _ = network.send(packet)?;
network.advance_with(
  Duration::from_millis(300),
  |packet| {
    // handle packets that are finalised
    // only packets that are completed will be called by this handle
  }
);
# Ok(()) }; f().unwrap();
```

[`Latency`]: crate::Latency
[`Bandwidth`]: crate::Bandwidth
[`Network`]: crate::Network
[`Network::send`]: crate::Network::send
[`Node`]: crate::node::Node
[`Packet`]: crate::Packet
*/

pub mod data;
pub mod defaults;
pub mod geo;
pub mod link;
pub mod measure;
pub mod network;
pub mod node;
pub mod time;

#[cfg(test)]
use std::time::Duration;

pub use self::{
    link::{Link, LinkId},
    measure::{
        Bandwidth, Latency, PacketLoss, PacketLossParseError, PacketLossRate, PacketLossRateError,
    },
    network::{
        LinkBuilder, Network, Packet, PacketBuildError, PacketBuilder, PacketId, RouteError,
        SendError,
    },
    node::{Node, NodeId},
};

#[test]
fn minimum_step_duration_reflects_most_constrained_channel() {
    let mut network = Network::<()>::new();

    // Empty network: no channels, no constraint.
    assert_eq!(network.minimum_step_duration(), Duration::ZERO);

    let a = network.new_node().build();
    let b = network.new_node().build();

    // Default node bandwidth is MAX — minimum step is 1 µs, not a concern.
    // Add a 10 Kbps link: ceil(8_000_000 / 10_000) = 800 µs.
    network
        .configure_link(a, b)
        .set_bandwidth(Bandwidth::new(10_000))
        .apply();
    assert_eq!(network.minimum_step_duration(), Duration::from_micros(800));

    // Adding a slower node upload tightens the constraint further.
    // 1 Kbps upload: ceil(8_000_000 / 1_000) = 8_000 µs.
    network
        .new_node()
        .set_upload_bandwidth(Bandwidth::new(1_000))
        .build();
    assert_eq!(
        network.minimum_step_duration(),
        Duration::from_micros(8_000),
    );
}

#[test]
fn simple() {
    let mut network = Network::<()>::new();
    let n1 = network.new_node().build();
    let n2 = network.new_node().build();

    network.configure_link(n1, n2).apply();

    let packet = Packet::builder(network.packet_id_generator())
        .from(n1)
        .to(n2)
        .data(())
        .build()
        .unwrap();

    network.send(packet).unwrap();

    let mut packet_received = false;
    network.advance_with(Duration::from_millis(5), |_| {
        packet_received = true;
    });

    assert!(packet_received);
}
