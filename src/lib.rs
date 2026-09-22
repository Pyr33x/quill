//! Quill is a lightweight pub/sub topic-matching engine.
//!
//! It parses hierarchical topics and matches MQTT-style subscription patterns
//! (`+` single-level, `#` multi-level).
//!
//! This crate intentionally does **not** provide networking, persistence,
//! QoS, or delivery queues — consumers build those on top of match results.

#![deny(missing_docs)]

mod pattern;
mod topic;

pub use pattern::{Pattern, Segment};
pub use topic::{ParseError, Topic, DEFAULT_DELIMITER};
