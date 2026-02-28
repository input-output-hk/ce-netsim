# Network Simulator

[![Lints](https://github.com/input-output-hk/ce-netsim/actions/workflows/lints.yml/badge.svg)](https://github.com/input-output-hk/ce-netsim/actions/workflows/lints.yml)
[![Tests](https://github.com/input-output-hk/ce-netsim/actions/workflows/tests.yml/badge.svg)](https://github.com/input-output-hk/ce-netsim/actions/workflows/tests.yml)

In-process network simulation for testing distributed protocols. Build
a topology of nodes connected by configurable links — each with its own
bandwidth, latency, and packet-loss rate — and observe how your protocol
behaves under realistic network conditions, all without touching real
sockets.

[**Try the interactive demo**](https://input-output-hk.github.io/ce-netsim/)

## Why netsim-core?

`netsim-core` is the low-level engine that makes this possible. It models
**UDP-style** message passing with:

- **Per-node bandwidth and buffers** — upload and download are independent,
  each with configurable throughput and buffer capacity.
- **Full-duplex links** — each direction (A→B and B→A) has its own bandwidth
  channel. Saturating one direction does not affect the other.
- **Latency and packet loss** — links introduce configurable delay and
  probabilistic drops.
- **Deterministic time** — you control when the clock advances, so simulations
  can run faster than real time, be paused for inspection, or replayed exactly.

```text
Network::send()
      │
      ▼ upload buffer (bytes wait here until bandwidth allows)
 [ Sender Node ]
      │
      │ outbound channel (upload bandwidth limit)
      ▼
  [ Link ] ─── latency ──► delivers after N ms of simulated time
      │          └─ bandwidth per direction
      │          └─ packet loss (probabilistic drop)
      ▼
 [ Recipient Node ]
      │ inbound channel (download bandwidth limit)
      ▼ download buffer (bytes wait here until advance_with delivers them)
Network::advance_with() closure receives the packet
```

Because `netsim-core` owns no threads and no wall-clock, it compiles to
**any target** — native, WASM, embedded — and integrates into any test
harness or simulation loop you already have.

## Crates

| Crate | Description |
|-------|-------------|
| **netsim-core** | Tick-based simulation engine (no threads, no IO) |
| **netsim** | Batteries-included wrapper with real-time pacing and async support |
| **netsim-demo** | Interactive browser playground (Leptos + WASM) |

## Quick start

```rust
use netsim_core::network::{Network, Packet};
use std::time::Duration;

let mut network: Network<&str> = Network::new();

let n1 = network.new_node().build();
let n2 = network
    .new_node()
    .set_download_bandwidth("100mbps".parse().unwrap())
    .build();

network.configure_link(n1, n2)
    .set_latency(netsim_core::Latency::new(Duration::from_millis(20)))
    .apply();

let packet = Packet::builder(network.packet_id_generator())
    .from(n1)
    .to(n2)
    .data("hello")
    .build()
    .unwrap();

network.send(packet).unwrap();

network.advance_with(Duration::from_millis(50), |pkt| {
    println!("delivered: {:?}", pkt.data());
});
```

## Examples

```sh
cargo run --example simple
cargo run --example simple_async
```

## Documentation

```sh
cargo doc --open --no-deps
```

## License

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.

[Apache-2.0](LICENSE)

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
