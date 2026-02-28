# netsim-core

Low-level, deterministic network simulation primitives for Rust.

`netsim-core` provides the data structures and algorithms for modelling
nodes, links, and in-flight packets. It deliberately does **not** manage
wall-clock time or threads — you control when time advances, which makes it
easy to run simulations faster than real time, pause them for inspection,
or replay them deterministically.

For a batteries-included, thread-driven experience with real-time pacing,
use the [`netsim`](../netsim) crate instead.

## Features

- **Deterministic simulation** — seed the RNG for fully reproducible runs.
- **User-controlled clock** — you decide the step size and when time advances.
- **Full-duplex links** — each direction has independent bandwidth; saturating
  one direction does not affect throughput in the other.
- **Configurable per-node and per-link parameters** — upload/download bandwidth,
  buffer sizes, latency, and packet loss rate.
- **UDP semantics** — no acknowledgements, no retries. Dropped packets are
  silent, matching real-world UDP behaviour.
- **Zero-copy friendly** — send any `T: Data + Send + 'static` through the
  network. An optional FFI drop handler supports C-allocated payloads.

## Quick start

Add to your `Cargo.toml`:

```toml
[dependencies]
netsim-core = "0.1"
```

### Example

```rust
use netsim_core::{Bandwidth, Latency, network::{Network, Packet}};
use std::time::Duration;

fn main() {
    // 1. Create a network
    let mut network = Network::<&str>::new();

    // 2. Add nodes
    let sender = network
        .new_node()
        .set_upload_bandwidth(Bandwidth::new(80_000_000)) // 80 Mbps
        .build();
    let receiver = network.new_node().build();

    // 3. Connect them with a link
    network
        .configure_link(sender, receiver)
        .set_latency(Latency::new(Duration::from_millis(50)))
        .set_bandwidth(Bandwidth::new(100_000_000)) // 100 Mbps
        .apply();

    // 4. Build and send a packet
    let packet = Packet::builder(network.packet_id_generator())
        .from(sender)
        .to(receiver)
        .data("Hello, network!")
        .build()
        .unwrap();
    network.send(packet).unwrap();

    // 5. Advance time — the closure is called for each delivered packet
    network.advance_with(Duration::from_millis(100), |pkt| {
        println!("{} -> {}: {}", pkt.from(), pkt.to(), pkt.into_inner());
    });
}
```

## Core concepts

### Network

The [`Network<T>`] is the entry point. It holds all nodes, links, and
in-flight packets. The type parameter `T` is your message type.

### Nodes

A [`Node`] represents a network endpoint with:
- An **upload buffer** — bytes queued for sending. If full, `send()` returns
  `SendError::SenderBufferFull`.
- A **download buffer** — bytes that have arrived but haven't been read yet.
  If it overflows, the packet is marked corrupted and silently dropped.
- Per-node upload and download **bandwidth** limits.

Create nodes with `network.new_node()` which returns a builder:

```rust
# use netsim_core::{Bandwidth, network::Network};
# let mut network = Network::<()>::new();
let node = network
    .new_node()
    .set_upload_bandwidth(Bandwidth::new(10_000_000)) // 10 Mbps
    .set_upload_buffer(1_024 * 1_024)                 // 1 MB
    .build();
```

### Links

A [`Link`] connects two nodes. It has:
- **Latency** — one-way delay before bytes start flowing.
- **Bandwidth** — applied independently per direction (full-duplex).
- **Packet loss** — probabilistic drop rate (0% to 100%).

```rust
# use netsim_core::{Bandwidth, Latency, PacketLoss, network::Network};
# use std::time::Duration;
# let mut network = Network::<()>::new();
# let a = network.new_node().build();
# let b = network.new_node().build();
network
    .configure_link(a, b)
    .set_latency(Latency::new(Duration::from_millis(20)))
    .set_bandwidth(Bandwidth::new(100_000_000))
    .set_packet_loss(PacketLoss::rate(0.01).unwrap()) // 1% loss
    .apply();
```

Nodes are **not connected by default**. You must call `configure_link()`
before sending packets between them.

### Packets and the `Data` trait

Any type `T` sent through the network must implement the [`Data`] trait:

```rust
use netsim_core::data::Data;

struct MyMessage {
    payload: Vec<u8>,
}

impl Data for MyMessage {
    fn bytes_size(&self) -> u64 {
        self.payload.len() as u64
    }
}
```

The byte size drives bandwidth and buffer accounting. Common types (`()`,
`u8`, `String`, `Vec<u8>`, `Box<[u8]>`, `[u8; N]`, `&'static str`) already
implement `Data`.

### Advancing time

Call `advance_with()` to step the simulation forward. The closure receives
each packet that completes transit during that step:

```rust
# use netsim_core::network::Network;
# use std::time::Duration;
# let mut network = Network::<()>::new();
network.advance_with(Duration::from_millis(1), |packet| {
    // Handle delivered packet
});
```

Choose your step size to match your simulation needs. Use
`network.minimum_step_duration()` to find the smallest step that lets every
configured bandwidth channel transfer at least 1 byte.

### Deterministic replay

Seed the network's RNG for reproducible packet-loss sequences:

```rust
# use netsim_core::network::Network;
let mut network = Network::<()>::new();
network.set_seed(42); // same seed = same drops every time
```

## Inspecting network state

After any step you can query the network directly:

```rust
# use netsim_core::{network::Network, link::LinkId};
# let mut network = Network::<()>::new();
# let n1 = network.new_node().build();
# let n2 = network.new_node().build();
# network.configure_link(n1, n2).apply();
// Node state
let node = network.node(n1).unwrap();
println!("upload buffer: {}/{}", node.upload_buffer_used(), node.upload_buffer_max());

// Link state
let link_id = LinkId::new((n1, n2));
let link = network.link(link_id).unwrap();
println!("latency: {}, loss: {}", link.latency(), link.packet_loss());

// Network-wide
println!("packets in flight: {}", network.packets_in_transit());
println!("simulation round:  {:?}", network.round());
```

## Bandwidth string parsing

Bandwidth values can be parsed from human-readable strings:

```rust
# use netsim_core::Bandwidth;
let bw: Bandwidth = "100mbps".parse().unwrap();
assert_eq!(bw, Bandwidth::new(100_000_000));

let bw: Bandwidth = "1.5gbps".parse().unwrap();
assert_eq!(bw, Bandwidth::new(1_500_000_000));
```

Supported units: `bps`, `kbps`, `mbps`, `gbps` (SI, 1 kbps = 1000 bps).

## License

Apache-2.0
