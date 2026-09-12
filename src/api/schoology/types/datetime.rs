use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

const FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// A Schoology date formatted as `YYYY-MM-DD HH:MM:SS`, interpreted as UTC.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SchoologyDatetime(pub DateTime<Utc>);

impl<'de> Deserialize<'de> for SchoologyDatetime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(Self(
            NaiveDateTime::parse_from_str(&value, FORMAT)
                .map(|datetime| datetime.and_utc())
                .unwrap_or_default(),
        ))
    }
}

impl Serialize for SchoologyDatetime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.format(FORMAT).to_string())
    }
}
