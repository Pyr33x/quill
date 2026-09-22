//! Hierarchical topic paths.

use std::fmt;

/// Default segment delimiter (MQTT-style).
pub const DEFAULT_DELIMITER: char = '/';

/// Error returned when parsing a [`Topic`] or [`Pattern`](crate::Pattern).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The input string was empty.
    Empty,
    /// A segment was empty (leading, trailing, or doubled delimiter).
    EmptySegment,
    /// A multi-level wildcard (`#`) appeared somewhere other than the final segment.
    MultiLevelNotFinal,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "topic or pattern must not be empty"),
            Self::EmptySegment => {
                write!(f, "empty segment (leading, trailing, or doubled delimiter)")
            }
            Self::MultiLevelNotFinal => {
                write!(f, "multi-level wildcard '#' must be the final segment")
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// A hierarchical topic path made of non-empty segments.
///
/// Topics never contain wildcards; use [`Pattern`](crate::Pattern) for filters.
///
/// # Examples
///
/// ```
/// use quill::Topic;
///
/// let topic = Topic::parse("sensors/kitchen/temp").unwrap();
/// assert_eq!(topic.segments(), &["sensors", "kitchen", "temp"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Topic {
    segments: Vec<String>,
    delimiter: char,
}

impl Topic {
    /// Parse `input` using the [default delimiter](DEFAULT_DELIMITER).
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        Self::parse_with(input, DEFAULT_DELIMITER)
    }

    /// Parse `input` using a custom segment `delimiter`.
    pub fn parse_with(input: &str, delimiter: char) -> Result<Self, ParseError> {
        let segments = split_segments(input, delimiter)?;
        Ok(Self {
            segments,
            delimiter,
        })
    }

    /// Borrowed topic segments in order.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// The delimiter used when this topic was parsed.
    pub fn delimiter(&self) -> char {
        self.delimiter
    }
}

impl TryFrom<&str> for Topic {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl fmt::Display for Topic {
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

/// Split `input` on `delimiter`, rejecting empty input and empty segments.
pub(crate) fn split_segments(input: &str, delimiter: char) -> Result<Vec<String>, ParseError> {
    if input.is_empty() {
        return Err(ParseError::Empty);
    }

    let mut segments = Vec::new();
    for part in input.split(delimiter) {
        if part.is_empty() {
            return Err(ParseError::EmptySegment);
        }
        segments.push(part.to_owned());
    }
    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_happy_path() {
        let t = Topic::parse("sensors/kitchen/temp").unwrap();
        assert_eq!(t.segments(), &["sensors", "kitchen", "temp"]);
        assert_eq!(t.delimiter(), '/');
        assert_eq!(t.to_string(), "sensors/kitchen/temp");
    }

    #[test]
    fn parse_single_segment() {
        let t = Topic::parse("sensors").unwrap();
        assert_eq!(t.segments(), &["sensors"]);
    }

    #[test]
    fn parse_custom_delimiter() {
        let t = Topic::parse_with("a.b.c", '.').unwrap();
        assert_eq!(t.segments(), &["a", "b", "c"]);
        assert_eq!(t.delimiter(), '.');
        assert_eq!(t.to_string(), "a.b.c");
    }

    #[test]
    fn reject_empty() {
        assert_eq!(Topic::parse(""), Err(ParseError::Empty));
    }

    #[test]
    fn reject_leading_delimiter() {
        assert_eq!(Topic::parse("/a"), Err(ParseError::EmptySegment));
    }

    #[test]
    fn reject_trailing_delimiter() {
        assert_eq!(Topic::parse("a/"), Err(ParseError::EmptySegment));
    }

    #[test]
    fn reject_doubled_delimiter() {
        assert_eq!(Topic::parse("a//b"), Err(ParseError::EmptySegment));
    }

    #[test]
    fn try_from_str() {
        let t = Topic::try_from("x/y").unwrap();
        assert_eq!(t.segments(), &["x", "y"]);
    }
}
