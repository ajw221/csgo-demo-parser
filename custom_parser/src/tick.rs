use prost::Message;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct CnetMsgTick {
    #[prost(uint32, tag = "1")]
    pub tick: u32,
    #[prost(uint32, optional, tag = "4")]
    pub host_computationtime: Option<u32>,
    #[prost(uint32, optional, tag = "5")]
    pub host_computationtime_std_deviation: Option<u32>,
    #[prost(uint32, optional, tag = "6")]
    pub host_framestarttime_std_deviation: Option<u32>,
    #[prost(uint32, optional, tag = "7")]
    pub hltv_replay_flags: Option<u32>,
}
