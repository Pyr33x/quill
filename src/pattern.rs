//! Subscription patterns with MQTT-style wildcards.

use std::fmt;

use crate::topic::{DEFAULT_DELIMITER, ParseError, Topic, split_segments};

/// One segment of a [`Pattern`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Segment {
    /// Exact segment match.
    Literal(String),
    /// Single-level wildcard (`+`): matches exactly one topic segment.
    Single,
    /// Multi-level wildcard (`#`): matches zero or more remaining segments.
    /// Must be the final segment of a pattern.
    Multi,
}

impl Segment {
    fn from_raw(raw: &str) -> Self {
        match raw {
            "+" => Self::Single,
            "#" => Self::Multi,
            other => Self::Literal(other.to_owned()),
        }
    }
}

impl fmt::Display for Segment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literal(s) => write!(f, "{s}"),
            Self::Single => write!(f, "+"),
            Self::Multi => write!(f, "#"),
        }
    }
}

/// A subscription filter that may include wildcards.
///
/// Wildcards follow MQTT conventions:
/// - `+` matches exactly one level
/// - `#` matches the remainder of the topic (including zero levels) and may
///   only appear as the **last** segment
///
/// # Examples
///
/// ```
/// use quill::{Pattern, Topic};
///
/// let pat = Pattern::parse("sensors/+/temp").unwrap();
/// let topic = Topic::parse("sensors/kitchen/temp").unwrap();
/// assert!(pat.matches(&topic));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Pattern {
    segments: Vec<Segment>,
    delimiter: char,
}

impl Pattern {
    /// Parse `input` using the [default delimiter](DEFAULT_DELIMITER).
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        Self::parse_with(input, DEFAULT_DELIMITER)
    }

    /// Parse `input` using a custom segment `delimiter`.
    pub fn parse_with(input: &str, delimiter: char) -> Result<Self, ParseError> {
        let raw = split_segments(input, delimiter)?;
        let segments: Vec<Segment> = raw.iter().map(|s| Segment::from_raw(s)).collect();
        validate_segments(&segments)?;
        Ok(Self {
            segments,
            delimiter,
        })
    }

    /// Borrowed pattern segments in order.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// The delimiter used when this pattern was parsed.
    pub fn delimiter(&self) -> char {
        self.delimiter
    }

    /// Returns `true` if this pattern matches `topic`.
    ///
    /// Matching compares segments only; delimiters need not be equal as long
    /// as both values were parsed consistently by the caller.
    pub fn matches(&self, topic: &Topic) -> bool {
        matches_rec(self.segments(), topic.segments(), 0, 0)
    }
}

impl TryFrom<&str> for Pattern {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, seg) in self.segments.iter().enumerate() {
            if i > 0 {
                write!(f, "{}", self.delimiter)?;
            }
            write!(f, "{seg}")?;
        }
        Ok(())
    }
}

fn validate_segments(segments: &[Segment]) -> Result<(), ParseError> {
    let last = segments.len().saturating_sub(1);
    for (i, seg) in segments.iter().enumerate() {
        if matches!(seg, Segment::Multi) && i != last {
            return Err(ParseError::MultiLevelNotFinal);
        }
    }
    Ok(())
}

fn matches_rec(pat: &[Segment], topic: &[String], pi: usize, ti: usize) -> bool {
    if pi == pat.len() {
        return ti == topic.len();
    }

    match &pat[pi] {
        Segment::Multi => {
            // `#` must be final (validated at parse); matches any suffix.
            true
        }
        Segment::Single => {
            if ti >= topic.len() {
                return false;
            }
            matches_rec(pat, topic, pi + 1, ti + 1)
        }
        Segment::Literal(lit) => {
            if ti >= topic.len() || topic[ti] != *lit {
                return false;
            }
            matches_rec(pat, topic, pi + 1, ti + 1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn topic(s: &str) -> Topic {
        Topic::parse(s).unwrap()
    }

    fn pattern(s: &str) -> Pattern {
        Pattern::parse(s).unwrap()
    }

    #[test]
    fn parse_literals_and_wildcards() {
        let p = pattern("sensors/+/temp/#");
        assert_eq!(
            p.segments(),
            &[
                Segment::Literal("sensors".into()),
                Segment::Single,
                Segment::Literal("temp".into()),
                Segment::Multi,
            ]
        );
    }

    #[test]
    fn parse_hash_alone() {
        let p = pattern("#");
        assert_eq!(p.segments(), &[Segment::Multi]);
    }

    #[test]
    fn reject_multi_not_final() {
        assert_eq!(
            Pattern::parse("sensors/#/temp"),
            Err(ParseError::MultiLevelNotFinal)
        );
    }

    #[test]
    fn reject_empty_and_bad_segments() {
        assert_eq!(Pattern::parse(""), Err(ParseError::Empty));
        assert_eq!(Pattern::parse("/a"), Err(ParseError::EmptySegment));
        assert_eq!(Pattern::parse("a/"), Err(ParseError::EmptySegment));
        assert_eq!(Pattern::parse("a//b"), Err(ParseError::EmptySegment));
    }

    #[test]
    fn plus_is_only_whole_segment() {
        // "foo+" is a literal, not a wildcard.
        let p = pattern("foo+");
        assert_eq!(p.segments(), &[Segment::Literal("foo+".into())]);
        assert!(p.matches(&topic("foo+")));
        assert!(!p.matches(&topic("foo")));
    }

    #[test]
    fn exact_match() {
        let p = pattern("sensors/kitchen/temp");
        assert!(p.matches(&topic("sensors/kitchen/temp")));
        assert!(!p.matches(&topic("sensors/kitchen/humidity")));
        assert!(!p.matches(&topic("sensors/kitchen")));
    }

    #[test]
    fn single_level_wildcard() {
        let p = pattern("sensors/+/temp");
        assert!(p.matches(&topic("sensors/kitchen/temp")));
        assert!(p.matches(&topic("sensors/garage/temp")));
        assert!(!p.matches(&topic("sensors/kitchen/humidity")));
        assert!(!p.matches(&topic("sensors/kitchen/pantry/temp")));
        assert!(!p.matches(&topic("sensors/temp")));
    }

    #[test]
    fn multi_level_wildcard() {
        let p = pattern("sensors/#");
        assert!(p.matches(&topic("sensors")));
        assert!(p.matches(&topic("sensors/kitchen")));
        assert!(p.matches(&topic("sensors/kitchen/temp")));
        assert!(!p.matches(&topic("actuators/relay")));
    }

    #[test]
    fn hash_matches_everything() {
        let p = pattern("#");
        assert!(p.matches(&topic("a")));
        assert!(p.matches(&topic("a/b/c")));
    }

    #[test]
    fn mixed_wildcards() {
        let p = pattern("+/+/temp/#");
        assert!(p.matches(&topic("sensors/kitchen/temp")));
        assert!(p.matches(&topic("sensors/kitchen/temp/celsius")));
        assert!(!p.matches(&topic("sensors/kitchen/humidity")));
    }

    #[test]
    fn display_round_trip_shape() {
        let p = pattern("a/+/b/#");
        assert_eq!(p.to_string(), "a/+/b/#");
    }

    #[test]
    fn custom_delimiter() {
        let p = Pattern::parse_with("a.+.b", '.').unwrap();
        let t = Topic::parse_with("a.x.b", '.').unwrap();
        assert!(p.matches(&t));
    }
}
