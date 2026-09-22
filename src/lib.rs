//! Quill is a lightweight pub/sub **topic-matching** engine.
//!
//! It parses hierarchical topics, matches MQTT-style subscription patterns
//! (`+` single-level, `#` multi-level), and routes publishes to matching
//! subscribers via a trie-backed [`Registry`].
//!
//! This crate intentionally does **not** provide networking, persistence,
//! QoS, or delivery queues — consumers build those on top of match results.

#![deny(missing_docs)]

mod pattern;
mod registry;
mod topic;

pub use pattern::{Pattern, Segment};
pub use registry::Registry;
pub use topic::{DEFAULT_DELIMITER, ParseError, Topic};
