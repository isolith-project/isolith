//! The dynamically typed value used across formulas, checks, rules and
//! reports: number, boolean, or string.
//!
//! This is our own knowledge-light value model (documented not parity): CATIA
//! V5 Knowledgeware has a richer typed quantity model (length, angle, mass,
//! ... with a units engine). Here everything numeric is a plain `f64` in the
//! document's unit of measure (typically mm); there is no units conversion.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A runtime value in the rules/checks sandbox.
///
/// Serialized to plain JSON (number / boolean / string), not an enum tag, so
/// reports and the independent Python verifier read scalar values directly.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A numeric value (`f64`, the program's double-everywhere discipline).
    Number(f64),
    /// A boolean value.
    Bool(bool),
    /// A string value.
    Str(String),
}

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Value::Number(n) => serializer.serialize_f64(*n),
            Value::Bool(b) => serializer.serialize_bool(*b),
            Value::Str(s) => serializer.serialize_str(s),
        }
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Value;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "a number, boolean or string")
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Value, E> {
                Ok(Value::Bool(v))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Value, E> {
                Ok(Value::Number(v))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Value, E> {
                Ok(Value::Number(v as f64))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Value, E> {
                Ok(Value::Number(v as f64))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Value, E> {
                Ok(Value::Str(v.to_string()))
            }
            fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Value, E> {
                Ok(Value::Str(v))
            }
        }
        deserializer.deserialize_any(V)
    }
}

impl Value {
    /// The value as `f64`, when it is numeric.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// The value as `&str`, when it is a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The value as `bool`, when it is a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// True when this value is numeric.
    pub fn is_number(&self) -> bool {
        matches!(self, Value::Number(_))
    }

    /// The type name used in error messages ("number", "boolean", "string").
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Number(_) => "number",
            Value::Bool(_) => "boolean",
            Value::Str(_) => "string",
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{}", format_number(*n)),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Str(s) => write!(f, "{s}"),
        }
    }
}

/// Format an `f64` compactly: whole numbers print without a `.0` suffix, so
/// messages read "6 mm" rather than "6.000000000000000 mm".
pub(crate) fn format_number(n: f64) -> String {
    if n == n.trunc() && n.is_finite() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Number(v)
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Str(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Str(v.to_string())
    }
}
