//! Concurrent subscription registry backed by a topic-segment trie.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::RwLock;

use crate::pattern::{Pattern, Segment};
use crate::topic::Topic;

/// Concurrent registry that maps subscription [`Pattern`]s to subscriber handles.
///
/// `S` is an opaque subscriber identity/handle chosen by the caller (for example
/// an ID, `Arc<Sender<_>>`, or similar). Quill never assumes sockets or channels.
///
/// # Concurrency
///
/// The registry is wrapped in [`std::sync::RwLock`]: publishes (matches) take a
/// read lock and can proceed in parallel, while subscribe/unsubscribe take a
/// write lock. That matches the expected read-heavy pub/sub workload without
/// pulling in an external lock crate.
///
/// # Examples
///
/// ```
/// use quill::{Pattern, Registry, Topic};
///
/// let registry = Registry::new();
/// registry.subscribe(Pattern::parse("sensors/+/temp").unwrap(), "sub-1");
///
/// let topic = Topic::parse("sensors/kitchen/temp").unwrap();
/// let matched = registry.matching(&topic);
/// assert_eq!(matched, vec!["sub-1"]);
/// ```
#[derive(Debug, Default)]
pub struct Registry<S> {
    inner: RwLock<Inner<S>>,
}

#[derive(Debug)]
struct Inner<S> {
    root: Node<S>,
    /// Reverse index for efficient unsubscribe.
    by_subscriber: HashMap<S, HashSet<Pattern>>,
}

#[derive(Debug)]
struct Node<S> {
    /// Exact-segment children.
    children: HashMap<String, Node<S>>,
    /// Child for the single-level wildcard `+`.
    single: Option<Box<Node<S>>>,
    /// Subscribers whose pattern ends with `#` at this node.
    multi: HashSet<S>,
    /// Subscribers whose pattern ends at this node (no remaining segments).
    subscribers: HashSet<S>,
}

impl<S> Default for Node<S> {
    fn default() -> Self {
        Self {
            children: HashMap::new(),
            single: None,
            multi: HashSet::new(),
            subscribers: HashSet::new(),
        }
    }
}

impl<S> Default for Inner<S> {
    fn default() -> Self {
        Self {
            root: Node::default(),
            by_subscriber: HashMap::new(),
        }
    }
}

impl<S> Registry<S>
where
    S: Clone + Eq + Hash,
{
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(Inner::default()),
        }
    }

    /// Register `subscriber` under `pattern`.
    ///
    /// Subscribing the same `(pattern, subscriber)` pair again is a no-op.
    pub fn subscribe(&self, pattern: Pattern, subscriber: S) {
        let mut inner = self.inner.write().expect("registry lock poisoned");
        insert_pattern(&mut inner.root, pattern.segments(), &subscriber);
        inner
            .by_subscriber
            .entry(subscriber)
            .or_default()
            .insert(pattern);
    }

    /// Remove `subscriber` from `pattern`.
    ///
    /// Returns `true` if the subscription existed.
    pub fn unsubscribe(&self, pattern: &Pattern, subscriber: &S) -> bool {
        let mut inner = self.inner.write().expect("registry lock poisoned");
        let Some(patterns) = inner.by_subscriber.get_mut(subscriber) else {
            return false;
        };
        if !patterns.remove(pattern) {
            return false;
        }
        if patterns.is_empty() {
            inner.by_subscriber.remove(subscriber);
        }
        remove_pattern(&mut inner.root, pattern.segments(), subscriber);
        true
    }

    /// Remove `subscriber` from every pattern it is registered under.
    ///
    /// Returns the number of patterns removed.
    pub fn unsubscribe_all(&self, subscriber: &S) -> usize {
        let mut inner = self.inner.write().expect("registry lock poisoned");
        let Some(patterns) = inner.by_subscriber.remove(subscriber) else {
            return 0;
        };
        let count = patterns.len();
        for pattern in patterns {
            remove_pattern(&mut inner.root, pattern.segments(), subscriber);
        }
        count
    }

    /// Return clones of every subscriber whose pattern matches `topic`.
    ///
    /// Order is unspecified. Duplicate handles are possible only if the same
    /// subscriber was registered under multiple matching patterns; each
    /// distinct `(pattern, subscriber)` contributes at most once per pattern,
    /// but the same `S` may appear once per matching pattern. For a deduplicated
    /// set, collect into a [`HashSet`].
    pub fn matching(&self, topic: &Topic) -> Vec<S> {
        let inner = self.inner.read().expect("registry lock poisoned");
        let mut out = Vec::new();
        collect_matches(&inner.root, topic.segments(), 0, &mut out);
        out
    }

    /// Invoke `f` for each matching subscriber with the given `payload`.
    ///
    /// This performs no delivery, queuing, or backpressure — it only fans out
    /// the match set to the callback.
    pub fn fanout<T>(&self, topic: &Topic, payload: &T, mut f: impl FnMut(&S, &T)) {
        for subscriber in self.matching(topic) {
            f(&subscriber, payload);
        }
    }
}

fn insert_pattern<S: Clone + Eq + Hash>(node: &mut Node<S>, segments: &[Segment], subscriber: &S) {
    if segments.is_empty() {
        node.subscribers.insert(subscriber.clone());
        return;
    }

    match &segments[0] {
        Segment::Multi => {
            // `#` is always final (enforced by Pattern::parse).
            node.multi.insert(subscriber.clone());
        }
        Segment::Single => {
            let child = node.single.get_or_insert_with(|| Box::new(Node::default()));
            insert_pattern(child, &segments[1..], subscriber);
        }
        Segment::Literal(lit) => {
            let child = node.children.entry(lit.clone()).or_default();
            insert_pattern(child, &segments[1..], subscriber);
        }
    }
}

fn remove_pattern<S: Eq + Hash>(node: &mut Node<S>, segments: &[Segment], subscriber: &S) {
    if segments.is_empty() {
        node.subscribers.remove(subscriber);
        return;
    }

    match &segments[0] {
        Segment::Multi => {
            node.multi.remove(subscriber);
        }
        Segment::Single => {
            if let Some(child) = node.single.as_mut() {
                remove_pattern(child, &segments[1..], subscriber);
                if child.is_empty() {
                    node.single = None;
                }
            }
        }
        Segment::Literal(lit) => {
            if let Some(child) = node.children.get_mut(lit) {
                remove_pattern(child, &segments[1..], subscriber);
                if child.is_empty() {
                    node.children.remove(lit);
                }
            }
        }
    }
}

impl<S> Node<S> {
    fn is_empty(&self) -> bool {
        self.children.is_empty()
            && self.single.is_none()
            && self.multi.is_empty()
            && self.subscribers.is_empty()
    }
}

fn collect_matches<S: Clone>(node: &Node<S>, topic: &[String], depth: usize, out: &mut Vec<S>) {
    // Any `#` registered at this node matches the remaining suffix (incl. empty).
    out.extend(node.multi.iter().cloned());

    if depth == topic.len() {
        out.extend(node.subscribers.iter().cloned());
        return;
    }

    let seg = &topic[depth];

    if let Some(child) = node.children.get(seg) {
        collect_matches(child, topic, depth + 1, out);
    }

    if let Some(child) = node.single.as_ref() {
        collect_matches(child, topic, depth + 1, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::thread;

    fn pat(s: &str) -> Pattern {
        Pattern::parse(s).unwrap()
    }

    fn topic(s: &str) -> Topic {
        Topic::parse(s).unwrap()
    }

    fn matched_set(registry: &Registry<&'static str>, t: &str) -> HashSet<&'static str> {
        registry.matching(&topic(t)).into_iter().collect()
    }

    #[test]
    fn subscribe_and_match() {
        let r = Registry::new();
        r.subscribe(pat("sensors/+/temp"), "a");
        assert_eq!(
            matched_set(&r, "sensors/kitchen/temp"),
            HashSet::from(["a"])
        );
        assert!(matched_set(&r, "sensors/kitchen/humidity").is_empty());
    }

    #[test]
    fn multiple_subscribers_and_patterns() {
        let r = Registry::new();
        r.subscribe(pat("sensors/+/temp"), "exactish");
        r.subscribe(pat("sensors/#"), "multi");
        r.subscribe(pat("#"), "all");

        let set = matched_set(&r, "sensors/kitchen/temp");
        assert_eq!(set, HashSet::from(["exactish", "multi", "all"]));
    }

    #[test]
    fn unsubscribe_removes_match() {
        let r = Registry::new();
        r.subscribe(pat("a/b"), "s1");
        r.subscribe(pat("a/#"), "s1");
        assert!(r.unsubscribe(&pat("a/b"), &"s1"));
        assert_eq!(matched_set(&r, "a/b"), HashSet::from(["s1"]));
        assert!(r.unsubscribe(&pat("a/#"), &"s1"));
        assert!(matched_set(&r, "a/b").is_empty());
        assert!(!r.unsubscribe(&pat("a/b"), &"s1"));
    }

    #[test]
    fn unsubscribe_all() {
        let r = Registry::new();
        r.subscribe(pat("a/b"), "s1");
        r.subscribe(pat("x/#"), "s1");
        r.subscribe(pat("a/b"), "s2");
        assert_eq!(r.unsubscribe_all(&"s1"), 2);
        assert_eq!(matched_set(&r, "a/b"), HashSet::from(["s2"]));
        assert!(matched_set(&r, "x/y").is_empty());
    }

    #[test]
    fn overlapping_plus_and_hash() {
        let r = Registry::new();
        r.subscribe(pat("sensors/+/temp"), "plus");
        r.subscribe(pat("sensors/#"), "hash");

        assert_eq!(
            matched_set(&r, "sensors/kitchen/temp"),
            HashSet::from(["plus", "hash"])
        );
        assert_eq!(matched_set(&r, "sensors/kitchen"), HashSet::from(["hash"]));
    }

    #[test]
    fn hash_at_root() {
        let r = Registry::new();
        r.subscribe(pat("#"), "all");
        assert_eq!(matched_set(&r, "any/thing"), HashSet::from(["all"]));
    }

    #[test]
    fn fanout_invokes_callback() {
        let r = Registry::new();
        r.subscribe(pat("a/+"), 1);
        r.subscribe(pat("a/b"), 2);

        let mut seen = Vec::new();
        r.fanout(&topic("a/b"), &"payload", |sub, payload| {
            seen.push((*sub, *payload));
        });
        seen.sort_by_key(|(id, _)| *id);
        assert_eq!(seen, vec![(1, "payload"), (2, "payload")]);
    }

    #[test]
    fn concurrent_subscribe_and_match() {
        let r = Arc::new(Registry::new());
        r.subscribe(pat("n/#"), 0);

        let mut handles = Vec::new();
        for i in 1..=8 {
            let reg = Arc::clone(&r);
            handles.push(thread::spawn(move || {
                reg.subscribe(pat("n/+"), i);
                let m = reg.matching(&topic("n/x"));
                assert!(m.contains(&0));
                assert!(m.contains(&i));
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
    }
}
