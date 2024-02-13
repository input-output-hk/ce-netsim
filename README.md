# Network Simulator

[![Lints](https://github.com/input-output-hk/ce-netsim/actions/workflows/lints.yml/badge.svg)](https://github.com/input-output-hk/ce-netsim/actions/workflows/lints.yml)
[![Tests](https://github.com/input-output-hk/ce-netsim/actions/workflows/tests.yml/badge.svg)](https://github.com/input-output-hk/ce-netsim/actions/workflows/tests.yml)

Network simulator is a small rust framework to simulate network without going
outside of the process. It doesn't simulate low level network, but allow to
simulate a topology with bandwidth and delay between nodes.

The goal is to be language agnostic. This can be used in rust directly,
but we also export the framework as a C interface. We are also interested
in a WASM export. Two versions of the framework are offered here, an async
version and a sync version.

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

# License

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.

[Apache-2.0](LICENSE)

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
