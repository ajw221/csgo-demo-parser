use std::{
    borrow::Cow,
    io::{Read, Seek},
};

use crate::{
    equipment::{_delete_thrown_grenade, EQ_DECOY, EQ_FLASH, EQ_INCENDIARY, EQ_MOLOTOV, EQ_SMOKE},
    GLOBAL_DISPATCHER, GRENADE_MODEL_INDICES, GRENADE_PROJECTILES, PLAYERS_BY_ENTITY_ID,
};
use ahash::AHashMap as HashMap;
use custom_bitreader::BitReader;
use prost::Message;

use crate::{
    common::Vector64,
    equipment::Equipment,
    sendtable::{SendPropertyFlagsTrait, SendTableProperty},
    serverclass::{FlattenedPropEntry, PropertyValueEnum, ServerClass},
};

pub const _ENTITY_HANDLE_INDEX_MASK: i32 = 2047;
pub const _INVALID_ENTITY_HANDLE: i32 = 2097151;
pub const ENTITY_PLAYER_POSITION_XY_PROP: &str = "cslocaldata.m_vecOrigin";
pub const ENTITY_PLAYER_POSITION_Z_PROP: &str = "cslocaldata.m_vecOrigin[2]";
pub const ENTITY_OTHER_POSITION_CELL_BITS_PROP: &str = "m_cellbits";
pub const ENTITY_OTHER_POSITION_X_PROP: &str = "m_cellX";
pub const ENTITY_OTHER_POSITION_Y_PROP: &str = "m_cellY";
pub const ENTITY_OTHER_POSITION_Z_PROP: &str = "m_cellZ";
pub const ENTITY_OTHER_POSITION_ORIGIN_PROP: &str = "m_vecOrigin";

#[derive(Clone, PartialEq, Eq, Message)]
pub struct CsvcMsgPacketEntities {
    #[prost(int32, optional, tag = "1")]
    pub max_entries: Option<i32>,
    #[prost(int32, optional, tag = "2")]
    pub updated_entries: Option<i32>,
    #[prost(bool, optional, tag = "3")]
    pub is_delta: Option<bool>,
    #[prost(bool, optional, tag = "4")]
    pub update_baseline: Option<bool>,
    #[prost(int32, optional, tag = "5")]
    pub baseline: Option<i32>,
    #[prost(int32, optional, tag = "6")]
    pub delta_from: Option<i32>,
    #[prost(bytes = "vec", optional, tag = "7")]
    pub entity_data: Option<Vec<u8>>,
}

#[inline]
pub fn _coord_from_cell_f32(cell: i32, cell_width: i32, offset: f32) -> f32 {
    (cell * cell_width - 16384) as f32 + offset
}
#[inline]
pub fn coord_from_cell_f64(cell: i32, cell_width: i32, offset: f64) -> f64 {
    (cell * cell_width - 16384) as f64 + offset
}

#[derive(Clone)]
pub struct Entity {
    pub server_class: ServerClass,
    pub id: i32,
    pub props: Vec<Property>,
    pub position: fn(&Entity) -> Vector64,
    pub last_position: Vector64,
    pub inventory: Option<HashMap<i32, Equipment>>,
    pub wep_prefix: Cow<'static, str>,

    pub weapon_cache: Option<[i32; 64]>,
    pub position_history: HashMap<isize, Vector64>,
    pub created_on_tick: isize,
    pub is_in_buyzone: bool,
    pub buyzone_leave_pos: Vector64,
    pub team: u8,

    pub last_flash_duration: f64,
    pub current_flash_frame_agg: u64,
}

impl Default for Entity {
    fn default() -> Self {
        Self {
            server_class: ServerClass {
                id: -1,
                name: "".to_string(),
                dt_id: -1,
                dt_name: "".to_string(),
                base_classes_by_name: None,
                flattened_props: None,
                prop_name_to_idx: None,

                instance_baseline: None,
                preprocessed_baseline: None,

                created_handlers: None,

                index: -1,
            },
            id: -1,
            props: Vec::with_capacity(0),
            position: |_| Vector64::default(),
            last_position: Vector64::default(),
            inventory: None,
            wep_prefix: Cow::Borrowed(""),
            weapon_cache: None,
            position_history: HashMap::new(),
            created_on_tick: -1,
            is_in_buyzone: false,
            buyzone_leave_pos: Vector64::default(),

            team: 0,

            last_flash_duration: 0_f64,
            current_flash_frame_agg: 0_u64,
        }
    }
}

impl Entity {
    #[inline]
    pub fn is_blind(&self) -> bool {
        if let Some(prop) = self.property("m_flFlashDuration") {
            if let PropertyValueEnum::Float(Cow::Owned(flash_duration)) = prop.value {
                return flash_duration > 0.0;
            }
        }
        false
    }

    #[inline]
    pub fn view_direction_x(&self) -> f64 {
        if let Some(prop) = self.property("m_angEyeAngles[1]") {
            if let PropertyValueEnum::Float(Cow::Owned(view)) = prop.value {
                return view;
            }
        }

        0.0
    }

    #[inline]
    pub fn view_direction_y(&self) -> f64 {
        if let Some(prop) = self.property("m_angEyeAngles[0]") {
            if let PropertyValueEnum::Float(Cow::Owned(view)) = prop.value {
                return view;
            }
        }

        0.0
    }

    #[inline]
    pub fn eye_position_z(&self) -> f64 {
        if let Some(prop) = self.property("localdata.m_vecViewOffset[2]") {
            if let PropertyValueEnum::Float(Cow::Owned(z)) = prop.value {
                return z;
            }
        }

        0.0
    }

    #[inline]
    pub fn last_place_name(&self) -> String {
        if let Some(prop) = self.property("m_szLastPlaceName") {
            if let PropertyValueEnum::String(Cow::Owned(last_place_name)) = prop.value {
                return last_place_name;
            }
        }
        "".to_string()
    }

    #[inline]
    pub fn debug_props(&self, filter: Option<&str>, _value_type: Option<PropertyValueEnum>) {
        let mut curr_props = self.props.clone();
        curr_props.sort_by_key(|prop| prop.entry.name.to_lowercase());
        if let Some(filter) = filter {
            curr_props.retain(|prop| prop.entry.name.contains(filter));
        }
        for prop in curr_props.iter() {
            println!("{:?} -> {:?}", prop.entry.name, prop.value);
        }
    }

    #[inline]
    pub fn is_alive(&self) -> bool {
        if let Some(health_prop) = self.property("m_iHealth") {
            if let PropertyValueEnum::Integer(Cow::Owned(health)) = health_prop.value {
                return health > 0;
            }
        }
        false
    }

    #[inline]
    pub async fn destroy(&mut self) {
        if self._is_grenade() {
            let deleted_proj = GRENADE_PROJECTILES.lock().await.remove(&self.id);
            if let Some(proj) = deleted_proj {
                if proj.weapon_instance.eq_type == EQ_FLASH {
                    todo!();
                }

                let is_inferno = proj.weapon_instance.eq_type == EQ_MOLOTOV
                    || proj.weapon_instance.eq_type == EQ_INCENDIARY;
                let is_smoke = proj.weapon_instance.eq_type == EQ_SMOKE;
                let is_decoy = proj.weapon_instance.eq_type == EQ_DECOY;

                if !is_inferno && !is_smoke && !is_decoy {
                    _delete_thrown_grenade(&proj.thrower, proj.weapon_instance.eq_type).await;
                }
            }
        }
    }

    #[inline]
    fn _get_position_prop_names(&self) -> &'static [&'static str] {
        match self.is_player() {
            true => &["cslocaldata.m_vecOrigin", "cslocaldata.m_vecOrigin[2]"],
            false => &["m_cellX", "m_cellY", "m_cellZ", "m_vecOrigin"],
        }
    }

    #[inline]
    fn _get_entity_type(&self) -> &'static str {
        if self._is_game_rules() {
            "game_rules"
        } else if self._is_planted_bomb() {
            "planted_bomb"
        } else if self._is_bomb() {
            "bomb"
        } else if self._is_base_trigger() {
            "base_trigger"
        } else if self._is_player_resource() {
            "player_resource"
        } else if self.is_player() {
            "player"
        } else if self._is_team() {
            "team"
        } else if self._is_weapon() {
            "weapon"
        } else if self._is_grenade() {
            "grenade"
        } else if self._is_inferno() {
            "inferno"
        } else {
            "other"
        }
    }

    #[inline]
    pub fn is_player(&self) -> bool {
        self.server_class.name == "CCSPlayer"
    }

    #[inline]
    pub fn _is_team(&self) -> bool {
        self.server_class.name == "CCSTeam"
    }

    #[inline]
    pub fn _is_weapon(&self) -> bool {
        self.server_class._base_class_exists("DT_WeaponCSBase")
            && !self.server_class._base_class_exists("DT_BaseCSGrenade")
    }

    #[inline]
    pub fn _is_grenade(&self) -> bool {
        self.server_class._base_class_exists("DT_BaseCSGrenade")
            || self.server_class._base_class_exists("DT_BaseGrenade")
    }

    #[inline]
    pub fn _is_inferno(&self) -> bool {
        self.server_class.name == "CInferno"
    }

    #[inline]
    pub fn _is_player_resource(&self) -> bool {
        self.server_class.name == "CCSPlayerResource"
    }

    #[inline]
    pub fn _is_base_trigger(&self) -> bool {
        self.server_class.name == "CBaseTrigger"
    }

    #[inline]
    pub fn _is_bomb(&self) -> bool {
        self.server_class.name == "CC4"
    }

    #[inline]
    pub fn _is_planted_bomb(&self) -> bool {
        self.server_class.name == "CPlantedC4"
    }

    #[inline]
    pub fn _is_game_rules(&self) -> bool {
        self.server_class.name == "CCSGameRulesProxy"
    }

    #[inline]
    pub fn property(&self, name: &str) -> Option<Property> {
        if let Some(prop_name_to_idx) = &self.server_class.prop_name_to_idx {
            return prop_name_to_idx
                .get(name)
                .map(|idx| unsafe { self.props.get_unchecked(*idx as usize).to_owned() });
        }
        None
    }

    #[inline]
    pub fn _property_value_must(&self, name: &str) -> PropertyValueEnum {
        self.property(name).unwrap().value
    }

    #[inline]
    pub fn apply_baseline(&mut self) {
        if let Some(preprocessed_baseline) = &mut self.server_class.preprocessed_baseline {
            for idx in 0..preprocessed_baseline.len() {
                self.props[idx].value = preprocessed_baseline[&(idx as i32)].to_owned();
            }
        }
    }

    #[inline]
    pub async fn apply_update<T: Read + Seek + Send>(
        &mut self,
        reader: &mut BitReader<T>,
        prop_indices: &mut Vec<u32>,
    ) {
        prop_indices.clear();
        let mut idx = -1_i32;
        let new_way = reader.read_bit();
        idx = reader.read_field_index(idx as isize, new_way) as i32;
        while idx != -1 {
            prop_indices.push(idx as u32);
            idx = reader.read_field_index(idx as isize, new_way) as i32;
        }
        let is_blind = self.is_blind();
        let mut position_updated = false;
        for idx in prop_indices {
            decode_prop(&mut self.props[*idx as usize], reader);
            match self.props[*idx as usize].entry.name.as_str() {
                ENTITY_PLAYER_POSITION_XY_PROP | ENTITY_PLAYER_POSITION_Z_PROP => {
                    if self.is_player() {
                        position_updated = true;
                    }
                }
                ENTITY_OTHER_POSITION_X_PROP
                | ENTITY_OTHER_POSITION_Y_PROP
                | ENTITY_OTHER_POSITION_Z_PROP
                | ENTITY_OTHER_POSITION_ORIGIN_PROP => {
                    if !self.is_player() {
                        position_updated = true;
                    }
                }
                "m_nModelIndex" => {
                    if let Some(proj) = GRENADE_PROJECTILES.lock().await.get_mut(&self.id) {
                        if let PropertyValueEnum::Integer(Cow::Owned(handle)) =
                            self.props[*idx as usize].value
                        {
                            if let Some(wep_type) = GRENADE_MODEL_INDICES.lock().await.get(&handle)
                            {
                                proj.wep_type = wep_type.to_owned();
                            }
                        }
                    }
                }
                "m_hThrower" | "m_hOwnerEntity" => {
                    if let Some(proj) = GRENADE_PROJECTILES.lock().await.get_mut(&self.id) {
                        if let PropertyValueEnum::Integer(Cow::Owned(handle)) =
                            self.props[*idx as usize].value
                        {
                            if handle != _INVALID_ENTITY_HANDLE {
                                let entity_id = handle & _ENTITY_HANDLE_INDEX_MASK;
                                if let Some(person) =
                                    PLAYERS_BY_ENTITY_ID.lock().await.get(&entity_id)
                                {
                                    if self.props[*idx as usize].entry.name == "m_hThrower" {
                                        proj.thrower = Some(person.to_owned());
                                    } else if self.props[*idx as usize].entry.name
                                        == "m_hOwnerEntity"
                                    {
                                        proj.owner = Some(person.to_owned());
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if self.is_player() {
            if let PropertyValueEnum::Integer(Cow::Owned(buyzone_val)) =
                self._property_value_must("m_bInBuyZone")
            {
                let is_in_buyzone = buyzone_val == 1;
                if self.is_in_buyzone && !is_in_buyzone {
                    let pos = self.get_position();

                    let team = self._property_value_must("m_iTeamNum").as_integer();

                    GLOBAL_DISPATCHER
                        .lock()
                        .await
                        .emit(
                            "player_left_buyzone",
                            ((self.id, team), (pos.x.into_owned(), pos.y.into_owned())),
                        )
                        .await;
                }
                self.is_in_buyzone = is_in_buyzone;
            }

            if is_blind && !self.is_blind() {
                self.last_flash_duration = 0.0;
                self.current_flash_frame_agg = 0;
            }
        } else if let Some(proj) = GRENADE_PROJECTILES.lock().await.get_mut(&self.id) {
            if position_updated {
                let new_pos = self.get_position();

                if new_pos != self.last_position {
                    proj.trajectory.push(new_pos.to_owned());
                    self.last_position = new_pos;
                }
            }
        }
    }

    #[inline]
    pub fn initialize(entity: &mut Entity) {
        if entity.is_player() {
            entity.position = Self::player_initialize;
        } else {
            entity.position = Self::other_initialize;
        }
    }

    #[inline]
    pub fn player_initialize(entity: &Entity) -> Vector64 {
        if let (PropertyValueEnum::Vector(Cow::Owned(xy)), PropertyValueEnum::Float(z)) = (
            entity._property_value_must(ENTITY_PLAYER_POSITION_XY_PROP),
            entity._property_value_must(ENTITY_PLAYER_POSITION_Z_PROP),
        ) {
            return Vector64 {
                x: xy.x,
                y: xy.y,
                z,
            };
        }
        Vector64::default()
    }

    #[inline]
    pub fn other_initialize(entity: &Entity) -> Vector64 {
        if let (
            PropertyValueEnum::Integer(Cow::Owned(cell_bits)),
            PropertyValueEnum::Integer(Cow::Owned(cell_x)),
            PropertyValueEnum::Integer(Cow::Owned(cell_y)),
            PropertyValueEnum::Integer(Cow::Owned(cell_z)),
            PropertyValueEnum::Vector(Cow::Owned(offset)),
        ) = (
            entity._property_value_must(ENTITY_OTHER_POSITION_CELL_BITS_PROP),
            entity._property_value_must(ENTITY_OTHER_POSITION_X_PROP),
            entity._property_value_must(ENTITY_OTHER_POSITION_Y_PROP),
            entity._property_value_must(ENTITY_OTHER_POSITION_Z_PROP),
            entity._property_value_must(ENTITY_OTHER_POSITION_ORIGIN_PROP),
        ) {
            let cell_width = 1 << cell_bits;
            return Vector64 {
                x: Cow::Owned(coord_from_cell_f64(
                    cell_x,
                    cell_width,
                    offset.x.into_owned(),
                )),
                y: Cow::Owned(coord_from_cell_f64(
                    cell_y,
                    cell_width,
                    offset.y.into_owned(),
                )),
                z: Cow::Owned(coord_from_cell_f64(
                    cell_z,
                    cell_width,
                    offset.z.into_owned(),
                )),
            };
        }

        Vector64::default()
    }

    #[inline]
    pub fn get_position(&self) -> Vector64 {
        (self.position)(self)
    }

    #[inline]
    pub fn on_position_update(_entity: &mut Entity) {
        todo!();
    }

    #[inline]
    fn _fire_player_pos_update(
        entity: &mut Entity,
        prop_val: &mut PropertyValueEnum,
        h: fn(&mut Entity, &mut PropertyValueEnum, &Vector64),
    ) {
        let new_pos = entity.get_position();
        if new_pos != entity.last_position {
            h(entity, prop_val, &new_pos);
            entity.last_position = new_pos;
        }
    }

    #[inline]
    pub fn _active_weapon_id(&self) -> i32 {
        if self.is_player() {
            let active_weapon_prop = self.property("m_hActiveWeapon");
            if let Some(active_weapon_prop) = active_weapon_prop {
                if let PropertyValueEnum::Integer(Cow::Owned(handle)) = active_weapon_prop.value {
                    return handle & 2047;
                }
            }
        }
        -1
    }

    #[inline]
    pub fn _last_place_name(&self) -> Cow<'static, str> {
        if self.is_player() {
            let last_place_name_property = self.property("m_szLastPlaceName");
            if let Some(last_place_name_property) = last_place_name_property {
                if let PropertyValueEnum::String(str_val) = last_place_name_property.value {
                    return str_val;
                }
            }
        }
        Cow::Borrowed("")
    }
}

type UpdateHandler = fn(&mut Entity, &mut PropertyValueEnum);

#[derive(Clone)]
pub struct Property {
    pub entry: FlattenedPropEntry,
    pub value: PropertyValueEnum,
    pub update_handlers: Option<Vec<UpdateHandler>>,
}

impl Property {
    #[inline]
    pub fn on_update(prop: &mut Property, h: fn(&mut Entity, &mut PropertyValueEnum)) {
        if let Some(update_handlers) = prop.update_handlers.as_mut() {
            update_handlers.push(h);
        }
    }
}

#[inline]
pub fn decode_prop<T: Read + Seek + Send>(prop: &mut Property, reader: &mut BitReader<T>) {
    match prop.entry.prop.raw_type {
        0 => {
            prop.value =
                PropertyValueEnum::Integer(Cow::Owned(decode_int(&prop.entry.prop, reader)))
        }
        1 => prop.value = PropertyValueEnum::Float(decode_float(&prop.entry.prop, reader)),
        2 => {
            prop.value =
                PropertyValueEnum::Vector(Cow::Owned(decode_vector(&prop.entry.prop, reader)))
        }
        3 => {
            prop.value =
                PropertyValueEnum::Vector(Cow::Owned(decode_vectorxy(&prop.entry.prop, reader)))
        }
        4 => prop.value = PropertyValueEnum::String(Cow::Owned(decode_string(reader))),
        5 => prop.value = PropertyValueEnum::Array(decode_array(&prop.entry, reader)),
        _ => panic!("Unknown prop type {}", prop.entry.prop.raw_type),
    }
}

#[inline]
fn decode_int<T: Read + Seek + Send>(prop: &SendTableProperty, reader: &mut BitReader<T>) -> i32 {
    match prop.flags.has_flag_set(524288) {
        true => match prop.flags.has_flag_set(1) {
            true => reader.read_varint32() as i32,
            false => reader.read_signed_varint32(),
        },
        false => match prop.flags.has_flag_set(1) {
            true => reader.read_int(prop.num_bits as usize) as i32,
            false => reader.read_signed_int(prop.num_bits as usize) as i32,
        },
    }
}

#[inline]
fn decode_float<T: Read + Seek + Send>(
    prop: &SendTableProperty,
    reader: &mut BitReader<T>,
) -> Cow<'static, f64> {
    match prop.flags & 258086 != 0 {
        true => decode_special_float(prop, reader),
        false => Cow::Owned(
            prop.low_value as f64
                + ((prop.high_value - prop.low_value) as f64
                    * (reader.read_int(prop.num_bits as usize) as f64
                        / (((1 << prop.num_bits) - 1) as f64))),
        ),
    }
}

#[inline]
fn decode_special_float<T: Read + Seek + Send>(
    prop: &SendTableProperty,
    reader: &mut BitReader<T>,
) -> Cow<'static, f64> {
    if prop.flags.has_flag_set(2) {
        Cow::Owned(reader.read_bitcoord() as f64)
    } else if prop.flags.has_flag_set(4096) {
        Cow::Owned(reader.read_bitcoordmp(false, false) as f64)
    } else if prop.flags.has_flag_set(8192) {
        Cow::Owned(reader.read_bitcoordmp(false, true) as f64)
    } else if prop.flags.has_flag_set(16384) {
        Cow::Owned(reader.read_bitcoordmp(true, false) as f64)
    } else if prop.flags.has_flag_set(4) {
        Cow::Owned(reader.read_float() as f64)
    } else if prop.flags.has_flag_set(32) {
        Cow::Owned(reader.read_bitnormal() as f64)
    } else {
        Cow::Owned(reader.read_bitcellcoord(
            prop.num_bits as usize,
            prop.flags.has_flag_set(65536),
            prop.flags.has_flag_set(131072),
        ) as f64)
    }
}

#[inline]
fn decode_vector<T: Read + Seek + Send>(
    prop: &SendTableProperty,
    reader: &mut BitReader<T>,
) -> Vector64 {
    let x = decode_float(prop, reader);
    let y = decode_float(prop, reader);
    Vector64 {
        x: x.clone(),
        y: y.clone(),
        z: match !prop.flags.has_flag_set(32) {
            true => decode_float(prop, reader),
            false => {
                let _x = x.into_owned();
                let _y = y.into_owned();
                let absolute = (_x * _x) + (_y * _y);
                let is_neg = reader.read_bit();
                match absolute < 1.0 {
                    true => match is_neg {
                        true => Cow::Owned(-f64::sqrt(1.0 - absolute)),
                        false => Cow::Owned(f64::sqrt(1.0 - absolute)),
                    },
                    false => Cow::Owned(0.0),
                }
            }
        },
    }
}

#[inline]
fn decode_vectorxy<T: Read + Seek + Send>(
    prop: &SendTableProperty,
    reader: &mut BitReader<T>,
) -> Vector64 {
    Vector64 {
        x: decode_float(prop, reader),
        y: decode_float(prop, reader),
        ..Default::default()
    }
}

#[inline]
fn decode_string<T: Read + Seek + Send>(reader: &mut BitReader<T>) -> String {
    let length = std::cmp::min(reader.read_int(9), 512);
    reader.read_cstring(length)
}

#[inline]
fn decode_array<T: Read + Seek + Send>(
    fprop: &FlattenedPropEntry,
    reader: &mut BitReader<T>,
) -> Vec<PropertyValueEnum> {
    let num_bits = f64::floor(f64::log2(fprop.prop.num_elems as f64) + 1.0);
    let mut res = vec![PropertyValueEnum::None; reader.read_int(num_bits as usize)];
    let mut tmp = Property {
        entry: FlattenedPropEntry {
            prop: fprop.array_elem_prop.clone().unwrap(),
            ..Default::default()
        },
        value: PropertyValueEnum::None,
        update_handlers: None,
    };

    for i in res.iter_mut() {
        decode_prop(&mut tmp, reader);
        *i = tmp.value.to_owned();
    }

    res
}
