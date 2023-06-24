mod commands;
pub mod common;
pub mod convar;
mod entity;
mod equipment;
pub mod events;
pub mod gameevent;
mod header;
mod player;
mod sendtable;
mod serializers;
pub mod serverclass;
pub mod serverinfo;
mod stringtable;
mod team;
mod tick;

use std::{
    borrow::Cow,
    fs::File,
    io::{Cursor, Read, Seek},
    path::Path,
    sync::Arc,
};

use common::Vector64;
use serde_json::json;
use tokio::sync::Mutex;

use ahash::AHashMap as HashMap;
use commands::{MessageType, PacketCommand};
use convar::CnetMsgSetConVar;
use custom_bitreader::BitReader;
use custom_dispatcher::EventEmitter as AsyncDispatcher;
use entity::{CsvcMsgPacketEntities, Entity};
use equipment::{
    bind_weapons, map_equipment, Equipment, GrenadeProjectile, _add_thrown_grenade,
    get_player_weapon, EQ_DECOY, EQ_FLASH, EQ_HE, EQ_INCENDIARY, EQ_MOLOTOV, EQ_SMOKE,
};
use events::{
    BombPlanted, FlashbangDetonate, PlayerDeath, PlayerFootstep, PlayerHurt, PlayerSpawn, RoundEnd,
    RoundStart, SmokeGrenadeDetonate, SmokeGrenadeExpired, WeaponFire,
};
use gameevent::{CsvcMsgGameEvent, CsvcMsgGameEventList, GameEvent};
use header::Header;
use lazy_static::lazy_static;
use player::{bind_players, Player, PlayerInfo};
use prost::Message;
use sendtable::{
    CsvcMsgSendTable, ExcludeEntry, SendPropertyFlags, SendPropertyFlagsTrait, SendTable,
    SendTableProperty,
};
use serverclass::{FlattenedPropEntry, ServerClass};
use serverinfo::CsvcMsgServerInfo;
use stringtable::{CsvcMsgCreateStringTable, CsvcMsgUpdateStringTable};
use team::bind_team_states;

use crate::{equipment::_retrieve_potential_thrower_owner, serverclass::PropertyValueEnum};

const SEND_TABLES_MIN: usize = 480;
const SERVER_CLASSES_MIN: usize = 284;
const INSTANCE_BASELINES_MIN: usize = 59;
const ENTITIES_MIN: usize = 531;
const STRING_TABLES_MIN: usize = 19;
const GAME_EVENT_LIST_MIN: usize = 273;

const BYTES_VEC_MAX: usize = 50130;
const PROP_INDICES_VEC_CAP: usize = 64;
const USER_DATA_VEC_CAP: usize = 10608;
const ENTRY_STRING_CAP: usize = 85;

const FLATTENED_PROPS_CAP: usize = 256;

const DEFAULT_PRIO_VAL: i32 = 1;
const DEFAULT_PRIO_KEY: i32 = 64;
const ONE_BYTE: usize = 8;
const TWO_BYTES: usize = 16;
const THIRTY_TWO: usize = 32;
const N_COMMAND_INFO_BITS: usize = (152 + 4 + 4) << 3;
const BEGIN_CHUNK_SHIFT: usize = 3;
const SERVER_CLASS_IDENTIFIER: u32 = 9;

const PROP_FLAG_EXCLUDE: SendPropertyFlags = 64;
const PROP_FLAG_INSIDE_ARRAY: SendPropertyFlags = 256;
const PROP_FLAG_COLLAPSIBLE: SendPropertyFlags = 2048;
const PROP_FLAG_CHANGES_OFTEN: SendPropertyFlags = 262144;

const PROP_TYPE_DATA_TABLE: i32 = 6;

const MAX_HISTORY_LENGTH: usize = 31;
const MAX_VARINT32_BYTES: usize = 5;
const N_USER_DATA_BITS: usize = 14;

const ST_NAME_INSTANCE_BASELINE: &str = "instancebaseline";
const ST_NAME_MODEL_PRECACHE: &str = "modelprecache";
const ST_NAME_USER_INFO: &str = "userinfo";

pub const PLAYER_NAME_MAX_LENGTH: usize = 128;
pub const GUID_LENGTH: usize = 33;

lazy_static! {
    // Globally available caches
    pub(crate) static ref BYTES_VEC: Mutex<Vec<u8>> = Mutex::new(vec![0_u8; BYTES_VEC_MAX]);
    pub(crate) static ref PROP_INDICES_VEC: Mutex<Vec<u32>> = Mutex::new(Vec::with_capacity(PROP_INDICES_VEC_CAP));
    pub(crate) static ref USER_DATA_VEC: Mutex<Vec<u8>> = Mutex::new(Vec::with_capacity(USER_DATA_VEC_CAP));
    pub(crate) static ref ENTRY_STRING: Mutex<String> = Mutex::new(String::with_capacity(ENTRY_STRING_CAP));
    pub(crate) static ref HIST_VEC: Mutex<Vec<String>> = Mutex::new(Vec::with_capacity(THIRTY_TWO));

    static ref SEND_TABLES: Mutex<Vec<SendTable>> = Mutex::new(Vec::with_capacity(SEND_TABLES_MIN));
    static ref SEND_TABLES_BY_NAME: Mutex<HashMap<String, SendTable>> = Mutex::new(HashMap::with_capacity(SEND_TABLES_MIN));
    static ref SERVER_CLASSES: Mutex<Vec<ServerClass>> = Mutex::new(Vec::with_capacity(SERVER_CLASSES_MIN));
    static ref SERVER_CLASSES_BY_NAME: Mutex<HashMap<String, ServerClass>> = Mutex::new(HashMap::with_capacity(SERVER_CLASSES_MIN));
    static ref INSTANCE_BASELINES: Mutex<HashMap<i32, Vec<u8>>> = Mutex::new(HashMap::with_capacity(INSTANCE_BASELINES_MIN));
    static ref SERVER_CLASS_BITS: Mutex<i32> = Mutex::new(0_i32);

    static ref STRING_TABLES: Mutex<Vec<CsvcMsgCreateStringTable>> = Mutex::new(Vec::with_capacity(STRING_TABLES_MIN));

    pub static ref ENTITIES: Arc<Mutex<HashMap<i32, Entity>>> = Arc::new(Mutex::new(HashMap::with_capacity(ENTITIES_MIN)));

    pub static ref PLAYER_INFO_BY_USER_ID: Arc<Mutex<HashMap<i32, PlayerInfo>>> = Arc::new(Mutex::new(HashMap::with_capacity(16)));
    pub static ref RAW_PLAYERS: Arc<Mutex<HashMap<i32, PlayerInfo>>> = Arc::new(Mutex::new(HashMap::with_capacity(16)));

    pub static ref PLAYERS_BY_ENTITY_ID: Arc<Mutex<HashMap<i32, Player>>> = Arc::new(Mutex::new(HashMap::with_capacity(16)));
    pub static ref PLAYERS_BY_USER_ID: Arc<Mutex<HashMap<u32, Player>>> = Arc::new(Mutex::new(HashMap::with_capacity(16)));
    static ref PLAYERS_BY_STEAM_ID: Arc<Mutex<HashMap<u64, Player>>> = Arc::new(Mutex::new(HashMap::with_capacity(16)));

    pub static ref GRENADE_PROJECTILES: Arc<Mutex<HashMap<i32, GrenadeProjectile>>> = Arc::new(Mutex::new(HashMap::new()));
    pub static ref INGAME_TICK: Arc<Mutex<isize>> = Arc::new(Mutex::new(-1));

    static ref MODEL_PRECACHE: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    pub static ref GRENADE_MODEL_INDICES: Arc<Mutex<HashMap<i32, i32>>> = Arc::new(Mutex::new(HashMap::new()));
    pub static ref THROWN_GRENADES: Arc<Mutex<HashMap<i32, Vec<Equipment>>>> = Arc::new(Mutex::new(HashMap::new()));
    static ref DELAYED_EVENTS: Mutex<Vec<GameEvent>> = Mutex::new(Vec::new());

    static ref AGG: Mutex<u128> = Mutex::new(0);

    pub static ref SERVER_CONVARS: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    pub static ref SERVER_INFO: Mutex<CsvcMsgServerInfo> = Mutex::new(CsvcMsgServerInfo::default());
    pub static ref TICKRATE: Mutex<f32> = Mutex::new(0_f32);

    pub static ref HEADER: Mutex<Header> = Mutex::new(Header::default());

    pub static ref GLOBAL_DISPATCHER: Mutex<AsyncDispatcher> = Mutex::new(AsyncDispatcher::new());
    pub static ref PLAYER_INFO_BY_STEAM_ID: Mutex<HashMap<u64, PlayerInfo>> = Mutex::new(HashMap::with_capacity(16));
}

pub struct Parser {
    bitreader: BitReader<Cursor<Cow<'static, [u8]>>>,

    pub ingame_tick: isize,

    pub dispatcher: AsyncDispatcher,
    pub game_event_list: HashMap<i32, GameEvent>,

    pub agg: u128,
}

impl Parser {
    pub async fn new_from_file<P: AsRef<Path>>(
        path: P,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut f = File::open(path)?;
        let mut buf = vec![0; f.metadata()?.len() as usize];
        f.read_exact(&mut buf)?;

        let mut bitreader = BitReader::new_large_bit_reader(Cursor::new(Cow::Owned(buf)));

        *HEADER.lock().await = Header::_parse(&mut bitreader);
        Ok(Parser {
            bitreader,
            ingame_tick: -1,
            dispatcher: AsyncDispatcher::new(),
            game_event_list: HashMap::with_capacity(GAME_EVENT_LIST_MIN),
            agg: 0,
        })
    }

    pub async fn parse_to_end(&mut self) {
        loop {
            let command = PacketCommand::from(self.bitreader.read_single_byte());
            let tick = self.bitreader.read_signed_int(THIRTY_TWO);
            self.bitreader.skip(ONE_BYTE);

            self.ingame_tick = tick;
            *INGAME_TICK.lock().await = tick;
            match command {
                PacketCommand::Signon | PacketCommand::Packet => {
                    self.bitreader.skip(N_COMMAND_INFO_BITS);

                    let begin_chunk = self.bitreader.read_signed_int(THIRTY_TWO) as usize;
                    self.bitreader.begin_chunk(begin_chunk << BEGIN_CHUNK_SHIFT);

                    let mut bytes_vec = BYTES_VEC.lock().await;
                    while !self.bitreader.chunk_finished() {
                        let cmd = MessageType::from(self.bitreader.read_varint32());
                        let size = self.bitreader.read_varint32() as usize;

                        self.bitreader.begin_chunk(size << BEGIN_CHUNK_SHIFT);

                        if cmd.is_skippable() {
                            self.bitreader.end_chunk();
                            continue;
                        }

                        self.bitreader.read_bytes_into(&mut bytes_vec, size);

                        match cmd {
                            MessageType::SetConVar => {
                                self.handle_set_convars(&bytes_vec[0..size]).await
                            }
                            MessageType::ServerInfo => {
                                let msg = CsvcMsgServerInfo::decode(&bytes_vec[0..size])
                                    .expect("Failed to decode CsvcMsgServerInfo.");
                                let tickrate = 1.0 / msg.tick_interval();
                                if tickrate == 0_f32 {
                                    *TICKRATE.lock().await = 128.0;
                                } else {
                                    *TICKRATE.lock().await = tickrate.round();
                                }
                                *SERVER_INFO.lock().await = msg;
                            }
                            MessageType::CreateStringTable => {
                                handle_create_string_table(&bytes_vec[0..size]).await
                            }
                            MessageType::UpdateStringTable => {
                                handle_update_string_table(&bytes_vec[0..size]).await
                            }
                            MessageType::GameEvent => {
                                self.handle_game_event(&bytes_vec[0..size]).await
                            }
                            MessageType::PacketEntities => {
                                self.handle_packet_entities(&bytes_vec[0..size]).await
                            }
                            MessageType::GameEventList => {
                                self.handle_game_event_list(&bytes_vec[0..size]).await
                            }
                            _ => {}
                        }

                        self.bitreader.end_chunk();
                    }
                    self.bitreader.end_chunk();
                }
                PacketCommand::Sync => {}
                PacketCommand::Console => {}
                PacketCommand::User => {}
                PacketCommand::Data => {
                    let mut send_tables = SEND_TABLES.lock().await;

                    let begin_chunk = self.bitreader.read_signed_int(THIRTY_TWO) as usize;
                    self.bitreader.begin_chunk(begin_chunk << BEGIN_CHUNK_SHIFT);
                    loop {
                        let t = self.bitreader.read_varint32();
                        if t != SERVER_CLASS_IDENTIFIER {
                            panic!("Expected SendTable ({SERVER_CLASS_IDENTIFIER}), got ({t})");
                        }

                        let size = self.bitreader.read_varint32() as usize;
                        self.bitreader.begin_chunk(size << BEGIN_CHUNK_SHIFT);

                        let st = CsvcMsgSendTable::decode(self.bitreader.read_bytes(size).as_ref())
                            .expect("Failed to decode CsvcMsgSendTable.");
                        self.bitreader.end_chunk();

                        let mut send_table = SendTable::from(st);
                        send_table.index = send_tables.len() as i32;

                        if send_table.is_end {
                            break;
                        }

                        send_tables.push(send_table.to_owned());
                        SEND_TABLES_BY_NAME
                            .lock()
                            .await
                            .insert(send_table.name.to_owned(), send_table.to_owned());
                    }

                    let server_class_count = self.bitreader.read_int(TWO_BYTES);

                    for i in 0..server_class_count {
                        let mut server_class =
                            ServerClass::new(i, &mut self.bitreader, server_class_count);

                        if let Some(instance_baseline) =
                            INSTANCE_BASELINES.lock().await.get(&(i as i32))
                        {
                            server_class.instance_baseline = Some(instance_baseline.to_owned());
                        }

                        SERVER_CLASSES.lock().await.push(server_class.to_owned());
                        SERVER_CLASSES_BY_NAME
                            .lock()
                            .await
                            .insert(server_class.dt_name.to_owned(), server_class.to_owned());
                    }

                    for i in 0..server_class_count {
                        let mut server_classes = SERVER_CLASSES.lock().await;
                        if let Some(msg) = send_tables.get(server_classes[i].index as usize) {
                            let mut sc_map: HashMap<String, ServerClass> = HashMap::new();
                            let mut ex_map: HashMap<String, ExcludeEntry> = HashMap::new();

                            gather_prerequisites(msg, true, &mut sc_map, &mut ex_map).await;

                            server_classes[i].base_classes_by_name = Some(sc_map.to_owned());
                            SERVER_CLASSES_BY_NAME
                                .lock()
                                .await
                                .get_mut(&server_classes[i].dt_name)
                                .unwrap()
                                .base_classes_by_name = Some(sc_map.to_owned());

                            gather_props(msg, i, "", &mut server_classes, &ex_map).await;

                            if let Some(flattened_props) = &mut server_classes[i].flattened_props {
                                let mut prio_set: HashMap<i32, i32> =
                                    HashMap::from([(DEFAULT_PRIO_KEY, DEFAULT_PRIO_VAL)]);

                                for fp in flattened_props.iter() {
                                    prio_set.insert(fp.prop.priority, DEFAULT_PRIO_VAL);
                                }

                                let mut prios: Vec<i32> = Vec::with_capacity(prio_set.len());
                                for (prio, _) in prio_set {
                                    prios.push(prio);
                                }
                                prios.sort();

                                let mut start = 0;

                                for prio in prios {
                                    loop {
                                        let mut cp = start;
                                        while cp < flattened_props.len() {
                                            let prop = &flattened_props[cp].prop;
                                            if prop.priority == prio
                                                || (prio == DEFAULT_PRIO_KEY
                                                    && prop
                                                        .flags
                                                        .has_flag_set(PROP_FLAG_CHANGES_OFTEN))
                                            {
                                                if start != cp {
                                                    flattened_props.swap(start, cp);
                                                }

                                                start += 1;
                                                break;
                                            }
                                            cp += 1;
                                        }

                                        if cp == flattened_props.len() {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    for (_idx, server_class) in SERVER_CLASSES.lock().await.iter_mut().enumerate() {
                        if let Some(flattened_props) = &server_class.flattened_props {
                            let mut prop_name_to_idx =
                                HashMap::with_capacity(flattened_props.len());
                            for (i, fp) in flattened_props.iter().enumerate() {
                                prop_name_to_idx.insert(fp.name.to_string(), i as i32);
                                if let Some(scbn) = SERVER_CLASSES_BY_NAME
                                    .lock()
                                    .await
                                    .get_mut(&server_class.dt_name)
                                {
                                    if let Some(scbn_prop_name_to_idx) = &mut scbn.prop_name_to_idx
                                    {
                                        scbn_prop_name_to_idx.insert(fp.name.to_string(), i as i32);
                                    }
                                }
                            }
                            server_class.prop_name_to_idx = Some(prop_name_to_idx);
                        }
                    }
                    *SERVER_CLASS_BITS.lock().await =
                        f32::ceil(f32::log2(SERVER_CLASSES.lock().await.len() as f32)) as i32;

                    self.bitreader.end_chunk();

                    map_equipment().await;
                    bind_entities().await;
                }
                PacketCommand::Stop => break,
                PacketCommand::Custom => {}
                PacketCommand::String => {}
            }

            self.dispatcher.emit("frame_done", tick).await;
            for (entity_id, _) in PLAYERS_BY_ENTITY_ID.lock().await.iter() {
                if let Some(entity) = ENTITIES.lock().await.get_mut(entity_id) {
                    entity
                        .position_history
                        .insert(self.ingame_tick, entity.get_position());

                    if let Some(prop) = entity.property("m_iTeamNum") {
                        if let PropertyValueEnum::Integer(Cow::Owned(team)) = prop.value {
                            entity.team = team as u8;
                        }
                    }

                    if entity.is_blind() {
                        entity.current_flash_frame_agg += 1;
                    }
                }
            }

            for (_, player_thrown_grenades) in THROWN_GRENADES.lock().await.iter() {
                for thrown_grenade in player_thrown_grenades {
                    if let (Some(proj), Some(entity)) = (
                        GRENADE_PROJECTILES
                            .lock()
                            .await
                            .get_mut(&thrown_grenade.entity_id),
                        ENTITIES.lock().await.get_mut(&thrown_grenade.entity_id),
                    ) {
                        let current_position = entity.get_position();
                        if proj.trajectory.is_empty() {
                            if current_position != Vector64::default() {
                                proj.trajectory.push(current_position.to_owned());
                                entity.last_position = current_position;
                            }
                        } else if proj.trajectory[proj.trajectory.len() - 1] != current_position {
                            proj.trajectory.push(current_position.to_owned());
                            entity.last_position = current_position;
                        }
                    }
                }
            }
        }

        self.agg = *AGG.lock().await;
    }

    async fn handle_set_convars(&mut self, bytes: &[u8]) {
        let msg =
            CnetMsgSetConVar::decode(bytes).expect("Failed to decode bytes into CnetMsgSetConVar.");

        if let Some(convars) = msg.convars {
            for cvar in convars.cvars {
                SERVER_CONVARS.lock().await.insert(cvar.name, cvar.value);
            }
        }
    }

    async fn handle_game_event(&mut self, bytes: &[u8]) {
        let msg =
            CsvcMsgGameEvent::decode(bytes).expect("Failed to decode bytes into CsvcMsgGameEvent.");

        let game_event = &self.game_event_list[&msg.eventid];

        match game_event.name.as_str() {
            "bomb_planted" => {
                self.dispatcher
                    .emit(&game_event.name, game_event.into_type::<BombPlanted>(msg))
                    .await;
            }
            "flashbang_detonate" => {
                self.dispatcher
                    .emit(
                        &game_event.name,
                        game_event.into_type::<FlashbangDetonate>(msg),
                    )
                    .await;
            }
            "smokegrenade_detonate" => {
                self.dispatcher
                    .emit(
                        &game_event.name,
                        game_event.into_type::<SmokeGrenadeDetonate>(msg),
                    )
                    .await;
            }
            "smokegrenade_expired" => {
                self.dispatcher
                    .emit(
                        &game_event.name,
                        game_event.into_type::<SmokeGrenadeExpired>(msg),
                    )
                    .await;
            }
            "player_footstep" => {
                self.dispatcher
                    .emit(
                        &game_event.name,
                        game_event.into_type::<PlayerFootstep>(msg),
                    )
                    .await;
            }
            "player_spawn" => {
                let obj = game_event.to_object(&msg);
                let player_spawn = serde_json::from_value::<PlayerSpawn>(json!(obj)).unwrap();

                self.dispatcher
                    .emit(
                        "player_spawned",
                        (player_spawn.teamnum, player_spawn.userid),
                    )
                    .await;
            }
            "round_announce_match_start" => {
                println!("MATCH STARTING");
                self.dispatcher
                    .emit(&game_event.name, self.ingame_tick)
                    .await;
            }
            "round_start" => {
                self.dispatcher
                    .emit(&game_event.name, game_event.into_type::<RoundStart>(msg))
                    .await;
            }
            "round_freeze_end" => {
                self.dispatcher
                    .emit(&game_event.name, self.ingame_tick)
                    .await;
            }
            "buytime_ended" => {
                self.dispatcher
                    .emit(&game_event.name, self.ingame_tick)
                    .await;
            }
            "round_end" => {
                self.dispatcher
                    .emit(&game_event.name, game_event.into_type::<RoundEnd>(msg))
                    .await;
            }
            "round_officially_ended" => {
                self.dispatcher
                    .emit(&game_event.name, self.ingame_tick)
                    .await;
            }
            "player_death" => {
                self.dispatcher
                    .emit(&game_event.name, game_event.into_type::<PlayerDeath>(msg))
                    .await;
            }
            "player_hurt" => {
                self.dispatcher
                    .emit(&game_event.name, game_event.into_type::<PlayerHurt>(msg))
                    .await;
            }
            "weapon_fire" => {
                self.dispatcher
                    .emit(&game_event.name, game_event.into_type::<WeaponFire>(msg))
                    .await;
            }
            _ => {}
        };
    }

    async fn handle_game_event_list(&mut self, bytes: &[u8]) {
        let msg = CsvcMsgGameEventList::decode(bytes)
            .expect("Failed to decode bytes into CsvcMsgGameEventList.");

        for d in msg.descriptors {
            self.game_event_list.insert(d.eventid, GameEvent::from(d));
        }
    }

    #[inline]
    async fn handle_packet_entities(&mut self, bytes: &[u8]) {
        let pe = CsvcMsgPacketEntities::decode(bytes)
            .expect("Failed to decode bytes into CsvcMsgPacketEntities");

        let mut server_classes = SERVER_CLASSES.lock().await;
        let mut prop_indices_vec = PROP_INDICES_VEC.lock().await;
        let server_class_bits = SERVER_CLASS_BITS.lock().await;

        let mut r = BitReader::new_small_bit_reader(Cursor::new(pe.entity_data()));

        let mut current_entity = -1_i32;
        for _ in 0..pe.updated_entries() {
            current_entity += 1 + r.read_ubitint() as i32;

            let cmd = r.read_bits_to_bytes(2);
            if cmd & 1 == 0 {
                if cmd & 2 != 0 {
                    let entity = read_enter_pvs(
                        &mut r,
                        current_entity,
                        &mut server_classes,
                        &mut prop_indices_vec,
                        *server_class_bits,
                    )
                    .await;

                    {
                        if let Some(raw_player) = RAW_PLAYERS.lock().await.get(&(entity.id - 1)) {
                            if let (Some(player_info_by_user_id), Some(player_info_by_steam_id)) = (
                                PLAYER_INFO_BY_USER_ID
                                    .lock()
                                    .await
                                    .get_mut(&(raw_player.user_id as i32)),
                                PLAYER_INFO_BY_STEAM_ID
                                    .lock()
                                    .await
                                    .get_mut(&raw_player.xuid),
                            ) {
                                player_info_by_user_id.entity_id = entity.id;
                                player_info_by_steam_id.entity_id = entity.id;
                            }
                        }
                    }

                    ENTITIES
                        .lock()
                        .await
                        .insert(current_entity, entity.to_owned());

                    if let Some(proj) = GRENADE_PROJECTILES.lock().await.get_mut(&entity.id) {
                        _retrieve_potential_thrower_owner(&entity, proj).await;

                        proj.weapon_instance = get_player_weapon(&proj.thrower, proj.wep_type);

                        let person = if proj.thrower.is_some() {
                            &proj.thrower
                        } else {
                            &proj.owner
                        };

                        _add_thrown_grenade(person, &proj.weapon_instance).await;

                        self.dispatcher
                            .emit("grenade_projectile_throw", entity.id)
                            .await;
                    }
                } else if let Some(entity) = ENTITIES.lock().await.get_mut(&current_entity) {
                    entity.apply_update(&mut r, &mut prop_indices_vec).await;
                }
            } else if cmd & 2 != 0 {
                if let Some(entity) = &mut ENTITIES.lock().await.remove(&current_entity) {
                    if let Some(proj) = &mut GRENADE_PROJECTILES.lock().await.remove(&entity.id) {
                        if let Some(thrower) = &proj.thrower {
                            self.dispatcher
                                .emit(
                                    "grenade_projectile_destroyed",
                                    (
                                        entity.id,
                                        proj.wep_type,
                                        thrower.entity_id,
                                        proj.trajectory.clone(),
                                    ),
                                )
                                .await;
                        }
                    }
                }
            }
        }
    }
}

#[inline]
async fn bind_entities() {
    bind_team_states().await;
    bind_players().await;
    bind_weapons().await;
}

#[inline]
async fn handle_create_string_table(bytes: &[u8]) {
    let msg = CsvcMsgCreateStringTable::decode(bytes)
        .expect("Failed to decode bytes into CsvcMsgCreateStringTable.");

    process_string_table(&msg).await;

    STRING_TABLES.lock().await.push(msg);
}

#[inline]
async fn handle_update_string_table(bytes: &[u8]) {
    let msg = CsvcMsgUpdateStringTable::decode(bytes)
        .expect("Failed to decode bytes into CsvcMsgUpdateStringTable.");
    let mut string_tables = STRING_TABLES.lock().await;
    let create_msg = unsafe { string_tables.get_unchecked_mut(msg.table_id() as usize) };
    match create_msg.name() {
        ST_NAME_USER_INFO | ST_NAME_INSTANCE_BASELINE | ST_NAME_MODEL_PRECACHE => {
            create_msg.num_entries = msg.num_changed_entries;
            create_msg.string_data = msg.string_data;

            process_string_table(create_msg).await;
        }
        _ => {}
    }
}

#[inline]
async fn read_enter_pvs<T: Read + Seek + Send>(
    r: &mut BitReader<T>,
    id: i32,
    server_classes: &mut [ServerClass],
    prop_indices_vec: &mut Vec<u32>,
    server_class_bits: i32,
) -> Entity {
    let sc_id = r.read_int(server_class_bits as usize);
    r.skip(10);
    server_classes[sc_id]
        .new_entity(r, id, prop_indices_vec)
        .await
}

#[inline]
async fn process_string_table(tab: &CsvcMsgCreateStringTable) {
    let mut user_data = USER_DATA_VEC.lock().await;
    let mut entry = ENTRY_STRING.lock().await;
    let mut hist = HIST_VEC.lock().await;
    let mut model_precache = MODEL_PRECACHE.lock().await;
    let mut server_classes = SERVER_CLASSES.lock().await;

    if tab.name() == ST_NAME_MODEL_PRECACHE {
        let size = tab.max_entries() as usize - model_precache.len();
        model_precache.append(&mut vec!["".to_string(); size]);
    }

    let mut br = BitReader::new_small_bit_reader(Cursor::new(tab.string_data()));

    if br.read_bit() {
        panic!("Can't decode");
    }

    let mut n_tmp = tab.max_entries();
    let mut n_entry_bits = 0;

    while n_tmp != 0 {
        n_tmp >>= 1;
        n_entry_bits += 1;
    }

    if n_entry_bits > 0 {
        n_entry_bits -= 1;
    }

    hist.clear();
    let mut last_entry = -1;

    for _i in 0..tab.num_entries() {
        let mut entry_index = last_entry + 1;
        if !br.read_bit() {
            entry_index = br.read_int(n_entry_bits as usize) as i32;
        }

        last_entry = entry_index;

        if entry_index < 0 || entry_index >= tab.max_entries() {
            panic!("Something went to shit");
        }

        entry.clear();
        if br.read_bit() {
            if br.read_bit() {
                let idx = br.read_int(MAX_VARINT32_BYTES);
                let bytes_2_cp = br.read_int(MAX_VARINT32_BYTES);

                entry.extend(hist[idx][..bytes_2_cp].chars());
                entry.extend(br.read_string().chars());
            } else {
                entry.extend(br.read_string().chars());
            }
        }

        if hist.len() > MAX_HISTORY_LENGTH {
            *hist = hist[1..].to_vec();
        }

        hist.push(entry.to_owned());

        user_data.clear();
        if br.read_bit() {
            if tab.user_data_fixed_size() {
                user_data.clear();
                user_data.append(&mut vec![
                    br.read_bits_to_bytes(tab.user_data_size_bits() as usize)
                ]);
            } else {
                let b = br.read_int(N_USER_DATA_BITS);
                user_data.clear();
                user_data.append(&mut br.read_bytes(b));
            }
        }

        if user_data.is_empty() {
            continue;
        }

        match tab.name() {
            ST_NAME_USER_INFO => {
                let player_info: PlayerInfo = parse_player_info(&user_data);

                PLAYER_INFO_BY_USER_ID
                    .lock()
                    .await
                    .insert(player_info.user_id as i32, player_info.to_owned());
                RAW_PLAYERS
                    .lock()
                    .await
                    .insert(entry_index, player_info.to_owned());
                PLAYER_INFO_BY_STEAM_ID
                    .lock()
                    .await
                    .insert(player_info.xuid, player_info);
            }
            ST_NAME_INSTANCE_BASELINE => {
                let class_id = entry.parse::<usize>().expect("Error parsing class_id.");
                if let Some(sc) = server_classes.get_mut(class_id) {
                    sc.instance_baseline = Some(user_data.to_owned());
                } else {
                    INSTANCE_BASELINES
                        .lock()
                        .await
                        .insert(class_id as i32, user_data.to_owned());
                }
            }
            ST_NAME_MODEL_PRECACHE => {
                model_precache[entry_index as usize] = entry.to_owned();
            }
            _ => {}
        }
    }

    if tab.name() == ST_NAME_MODEL_PRECACHE {
        let hm = HashMap::from([
            ("flashbang", EQ_FLASH),
            ("fraggrenade", EQ_HE),
            ("smokegrenade", EQ_SMOKE),
            ("molotov", EQ_MOLOTOV),
            ("incendiarygrenade", EQ_INCENDIARY),
            ("decoy", EQ_DECOY),
        ]);
        for (i, name) in model_precache.iter().enumerate() {
            for (eq_name, eq) in hm.iter() {
                if name.contains(eq_name) {
                    GRENADE_MODEL_INDICES
                        .lock()
                        .await
                        .insert(i as i32, eq.to_owned());
                }
            }
        }
    }
}

#[inline]
fn parse_player_info(bytes: &[u8]) -> PlayerInfo {
    PlayerInfo::from(bytes)
}

#[inline]
#[async_recursion::async_recursion]
async fn gather_prerequisites(
    send_table: &SendTable,
    collect_base_classes: bool,
    sc_map: &mut HashMap<String, ServerClass>,
    ex_map: &mut HashMap<String, ExcludeEntry>,
) {
    for stp in send_table.properties.iter() {
        if stp.flags.has_flag_set(PROP_FLAG_EXCLUDE) {
            let exclude = ExcludeEntry {
                var_name: stp.name.to_owned(),
                dt_name: stp.dt_name.to_owned(),
                excluding_dt: send_table.name.to_owned(),
            };
            ex_map.insert(stp.name.to_owned(), exclude);
        }

        if stp.raw_type == 6 {
            let st = SEND_TABLES_BY_NAME.lock().await[&stp.dt_name].to_owned();
            gather_prerequisites(
                &st,
                collect_base_classes && stp.name == "baseclass",
                sc_map,
                ex_map,
            )
            .await;
            if let (Some(scbn), true, "baseclass") = (
                SERVER_CLASSES_BY_NAME.lock().await.get(&stp.dt_name),
                collect_base_classes,
                stp.name.as_str(),
            ) {
                sc_map.insert(stp.dt_name.to_owned(), scbn.to_owned());
            }
        }
    }
}

async fn gather_props(
    send_table: &SendTable,
    server_class_index: usize,
    prefix: &str,
    server_classes: &mut [ServerClass],
    ex_map: &HashMap<String, ExcludeEntry>,
) {
    let mut tmp_flattened_props: Vec<FlattenedPropEntry> = Vec::with_capacity(FLATTENED_PROPS_CAP);
    gather_props_iterate(
        send_table,
        server_class_index,
        prefix,
        &mut tmp_flattened_props,
        server_classes,
        ex_map,
    )
    .await;
    if let Some(flattened_props) = &mut server_classes[server_class_index].flattened_props {
        flattened_props.append(&mut tmp_flattened_props);
    } else {
        server_classes[server_class_index].flattened_props = Some(tmp_flattened_props);
    }
}

#[async_recursion::async_recursion]
async fn gather_props_iterate(
    send_table: &SendTable,
    server_class_index: usize,
    prefix: &str,
    flattened_props: &mut Vec<FlattenedPropEntry>,
    server_classes: &mut [ServerClass],
    ex_map: &HashMap<String, ExcludeEntry>,
) {
    for (i, prop) in send_table.properties.iter().enumerate() {
        if !(prop.flags.has_flag_set(PROP_FLAG_INSIDE_ARRAY)
            || prop.flags.has_flag_set(PROP_FLAG_EXCLUDE)
            || is_prop_excluded(send_table, prop, ex_map))
        {
            if prop.raw_type == PROP_TYPE_DATA_TABLE {
                let s_table = SEND_TABLES_BY_NAME.lock().await[&prop.dt_name].to_owned();

                if prop.flags.has_flag_set(PROP_FLAG_COLLAPSIBLE) {
                    gather_props_iterate(
                        &s_table,
                        server_class_index,
                        prefix,
                        flattened_props,
                        server_classes,
                        ex_map,
                    )
                    .await;
                } else {
                    let mut n_fix = prefix.to_string();
                    if !prop.name.is_empty() {
                        n_fix.push_str(&format!("{}.", &prop.name));
                    }
                    gather_props(&s_table, server_class_index, &n_fix, server_classes, ex_map)
                        .await;
                }
            } else {
                flattened_props.push(FlattenedPropEntry {
                    name: format!("{}{}", &prefix, &prop.name),
                    prop: prop.to_owned(),
                    array_elem_prop: if prop.raw_type == 5 {
                        Some(send_table.properties[i - 1].to_owned())
                    } else {
                        None
                    },
                    index: flattened_props.len() as i32,
                });
            }
        }
    }
}

#[inline]
fn is_prop_excluded(
    st: &SendTable,
    stp: &SendTableProperty,
    ex: &HashMap<String, ExcludeEntry>,
) -> bool {
    if let Some(exclude) = ex.get(&stp.name) {
        if exclude.dt_name == st.name {
            return true;
        }
    }
    false
}
