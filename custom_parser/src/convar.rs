use prost::Message;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct CnetMsgSetConVar {
    #[prost(message, optional, tag = "1")]
    pub convars: Option<CMsgCVars>,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct CMsgCVars {
    #[prost(message, repeated, tag = "1")]
    pub cvars: Vec<CVar>,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct CVar {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub value: String,
    #[prost(uint32, optional, tag = "3")]
    pub dictionary_name: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConVar {
    pub cash_win_bomb: i64,
    pub cash_win_defuse: i64,
    pub round_restart_delay: i64,
    pub freezetime: i64,
    pub buy_time: i64,
    pub max_rounds: i64,
    pub timeouts_allowed: i64,
    pub min_update_rate: i64,
}
