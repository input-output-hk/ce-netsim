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

# License

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.

[Apache-2.0](LICENSE)

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.