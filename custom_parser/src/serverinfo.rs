use prost::Message;
use serde::{Deserialize, Serialize};

use crate::serializers::string_float_serializer;

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct CsvcMsgServerInfo {
    #[prost(int32, optional, tag = "1")]
    pub protocol: Option<i32>,
    #[prost(int32, optional, tag = "2")]
    pub server_count: Option<i32>,
    #[prost(bool, optional, tag = "3")]
    pub is_dedicated: Option<bool>,
    #[prost(bool, optional, tag = "4")]
    pub is_official_valve_server: Option<bool>,
    #[prost(bool, optional, tag = "5")]
    pub is_hltv: Option<bool>,
    #[prost(bool, optional, tag = "6")]
    pub is_replay: Option<bool>,
    #[prost(bool, optional, tag = "21")]
    pub is_redirecting_to_proxy_relay: Option<bool>,
    #[prost(int32, optional, tag = "7")]
    pub c_os: Option<i32>,
    #[prost(fixed32, optional, tag = "8")]
    pub map_crc: Option<u32>,
    #[prost(fixed32, optional, tag = "9")]
    pub client_crc: Option<u32>,
    #[prost(fixed32, optional, tag = "10")]
    pub string_table_crc: Option<u32>,
    #[prost(int32, optional, tag = "11")]
    pub max_clients: Option<i32>,
    #[prost(int32, optional, tag = "12")]
    pub max_classes: Option<i32>,
    #[prost(int32, optional, tag = "13")]
    pub player_slot: Option<i32>,
    #[prost(float, optional, tag = "14")]
    pub tick_interval: Option<f32>,
    #[prost(string, optional, tag = "15")]
    pub game_dir: Option<String>,
    #[prost(string, optional, tag = "16")]
    pub map_name: Option<String>,
    #[prost(string, optional, tag = "17")]
    pub map_group_name: Option<String>,
    #[prost(string, optional, tag = "18")]
    pub sky_name: Option<String>,
    #[prost(string, optional, tag = "19")]
    pub host_name: Option<String>,
    #[prost(uint32, optional, tag = "20")]
    pub public_ip: Option<u32>,
    #[prost(uint64, optional, tag = "22")]
    pub ugc_map_id: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MapInfo {
    #[serde(rename = "CTSpawn_x")]
    pub ct_spawn_x: String,
    #[serde(rename = "CTSpawn_y")]
    pub ct_spawn_y: String,
    #[serde(rename = "TSpawn_x")]
    pub t_spawn_x: String,
    #[serde(rename = "TSpawn_y")]
    pub t_spawn_y: String,
    #[serde(rename = "bombA_x")]
    pub bomb_a_x: String,
    #[serde(rename = "bombA_y")]
    pub bomb_a_y: String,
    #[serde(rename = "bombB_x")]
    pub bomb_b_x: String,
    #[serde(rename = "bombB_y")]
    pub bomb_b_y: String,
    inset_bottom: String,
    inset_left: String,
    inset_right: String,
    inset_top: String,
    material: String,
    pub pos_x: String,
    pub pos_y: String,
    pub scale: String,
    #[serde(rename = "verticalsections")]
    vertical_sections: MapInfoVertSections,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MapInfoVertSections {
    pub default: MapInfoVertSection,
    pub lower: MapInfoVertSection,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct MapInfoVertSection {
    pub altitude_max: String,
    pub altitude_min: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MapInfoObj {
    // de_ancient: MapInfo,
    // de_cache: MapInfo,
    // de_dust2: MapInfo,
    // de_inferno: MapInfo,
    // de_mirage: MapInfo,
    pub de_nuke: MapInfoParsed,
    // de_overpass: MapInfo,
    // de_train: MapInfo,
    // de_vertigo: MapInfo,
}

impl Default for MapInfoObj {
    fn default() -> Self {
        Self {
            de_nuke: MapInfoParsed {
                ct_spawn_x: 0.82,
                ct_spawn_y: 0.45,
                t_spawn_x: 0.19,
                t_spawn_y: 0.54,
                bomb_a_x: 0.58,
                bomb_a_y: 0.48,
                bomb_b_x: 0.58,
                bomb_b_y: 0.58,
                inset_bottom: 0.2,
                inset_left: 0.33,
                inset_right: 0.2,
                inset_top: 0.2,
                material: "overviews/de_nuke".to_string(),
                pos_x: -3453.,
                pos_y: 2887.,
                scale: 7.,
            },
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MapInfoParsed {
    #[serde(rename = "CTSpawn_x", with = "string_float_serializer")]
    pub ct_spawn_x: f64,
    #[serde(rename = "CTSpawn_y", with = "string_float_serializer")]
    pub ct_spawn_y: f64,
    #[serde(rename = "TSpawn_x", with = "string_float_serializer")]
    pub t_spawn_x: f64,
    #[serde(rename = "TSpawn_y", with = "string_float_serializer")]
    pub t_spawn_y: f64,
    #[serde(rename = "bombA_x", with = "string_float_serializer")]
    pub bomb_a_x: f64,
    #[serde(rename = "bombA_y", with = "string_float_serializer")]
    pub bomb_a_y: f64,
    #[serde(rename = "bombB_x", with = "string_float_serializer")]
    pub bomb_b_x: f64,
    #[serde(rename = "bombB_y", with = "string_float_serializer")]
    pub bomb_b_y: f64,
    #[serde(with = "string_float_serializer")]
    pub inset_bottom: f64,
    #[serde(with = "string_float_serializer")]
    pub inset_left: f64,
    #[serde(with = "string_float_serializer")]
    pub inset_right: f64,
    #[serde(with = "string_float_serializer")]
    pub inset_top: f64,
    pub material: String,
    #[serde(with = "string_float_serializer")]
    pub pos_x: f64,
    #[serde(with = "string_float_serializer")]
    pub pos_y: f64,
    #[serde(with = "string_float_serializer")]
    pub scale: f64,
    // vertical_sections: MapInfoVertSections,
}
