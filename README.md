# Network Simulator

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
- [ ] move all of the common features of the `SimContext` and `SimMux` to the `netsim-core`
- [ ] add policies to mitigate the dropping of packets
- [ ] update Context/Mux to distribute the `SimId`