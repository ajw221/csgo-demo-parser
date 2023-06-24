use std::{borrow::Cow, io::Cursor};

use ahash::AHashMap as HashMap;
use custom_bitreader::BitReader;

use crate::{
    common::Vector64,
    entity::Entity,
    equipment::Equipment,
    serverclass::PropertyValueEnum,
    team::{Team, TeamState},
    ENTITIES, GUID_LENGTH, PLAYERS_BY_ENTITY_ID, PLAYERS_BY_STEAM_ID, PLAYERS_BY_USER_ID,
    PLAYER_NAME_MAX_LENGTH, RAW_PLAYERS, SERVER_CLASSES, SERVER_CLASSES_BY_NAME,
};

pub trait IsPlayer {
    fn get_name(&self) -> &str;
    fn get_user_id(&self) -> u32;
    fn get_entity_id(&self) -> i32;
}

macro_rules! impl_player {
    ($name:ident) => {
        impl IsPlayer for $name {
            fn get_name(&self) -> &str {
                &self.name
            }

            fn get_user_id(&self) -> u32 {
                self.user_id
            }

            fn get_entity_id(&self) -> i32 {
                self.entity_id
            }
        }
    };
}

impl_player!(Player);
impl_player!(PlayerInfo);

#[derive(Clone)]
pub struct Player {
    pub steam_id: u64,
    pub last_alive_position: Vector64,
    pub user_id: u32,
    pub name: String,
    pub inventory: HashMap<i32, Equipment>,

    pub entity_id: i32,
    pub entity: Entity,
    pub flash_duration: f64,
    pub flash_tick: i32,
    pub team_state: TeamState,
    pub team: Team,
    pub is_bot: bool,
    pub is_connected: bool,
    pub is_defusing: bool,
    pub is_planting: bool,
    pub is_reloading: bool,
    pub is_unknown: bool,
}

impl Player {
    pub async fn _is_alive(&self) -> bool {
        if let Some(entity) = ENTITIES.lock().await.get(&self.entity_id) {
            if let PropertyValueEnum::Integer(Cow::Owned(health)) =
                entity._property_value_must("m_iHealth")
            {
                return health > 0;
            }
        }

        false
    }
}

#[derive(Debug, Clone)]
pub struct PlayerInfo {
    pub version: u64,
    pub xuid: u64,
    pub name: String,
    pub user_id: u32,
    pub guid: String,
    pub friends_id: u64,
    pub friends_name: String,
    pub custom_files_0: i32,
    pub custom_files_1: i32,
    pub custom_files_2: i32,
    pub custom_files_3: i32,
    pub files_downloaded: u8,
    pub is_fake_player: bool,
    pub is_hltv: bool,
    pub entity_id: i32,
}

impl From<&[u8]> for PlayerInfo {
    fn from(bytes: &[u8]) -> Self {
        let mut br = BitReader::new_small_bit_reader(Cursor::new(Cow::Borrowed(bytes)));
        let version = uint64(br.read_bytes(8));
        let xuid = uint64(br.read_bytes(8));
        let name = br.read_cstring(PLAYER_NAME_MAX_LENGTH);
        let user_id = uint32(br.read_bytes(4));
        let guid = br.read_cstring(GUID_LENGTH);
        br.skip(24);
        let friends_id_bytes = br.read_bytes(4);
        let friends_id = friendsid(friends_id_bytes);
        let friends_name = br.read_cstring(PLAYER_NAME_MAX_LENGTH);
        let is_fake_player = br.read_single_byte() != 0;
        let is_hltv = br.read_single_byte() != 0;
        let custom_files_0 = br.read_int(32) as i32;
        let custom_files_1 = br.read_int(32) as i32;
        let custom_files_2 = br.read_int(32) as i32;
        let custom_files_3 = br.read_int(32) as i32;
        let files_downloaded = br.read_single_byte();
        let entity_id = -1;
        Self {
            version,
            xuid,
            name,
            user_id,
            guid,
            friends_id,
            friends_name,
            is_fake_player,
            is_hltv,
            custom_files_0,
            custom_files_1,
            custom_files_2,
            custom_files_3,
            files_downloaded,
            entity_id,
        }
    }
}

#[inline]
fn friendsid(b: Vec<u8>) -> u64 {
    b[2] as u64 | (b[1] as u64) << 8 | (b[0] as u64) << 16
}

#[inline]
fn uint64(b: Vec<u8>) -> u64 {
    b[7] as u64
        | (b[6] as u64) << 8
        | (b[5] as u64) << 16
        | (b[4] as u64) << 24
        | (b[3] as u64) << 32
        | (b[2] as u64) << 40
        | (b[1] as u64) << 48
        | (b[0] as u64) << 56
}

#[inline]
fn uint32(b: Vec<u8>) -> u32 {
    b[3] as u32 | (b[2] as u32) << 8 | (b[1] as u32) << 16 | (b[0] as u32) << 24
}

pub async fn bind_players() {
    SERVER_CLASSES.lock().await[SERVER_CLASSES_BY_NAME.lock().await["DT_CSPlayer"].index as usize]
        .created_handlers = Some(vec![|id| {
        Box::pin(async move {
            bind_new_player(id).await;
        })
    }])
}

async fn bind_new_player(entity_id: i32) {
    create_or_update_player(entity_id).await;
}

async fn index_player_by_steam_id(pl: &Player) {
    if pl.is_bot && pl.steam_id > 0 {
        PLAYERS_BY_STEAM_ID
            .lock()
            .await
            .insert(pl.steam_id, pl.to_owned());
    }
}

async fn create_or_update_player(entity_id: i32) {
    let raw_players = RAW_PLAYERS.lock().await;
    let mut players_by_entity_id = PLAYERS_BY_ENTITY_ID.lock().await;
    let mut players_by_user_id = PLAYERS_BY_USER_ID.lock().await;

    let player = players_by_entity_id.get_mut(&entity_id);
    let found_player: Player;
    match player {
        Some(pl) => {
            pl.entity_id = entity_id;
            pl.is_connected = true;
            found_player = pl.to_owned();
            index_player_by_steam_id(pl).await;
        }
        None => match raw_players.get(&(entity_id - 1)) {
            Some(rp) => match players_by_user_id.get_mut(&rp.user_id) {
                Some(pl) => {
                    pl.entity_id = entity_id;
                    pl.is_connected = true;
                    players_by_entity_id.insert(entity_id, pl.to_owned());
                    found_player = pl.to_owned();
                    index_player_by_steam_id(pl).await;
                }
                None => {
                    let player = Player {
                        name: rp.name.to_owned(),
                        steam_id: rp.xuid,
                        is_bot: rp.is_fake_player || rp.guid == "BOT",
                        user_id: rp.user_id,

                        entity_id,
                        is_connected: true,

                        // Unused/Defaults
                        last_alive_position: Vector64::default(),
                        inventory: HashMap::with_capacity(8),
                        entity: Entity::default(),
                        flash_duration: 0.0,
                        flash_tick: 0,
                        team_state: TeamState::default(),
                        team: Team::default(),
                        is_defusing: false,
                        is_planting: false,
                        is_reloading: false,
                        is_unknown: false,
                    };

                    index_player_by_steam_id(&player).await;
                    players_by_entity_id.insert(entity_id, player.to_owned());
                    found_player = player;
                }
            },
            None => {
                let player = Player {
                    name: String::from("unknown"),
                    is_unknown: true,

                    entity_id,
                    is_connected: true,

                    // Unused/Defaults
                    steam_id: 0,
                    user_id: 0,
                    is_bot: false,
                    last_alive_position: Vector64::default(),
                    inventory: HashMap::with_capacity(8),
                    entity: Entity::default(),
                    flash_duration: 0.0,
                    flash_tick: 0,
                    team_state: TeamState::default(),
                    team: Team::default(),
                    is_defusing: false,
                    is_planting: false,
                    is_reloading: false,
                };

                players_by_entity_id.insert(entity_id, player.to_owned());
                found_player = player;
            }
        },
    };

    if let Some(rp) = raw_players.get(&(entity_id - 1)) {
        players_by_user_id.insert(rp.user_id, found_player);
    }
}
