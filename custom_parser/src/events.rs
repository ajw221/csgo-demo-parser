use prost::Message;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::gameevent::{CsvcMsgGameEvent, GameEvent};

pub mod macros {
    #[macro_export]
    macro_rules! impl_event {
        ($name:ident) => {
            impl From<(GameEvent, CsvcMsgGameEvent)> for $name {
                fn from(e: (GameEvent, CsvcMsgGameEvent)) -> Self {
                    let game_event = e.0;
                    let msg = e.1;

                    serde_json::from_value::<$name>(json!(game_event.to_object(&msg))).unwrap()
                }
            }
        };
    }

    pub use impl_event;
}

#[derive(Deserialize, Serialize, Message)]
pub struct PlayerDeath {
    #[prost(string, tag = "1")]
    weapon_fauxitemid: String,
    #[prost(string, tag = "2")]
    weapon_itemid: String,
    #[prost(bool, tag = "3")]
    noscope: bool,
    #[prost(int32, tag = "4")]
    revenge: i32,
    #[prost(int32, tag = "5")]
    assister: i32,
    #[prost(int32, tag = "6")]
    penetrated: i32,
    #[prost(bool, tag = "7")]
    noreplay: bool,
    #[prost(int32, tag = "8")]
    attacker: i32,
    #[prost(bool, tag = "9")]
    headshot: bool,
    #[prost(bool, tag = "10")]
    thrusmoke: bool,
    #[prost(bool, tag = "11")]
    attackerblind: bool,
    #[prost(int32, tag = "12")]
    userid: i32,
    #[prost(string, tag = "13")]
    weapon: String,
    #[prost(int32, tag = "14")]
    dominated: i32,
    #[prost(string, tag = "15")]
    distance: String,
    #[prost(bool, tag = "16")]
    assistedflash: bool,
    #[prost(int32, tag = "17")]
    wipe: i32,
    #[prost(string, tag = "18")]
    weapon_originalowner_xuid: String,
}

#[derive(Deserialize, Serialize, Message)]
pub struct RoundEnd {
    #[prost(int32, tag = "1")]
    pub winner: i32,
    #[prost(int32, tag = "2")]
    pub reason: i32,
    #[prost(int32, tag = "3")]
    pub nomusic: i32,
    #[prost(int32, tag = "4")]
    pub legacy: i32,
    #[prost(string, tag = "5")]
    pub message: String,
    #[prost(int32, tag = "6")]
    pub player_count: i32,
}

#[derive(Deserialize, Serialize, Message)]
pub struct PlayerHurt {
    #[prost(int32, tag = "1")]
    dmg_health: i32,
    #[prost(int32, tag = "2")]
    health: i32,
    #[prost(string, tag = "3")]
    weapon: String,
    #[prost(int32, tag = "4")]
    attacker: i32,
    #[prost(int32, tag = "5")]
    userid: i32,
    #[prost(int32, tag = "6")]
    armor: i32,
    #[prost(int32, tag = "7")]
    dmg_armor: i32,
    #[prost(int32, tag = "8")]
    hitgroup: i32,
}

#[derive(Deserialize, Serialize, Message)]
pub struct WeaponFire {
    #[prost(int32, tag = "1")]
    userid: i32,
    #[prost(string, tag = "2")]
    weapon: String,
    #[prost(bool, tag = "3")]
    silenced: bool,
}

#[derive(Deserialize, Serialize, Message)]
pub struct RoundStart {
    #[prost(string, tag = "1")]
    objective: String,
    #[prost(int32, tag = "2")]
    fraglimit: i32,
    #[prost(int32, tag = "3")]
    timelimit: i32,
}

#[derive(Deserialize, Serialize, Message)]
pub struct PlayerSpawn {
    #[prost(int32, tag = "1")]
    pub teamnum: i32,
    #[prost(int32, tag = "2")]
    pub userid: i32,
}

#[derive(Deserialize, Serialize, Message)]
pub struct PlayerFootstep {
    #[prost(int32, tag = "1")]
    pub userid: i32,
}

#[derive(Deserialize, Serialize, Message)]
pub struct SmokeGrenadeDetonate {
    #[prost(int32, tag = "1")]
    entityid: i32,
    #[prost(int32, tag = "2")]
    userid: i32,
    #[prost(string, tag = "3")]
    x: String,
    #[prost(string, tag = "4")]
    y: String,
    #[prost(string, tag = "5")]
    z: String,
}

#[derive(Deserialize, Serialize, Message)]
pub struct SmokeGrenadeExpired {
    #[prost(int32, tag = "1")]
    entityid: i32,
    #[prost(int32, tag = "2")]
    userid: i32,
    #[prost(string, tag = "3")]
    x: String,
    #[prost(string, tag = "4")]
    y: String,
    #[prost(string, tag = "5")]
    z: String,
}

#[derive(Deserialize, Serialize, Message)]
pub struct FlashbangDetonate {
    #[prost(int32, tag = "1")]
    pub entityid: i32,
    #[prost(int32, tag = "2")]
    pub userid: i32,
    #[prost(string, tag = "3")]
    pub x: String,
    #[prost(string, tag = "4")]
    pub y: String,
    #[prost(string, tag = "5")]
    pub z: String,
}

#[derive(Deserialize, Serialize, Message)]
pub struct BombPlanted {
    #[prost(int32, tag = "1")]
    pub site: i32,
    #[prost(int32, tag = "2")]
    pub userid: i32,
}

macros::impl_event!(PlayerDeath);
macros::impl_event!(RoundEnd);
macros::impl_event!(PlayerHurt);
macros::impl_event!(WeaponFire);
macros::impl_event!(RoundStart);
macros::impl_event!(PlayerSpawn);
macros::impl_event!(PlayerFootstep);
macros::impl_event!(SmokeGrenadeDetonate);
macros::impl_event!(SmokeGrenadeExpired);
macros::impl_event!(FlashbangDetonate);
macros::impl_event!(BombPlanted);
