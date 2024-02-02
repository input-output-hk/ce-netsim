# Network Simulator

## Doc

run `cargo doc --open --no-deps`

## Example

In the `simple.rs` example we show how a message can be delayed. In this
example you should see the message took approximately 1seconds to be received
by `net2` from `net1`.

```
cargo run --example simple
```