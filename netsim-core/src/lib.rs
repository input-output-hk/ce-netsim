/*!
# `netsim` core library

This crate provides the core tools for implementing network protocol simulations.

It currently implements a [*UDP*] style of message passing:

* messages are sent without expecting an _acknowledgement_.
* messages are stored in the receiver's buffer. Not reading messages will
  mean new messages will be dropped and lost.

When dealing directly with the core library it is important to note that
the passage of time is directly handled by the user of the library.
This allows to run simulations at different speed to observe certain
conditions.

Though for a more _realistic_ experience it is preferable to utilise
[`netsim`] or [`netsim-async`] crates.

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
pub mod stats;
pub mod time;

#[cfg(test)]
use std::time::Duration;

pub use self::{
    link::LinkId,
    measure::{Bandwidth, Latency, PacketLoss},
    network::{LinkBuilder, Network, Packet, PacketBuilder, PacketId, RouteError, SendError},
    node::NodeId,
    stats::{LinkStats, NetworkStats, NodeStats},
};

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
