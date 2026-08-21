use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

// A string that can also accept a number
#[derive(Debug, Default, Clone)]
pub struct LooseString(pub String);

impl<'de> Deserialize<'de> for LooseString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Value::deserialize(deserializer)? {
            Value::String(v) => Ok(Self(v)),
            Value::Number(v) => Ok(Self(v.to_string())),
            Value::Null => Ok(Self(String::new())),
            v => Err(serde::de::Error::custom(format!(
                "expected a string or number, got {v}"
            ))),
        }
    }
}

impl Serialize for LooseString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
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
