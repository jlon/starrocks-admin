//! Value parsing utilities for StarRocks profile metrics
//!
//! Handles parsing of duration strings, byte sizes, numbers, and percentages
//! following StarRocks official format conventions.

use crate::services::profile_analyzer::parser::error::{ParseError, ParseResult};
use once_cell::sync::Lazy;
use regex::Regex;
use std::time::Duration;

// Legacy patterns for backward compatibility
static TIME_COMPONENT_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(-?\d+(?:\.\d+)?)\s*(ms|us|μs|ns|h|m|s)").unwrap());

static BYTES_REGEX: Lazy<Regex> = Lazy::new(|| {
    // Support formats: "558.156 GB", "2.167KB", "1024B", "0.000 B"
    Regex::new(r"^(-?\d+\.?\d*)\s*(TB|GB|MB|KB|K|M|G|T|B)\b").unwrap()
});

static NUMBER_WITH_PAREN_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[\d,.]+[KMGB]?\s*\((-?\d+(?:\.\d+)?)\)").unwrap());

static NUMBER_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(-?[\d,.]+)").unwrap());

/// Value parser for StarRocks profile metrics
pub struct ValueParser;

impl ValueParser {
    /// Parse duration string to Duration
    ///
    /// Supports formats like: "1h30m", "7s854ms", "123ms", "5.540us", "390ns"
    ///
    /// # Examples
    /// ```ignore
    /// let d = ValueParser::parse_duration("1h30m").unwrap();
    /// assert_eq!(d.as_secs(), 5400);
    /// ```
    pub fn parse_duration(input: &str) -> ParseResult<Duration> {
        let input = input.trim();

        // Handle zero case
        if input == "0" {
            return Ok(Duration::from_nanos(0));
        }

        let mut total_ns: f64 = 0.0;
        let mut found_any = false;

        // Extract all time components
        for cap in TIME_COMPONENT_REGEX.captures_iter(input) {
            found_any = true;

            let num_str = cap.get(1).unwrap().as_str();
            let num: f64 = num_str.parse().map_err(|_| {
                ParseError::ParseDurationError(format!(
                    "Invalid number '{}' in duration '{}'",
                    num_str, input
                ))
            })?;

            let unit = cap.get(2).unwrap().as_str();

            // Convert to nanoseconds based on unit
            let ns = match unit {
                "h" => num * 3600.0 * 1_000_000_000.0,
                "m" => num * 60.0 * 1_000_000_000.0,
                "s" => num * 1_000_000_000.0,
                "ms" => num * 1_000_000.0,
                "us" | "μs" => num * 1_000.0,
                "ns" => num,
                _ => 0.0,
            };

            total_ns += ns;
        }

        if !found_any {
            return Err(ParseError::ParseDurationError(format!(
                "No valid time components found in '{}'",
                input
            )));
        }

        Ok(Duration::from_nanos(total_ns as u64))
    }

    /// Parse duration string to milliseconds
    pub fn parse_time_to_ms(input: &str) -> ParseResult<f64> {
        let duration = Self::parse_duration(input)?;
        Ok(duration.as_nanos() as f64 / 1_000_000.0)
    }

    /// Parse bytes string to u64
    ///
    /// Supports formats like: "45.907 GB", "2.167KB", "1024"
    pub fn parse_bytes(input: &str) -> ParseResult<u64> {
        Self::parse_bytes_to_u64(input)
    }

    /// Parse bytes from string like "45.907 GB" to u64 bytes
    pub fn parse_bytes_to_u64(input: &str) -> ParseResult<u64> {
        let original = input.trim();
        let input = original.to_uppercase();

        // Check for parenthesized raw value first: "2.174K (2174)"
        if let Some(cap) = NUMBER_WITH_PAREN_REGEX.captures(&input) {
            let raw = cap.get(1).unwrap().as_str();
            return raw.parse::<u64>().map_err(|e| {
                ParseError::ParseBytesError(format!("Failed to parse raw bytes '{}': {}", raw, e))
            });
        }

        // Try standard byte format
        if let Some(cap) = BYTES_REGEX.captures(&input) {
            let num_str = cap.get(1).unwrap().as_str().replace(",", "");
            let num: f64 = num_str.parse().map_err(|e| {
                ParseError::ParseBytesError(format!("Invalid number '{}': {}", num_str, e))
            })?;

            let unit = cap.get(2).unwrap().as_str();

            let multiplier: f64 = match unit {
                "B" => 1.0,
                "K" | "KB" => 1024.0,
                "M" | "MB" => 1024.0 * 1024.0,
                "G" | "GB" => 1024.0 * 1024.0 * 1024.0,
                "T" | "TB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
                _ => {
                    return Err(ParseError::ParseBytesError(format!(
                        "Unknown byte unit: {}",
                        unit
                    )));
                },
            };

            return Ok((num * multiplier).floor() as u64);
        }

        // Try plain number
        let temp = input.replace(",", "");
        let cleaned = temp.split_whitespace().next().unwrap_or(&input);
        cleaned.parse::<u64>().map_err(|e| {
            ParseError::ParseBytesError(format!("Cannot parse bytes from '{}': {}", input, e))
        })
    }

    /// Parse number from string, handling various formats
    ///
    /// Supports: "2.174K (2174)", "1,234,567", "334"
    pub fn parse_number<T>(input: &str) -> ParseResult<T>
    where
        T: std::str::FromStr,
        <T as std::str::FromStr>::Err: std::fmt::Display,
    {
        let input = input.trim();

        // Check for parenthesized raw value
        if let Some(cap) = NUMBER_WITH_PAREN_REGEX.captures(input) {
            let raw = cap.get(1).unwrap().as_str();
            return raw.parse::<T>().map_err(|e| {
                ParseError::ParseNumberError(format!(
                    "Failed to parse number from parentheses '{}': {}",
                    raw, e
                ))
            });
        }

        // Try standard number format
        if let Some(cap) = NUMBER_REGEX.captures(input) {
            let num_str = cap.get(1).unwrap().as_str().replace(",", "");
            return num_str.parse::<T>().map_err(|e| {
                ParseError::ParseNumberError(format!("Failed to parse number '{}': {}", num_str, e))
            });
        }

        Err(ParseError::ParseNumberError(format!("Cannot extract number from '{}'", input)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration_hours_minutes() {
        let d = ValueParser::parse_duration("1h30m").unwrap();
        assert_eq!(d.as_secs(), 5400);
    }

    #[test]
    fn test_parse_duration_seconds_millis() {
        let d = ValueParser::parse_duration("7s854ms").unwrap();
        assert_eq!(d.as_nanos(), 7_854_000_000);
    }

    #[test]
    fn test_parse_duration_millis() {
        let d = ValueParser::parse_duration("123ms").unwrap();
        assert_eq!(d.as_nanos(), 123_000_000);

        let d = ValueParser::parse_duration("123.456ms").unwrap();
        assert_eq!(d.as_nanos(), 123_456_000);
    }

    #[test]
    fn test_parse_duration_micros() {
        let d = ValueParser::parse_duration("5.540us").unwrap();
        assert_eq!(d.as_nanos(), 5540);
    }

    #[test]
    fn test_parse_bytes_with_unit() {
        assert_eq!(ValueParser::parse_bytes("2.167KB").unwrap(), 2219);
        assert_eq!(ValueParser::parse_bytes("0.000B").unwrap(), 0);
    }

    #[test]
    fn test_parse_bytes_with_parentheses() {
        assert_eq!(ValueParser::parse_bytes("2.174K (2174)").unwrap(), 2174);
    }

    #[test]
    fn test_parse_number_with_commas() {
        let n: u64 = ValueParser::parse_number("1,234,567").unwrap();
        assert_eq!(n, 1234567);
    }
}
