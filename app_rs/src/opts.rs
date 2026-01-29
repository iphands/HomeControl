//! Configuration option types for animation modes.
//!
//! This module provides typed configuration values that can be serialized
//! to/from JSON for API communication with frontend clients.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// A typed configuration value for animation modes.
///
/// `OptValue` wraps a JSON value with type information to ensure
/// proper deserialization and validation on the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OptValue {
    /// The actual value stored as JSON.
    #[serde(rename = "val")]
    pub value: serde_json::Value,

    /// The type name for frontend type checking.
    #[serde(rename = "type")]
    pub type_name: String,
}

impl OptValue {
    /// Creates an integer option value.
    ///
    /// # Example
    /// ```
    /// let opt = OptValue::int(42);
    /// assert_eq!(opt.type_name, "int");
    /// ```
    pub fn int(val: i64) -> Self {
        Self {
            value: serde_json::Value::Number(val.into()),
            type_name: "int".to_string(),
        }
    }

    /// Creates a floating-point option value.
    ///
    /// # Panics
    /// Panics if the value is NaN or infinity.
    pub fn float(val: f64) -> Self {
        Self {
            value: serde_json::Value::Number(serde_json::Number::from_f64(val).expect("float value must be finite")),
            type_name: "float".to_string(),
        }
    }

    /// Creates a boolean option value.
    pub fn bool(val: bool) -> Self {
        Self {
            value: serde_json::Value::Bool(val),
            type_name: "bool".to_string(),
        }
    }

    /// Creates a color option value from RGB components.
    ///
    /// Colors are stored as JSON arrays `[r, g, b]` where each component
    /// is in the range 0-255.
    pub fn color(rgb: [u8; 3]) -> Self {
        Self {
            value: serde_json::Value::Array(vec![
                serde_json::Value::Number(rgb[0].into()),
                serde_json::Value::Number(rgb[1].into()),
                serde_json::Value::Number(rgb[2].into()),
            ]),
            type_name: "color".to_string(),
        }
    }

    /// Parses a color value from various input formats.
    ///
    /// Supports:
    /// - Hex strings: `"#RRGGBB"`
    /// - JSON arrays: `[r, g, b]`
    ///
    /// # Errors
    /// Returns an error if the value cannot be parsed as a valid color.
    pub fn parse_color(val: &serde_json::Value) -> Result<[u8; 3]> {
        match val {
            serde_json::Value::String(s) => Self::parse_hex_color(s),
            serde_json::Value::Array(arr) if arr.len() == 3 => Self::parse_rgb_array(arr),
            _ => Err(Error::InvalidColor(format!("expected hex string or RGB array, got {val}"))),
        }
    }

    /// Parses a hex color string in the format `#RRGGBB`.
    fn parse_hex_color(s: &str) -> Result<[u8; 3]> {
        if !s.starts_with('#') || s.len() != 7 {
            return Err(Error::InvalidColor(format!("hex color must be in format #RRGGBB, got {s}")));
        }

        let parse_component = |range: std::ops::Range<usize>| {
            u8::from_str_radix(&s[range], 16).map_err(|_| Error::InvalidColor(format!("invalid hex component in {s}")))
        };

        Ok([parse_component(1..3)?, parse_component(3..5)?, parse_component(5..7)?])
    }

    /// Parses an RGB array from JSON values.
    fn parse_rgb_array(arr: &[serde_json::Value]) -> Result<[u8; 3]> {
        let parse_component = |idx: usize| {
            arr[idx]
                .as_u64()
                .and_then(|v| if v <= 255 { Some(v as u8) } else { None })
                .ok_or_else(|| Error::InvalidColor(format!("RGB component {idx} must be in range 0-255, got {}", arr[idx])))
        };

        Ok([parse_component(0)?, parse_component(1)?, parse_component(2)?])
    }

    /// Converts this option value to a hex color string.
    ///
    /// Returns `None` if the value is not a valid color.
    pub fn to_hex_color(&self) -> Option<String> {
        Self::parse_color(&self.value)
            .map(|[r, g, b]| format!("#{r:02x}{g:02x}{b:02x}"))
            .ok()
    }
}

/// A collection of options keyed by name.
pub type OptMap = std::collections::HashMap<String, OptValue>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_int_creation() {
        let opt = OptValue::int(42);
        assert_eq!(opt.type_name, "int");
        assert_eq!(opt.value, serde_json::json!(42));
    }

    #[test]
    fn test_float_creation() {
        let opt = OptValue::float(3.14);
        assert_eq!(opt.type_name, "float");
        assert!(opt.value.as_f64().unwrap() - 3.14 < 0.001);
    }

    #[test]
    fn test_bool_creation() {
        let opt = OptValue::bool(true);
        assert_eq!(opt.type_name, "bool");
        assert_eq!(opt.value, serde_json::json!(true));
    }

    #[test]
    fn test_color_creation() {
        let opt = OptValue::color([255, 128, 0]);
        assert_eq!(opt.type_name, "color");
        assert_eq!(opt.value, serde_json::json!([255, 128, 0]));
    }

    #[test]
    fn test_parse_hex_color() {
        let value = serde_json::json!("#FF8000");
        let color = OptValue::parse_color(&value).unwrap();
        assert_eq!(color, [255, 128, 0]);
    }

    #[test]
    fn test_parse_rgb_array() {
        let value = serde_json::json!([255, 128, 0]);
        let color = OptValue::parse_color(&value).unwrap();
        assert_eq!(color, [255, 128, 0]);
    }

    #[test]
    fn test_to_hex_color() {
        let opt = OptValue::color([255, 128, 0]);
        assert_eq!(opt.to_hex_color(), Some("#ff8000".to_string()));
    }

    #[test]
    fn test_invalid_hex_format() {
        let value = serde_json::json!("FF8000");
        assert!(OptValue::parse_color(&value).is_err());
    }

    #[test]
    fn test_invalid_rgb_range() {
        let value = serde_json::json!([300, 0, 0]);
        assert!(OptValue::parse_color(&value).is_err());
    }
}
