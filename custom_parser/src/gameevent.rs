use ahash::AHashMap as HashMap;
use prost::Message;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameEventType {
    PlayerDeath,
    PlayerConnect,
    InfernoExpire,
    InfernoStartBurn,
    Unknown,
}

impl From<String> for GameEventType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "player_death" => Self::PlayerDeath,
            "inferno_expire" => Self::InfernoExpire,
            "inferno_startburn" => Self::InfernoStartBurn,
            "player_connect" => Self::PlayerConnect,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Message)]
pub struct GameEvent {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(int32, tag = "2")]
    pub id: i32,
    #[prost(string, repeated, tag = "3")]
    pub key_names: Vec<String>,
}

impl From<DescriptorT> for GameEvent {
    fn from(d: DescriptorT) -> Self {
        GameEvent {
            name: d.name,
            id: d.eventid,
            key_names: d.keys.into_iter().map(|key| key.name).collect(),
        }
    }
}

impl GameEvent {
    pub fn to_object(&self, event_msg: &CsvcMsgGameEvent) -> HashMap<String, Value> {
        let mut event: HashMap<String, Value> = HashMap::new();
        for i in 0..self.key_names.len() {
            let key_name = &self.key_names[i];
            let value = &event_msg.keys[i];

            let event_value = match value.r#type {
                1 => json!(value.val_string),
                2 => json!(value.val_float.to_string()),
                3 => json!(value.val_long),
                4 => json!(value.val_short),
                5 => json!(value.val_byte),
                6 => json!(value.val_bool),
                7 => json!(value.val_uint64),
                8 => json!(value.val_wstring),
                _ => json!({}),
            };
            event.insert(key_name.to_string(), event_value);
        }

        event
    }

    pub fn into_type<T: From<(GameEvent, CsvcMsgGameEvent)> + Message>(
        &self,
        event_msg: CsvcMsgGameEvent,
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        T::from((self.to_owned(), event_msg))
            .encode(&mut buf)
            .expect("Failed to encode GameEvent");
        buf
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct GameEventObject {
    pub value: HashMap<String, Value>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Message)]
pub struct CsvcMsgGameEvent {
    #[prost(string, tag = "1")]
    pub event_name: String,
    #[prost(int32, tag = "2")]
    pub eventid: i32,
    #[prost(message, repeated, tag = "3")]
    pub keys: Vec<KeyT>,
    #[prost(int32, tag = "4")]
    pub passthrough: i32,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Message)]
pub struct KeyT {
    #[prost(int32, tag = "1")]
    pub r#type: i32,
    #[prost(string, tag = "2")]
    pub val_string: String,
    #[prost(float, tag = "3")]
    pub val_float: f32,
    #[prost(int32, tag = "4")]
    pub val_long: i32,
    #[prost(int32, tag = "5")]
    pub val_short: i32,
    #[prost(int32, tag = "6")]
    pub val_byte: i32,
    #[prost(bool, tag = "7")]
    pub val_bool: bool,
    #[prost(uint64, tag = "8")]
    pub val_uint64: u64,
    #[prost(bytes = "vec", tag = "9")]
    pub val_wstring: Vec<u8>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct CsvcMsgGameEventList {
    #[prost(message, repeated, tag = "1")]
    pub descriptors: Vec<DescriptorT>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct DescriptorT {
    #[prost(int32, tag = "1")]
    pub eventid: i32,
    #[prost(string, tag = "2")]
    pub name: String,
    #[prost(message, repeated, tag = "3")]
    pub keys: Vec<DescriptorKeyT>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct DescriptorKeyT {
    #[prost(int32, tag = "1")]
    pub r#type: i32,
    #[prost(string, tag = "2")]
    pub name: String,
}
