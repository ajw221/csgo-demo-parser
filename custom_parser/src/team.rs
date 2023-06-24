use ahash::AHashMap as HashMap;
use lazy_static::lazy_static;
use parking_lot::Mutex;

use crate::{entity::Entity, player::PlayerInfo};

pub type Team = u8;

lazy_static! {
    pub static ref T_STATE: Mutex<TeamState> = Mutex::new(TeamState {
        team: 2,
        entity: None,
        members: HashMap::with_capacity(7),
        player_ids: Vec::with_capacity(7),
    });
    pub static ref CT_STATE: Mutex<TeamState> = Mutex::new(TeamState {
        team: 3,
        entity: None,
        members: HashMap::with_capacity(7),
        player_ids: Vec::with_capacity(7),
    });
}

#[derive(Clone, Default)]
pub struct TeamState {
    pub team: Team,
    pub entity: Option<Entity>,
    pub members: HashMap<i32, PlayerInfo>,
    pub player_ids: Vec<u8>,
}

pub async fn bind_team_states() {
    todo!()
}
