use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::fmt;
use std::ops::{Deref, DerefMut};

/// A float that can also be deserialized from a string or `null`.
#[derive(Debug, Default, Clone, Copy, PartialEq, PartialOrd)]
pub struct LooseFloat(pub f64);

impl<'de> Deserialize<'de> for LooseFloat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Value::deserialize(deserializer)? {
            Value::Number(value) => value
                .as_f64()
                .map(Self)
                .ok_or_else(|| serde::de::Error::custom("number is outside f64 range")),
            Value::String(value) if value.is_empty() => Ok(Self::default()),
            Value::String(value) => value.parse().map(Self).map_err(serde::de::Error::custom),
            Value::Null => Ok(Self::default()),
            value => Err(serde::de::Error::custom(format!(
                "expected a number or numeric string, got {value}"
            ))),
        }
    }
}

impl Serialize for LooseFloat {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl Deref for LooseFloat {
    type Target = f64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for LooseFloat {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AsRef<f64> for LooseFloat {
    fn as_ref(&self) -> &f64 {
        &self.0
    }
}

impl From<f64> for LooseFloat {
    fn from(value: f64) -> Self {
        Self(value)
    }
}

impl From<LooseFloat> for f64 {
    fn from(value: LooseFloat) -> Self {
        value.0
    }
}

impl PartialEq<f64> for LooseFloat {
    fn eq(&self, other: &f64) -> bool {
        self.0 == *other
    }
}

impl PartialEq<LooseFloat> for f64 {
    fn eq(&self, other: &LooseFloat) -> bool {
        *self == other.0
    }
}

impl fmt::Display for LooseFloat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
