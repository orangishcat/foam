use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::fmt;
use std::ops::{Deref, DerefMut};

/// An integer that can also be deserialized from a string or `null`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LooseInt(pub i64);

impl<'de> Deserialize<'de> for LooseInt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
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
                "expected an integer or integer string, got {value}"
            ))),
        }
    }
}

impl Serialize for LooseInt {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl Deref for LooseInt {
    type Target = i64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for LooseInt {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AsRef<i64> for LooseInt {
    fn as_ref(&self) -> &i64 {
        &self.0
    }
}

impl From<i64> for LooseInt {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

impl From<LooseInt> for i64 {
    fn from(value: LooseInt) -> Self {
        value.0
    }
}

impl PartialEq<i64> for LooseInt {
    fn eq(&self, other: &i64) -> bool {
        self.0 == *other
    }
}

impl PartialEq<LooseInt> for i64 {
    fn eq(&self, other: &LooseInt) -> bool {
        *self == other.0
    }
}

impl fmt::Display for LooseInt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
