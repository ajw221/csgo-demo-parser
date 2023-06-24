pub mod string_float_serializer {
    use serde::{de, Deserialize, Deserializer, Serializer};
    use serde_json::Value;

    pub fn serialize<S>(float: &f64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(float)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<f64, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(match Value::deserialize(deserializer)? {
            Value::String(s) => s.parse().map_err(de::Error::custom)?,
            Value::Number(num) => num.as_f64().ok_or(de::Error::custom("Invalid number"))?,
            _ => return Err(de::Error::custom("wrong type")),
        })
    }
}
