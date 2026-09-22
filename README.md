# Quill

[![Crates.io](https://img.shields.io/crates/v/quill-pubsub.svg?style=flat-square)](https://crates.io/crates/quill-pubsub)
[![Docs](https://img.shields.io/docsrs/quill-pubsub?style=flat-square)](https://docs.rs/quill-pubsub)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue?style=flat-square)](https://www.apache.org/licenses/LICENSE-2.0)
[![Rust Edition](https://img.shields.io/badge/edition-2024-orange?style=flat-square&logo=rust)](https://doc.rust-lang.org/edition-guide/rust-2024/)
[![Build](https://img.shields.io/github/actions/workflow/status/Pyr33x/quill/ci.yml?branch=master&style=flat-square&logo=github&label=build)](https://github.com/Pyr33x/quill/actions/workflows/ci.yml)
[![Tests](https://img.shields.io/github/actions/workflow/status/Pyr33x/quill/ci.yml?branch=master&style=flat-square&logo=github&label=tests)](https://github.com/Pyr33x/quill/actions/workflows/ci.yml)

A small Rust library for **pub/sub topic matching** — hierarchical topics, MQTT-style wildcards (`+` / `#`), and a concurrent trie-backed registry.

No networking, wire protocols, persistence, or delivery queues. Quill is the matching core you embed in a broker or event system.

## Install

```bash
cargo add quill-pubsub
```

## Usage

```rust
use quill::{Pattern, Registry, Topic};

let registry = Registry::new();
registry.subscribe(Pattern::parse("sensors/+/temp").unwrap(), "subscriber-a");
registry.subscribe(Pattern::parse("sensors/#").unwrap(), "subscriber-b");

let topic = Topic::parse("sensors/kitchen/temp").unwrap();
let matched = registry.matching(&topic);
// matched contains "subscriber-a" and "subscriber-b" (order unspecified)

registry.fanout(&topic, &42_u32, |subscriber, payload| {
    println!("{subscriber} <- {payload}");
});
```

## API

| Type | Role |
|------|------|
| [`Topic`](src/topic.rs) | Parsed hierarchical path (`sensors/kitchen/temp`) |
| [`Pattern`](src/pattern.rs) | Subscription filter with `+` and `#` |
| [`Registry`](src/registry.rs) | Concurrent subscribe / unsubscribe / match / fan-out |

## License

Licensed under the [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0).
