# Network Simulator

[![Lints](https://github.com/input-output-hk/ce-netsim/actions/workflows/lints.yml/badge.svg)](https://github.com/input-output-hk/ce-netsim/actions/workflows/lints.yml)
[![Tests](https://github.com/input-output-hk/ce-netsim/actions/workflows/tests.yml/badge.svg)](https://github.com/input-output-hk/ce-netsim/actions/workflows/tests.yml)

Two versions are offered here, an async version and a non async version.

## Doc

run `cargo doc --open --no-deps`

## Example

In the `simple.rs` example we show how a message can be delayed. In this
example you should see the message took approximately 1seconds to be received
by `net2` from `net1`.

```
cargo run --example simple
```

## Async Example

In the `simple_async.rs` example we show how a message can be delayed. In this
example you should see the message took approximately 1seconds to be received
by `net2` from `net1`.

```
cargo run --example simple_async
```

# TODOs

- [ ] rename `netsim-util` to `netsim-core`
- [x] move configurations to the `netsim-core` crate (`SimConfiguration` & `SimSocketConfiguration`)
- [x] move all of the common features of the `SimContext` and `SimMux` to the `netsim-core`
- [ ] add policies to mitigate the dropping of packets
- [x] update Context/Mux to distribute the `SimId`
- [ ] add command/instruction to go through the bus to the `SimMux` (update on connection speeds etc.)
- [ ] add name service so that nodes may be addressable by names