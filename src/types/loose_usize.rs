use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::ops::{Deref, DerefMut};

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
