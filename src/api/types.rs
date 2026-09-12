use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Debug, Default, Clone, Serialize)]
pub struct LooseString(pub String);

impl<'de> Deserialize<'de> for LooseString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            Value::String(value) => Ok(Self(value)),
            Value::Number(value) => Ok(Self(value.to_string())),
            Value::Null => Ok(Self::default()),
            value => Err(serde::de::Error::custom(format!(
                "expected a string or number, got {value}"
            ))),
        }
    }
}

impl Deref for LooseString {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for LooseString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct LooseUsize(pub usize);

impl<'de> Deserialize<'de> for LooseUsize {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            Value::Number(value) => value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .map(Self)
                .ok_or_else(|| serde::de::Error::custom("integer is outside usize range")),
            Value::String(value) if value.is_empty() => Ok(Self::default()),
            Value::String(value) => value.parse().map(Self).map_err(serde::de::Error::custom),
            Value::Null => Ok(Self::default()),
            value => Err(serde::de::Error::custom(format!(
                "expected an integer or integer string, got {value}"
            ))),
        }
    }
}

impl Deref for LooseUsize {
    type Target = usize;

    fn deref(&self) -> &usize {
        &self.0
    }
}

impl DerefMut for LooseUsize {
    fn deref_mut(&mut self) -> &mut usize {
        &mut self.0
    }
}

impl From<usize> for LooseUsize {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<LooseUsize> for usize {
    fn from(value: LooseUsize) -> Self {
        value.0
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct LooseInt(pub i64);

impl<'de> Deserialize<'de> for LooseInt {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            Value::Number(value) => value
                .as_i64()
                .map(Self)
                .ok_or_else(|| serde::de::Error::custom("integer is outside i64 range")),
            Value::String(value) if value.is_empty() => Ok(Self::default()),
            Value::String(value) => value.parse().map(Self).map_err(serde::de::Error::custom),
            Value::Bool(value) => Ok(Self(i64::from(value))),
            Value::Null => Ok(Self::default()),
            value => Err(serde::de::Error::custom(format!(
                "expected an integer, got {value}"
            ))),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct LooseFloat(pub f64);

impl<'de> Deserialize<'de> for LooseFloat {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            Value::Number(value) => value
                .as_f64()
                .map(Self)
                .ok_or_else(|| serde::de::Error::custom("number is outside f64 range")),
            Value::String(value) if value.is_empty() => Ok(Self::default()),
            Value::String(value) => value.parse().map(Self).map_err(serde::de::Error::custom),
            Value::Null => Ok(Self::default()),
            value => Err(serde::de::Error::custom(format!(
                "expected a number, got {value}"
            ))),
        }
    }
}
