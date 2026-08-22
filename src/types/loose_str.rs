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

impl std::ops::Deref for LooseString {
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
