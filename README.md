# Quill

A small Rust library for **pub/sub topic matching** — hierarchical topics, MQTT-style wildcards (`+` / `#`), and a trie-backed concurrent registry.

Quill does **not** implement networking, wire protocols, persistence, or delivery queues. Use it as the matching core inside a broker or event system.

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

## API overview

| Type | Role |
|------|------|
| [`Topic`](src/topic.rs) | Parsed hierarchical path (`sensors/kitchen/temp`) |
| [`Pattern`](src/pattern.rs) | Subscription filter with `+` and `#` |
| [`Registry`](src/registry.rs) | Concurrent subscribe / unsubscribe / match / fan-out |

## License

Licensed under the Apache License, Version 2.0.
