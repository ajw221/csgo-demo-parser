use std::borrow::Cow;

use lazy_static::lazy_static;
use tokio::sync::Mutex;

use ahash::AHashMap as HashMap;

use crate::{
    common::Vector64,
    entity::{Entity, _ENTITY_HANDLE_INDEX_MASK, _INVALID_ENTITY_HANDLE},
    player::{Player, PlayerInfo},
    serverclass::PropertyValueEnum,
    GRENADE_PROJECTILES, PLAYERS_BY_ENTITY_ID, PLAYERS_BY_STEAM_ID, PLAYER_INFO_BY_STEAM_ID,
    PLAYER_INFO_BY_USER_ID, SERVER_CLASSES, THROWN_GRENADES,
};

pub const _EQ_CLASSUNKNOWN: i32 = 0;
pub const _EQ_CLASSPISTOLS: i32 = 1;
pub const _EQ_CLASSSMG: i32 = 2;
pub const _EQ_CLASSHEAVY: i32 = 3;
pub const _EQ_CLASSRIFLE: i32 = 4;
pub const _EQ_CLASSEQUIPMENT: i32 = 5;
pub const _EQ_CLASSGRENADE: i32 = 6;

pub const EQ_UNKNOWN: i32 = 0;

// Pistols
pub const EQ_P2000: i32 = 1;
pub const EQ_GLOCK: i32 = 2;
pub const EQ_P250: i32 = 3;
pub const EQ_DEAGLE: i32 = 4;
pub const EQ_FIVESEVEN: i32 = 5;
pub const EQ_DUALBERETTAS: i32 = 6;
pub const EQ_TEC9: i32 = 7;
pub const EQ_CZ: i32 = 8;
pub const EQ_USP: i32 = 9;
pub const EQ_REVOLVER: i32 = 10;

// SMGs
pub const EQ_MP7: i32 = 101;
pub const EQ_MP9: i32 = 102;
pub const EQ_BIZON: i32 = 103;
pub const EQ_MAC10: i32 = 104;
pub const EQ_UMP: i32 = 105;
pub const EQ_P90: i32 = 106;
pub const EQ_MP5: i32 = 107;

// Heavy
pub const EQ_SAWEDOFF: i32 = 201;
pub const EQ_NOVA: i32 = 202;
pub const _EQ_MAG7: i32 = 203;
pub const EQ_SWAG7: i32 = 203;
pub const EQ_XM1014: i32 = 204;
pub const EQ_M249: i32 = 205;
pub const EQ_NEGEV: i32 = 206;

// Rifles
pub const EQ_GALIL: i32 = 301;
pub const EQ_FAMAS: i32 = 302;
pub const EQ_AK47: i32 = 303;
pub const EQ_M4A4: i32 = 304;
pub const EQ_M4A1: i32 = 305;
pub const EQ_SCOUT: i32 = 306;
pub const _EQ_SSG08: i32 = 306;
pub const EQ_SG556: i32 = 307;
pub const EQ_SG553: i32 = 307;
pub const EQ_AUG: i32 = 308;
pub const EQ_AWP: i32 = 309;
pub const EQ_SCAR20: i32 = 310;
pub const EQ_G3SG1: i32 = 311;

// Equipment
pub const EQ_ZEUS: i32 = 401;
pub const EQ_KEVLAR: i32 = 402;
pub const EQ_HELMET: i32 = 403;
pub const EQ_BOMB: i32 = 404;
pub const EQ_KNIFE: i32 = 405;
pub const EQ_DEFUSEKIT: i32 = 406;
pub const EQ_WORLD: i32 = 407;

// Grenades
pub const EQ_DECOY: i32 = 501;
pub const EQ_MOLOTOV: i32 = 502;
pub const EQ_INCENDIARY: i32 = 503;
pub const EQ_FLASH: i32 = 504;
pub const EQ_SMOKE: i32 = 505;
pub const EQ_HE: i32 = 506;

lazy_static! {
    pub static ref EQUIPMENT_MAPPING: Mutex<HashMap<i32, i32>> = Mutex::new(HashMap::new());
    pub static ref EQUIPMENT_NAME_TO_WEAPON: HashMap<String, i32> = HashMap::from([
        ("ak47".to_string(),                    EQ_AK47),
        ("aug".to_string(),                     EQ_AUG),
        ("awp".to_string(),                     EQ_AWP),
        ("bizon".to_string(),                   EQ_BIZON),
        ("c4".to_string(),                      EQ_BOMB),
        ("deagle".to_string(),                  EQ_DEAGLE),
        ("decoy".to_string(),                   EQ_DECOY),
        ("decoygrenade".to_string(),            EQ_DECOY),
        ("decoyprojectile".to_string(),         EQ_DECOY),
        ("decoy_projectile".to_string(),        EQ_DECOY),
        ("elite".to_string(),                   EQ_DUALBERETTAS),
        ("famas".to_string(),                   EQ_FAMAS),
        ("fiveseven".to_string(),               EQ_FIVESEVEN),
        ("flashbang".to_string(),               EQ_FLASH),
        ("g3sg1".to_string(),                   EQ_G3SG1),
        ("galil".to_string(),                   EQ_GALIL),
        ("galilar".to_string(),                 EQ_GALIL),
        ("glock".to_string(),                   EQ_GLOCK),
        ("hegrenade".to_string(),               EQ_HE),
        ("hkp2000".to_string(),                 EQ_P2000),
        ("incgrenade".to_string(),              EQ_INCENDIARY),
        ("incendiarygrenade".to_string(),       EQ_INCENDIARY),
        ("m249".to_string(),                    EQ_M249),
        ("m4a1".to_string(),                    EQ_M4A4),
        ("mac10".to_string(),                   EQ_MAC10),
        ("mag7".to_string(),                    EQ_SWAG7),
        ("molotov".to_string(),                 EQ_MOLOTOV),
        ("molotovgrenade".to_string(),          EQ_MOLOTOV),
        ("molotovprojectile".to_string(),       EQ_MOLOTOV),
        ("molotov_projectile".to_string(),      EQ_MOLOTOV),
        ("mp7".to_string(),                     EQ_MP7),
        ("mp5sd".to_string(),                   EQ_MP5),
        ("mp9".to_string(),                     EQ_MP9),
        ("negev".to_string(),                   EQ_NEGEV),
        ("nova".to_string(),                    EQ_NOVA),
        ("p250".to_string(),                    EQ_P250),
        ("p90".to_string(),                     EQ_P90),
        ("sawedoff".to_string(),                EQ_SAWEDOFF),
        ("scar20".to_string(),                  EQ_SCAR20),
        ("sg556".to_string(),                   EQ_SG556),
        ("smokegrenade".to_string(),            EQ_SMOKE),
        ("smokegrenadeprojectile".to_string(),  EQ_SMOKE),
        ("smokegrenade_projectile".to_string(), EQ_SMOKE),
        ("ssg08".to_string(),                   EQ_SCOUT),
        ("taser".to_string(),                   EQ_ZEUS),
        ("tec9".to_string(),                    EQ_TEC9),
        ("ump45".to_string(),                   EQ_UMP),
        ("xm1014".to_string(),                  EQ_XM1014),
        ("m4a1_silencer".to_string(),           EQ_M4A1),
        ("m4a1_silencer_off".to_string(),       EQ_M4A1),
        ("cz75a".to_string(),                   EQ_CZ),
        ("usp".to_string(),                     EQ_USP),
        ("usp_silencer".to_string(),            EQ_USP),
        ("usp_silencer_off".to_string(),        EQ_USP),
        ("world".to_string(),                   EQ_WORLD),
        ("inferno".to_string(),                 EQ_INCENDIARY),
        ("revolver".to_string(),                EQ_REVOLVER),
        ("vest".to_string(),                    EQ_KEVLAR),
        ("vesthelm".to_string(),                EQ_HELMET),
        ("defuser".to_string(),                 EQ_DEFUSEKIT),

        // These don't exist and/or used to crash the game with the give command
        ("scar17".to_string(),                  EQ_UNKNOWN),
        ("sensorgrenade".to_string(),           EQ_UNKNOWN),
        ("mp5navy".to_string(),                 EQ_UNKNOWN),
        ("p228".to_string(),                    EQ_UNKNOWN),
        ("scout".to_string(),                   EQ_UNKNOWN),
        ("sg550".to_string(),                   EQ_UNKNOWN),
        ("sg552".to_string(),                   EQ_UNKNOWN),
        ("tmp".to_string(),                     EQ_UNKNOWN),
        ("worldspawn".to_string(),              EQ_WORLD),
    ]);
    pub static ref EQUIPMENT_ELEMENT_TO_NAME: HashMap<i32, &'static str> = HashMap::from([
        (EQ_AK47,         "AK-47"),
        (EQ_AUG,          "AUG"),
        (EQ_AWP,          "AWP"),
        (EQ_BIZON,        "PP-Bizon"),
        (EQ_BOMB,         "C4"),
        (EQ_DEAGLE,       "Desert Eagle"),
        (EQ_DECOY,        "Decoy Grenade"),
        (EQ_DUALBERETTAS, "Dual Berettas"),
        (EQ_FAMAS,        "FAMAS"),
        (EQ_FIVESEVEN,    "Five-SeveN"),
        (EQ_FLASH,        "Flashbang"),
        (EQ_G3SG1,        "G3SG1"),
        (EQ_GALIL,        "Galil AR"),
        (EQ_GLOCK,        "Glock-18"),
        (EQ_HE,           "HE Grenade"),
        (EQ_P2000,        "P2000"),
        (EQ_INCENDIARY,   "Incendiary Grenade"),
        (EQ_M249,         "M249"),
        (EQ_M4A4,         "M4A4"),
        (EQ_MAC10,        "MAC-10"),
        (EQ_SWAG7,        "MAG-7"),
        (EQ_MOLOTOV,      "Molotov"),
        (EQ_MP7,          "MP7"),
        (EQ_MP5,          "MP5-SD"),
        (EQ_MP9,          "MP9"),
        (EQ_NEGEV,        "Negev"),
        (EQ_NOVA,         "Nova"),
        (EQ_P250,         "P250"),
        (EQ_P90,          "P90"),
        (EQ_SAWEDOFF,     "Sawed-Off"),
        (EQ_SCAR20,       "SCAR-20"),
        (EQ_SG553,        "SG 553"),
        (EQ_SMOKE,        "Smoke Grenade"),
        (EQ_SCOUT,        "SSG 08"),
        (EQ_ZEUS,         "Zeus x27"),
        (EQ_TEC9,         "Tec-9"),
        (EQ_UMP,          "UMP-45"),
        (EQ_XM1014,       "XM1014"),
        (EQ_M4A1,         "M4A1"),
        (EQ_CZ,           "CZ75 Auto"),
        (EQ_USP,          "USP-S"),
        (EQ_WORLD,        "World"),
        (EQ_REVOLVER,     "R8 Revolver"),
        (EQ_KEVLAR,       "Kevlar Vest"),
        (EQ_HELMET,       "Kevlar + Helmet"),
        (EQ_DEFUSEKIT,    "Defuse Kit"),
        (EQ_KNIFE,        "Knife"),
        (EQ_UNKNOWN,      "UNKNOWN"),
    ]);
    pub static ref EQUIPMENT_TO_ALTERNATIVE: HashMap<i32, i32> = HashMap::from([
        (EQ_P2000,     EQ_USP),
        (EQ_P250,      EQ_CZ),
        (EQ_FIVESEVEN, EQ_CZ),
        (EQ_TEC9,      EQ_CZ),
        (EQ_DEAGLE,    EQ_REVOLVER),
        (EQ_MP7,       EQ_MP5),
        (EQ_M4A4,      EQ_M4A1),
    ]);
}

#[inline]
pub async fn map_equipment() {
    for sc in SERVER_CLASSES.lock().await.iter() {
        match sc.name.as_str() {
            "CC4" => {
                EQUIPMENT_MAPPING.lock().await.insert(sc.id, EQ_BOMB);
            }
            "CWeaponNOVA" | "CWeaponSawedoff" | "CWeaponXM1014" => {
                EQUIPMENT_MAPPING
                    .lock()
                    .await
                    .insert(sc.id, map_equipment_name(&sc.name[7..].to_lowercase()));
            }
            "CKnife" => {
                EQUIPMENT_MAPPING.lock().await.insert(sc.id, EQ_KNIFE);
            }
            "CSnowball" | "CWeaponShield" | "CWeaponZoneRepulsor" => continue,
            _ => {
                if sc._base_class_exists("DT_WeaponCSBaseGun") {
                    EQUIPMENT_MAPPING
                        .lock()
                        .await
                        .insert(sc.id, map_equipment_name(&sc.dt_name[9..].to_lowercase()));
                } else if sc._base_class_exists("DT_BaseCSGrenade") {
                    EQUIPMENT_MAPPING
                        .lock()
                        .await
                        .insert(sc.id, map_equipment_name(&sc.dt_name[3..].to_lowercase()));
                }
            }
        }
    }
}

#[inline]
pub fn map_equipment_name(mut eq_name: &str) -> i32 {
    eq_name = eq_name.trim_start_matches("weapon_");
    if eq_name.contains("knife") || eq_name.contains("bayonet") {
        return EQ_KNIFE;
    }
    EQUIPMENT_NAME_TO_WEAPON[eq_name].to_owned()
}

#[derive(Clone)]
pub struct Equipment {
    pub eq_type: i32,
    pub entity: Option<Entity>,
    pub owner: Option<Player>,
    pub original_string: String,
    pub entity_id: i32,
    pub owner_entity_id: i32,
    pub landed: bool,
}

impl Default for Equipment {
    fn default() -> Self {
        Self {
            eq_type: -1,
            entity_id: -1,
            entity: None,
            owner: None,
            original_string: "".to_string(),
            owner_entity_id: -1,
            landed: false,
        }
    }
}

unsafe impl Send for Equipment {}
unsafe impl Sync for Equipment {}

#[derive(Clone, Default)]
pub struct GrenadeProjectile {
    pub entity_id: i32,
    pub weapon_instance: Equipment,
    pub thrower: Option<Player>,
    pub owner: Option<Player>,
    pub thrower_info: Option<PlayerInfo>,
    pub owner_info: Option<PlayerInfo>,
    pub trajectory: Vec<Vector64>,
    pub wep_type: i32,
}

pub async fn bind_weapons() {
    let mut server_classes = SERVER_CLASSES.lock().await;
    for sc in server_classes.iter_mut() {
        if sc._base_class_exists("DT_WeaponCSBase") && !sc._base_class_exists("DT_BaseCSGrenade") {
            match sc.created_handlers.as_mut() {
                Some(created_handlers) => {
                    created_handlers.push(|e| Box::pin(async move { bind_weapon(e).await }));
                }
                None => {
                    sc.created_handlers =
                        Some(vec![|e| Box::pin(async move { bind_weapon(e).await })])
                }
            };
        } else if sc._base_class_exists("DT_BaseCSGrenade")
            || sc._base_class_exists("DT_BaseGrenade")
        {
            match sc.created_handlers.as_mut() {
                Some(created_handlers) => created_handlers
                    .push(|e| Box::pin(async move { bind_grenade_projectiles(e).await })),
                None => {
                    sc.created_handlers = Some(vec![|e| {
                        Box::pin(async move { bind_grenade_projectiles(e).await })
                    }])
                }
            }
        }
    }
}

#[inline]
async fn bind_weapon(_entity_id: i32) {
    todo!()
}

#[inline]
async fn bind_grenade_projectiles(entity_id: i32) {
    GRENADE_PROJECTILES
        .lock()
        .await
        .entry(entity_id)
        .or_insert(GrenadeProjectile {
            entity_id,
            ..Default::default()
        });
}

#[inline]
pub fn get_player_weapon(player: &Option<Player>, wep_type: i32) -> Equipment {
    if let Some(pl) = player {
        if let Some(alt_type) = EQUIPMENT_TO_ALTERNATIVE.get(&wep_type) {
            for (_, wep) in pl.inventory.iter() {
                if wep.eq_type == wep_type || &wep.eq_type == alt_type {
                    return wep.to_owned();
                }
            }
        }
    }

    Equipment {
        eq_type: wep_type,
        ..Default::default()
    }
}

#[inline]
pub async fn _retrieve_potential_thrower_owner(entity: &Entity, proj: &mut GrenadeProjectile) {
    if let Some(prop) = entity.property("m_hThrower") {
        if let PropertyValueEnum::Integer(Cow::Owned(handle)) = prop.value {
            if handle != _INVALID_ENTITY_HANDLE {
                let entity_id = handle & _ENTITY_HANDLE_INDEX_MASK;
                if let Some(player) = PLAYERS_BY_ENTITY_ID.lock().await.get(&entity_id) {
                    proj.thrower = Some(player.to_owned());
                } else if let Some(player_info) =
                    PLAYER_INFO_BY_USER_ID.lock().await.get(&(entity_id - 1))
                {
                    proj.thrower_info = Some(player_info.to_owned());
                }
            }
        }
    } else if let Some(prop) = entity.property("m_hOwner") {
        if let PropertyValueEnum::Integer(Cow::Owned(handle)) = prop.value {
            if handle != _INVALID_ENTITY_HANDLE {
                let entity_id = handle & _ENTITY_HANDLE_INDEX_MASK;
                if let Some(player) = PLAYERS_BY_ENTITY_ID.lock().await.get(&entity_id) {
                    proj.owner = Some(player.to_owned());
                } else if let Some(player_info) =
                    PLAYER_INFO_BY_USER_ID.lock().await.get(&(entity_id - 1))
                {
                    proj.owner_info = Some(player_info.to_owned());
                }
            }
        }
    } else if let Some(prop) = entity.property("m_hOwnerEntity") {
        if let PropertyValueEnum::Integer(Cow::Owned(handle)) = prop.value {
            if handle != _INVALID_ENTITY_HANDLE {
                let entity_id = handle & _ENTITY_HANDLE_INDEX_MASK;
                if let Some(player) = PLAYERS_BY_ENTITY_ID.lock().await.get(&entity_id) {
                    proj.owner = Some(player.to_owned());
                } else if let Some(player_info) =
                    PLAYER_INFO_BY_USER_ID.lock().await.get(&(entity_id - 1))
                {
                    proj.owner_info = Some(player_info.to_owned());
                }
            }
        }
    } else if let Some(prop) = entity.property("m_hPrevOwner") {
        if let PropertyValueEnum::Integer(Cow::Owned(handle)) = prop.value {
            if handle != _INVALID_ENTITY_HANDLE {
                let entity_id = handle & _ENTITY_HANDLE_INDEX_MASK;
                if let Some(player) = PLAYERS_BY_ENTITY_ID.lock().await.get(&entity_id) {
                    proj.owner = Some(player.to_owned());
                } else if let Some(player_info) =
                    PLAYER_INFO_BY_USER_ID.lock().await.get(&(entity_id - 1))
                {
                    proj.owner_info = Some(player_info.to_owned());
                }
            }
        }
    } else if let (Some(lo_prop), Some(hi_prop)) = (
        entity.property("m_OriginalOwnerXuidLow"),
        entity.property("m_OriginalOwnerXuidHigh"),
    ) {
        if let (
            PropertyValueEnum::Integer(Cow::Owned(lo)),
            PropertyValueEnum::Integer(Cow::Owned(hi)),
        ) = (lo_prop.value, hi_prop.value)
        {
            let steam_id: u64 = (lo as u64) | ((hi as u64) << 32);
            if let Some(player) = PLAYERS_BY_STEAM_ID.lock().await.get(&steam_id) {
                proj.owner = Some(player.to_owned());
            } else if let Some(player_info) = PLAYER_INFO_BY_STEAM_ID.lock().await.get(&steam_id) {
                proj.owner_info = Some(player_info.to_owned());
            }
        }
    }
}

#[inline]
pub async fn _add_thrown_grenade(player: &Option<Player>, wep: &Equipment) {
    if let Some(pl) = player {
        let mut thrown_grenades = THROWN_GRENADES.lock().await;
        if let Some(nades) = thrown_grenades.get_mut(&pl.entity_id) {
            nades.push(wep.to_owned());
        } else {
            thrown_grenades.insert(pl.entity_id, vec![wep.to_owned()]);
        }
    }
}

#[inline]
pub async fn _delete_thrown_grenade(player: &Option<Player>, wep_type: i32) {
    if let Some(pl) = player {
        if let Some(weapons) = THROWN_GRENADES.lock().await.get_mut(&pl.entity_id) {
            let mut index_to_remove: Option<usize> = None;
            for (i, weapon) in weapons.iter_mut().enumerate() {
                if is_same_equipment(wep_type, weapon.eq_type) {
                    index_to_remove = Some(i);
                    break;
                }
            }
            if let Some(index) = index_to_remove {
                weapons.remove(index);
            }
        }
    }
}

#[inline]
fn is_same_equipment(a: i32, b: i32) -> bool {
    a == b || (a == EQ_INCENDIARY && b == EQ_MOLOTOV) || (b == EQ_INCENDIARY && a == EQ_MOLOTOV)
}
