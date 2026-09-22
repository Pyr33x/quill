//! Quill is a lightweight pub/sub topic-matching engine.
//!
//! This crate intentionally does **not** provide networking, persistence,
//! QoS, or delivery queues — consumers build those on top of match results.

#![deny(missing_docs)]
