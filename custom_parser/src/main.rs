#![allow(
    unreachable_code,
    unused_imports,
    unused_variables,
    unused_imports,
    unused_assignments,
    unused_mut
)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::diverging_sub_expression)]
use std::{borrow::Cow, fs::File, io::BufReader, sync::Arc};

use ahash::AHashMap as HashMap;
use glam::{DQuat, DVec3, Vec3};
use image::ImageFormat;
use lazy_static::lazy_static;
use plotters::{
    backend::RGBPixel,
    coord::types::RangedCoordf64,
    prelude::*,
    style::{
        full_palette::{ORANGE, PURPLE},
        text_anchor::{HPos, Pos, VPos},
    },
};
use prost::Message;
use tokio::sync::Mutex;

lazy_static! {
    static ref MATCH_STARTED: Mutex<bool> = Mutex::new(false);
    static ref ROUND_STARTED: Mutex<bool> = Mutex::new(false);
    static ref ROUND_IN_END_TIME: Mutex<bool> = Mutex::new(false);
    static ref ROUND_IN_FREEZE_TIME: Mutex<bool> = Mutex::new(false);
    static ref CURRENT_FRAME_IDX: Mutex<i64> = Mutex::new(0);
    // static ref CONV_PARSED: Mutex<bool> = Mutex::new(false);
    static ref CURRENT_ROUND: Arc<Mutex<GameRound>> = Arc::new(Mutex::new(GameRound {
        round_num: 0,
        start_tick: -1,
        freeze_time_end_tick: -1,
        bomb_plant_tick: -1,
        end_tick: -1,
        end_official_tick: -1,
        round_end_reason: RoundEndReason::Unknown,
        ct_team_data: TeamData::new(),
        t_team_data: TeamData::new(),
        trajectories: HashMap::new(),
        flashes: HashMap::new(),
    }));
    static ref GAME_ROUNDS: Mutex<Vec<GameRound>> = Mutex::new(Vec::with_capacity(30));
    static ref ROUND_RESTART_DELAY: Mutex<isize> = Mutex::new(5);
    static ref FREEZE_TIME: Mutex<isize> = Mutex::new(20);
    // static ref TICKRATE: Mutex<isize> = Mutex::new(128);
    static ref STARTING_POSITIONS: Mutex<HashMap<i32, Vector64>> = Mutex::new(HashMap::with_capacity(10));
    static ref FLASHBANG_EVENTS: Arc<Mutex<HashMap<isize, FlashbangDetonate>>> = Arc::new(Mutex::new(HashMap::with_capacity(20)));
    // static ref TEST_POS: Mutex<(f64, f64)> = Mutex::new((0., 0.));
}

use rust_demofile_final::{
    common::Vector64,
    events::{
        BombPlanted, FlashbangDetonate, PlayerFootstep, PlayerSpawn, RoundEnd, RoundStart,
        SmokeGrenadeDetonate, WeaponFire,
    },
    serverclass::PropertyValueEnum,
    serverinfo::{MapInfo, MapInfoObj, MapInfoParsed},
    Parser, ENTITIES, GLOBAL_DISPATCHER, GRENADE_PROJECTILES, INGAME_TICK, PLAYERS_BY_ENTITY_ID,
    PLAYERS_BY_USER_ID, SERVER_CONVARS, THROWN_GRENADES, TICKRATE,
};

async fn calculate_clock_time(tick: isize, current_round: &GameRound) -> String {
    // println!("STARTING CLOCK_TIME");
    let mut round_time = SERVER_CONVARS.lock().await["mp_roundtime"]
        .parse::<f64>()
        .unwrap();

    if tick <= 0 {
        return "00:00".to_string();
    }

    if round_time == 0.0 {
        round_time = SERVER_CONVARS.lock().await["mp_roundtime_defuse"]
            .parse::<f64>()
            .unwrap();
    }

    let seconds_remaining: f64;
    let phase_end_tick: isize;
    if current_round.bomb_plant_tick == -1 {
        phase_end_tick = current_round.freeze_time_end_tick;
        seconds_remaining =
            115. - ((tick as f64 - phase_end_tick as f64) / *TICKRATE.lock().await as f64);
    } else {
        phase_end_tick = current_round.bomb_plant_tick;
        seconds_remaining =
            40. - ((tick as f64 - phase_end_tick as f64) / *TICKRATE.lock().await as f64);
    }

    // println!("seconds_remaining: {:?}",seconds_remaining);
    let minutes = f64::floor(seconds_remaining / 60.) as i64;
    let seconds = f64::ceil(seconds_remaining - (60. * minutes as f64));

    if minutes < 0 || seconds < 0. {
        return "00:00".to_string();
    }
    // println!("ENDING CLOCK_TIME");
    format!("{}:{:0>2}", minutes, seconds)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut p = Parser::new_from_file("complexity-vs-00nation-nuke.dem").await?;
    let s = std::time::Instant::now();

    // p.dispatcher.on("player_death", |bytes: Vec<u8>| {
    //     Box::pin(async move {
    //         let player_death = PlayerDeath::decode(bytes.as_ref()).expect("Failed to decode PlayerDeath.");
    //         println!("{:?}",player_death);
    //     })
    // }).await;

    // GLOBAL_DISPATCHER
    //     .lock()
    //     .await
    //     .on("grenade_projectile_throw", |entity_id: i32| {
    //         Box::pin(async move {
    //             if *MATCH_STARTED.lock().await {
    //                 // println!("PROJECTILE THROWN WITH ID: {:?}", entity_id);
    //                 let proj = &GRENADE_PROJECTILES.lock().await[&entity_id];
    //                 let player_name = match &proj.thrower {
    //                     Some(player) => player.name.to_owned(),
    //                     None => "".to_string(),
    //                 };
    //                 println!(
    //                     "PROJECTILE THROWN: {:?} {:?} {:?}",
    //                     entity_id, proj.weapon_instance.original_string, player_name,
    //                 );
    //             }
    //         })
    //     })
    //     .await;

    p.dispatcher
        .on("bomb_planted", |bytes: Vec<u8>| {
            Box::pin(async move {
                let mut current_round = CURRENT_ROUND.lock().await;
                let _bomb_planted =
                    BombPlanted::decode(bytes.as_ref()).expect("Failed to decode BombPlanted.");

                current_round.bomb_plant_tick = *INGAME_TICK.lock().await;
            })
        })
        .await;

    p.dispatcher
        .on("flashbang_detonate", |bytes: Vec<u8>| {
            Box::pin(async move {
                // println!("FLASHED STARTED");
                let mut current_round = CURRENT_ROUND.lock().await;
                let tick = INGAME_TICK.lock().await;
                let players_by_user_id = PLAYERS_BY_USER_ID.lock().await;
                let players_by_entity_id = PLAYERS_BY_ENTITY_ID.lock().await;
                let entities = ENTITIES.lock().await;
                let map_info: MapInfoParsed = todo!();
                // let mut test_pos = TEST_POS.lock().await;
                // let grenade_projectiles = GRENADE_PROJECTILES.lock().await;
                let flashbang_detonate = FlashbangDetonate::decode(bytes.as_ref())
                    .expect("Failed to decode FlashbangDetonate.");

                // println!("{:?}", flashbang_detonate);
                // println!("TRAJECTORIES: {:?}", current_round.trajectories.len());
                // if let Some(trajectory) =
                //     current_round.trajectories.get(&flashbang_detonate.entityid)
                // {
                //     println!("TRAJECTORIES EXIST");
                //     let flashbang_pos = &trajectory[trajectory.len() - 1];
                //     // println!("{:?}", flashbang_detonate);

                let clock_time = calculate_clock_time(*tick, &current_round).await;
                if let Some(flasher_player) =
                    players_by_user_id.get(&(flashbang_detonate.userid as u32))
                {
                    if let Some(flasher_entity) = entities.get(&flasher_player.entity_id) {
                        // println!("{:?}({:?}) detonated a flashbang at {:?}",flasher_player.name,flasher_entity.team,clock_time);
                        // println!("{:?}",entity.team);

                        // let mut flash_data = FlashData {
                        //     clock_time,

                        // }
                        let mut enemy_flash_total = 0_f64;
                        let mut team_flash_total = 0_f64;
                        let mut enemy_flashes = Vec::new();
                        let mut team_flashes = Vec::new();
                        for (entity_id, player) in players_by_entity_id.iter() {
                            if let Some(entity) = entities.get(entity_id) {
                                let flash_duration_prop_value =
                                    entity._property_value_must("m_flFlashDuration");
                                if let PropertyValueEnum::Float(Cow::Owned(flash_duration)) =
                                    flash_duration_prop_value
                                {
                                    if flash_duration != 0.0 {
                                        let player_position = entity.get_position();
                                        let flashed_player = FlashedPlayer {
                                            name: player.name.to_string(),
                                            location: entity._last_place_name().to_string(),
                                            duration: flash_duration,
                                            team: entity.team,
                                            position: player_position.to_owned(),
                                            view_position: Vector64 {
                                                x: Cow::Owned(entity.view_direction_x()),
                                                y: Cow::Owned(entity.view_direction_y()),
                                                z: Cow::Owned(
                                                    player_position.z.into_owned()
                                                        + entity.eye_position_z(),
                                                ),
                                            },
                                            aim_angle_away_from_flash: -1_f64,
                                            full_blind_estimate: -1_f64,
                                        };

                                        // if let Some(trajectory) = current_round
                                        //     .trajectories
                                        //     .get(&flashbang_detonate.entityid)
                                        // {
                                        //     let flash_pos = &trajectory[trajectory.len() - 1];

                                        //     let player_x =
                                        //         flashed_player.position.x.clone().into_owned();
                                        //     let player_y =
                                        //         flashed_player.position.y.clone().into_owned();
                                        //     let player_z =
                                        //         flashed_player.position.z.clone().into_owned();
                                        //     let player_view_x =
                                        //         flashed_player.position.x.clone().into_owned();
                                        //     let player_view_y =
                                        //         flashed_player.position.y.clone().into_owned();

                                        //     let flash_x = flash_pos.x.clone().into_owned();
                                        //     let flash_y = flash_pos.y.clone().into_owned();
                                        //     let flash_z = flash_pos.z.clone().into_owned();

                                        //     flashed_player.aim_angle_away_from_flash =
                                        //         calculate_cone_angle(
                                        //             (
                                        //                 player_x / map_info.scale,
                                        //                 player_y / map_info.scale,
                                        //                 player_z / map_info.scale,
                                        //                 player_view_x,
                                        //                 player_view_y,
                                        //             ),
                                        //             (
                                        //                 flash_x / map_info.scale,
                                        //                 flash_y / map_info.scale,
                                        //                 flash_z / map_info.scale,
                                        //             ),
                                        //         );

                                        //     println!("IN THIS HERE NOW");
                                        //     println!(
                                        //         "\t{:?} -> {:?} {:?}",
                                        //         flashed_player.name,
                                        //         (player_x, player_y, player_z),
                                        //         (flash_x, flash_y, flash_z)
                                        //     )
                                        // }

                                        // entity.debug_props(None);
                                        // std::process::exit(1);
                                        // println!("\t{:?} -> {:?}\t{:?}",&flashed_player.name, &flashed_player.duration, &flashed_player.location);
                                        if entity.team == 2 || entity.team == 3 {
                                            if entity.team != flasher_entity.team {
                                                enemy_flash_total += flash_duration;
                                                enemy_flashes.push(flashed_player);
                                            } else if entity.team == flasher_entity.team {
                                                team_flash_total += flash_duration;
                                                team_flashes.push(flashed_player);
                                            }
                                        }

                                        // entity.debug_props(
                                        //     None,
                                        //     // Some(PropertyValueEnum::Vector(Cow::Owned(
                                        //     //     Vector64::default(),
                                        //     // ))),
                                        //     Some(PropertyValueEnum::Float(Cow::Owned(0.0))),
                                        //     // None,
                                        // );
                                        // std::process::exit(1);
                                    }
                                }
                                // entity.debug_props(Some("Team"));
                                // std::process::exit(1);
                            }
                        }
                        // println!("HERE");
                        if !team_flashes.is_empty() || !enemy_flashes.is_empty() {
                            let flash_data = FlashData {
                                tick: *tick,
                                clock_time,
                                team_flash_total,
                                enemy_flash_total,
                                team_flashes,
                                enemy_flashes,
                                entity_id: flashbang_detonate.entityid,
                            };
                            if let Some(flashes) =
                                current_round.flashes.get_mut(&flasher_player.name)
                            {
                                flashes.push(flash_data);
                            } else {
                                current_round
                                    .flashes
                                    .insert(flasher_player.name.to_string(), vec![flash_data]);
                            }
                        }
                        // println!("AFTER");
                    }
                }
                // println!();
                // println!("FLASHED ENDING");
                // }
            })
        })
        .await;

    p.dispatcher
        .on(
            "grenade_projectile_destroyed",
            |(entity_id, wep_type, thrower_entity_id, trajectory): (
                i32,
                i32,
                i32,
                Vec<Vector64>,
            )| {
                Box::pin(async move {
                    // println!(
                    //     "{:?} {:?} {:?} {:?}",
                    //     entity_id,
                    //     wep_type,
                    //     thrower_entity_id,
                    //     trajectory.len()
                    // );
                    // println!("IN THIS");
                    CURRENT_ROUND
                        .lock()
                        .await
                        .trajectories
                        .insert(entity_id, trajectory);
                })
            },
        )
        .await;

    GLOBAL_DISPATCHER
        .lock()
        .await
        .on(
            "player_left_buyzone",
            |((entity_id, team), (x_pos, y_pos)): ((i32, i32), (f64, f64))| {
                Box::pin(async move {
                    let mut current_round = CURRENT_ROUND.lock().await;
                    if team == 2 && current_round.t_team_data.best_spawn_entity_id.is_none() {
                        current_round.t_team_data.best_spawn_entity_id = Some(entity_id);
                    } else if team == 3 && current_round.ct_team_data.best_spawn_entity_id.is_none()
                    {
                        current_round.ct_team_data.best_spawn_entity_id = Some(entity_id);
                    }
                    // // println!("{:?} {:?} ({:?}, {:?})", entity_id, team, x_pos, y_pos);
                    // println!("BEFORE");
                    // let mut current_round = CURRENT_ROUND.lock().await;
                    // println!("\tBEFORE");
                    // // // let match_started = MATCH_STARTED.lock().await;
                    // // println!("\tBEFORE");
                    // // // let round_in_freeze_time = ROUND_IN_FREEZE_TIME.lock().await;
                    // // println!("\tBEFORE");
                    // // // let round_in_end_time = ROUND_IN_END_TIME.lock().await;
                    // // println!("\tBEFORE");
                    // let mut entities = ENTITIES.lock().await;
                    // println!("\tBEFORE");
                    // let players_by_entity_id = PLAYERS_BY_ENTITY_ID.lock().await;
                    // println!("\tBEFORE");
                    // // if *match_started && !*round_in_freeze_time && !*round_in_end_time {
                    // if let (Some(entity), Some(player)) = (
                    //     entities.get_mut(&entity_id),
                    //     players_by_entity_id.get(&entity_id),
                    // ) {
                    //     let map_info = MAP_INFO.lock().await;
                    //     let scale = map_info.de_nuke.scale;
                    //     let new_x_pos = (x_pos - map_info.de_nuke.pos_x) / scale;
                    //     let new_y_pos = (y_pos - map_info.de_nuke.pos_y) / scale;

                    //     entity.buyzone_leave_pos.x = Cow::Owned(new_x_pos);
                    //     entity.buyzone_leave_pos.y = Cow::Owned(new_y_pos);

                    //     if team == 2 {
                    //         if current_round.t_team_data.best_spawn_entity_id.is_none() {
                    //             current_round.t_team_data.best_spawn_entity_id = Some(entity.id);
                    //             current_round.t_team_data.best_spawn_name = player.name.to_owned();
                    //         }
                    //     }

                    //     if team == 3 {
                    //         if current_round.ct_team_data.best_spawn_entity_id.is_none() {
                    //             current_round.ct_team_data.best_spawn_entity_id = Some(entity.id);

                    //             current_round.ct_team_data.best_spawn_name = player.name.to_owned();
                    //         }
                    //     }
                    // }
                    // // }
                })
            },
        )
        .await;

    p.dispatcher
        .on(
            "player_left_buyzone",
            |((entity_id, team), (x_pos, y_pos)): ((i32, i32), (f64, f64))| {
                // println!("IN THIS");
                Box::pin(async move {
                    let mut current_round = CURRENT_ROUND.lock().await;
                    if *MATCH_STARTED.lock().await
                        && !*ROUND_IN_FREEZE_TIME.lock().await
                        && !*ROUND_IN_END_TIME.lock().await
                    {
                        if let (Some(entity), Some(player)) = (
                            ENTITIES.lock().await.get_mut(&entity_id),
                            PLAYERS_BY_ENTITY_ID.lock().await.get(&entity_id),
                        ) {
                            let map_info: MapInfoObj = todo!();
                            let scale = map_info.de_nuke.scale;
                            let new_x_pos = (x_pos - map_info.de_nuke.pos_x) / scale;
                            let new_y_pos = (y_pos - map_info.de_nuke.pos_y) / scale;

                            entity.buyzone_leave_pos.x = Cow::Owned(new_x_pos);
                            entity.buyzone_leave_pos.y = Cow::Owned(new_y_pos);

                            if team == 2 {
                                if current_round.t_team_data.best_spawn_entity_id.is_none() {
                                    current_round.t_team_data.best_spawn_entity_id =
                                        Some(entity.id);
                                    current_round.t_team_data.best_spawn_name =
                                        player.name.to_owned();
                                }
                            }

                            if team == 3 {
                                if current_round.ct_team_data.best_spawn_entity_id.is_none() {
                                    current_round.ct_team_data.best_spawn_entity_id =
                                        Some(entity.id);

                                    current_round.ct_team_data.best_spawn_name =
                                        player.name.to_owned();
                                }
                            }
                        }
                    }
                })
            },
        )
        .await;

    // p.dispatcher
    //     .on("player_spawned", |(team_num, user_id): (i32, i32)| {
    //         Box::pin(async move {
    //             let mut current_round = CURRENT_ROUND.lock().await;
    //             let map_info = MAP_INFO.lock().await;

    //             let scale = map_info.de_nuke.scale;
    //             // if let Some(player) = PLAYERS_BY_USER_ID.lock().await.get_mut(&(user_id as u32)) {
    //             //     player.team = team_num as u8;
    //             //     if let Some(entity) = ENTITIES.lock().await.get(&player.entity_id) {
    //             //         if player.team == 2
    //             //             && current_round.t_team_data.best_spawn_entity_id.is_none()
    //             //         {
    //             //             current_round.t_team_data.best_spawn_entity_id = Some(entity.id);
    //             //             current_round.t_team_data.best_spawn_name = player.name.to_owned();
    //             //         } else if player.team == 3
    //             //             && current_round.ct_team_data.best_spawn_entity_id.is_none()
    //             //         {
    //             //             current_round.ct_team_data.best_spawn_entity_id = Some(entity.id);
    //             //             current_round.ct_team_data.best_spawn_name = player.name.to_owned();
    //             //         }
    //             //         let pos = entity.get_position();
    //             //         let x_pos = (pos.x.into_owned()
    //             //             - map_info["de_nuke"].pos_x.parse::<f64>().unwrap())
    //             //             / scale;
    //             //         let y_pos = (pos.y.into_owned()
    //             //             - map_info["de_nuke"].pos_y.parse::<f64>().unwrap())
    //             //             / scale;
    //             //         current_round
    //             //             .spawns
    //             //             .insert(user_id, (x_pos, y_pos, player.name.to_owned()));
    //             //     }
    //             // }
    //             if let Some(entity) = ENTITIES.lock().await.get(&(user_id + 1)) {
    //                 // println!(
    //                 //     "POSITION HISTORY LEN: {:?}",
    //                 //     entity.position_history.len(),
    //                 //     // entity.position_history.get(&current_round.start_tick)
    //                 // );

    //                 // let mut position_history: Vec<isize> = entity
    //                 //     .position_history
    //                 //     .iter()
    //                 //     .map(|(k, v)| k.to_owned())
    //                 //     .collect();

    //                 // position_history.sort();
    //                 // for tick in position_history.iter() {
    //                 //     println!("{:?} {:?}", tick, entity.position_history.get(&tick));
    //                 // }

    //                 // println!(
    //                 //     "{:?} {:?} {:?}",
    //                 //     INGAME_TICK.lock().await,
    //                 //     entity.position_history.get(&current_round.start_tick),
    //                 //     entity
    //                 //         .position_history
    //                 //         .get(&current_round.freeze_time_end_tick)
    //                 // );

    //                 // println!("{:?} {:?} {:?}",current_round.start_tick, )

    //                 // std::process::exit(1);
    //                 // println!("{:?} {:?}", entity.id, user_id + 1);
    //                 // if player.team == 2 && current_round.t_team_data.best_spawn_entity_id.is_none()
    //                 // {
    //                 //     current_round.t_team_data.best_spawn_entity_id = Some(entity.id);
    //                 //     current_round.t_team_data.best_spawn_name = player.name.to_owned();
    //                 // } else if player.team == 3
    //                 //     && current_round.ct_team_data.best_spawn_entity_id.is_none()
    //                 // {
    //                 //     current_round.ct_team_data.best_spawn_entity_id = Some(entity.id);
    //                 //     current_round.ct_team_data.best_spawn_name = player.name.to_owned();
    //                 // }

    //                 if !entity
    //                     .position_history
    //                     .contains_key(&current_round.freeze_time_end_tick)
    //                 {
    //                     println!(
    //                         "{:?} | {:?} {:?}",
    //                         entity.position_history.len(),
    //                         entity
    //                             .position_history
    //                             .get(&(current_round.freeze_time_end_tick - 1)),
    //                         entity
    //                             .position_history
    //                             .get(&(current_round.freeze_time_end_tick + 1))
    //                     );
    //                 }
    //                 let pos = entity.position_history[&current_round.freeze_time_end_tick].clone();
    //                 // let pos = entity.get_position();
    //                 let x_pos = (pos.x.into_owned() - map_info.de_nuke.pos_x) / scale;
    //                 let y_pos = (pos.y.into_owned() - map_info.de_nuke.pos_y) / scale;
    //                 current_round.spawns.insert(user_id as u32, (x_pos, y_pos));
    //             }
    //         })
    //     })
    //     .await;

    // p.dispatcher
    //     .on("player_spawn", |bytes: Vec<u8>| {
    //         Box::pin(async move {
    //             // if *MATCH_STARTED.lock().await
    //             //     || *ROUND_STARTED.lock().await
    //             //     || *ROUND_IN_FREEZE_TIME.lock().await
    //             // {
    //             // tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    //             // std::thread::sleep(Duration::from_millis(100));
    //             let mut current_round = CURRENT_ROUND.lock().await;
    //             let map_info = MAP_INFO.lock().await;
    //             let player_spawn =
    //                 PlayerSpawn::decode(bytes.as_ref()).expect("Failed to decode PlayerSpawn.");
    //             // println!("{:?}", current_round);
    //             // println!("{:?}", player_spawn);

    //             let scale = map_info["de_nuke"].scale.parse::<f64>().unwrap();
    //             if let Some(player) = PLAYERS_BY_USER_ID
    //                 .lock()
    //                 .await
    //                 .get_mut(&(player_spawn.userid as u32))
    //             {
    //                 // println!("\tIN PLAYERS_BY_ENTITY_ID");
    //                 player.team = player_spawn.teamnum as u8;
    //                 if let Some(entity) = ENTITIES.lock().await.get(&player.entity_id) {
    //                     // println!("\t\tIN ENTITIES");
    //                     // let pos_val = entity._property_value_must("cslocaldata.m_vecOrigin");
    //                     // if let PropertyValueEnum::Vector(Cow::Owned(pos)) = pos_val {
    //                     let pos = entity.get_position();
    //                     let x_pos = (pos.x.into_owned()
    //                         - map_info["de_nuke"].pos_x.parse::<f64>().unwrap())
    //                         / scale;
    //                     let y_pos = (pos.y.into_owned()
    //                         - map_info["de_nuke"].pos_y.parse::<f64>().unwrap())
    //                         / scale;
    //                     current_round.spawns.insert(
    //                         player_spawn.userid,
    //                         (
    //                             x_pos,
    //                             y_pos,
    //                             // player.name.to_owned(),
    //                             // player.steam_id,
    //                             // entity._last_place_name().to_string(),
    //                             // entity.buyzone_leave_pos.to_owned(),
    //                         ),
    //                     );
    //                     // }
    //                 } else {
    //                     println!("ENTITY NOT FOUND AT ALL");
    //                 }
    //             } else {
    //                 println!("PLAYER NOT FOUND AT ALL");
    //             }
    //             // }
    //         })
    //     })
    //     .await;

    // p.dispatcher
    //     .on("position_history_update", |tick| {
    //         Box::pin(async move {
    //             for (_, player) in PLAYERS_BY_ENTITY_ID.lock().await.iter() {
    //                 if let Some(entity) = ENTITIES.lock().await.get_mut(&player.entity_id) {
    //                     entity.position_history.insert(tick, entity.get_position());
    //                 }
    //             }
    //         })
    //     })
    //     .await;

    p.dispatcher
        .on("round_start", |bytes: Vec<u8>| {
            Box::pin(async move {
                let mut round_started = ROUND_STARTED.lock().await;
                let mut current_round = CURRENT_ROUND.lock().await;
                let match_started = MATCH_STARTED.lock().await;

                if *round_started
                    && current_round.round_end_reason != RoundEndReason::Draw
                    && *match_started
                {
                    // println!("IN THIS THING");
                    println!("BUIDLING SPAWNS: round_start");
                    build_spawns(&mut current_round).await;
                    // println!(
                    //     "{:?} {:?}",
                    //     current_round.round_num, current_round.start_tick
                    // );
                    // println!("BEFORE PUSHING CURRENT ROUND");
                    GAME_ROUNDS.lock().await.push(current_round.to_owned());
                    // println!("HERE");
                    // std::process::exit(1);
                }

                // if *round_started {
                //     println!(
                //         "{:?} {:?} {:?}",
                //         round_started, current_round.round_end_reason, match_started,
                //     );
                //     std::process::exit(1);
                // }

                let _round_start =
                    RoundStart::decode(bytes.as_ref()).expect("Failed to decode RoundStart.");
                *round_started = true;
                *ROUND_IN_FREEZE_TIME.lock().await = true;
                *ROUND_IN_END_TIME.lock().await = false;
                *current_round = GameRound::new();
                // // println!("AFTER ROUND STUFF");
                // let players_by_entity_id = PLAYERS_BY_ENTITY_ID.lock().await;
                // let PLAYER_INFO_BY_USER_ID = PLAYER_INFO_BY_USER_ID.lock().await;
                // let entities = ENTITIES.lock().await;
                // let map_info = MAP_INFO.lock().await;
                let game_rounds = GAME_ROUNDS.lock().await;
                let ingame_tick = INGAME_TICK.lock().await;

                // let mut current_round = CURRENT_ROUND.lock();
                current_round.round_num = game_rounds.len() as u8 + 1;
                current_round.start_tick = *ingame_tick;

                // println!("\tROUND STARTING");

                // std::process::exit(1);
            })
        })
        .await;

    // p.dispatcher.on("player_death", |bytes: Vec<u8>| {
    //     let player_death =
    //         PlayerDeath::decode(bytes.as_ref()).expect("Failed to decode PlayerDeath.");
    //     // println!("{:?} {:?}", player_death, INGAME_TICK.lock());
    //     // std::process::exit(1);
    // });

    // p.dispatcher.on("player_hurt", |bytes: Vec<u8>| {
    //     let player_hurt = PlayerHurt::decode(bytes.as_ref()).expect("Failed to decode PlayerHurt.");
    //     // println!("{:?} {:?}",player_hurt, INGAME_TICK.lock());
    //     // std::process::exit(1);
    // });

    // // p.dispatcher.on("round_end", |bytes: Vec<u8>| {
    // //     let round_end = RoundEnd::decode(bytes.as_ref()).expect("Failed to decode RoundEnd.");
    // //     // println!("{:?} {:?}", round_end, INGAME_TICK.lock());
    // //     // std::process::exit(1);
    // // });

    p.dispatcher
        .on("weapon_fire", |bytes: Vec<u8>| {
            Box::pin(async move {
                let weapon_fire =
                    WeaponFire::decode(bytes.as_ref()).expect("Failed to decode WeaponFire.");
                // println!("{:?}", weapon_fire);
                // std::process::exit(1);
            })
        })
        .await;

    p.dispatcher
        .on("round_announce_match_start", |_tick: isize| {
            Box::pin(async move {
                *MATCH_STARTED.lock().await = true;

                // for (_, player) in PLAYERS_BY_USER_ID.lock().await.iter_mut() {
                //     if let Some(entity) = ENTITIES.lock().await.get(&player.entity_id) {
                //         let team = entity._property_value_must("m_iTeamNum").as_integer();
                //         player.team = team as u8;
                //     }
                // }
            })
        })
        .await;

    // p.dispatcher.on("round_start", |bytes: Vec<u8>| {
    //     let round_start = RoundStart::decode(bytes.as_ref()).expect("Failed to decode RoundStart.");
    //     *ROUND_STARTED.lock() = true;
    //     *ROUND_IN_FREEZE_TIME.lock() = true;
    //     *ROUND_IN_END_TIME.lock() = false;

    //     let mut current_round = CURRENT_ROUND.lock();
    //     let players_by_entity_id = PLAYERS_BY_ENTITY_ID.lock();
    //     let PLAYER_INFO_BY_USER_ID = PLAYER_INFO_BY_USER_ID.lock();

    //     let filename = format!("test_round_{}.png",current_round.round_num);
    //     let scale = MAP_INFO.lock()["de_nuke"].scale.parse::<f64>().unwrap();
    //     let root = BitMapBackend::new(&filename, (1024, 1024)).into_drawing_area();
    //     // root.fill(&TRANSPARENT).unwrap();
    //     root.fill(&WHITE).unwrap();
    //     let mut chart = ChartBuilder::on(&root).build_cartesian_2d(0.0..1024.0, -1024.0..0.0).unwrap();
    //     chart.configure_mesh().disable_mesh().draw().unwrap();

    //     let image = image::load(
    //         BufReader::new(
    //             File::open("map.png").map_err(|e| {
    //                 eprintln!("Unable to open file plotters-doc-data.png, please make sure you have clone this repo with --recursive");
    //                 e
    //             }).unwrap()),
    //         ImageFormat::Png,
    //     ).unwrap();
    //     // let scale = MAP_INFO.lock()["de_nuke"].scale.parse::<f64>().unwrap();
    //     // let x = (-1947.010009765625 - MAP_INFO.lock()["de_nuke"].pos_x.parse::<f64>().unwrap()) / scale;
    //     // let y = (-1102.1099853515625 - MAP_INFO.lock()["de_nuke"].pos_y.parse::<f64>().unwrap()) / scale;
    //     // println!("AFTER");
    //     // println!("{:?} {:?}", x, y);
    //     let elem = BitMapElement::from(((0.0, 0.0).into(), image));
    //     chart.draw_series(std::iter::once(elem)).unwrap();

    //     // for (entity_id, pos) in spawns.iter() {
    //     for (entity_id, player) in PLAYER_INFO_BY_USER_ID.iter() {
    //         let mut entity = ENTITIES.lock()[&player.entity_id].clone();
    //         let player_pos = entity.get_position();
    //         let x_pos = (player_pos.x.clone().into_owned() - MAP_INFO.lock()["de_nuke"].pos_x.parse::<f64>().unwrap()) / scale;
    //         let y_pos = (player_pos.y.clone().into_owned() - MAP_INFO.lock()["de_nuke"].pos_y.parse::<f64>().unwrap()) / scale;
    //         // let player = PLAYER_INFO_BY_USER_ID.lock()[&entity_id].clone();
    //         // let x_pos = (pos.0 - MAP_INFO.lock()["de_nuke"].pos_x.parse::<f64>().unwrap()) / scale;
    //         // let y_pos = (pos.1 - MAP_INFO.lock()["de_nuke"].pos_y.parse::<f64>().unwrap()) / scale;

    //         let player_ent = players_by_entity_id.get(entity_id);
    //         let team = match player_ent {
    //             Some(player_ent) => Some(player_ent.team_state.team),
    //             None => None,
    //         };

    //         if !player.is_fake_player && !player.is_hltv {
    //             println!("{:?} {:?} {:?} {:?} {:?}",team, player.is_fake_player, player.is_hltv, player.name, x_pos);
    //             chart.draw_series(PointSeries::of_element(
    //                 vec![(x_pos, y_pos)],
    //                 3,
    //                 &RED,
    //                 &|c, s, st| {
    //                     return EmptyElement::at(c)    // We want to construct a composed element on-the-fly
    //                     + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
    //                     + Text::new(format!("{:?}", player.name), (10, 0), TextStyle { color: WHITE.to_backend_color(), font: ("sans-serif", 10).into(), pos: Pos::new(HPos::Right, VPos::Top)});
    //                 },
    //             )).unwrap();
    //         }
    //     }
    //     println!();

    //     chart.draw_series(PointSeries::of_element(vec![(1024.0 * 0.82, -1024.0 * 0.45)], 5, &GREEN,                         &|c, s, st| {
    //                     return EmptyElement::at(c)    // We want to construct a composed element on-the-fly
    //                     + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
    //                     + Text::new(format!("{:?}", "CT_SPAWN"), (10, 0), TextStyle { color: WHITE.to_backend_color(), font: ("sans-serif", 10).into(), pos: Pos::new(HPos::Right, VPos::Top)});
    //                 },)).unwrap();

    //     /*
    //         false false "Grim" Vector64 { x: -1947.010009765625, y: -1102.1099853515625, z: -416.633056640625 }
    //         false false "coldzera" Vector64 { x: 2664.0, y: -496.0, z: -351.9689025878906 }
    //         false false "JT" Vector64 { x: -1878.0, y: -980.0, z: -417.25439453125 }
    //         false false "v$m" Vector64 { x: 2552.0, y: -424.0, z: -354.6258544921875 }
    //         true true "PGL Major - A Stream" Vector64 { x: -901.0, y: -2165.0, z: -55.0 }
    //         false false "leo_drk" Vector64 { x: 2704.0, y: -416.0, z: -354.9142150878906 }
    //         false false "FaNg" Vector64 { x: -1832.0, y: -1160.0, z: -415.96875 }
    //         false false "malbsMd" Vector64 { x: 2512.0, y: -504.0, z: -352.314453125 }
    //         false false "junior" Vector64 { x: -1947.010009765625, y: -965.1099853515625, z: -417.2048034667969 }
    //         false false "floppy" Vector64 { x: -1808.0, y: -1025.0, z: -416.13800048828125 }
    //         true true "(1)PGL Major - A Stream" Vector64 { x: 436.0, y: -121.0, z: -501.0 }
    //         false false "PGL Observer 1" Vector64 { x: -1878.0, y: -980.0, z: -417.25439453125 }
    //         false false "Mod" Vector64 { x: -1806.9384765625, y: -1026.35693359375, z: -416.1216735839844 }
    //         false false "7ry" Vector64 { x: 2484.312744140625, y: -499.2204284667969, z: -352.40130615234375 }
    //     */
    //     root.present().expect("THIS HERE");

    //     // let mut current_round = CURRENT_ROUND.lock();
    //     current_round.round_num = GAME_ROUNDS.lock().len() as u8 + 1;
    //     current_round.start_tick = *INGAME_TICK.lock();

    //     // println!("\tROUND STARTING");

    //     // std::process::exit(1);
    // });

    // p.dispatcher.on("spawns__", |spawns: Vec<(String, (f64, f64))>| {
    //     Box::pin(async move {
    //         let current_round = CURRENT_ROUND.lock().await;
    //         // let players_by_entity_id = PLAYERS_BY_ENTITY_ID.lock().await;
    //         let PLAYER_INFO_BY_USER_ID = PLAYER_INFO_BY_USER_ID.lock().await;
    //         let map_info = MAP_INFO.lock().await;
    //         let mut entities = ENTITIES.lock().await;

    //         let filename = format!("test_round_{}.png",current_round.round_num);
    //         let scale = map_info["de_nuke"].scale.parse::<f64>().unwrap();
    //         let root = BitMapBackend::new(&filename, (1024, 1024)).into_drawing_area();
    //         // root.fill(&TRANSPARENT).unwrap();
    //         root.fill(&WHITE).unwrap();
    //         let mut chart = ChartBuilder::on(&root).build_cartesian_2d(0.0..1024.0, -1024.0..0.0).unwrap();
    //         chart.configure_mesh().disable_mesh().draw().unwrap();

    //         let image = image::load(
    //             BufReader::new(
    //                 File::open("map.png").map_err(|e| {
    //                     eprintln!("Unable to open file plotters-doc-data.png, please make sure you have clone this repo with --recursive");
    //                     e
    //                 }).unwrap()),
    //             ImageFormat::Png,
    //         ).unwrap();
    //         // let scale = MAP_INFO.lock()["de_nuke"].scale.parse::<f64>().unwrap();
    //         // let x = (-1947.010009765625 - MAP_INFO.lock()["de_nuke"].pos_x.parse::<f64>().unwrap()) / scale;
    //         // let y = (-1102.1099853515625 - MAP_INFO.lock()["de_nuke"].pos_y.parse::<f64>().unwrap()) / scale;
    //         // println!("AFTER");
    //         // println!("{:?} {:?}", x, y);
    //         let elem = BitMapElement::from(((0.0, 0.0).into(), image));
    //         chart.draw_series(std::iter::once(elem)).unwrap();

    //         println!("SPAWNS: {:?}", spawns.len());
    //         // for (_entity_id, pos) in spawns.iter() {
    //         for (player_name, pos) in spawns.iter() {
    //             // match (PLAYER_INFO_BY_USER_ID.get(&entity_id), entities.get_mut(&entity_id)) {
    //             //     (Some(player), Some(entity)) => {
    //             //         let pos = entity.get_position();
    //             //         let x_pos = (pos.x.clone().into_owned() - map_info["de_nuke"].pos_x.parse::<f64>().unwrap()) / scale;
    //             //         let y_pos = (pos.y.clone().into_owned() - map_info["de_nuke"].pos_y.parse::<f64>().unwrap()) / scale;
    //             //         if !player.is_fake_player && !player.is_hltv {
    //             //             println!("{:?} {:?} {:.2} {:.2} {:?}",player.is_fake_player, player.is_hltv, x_pos, y_pos, player.name);
    //             //             chart.draw_series(PointSeries::of_element(
    //             //                 vec![(x_pos, y_pos)],
    //             //                 3,
    //             //                 &RED,
    //             //                 &|c, s, st| {
    //             //                     return EmptyElement::at(c)    // We want to construct a composed element on-the-fly
    //             //                     + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
    //             //                     + Text::new(format!("{:?}", player.name), (10, 0), TextStyle { color: WHITE.to_backend_color(), font: ("sans-serif", 10).into(), pos: Pos::new(HPos::Right, VPos::Top)});
    //             //                 },
    //             //             )).unwrap();
    //             //         }
    //             //     },
    //             //     _ => {
    //             //         println!("PLAYER NOT FOUND FOR ENTITY_ID: {entity_id}");
    //             //     }
    //             // }

    //             let x_pos = (pos.0 - map_info["de_nuke"].pos_x.parse::<f64>().unwrap()) / scale;
    //             let y_pos = (pos.1 - map_info["de_nuke"].pos_y.parse::<f64>().unwrap()) / scale;
    //             println!("{:?} {:.2} {:.2}",player_name,x_pos,y_pos);
    //             chart.draw_series(PointSeries::of_element(
    //                 vec![(x_pos, y_pos)],
    //                 3,
    //                 &RED,
    //                 &|c, s, st| {
    //                     return EmptyElement::at(c)    // We want to construct a composed element on-the-fly
    //                     + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
    //                     + Text::new(format!("{:?}", player_name), (10, 0), TextStyle { color: WHITE.to_backend_color(), font: ("sans-serif", 10).into(), pos: Pos::new(HPos::Right, VPos::Top)});
    //                 },
    //             )).unwrap();
    //             /*

    //                 // WRONG
    //                 false false 684.80 -469.86 "v$m"
    //                 false false 235.00 -558.86 "Mod"
    //                 false false 215.14 -569.87 "junior"
    //                 false false 231.57 -578.14 "Grim"
    //                 false false 879.57 -471.86 "coldzera"
    //                 false false 857.86 -473.00 "leo_drk"
    //                 false false 739.67 -634.78 "7ry"
    //                 false false 873.86 -483.29 "malbsMd"
    //                 PLAYER NOT FOUND FOR ENTITY_ID: 1
    //                 false false 225.00 -552.43 "FaNg"
    //                 false false 852.14 -484.43 "JT"
    //                 false false 215.14 -550.30 "floppy"

    //                 false false 580.97 -612.98 "junior"
    //                 false false 684.80 -469.86 "v$m"
    //                 false false 684.80 -469.86 "malbsMd"
    //                 false false 580.97 -612.98 "floppy"
    //                 PLAYER NOT FOUND FOR ENTITY_ID: 1
    //                 false false 723.51 -630.70 "Mod"
    //                 false false 684.80 -469.86 "leo_drk"
    //                 false false 739.67 -634.78 "7ry"
    //                 false false 684.80 -469.86 "coldzera"
    //                 false false 684.80 -469.86 "JT"
    //                 false false 739.67 -634.78 "FaNg"
    //                 false false 657.09 -452.73 "Grim"

    //             */
    //         }
    //         println!();

    //         chart.draw_series(PointSeries::of_element(vec![(1024.0 * 0.82, -1024.0 * 0.45)], 5, &GREEN,                         &|c, s, st| {
    //                         return EmptyElement::at(c)    // We want to construct a composed element on-the-fly
    //                         + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
    //                         + Text::new(format!("{:?}", "CT_SPAWN"), (10, 0), TextStyle { color: WHITE.to_backend_color(), font: ("sans-serif", 10).into(), pos: Pos::new(HPos::Right, VPos::Top)});
    //                     },)).unwrap();

    //         /*
    //             false false "Grim" Vector64 { x: -1947.010009765625, y: -1102.1099853515625, z: -416.633056640625 }
    //             false false "coldzera" Vector64 { x: 2664.0, y: -496.0, z: -351.9689025878906 }
    //             false false "JT" Vector64 { x: -1878.0, y: -980.0, z: -417.25439453125 }
    //             false false "v$m" Vector64 { x: 2552.0, y: -424.0, z: -354.6258544921875 }
    //             true true "PGL Major - A Stream" Vector64 { x: -901.0, y: -2165.0, z: -55.0 }
    //             false false "leo_drk" Vector64 { x: 2704.0, y: -416.0, z: -354.9142150878906 }
    //             false false "FaNg" Vector64 { x: -1832.0, y: -1160.0, z: -415.96875 }
    //             false false "malbsMd" Vector64 { x: 2512.0, y: -504.0, z: -352.314453125 }
    //             false false "junior" Vector64 { x: -1947.010009765625, y: -965.1099853515625, z: -417.2048034667969 }
    //             false false "floppy" Vector64 { x: -1808.0, y: -1025.0, z: -416.13800048828125 }
    //             true true "(1)PGL Major - A Stream" Vector64 { x: 436.0, y: -121.0, z: -501.0 }
    //             false false "PGL Observer 1" Vector64 { x: -1878.0, y: -980.0, z: -417.25439453125 }
    //             false false "Mod" Vector64 { x: -1806.9384765625, y: -1026.35693359375, z: -416.1216735839844 }
    //             false false "7ry" Vector64 { x: 2484.312744140625, y: -499.2204284667969, z: -352.40130615234375 }
    //         */
    //         root.present().expect("THIS HERE");
    //         std::process::exit(1);
    //     })
    // }).await;

    p.dispatcher
        .on("round_freeze_end", |_tick: isize| {
            Box::pin(async move {
                // let mut game_rounds = GAME_ROUNDS.lock().await;
                let mut current_round = CURRENT_ROUND.lock().await;
                let ingame_tick = INGAME_TICK.lock().await;
                let tickrate = TICKRATE.lock().await;
                // println!("\t\tFREEZE TIME ENDING");

                // let mut conv_parsed = CONV_PARSED.lock();
                // if !*conv_parsed {

                // }
                // if let Some(con_vars) = &*SERVER_CONVARS.lock() {
                //     *ROUND_RESTART_DELAY.lock() = con_vars.round_restart_delay as isize;
                // }
                // if let Some(con_vars) = SERVER_CONVARS.lock() {}
                // if let Some(round_restart_delay) = SERVER_CONVARS.lock().get("mp_round_restart_delay") {

                // }
                match SERVER_CONVARS.lock().await.get("mp_round_restart_delay") {
                    Some(round_restart_delay_string) if round_restart_delay_string != "0" => {
                        *ROUND_RESTART_DELAY.lock().await =
                            round_restart_delay_string.parse::<isize>().unwrap();
                    }
                    _ => {
                        *ROUND_RESTART_DELAY.lock().await = 5;
                    }
                }

                if let Some(freezetime) = SERVER_CONVARS.lock().await.get("mp_freezetime") {
                    *FREEZE_TIME.lock().await = freezetime.parse::<isize>().unwrap();
                }

                if !*ROUND_IN_FREEZE_TIME.lock().await {
                    if current_round.round_end_reason != RoundEndReason::Draw
                        && *MATCH_STARTED.lock().await
                    {
                        println!("BUIDLING SPAWNS: round_freeze_end");
                        build_spawns(&mut current_round).await;
                        // println!(
                        //     "{:?} {:?}",
                        //     current_round.round_num, current_round.start_tick
                        // );
                        println!("AFTER");
                        // game_rounds.push(current_round.to_owned());
                        GAME_ROUNDS.lock().await.push(current_round.to_owned());
                    }

                    *ROUND_STARTED.lock().await = true;
                    *ROUND_IN_END_TIME.lock().await = false;
                    *current_round = GameRound::new();

                    current_round.round_num = GAME_ROUNDS.lock().await.len() as u8 + 1;
                    current_round.start_tick =
                        *ingame_tick - (*tickrate as isize) * *FREEZE_TIME.lock().await;
                    current_round.freeze_time_end_tick = *ingame_tick;
                }

                *ROUND_IN_FREEZE_TIME.lock().await = false;
                current_round.freeze_time_end_tick = *ingame_tick;
            })
        })
        .await;

    p.dispatcher
        .on("round_end", |bytes: Vec<u8>| {
            Box::pin(async move {
                // println!("\t\tROUND ENDING");
                let e = RoundEnd::decode(bytes.as_ref()).expect("Failed to decode RoundEnd.");
                let mut round_started = ROUND_STARTED.lock().await;

                let mut current_round = CURRENT_ROUND.lock().await;
                if *round_started {
                    // TODO:
                    /*
                        if (gs.TeamTerrorists() != nil) && (gs.TeamCounterTerrorists() != nil) {
                            tTeam := gs.TeamTerrorists().ClanName()
                            ctTeam := gs.TeamCounterTerrorists().ClanName()
                            currentRound.TTeam = &tTeam
                            currentRound.CTTeam = &ctTeam
                        }
                    */
                }

                if !*round_started {
                    *round_started = true;

                    current_round.round_num = 0;
                    current_round.start_tick = 0;
                }

                current_round.end_tick = *INGAME_TICK.lock().await;
                current_round.end_official_tick = *INGAME_TICK.lock().await
                    + (*ROUND_RESTART_DELAY.lock().await + *TICKRATE.lock().await as isize);
                current_round.round_end_reason = e.reason.into();
            })
        })
        .await;

    p.dispatcher
        .on("round_officially_ended", |_tick: isize| {
            Box::pin(async move {
                let mut current_round = CURRENT_ROUND.lock().await;
                // println!("\tROUND OFFICIALLY ENDING");
                if !*ROUND_IN_END_TIME.lock().await {
                    current_round.end_tick = *INGAME_TICK.lock().await
                        - (*ROUND_RESTART_DELAY.lock().await * *TICKRATE.lock().await as isize);
                    current_round.end_official_tick = *INGAME_TICK.lock().await;
                } else {
                    current_round.end_tick = *INGAME_TICK.lock().await
                        - (*ROUND_RESTART_DELAY.lock().await * *TICKRATE.lock().await as isize);
                    current_round.end_official_tick = *INGAME_TICK.lock().await;
                }
                // // // std::process::exit(1);
                // if current_round.round_end_reason != RoundEndReason::Draw
                //     && *MATCH_STARTED.lock().await
                // {
                //     println!(
                //         "{:?} {:?}",
                //         current_round.round_num, current_round.start_tick
                //     );
                //     // dbg!(&current_round.spawns);
                //     // println!("WORKING");
                //     GAME_ROUNDS.lock().await.push(current_round.to_owned());
                //     std::process::exit(1);
                // }
            })
        })
        .await;

    p.dispatcher
        .on("smokegrenade_detonate", |bytes: Vec<u8>| {
            Box::pin(async move {
                let smoke_grenade_detonate = SmokeGrenadeDetonate::decode(bytes.as_ref())
                    .expect("Failed to decode SmokeGrenadeDetonate.");
                // println!("{:?}", smoke_grenade_detonate);
            })
        })
        .await;

    p.parse_to_end().await;

    println!("AGG: {:?}", p.agg as f64 / 1000000000.0);
    println!(
        "ELAPSED: {:?}",
        s.elapsed().as_nanos() as f64 / 1000000000.0
    );

    Ok(())
}

#[derive(Debug, Clone)]
pub struct GameRound {
    round_num: u8,
    start_tick: isize,
    freeze_time_end_tick: isize,
    bomb_plant_tick: isize,
    end_tick: isize,
    end_official_tick: isize,
    round_end_reason: RoundEndReason,
    // spawns: HashMap<u32, (f64, f64)>,
    ct_team_data: TeamData,
    t_team_data: TeamData,
    trajectories: HashMap<i32, Vec<Vector64>>,
    flashes: HashMap<String, Vec<FlashData>>,
}

impl GameRound {
    fn new() -> Self {
        GameRound {
            round_num: 0,
            start_tick: -1,
            freeze_time_end_tick: -1,
            bomb_plant_tick: -1,
            end_tick: -1,
            end_official_tick: -1,
            round_end_reason: RoundEndReason::Unknown,
            ct_team_data: TeamData::new(),
            t_team_data: TeamData::new(),
            trajectories: HashMap::new(),
            flashes: HashMap::new(),
        }
    }
}

unsafe impl Send for GameRound {}
unsafe impl Sync for GameRound {}

#[derive(Debug, Clone)]
pub struct TeamData {
    pub best_spawn_entity_id: Option<i32>,
    pub best_spawn_name: String,
    // pub best_spawn_entity_id: i32,
}

impl TeamData {
    fn new() -> Self {
        TeamData {
            best_spawn_entity_id: None,
            best_spawn_name: "".to_string(),
            // best_spawn_entity_id: -1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FlashData {
    pub tick: isize,
    pub clock_time: String,
    pub team_flash_total: f64,
    pub enemy_flash_total: f64,
    pub team_flashes: Vec<FlashedPlayer>,
    pub enemy_flashes: Vec<FlashedPlayer>,
    // pub position: Vector64,
    pub entity_id: i32,
}

#[derive(Debug, Clone)]
pub struct FlashedPlayer {
    pub name: String,
    pub location: String,
    pub duration: f64,
    pub team: u8,
    pub position: Vector64,
    pub view_position: Vector64,
    pub aim_angle_away_from_flash: f64,
    pub full_blind_estimate: f64,
}

impl FlashedPlayer {
    fn get_aim_angle_away_from_flash(
        &self,
        (flash_x, flash_y, flash_z): (f64, f64, f64),
        map_x: f64,
        map_y: f64,
        map_scale: f64,
    ) -> f64 {
        let player_pos = &self.position;
        let player_pos_x = player_pos.x.clone().into_owned();
        let player_pos_y = player_pos.y.clone().into_owned();
        let player_pos_z = player_pos.z.clone().into_owned();
        let x = (player_pos_x - map_x) / map_scale;
        let y = (player_pos_y - map_y) / map_scale;
        let z = player_pos_z / map_scale;

        calculate_cone_angle(
            (
                x,
                y,
                z,
                self.view_position.x.clone().into_owned(),
                self.view_position.y.clone().into_owned(),
            ),
            (flash_x, flash_y, flash_z),
        )
    }

    fn set_aim_angle_away_from_flash(
        &mut self,
        (flash_x, flash_y, flash_z): (f64, f64, f64),
        map_x: f64,
        map_y: f64,
        map_scale: f64,
    ) {
        self.aim_angle_away_from_flash = self.get_aim_angle_away_from_flash(
            (flash_x, flash_y, flash_z),
            map_x,
            map_y,
            map_scale,
        );
    }
}

unsafe impl Send for TeamData {}
unsafe impl Sync for TeamData {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoundEndReason {
    Unknown = 0,
    TargetBombed = 1,
    VIPEscaped = 2,
    VIPKilled = 3,
    TerroristsEscaped = 4,
    CTStoppedEsca = 5,
    TerroristsStopped = 6,
    BombDefused = 7,
    CTWin = 8,
    TerroristsWin = 9,
    Draw = 10,
    HostagesRescu = 11,
    TargetSav = 12,
    HostagesNotRescue = 13,
    TerroristsNotEscaped = 14,
    VIPNotEscaped = 15,
    GameStart = 16,
    TerroristsSurrend = 17,
    CTSurrend = 18,
}

impl From<i32> for RoundEndReason {
    fn from(value: i32) -> Self {
        match value {
            1 => RoundEndReason::TargetBombed,
            2 => RoundEndReason::VIPEscaped,
            3 => RoundEndReason::VIPKilled,
            4 => RoundEndReason::TerroristsEscaped,
            5 => RoundEndReason::CTStoppedEsca,
            6 => RoundEndReason::TerroristsStopped,
            7 => RoundEndReason::BombDefused,
            8 => RoundEndReason::CTWin,
            9 => RoundEndReason::TerroristsWin,
            10 => RoundEndReason::Draw,
            11 => RoundEndReason::HostagesRescu,
            12 => RoundEndReason::TargetSav,
            13 => RoundEndReason::HostagesNotRescue,
            14 => RoundEndReason::TerroristsNotEscaped,
            15 => RoundEndReason::VIPNotEscaped,
            16 => RoundEndReason::GameStart,
            17 => RoundEndReason::TerroristsSurrend,
            18 => RoundEndReason::CTSurrend,
            _ => RoundEndReason::Unknown,
        }
    }
}

// async fn build_spawns(current_round: &GameRound) {
//     // panic!("BUILDING SPAWNS");
//     // println!("BUILDING SPAWNS {:?}", round.spawns);
//     // let current_round = CURRENT_ROUND.lock().await;
//     // let current_round = round;
//     // println!("BEFORE FILENAME");
//     let players_by_user_id = PLAYERS_BY_USER_ID.lock().await;
//     let map_info = &MAP_INFO.lock().await["de_nuke"];
//     // dbg!(&map_info);
//     let filename = format!("test_round_{}.png", current_round.round_num);
//     // println!("BEFORE ROOT");
//     let root = BitMapBackend::new(&filename, (1024, 1024)).into_drawing_area();
//     // root.fill(&TRANSPARENT).unwrap();
//     // println!("AFTER ROOT");
//     root.fill(&WHITE).unwrap();
//     let mut chart = ChartBuilder::on(&root)
//         .build_cartesian_2d(0.0..1024.0, -1024.0..0.0)
//         .unwrap();
//     chart.configure_mesh().disable_mesh().draw().unwrap();
//     // println!("BEFORE IMAGE");
//     let image = image::load(
//         BufReader::new(
//             File::open("map.png").map_err(|e| {
//                 eprintln!("Unable to open file plotters-doc-data.png, please make sure you have clone this repo with --recursive");
//                 e
//             }).unwrap()),
//         ImageFormat::Png,
//     ).unwrap();
//     // println!("BEFORE ELEM");
//     let elem = BitMapElement::from(((0.0, 0.0).into(), image));
//     chart.draw_series(std::iter::once(elem)).unwrap();
//     // println!("BEFORE SPAWNS LOOP");
//     println!(
//         "SPAWNS FOR ROUND {:?} at tick {:?}",
//         current_round.round_num, current_round.start_tick
//     );
//     for (
//         user_id,
//         (
//             x_pos,
//             y_pos,
//             // player_name,
//             // steam_id,
//             // last_place_name
//         ),
//     ) in current_round.spawns.iter()
//     {
//         // println!("{:?}", player_name);
//         // println!(
//         //     "{:.2} {:.2} {:?} {:?} {:?}",
//         //     x_pos, y_pos, player_name, steam_id, last_place_name
//         // );
//         if let Some(player) = players_by_user_id.get(user_id) {
//             chart.draw_series(PointSeries::of_element(
//                 vec![(x_pos.to_owned(), y_pos.to_owned())],
//                 3,
//                 &RED,
//                 &|c, s, st| {
//                     return EmptyElement::at(c)    // We want to construct a composed element on-the-fly
//                     + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
//                     + Text::new(format!("{:?}", player.name), (10, 0), TextStyle { color: WHITE.to_backend_color(), font: ("sans-serif", 10).into(), pos: Pos::new(HPos::Right, VPos::Top)});
//                 },
//             )).unwrap();
//         }

//         // chart.draw_series(PointSeries::of_element(iter, size, style, cons))
//         // println!("{:?} {:?}", player_name, buyzone_leave_pos);

//         // chart
//         //     .draw_series(PointSeries::of_element(
//         //         vec![(
//         //             buyzone_leave_pos.x.clone().into_owned(),
//         //             buyzone_leave_pos.y.clone().into_owned(),
//         //         )],
//         //         3,
//         //         &BLUE,
//         //         &|c, s, st| {
//         //             return EmptyElement::at(c)
//         //                 + Circle::new((0, 0), s, st.filled())
//         //                 + Text::new(
//         //                     format!("{:?}", player_name),
//         //                     (10, 0),
//         //                     TextStyle {
//         //                         color: WHITE.to_backend_color(),
//         //                         font: ("sans-serif", 10).into(),
//         //                         pos: Pos::new(HPos::Left, VPos::Top),
//         //                     },
//         //                 );
//         //         },
//         //     ))
//         //     .unwrap();
//     }
//     // println!();

//     let ct_spawn_x = 1024.0 * map_info.ct_spawn_x.parse::<f64>().unwrap();
//     let ct_spawn_y = -1024.0 * map_info.ct_spawn_y.parse::<f64>().unwrap();

//     let t_spawn_x = 1024.0 * map_info.t_spawn_x.parse::<f64>().unwrap();
//     let t_spawn_y = -1024.0 * map_info.t_spawn_y.parse::<f64>().unwrap();

//     chart.draw_series(PointSeries::of_element(vec![(ct_spawn_x, ct_spawn_y)], 5, &GREEN,                         &|c, s, st| {
//         return EmptyElement::at(c)    // We want to construct a composed element on-the-fly
//         + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
//         + Text::new(format!("{:?}", "CT_SPAWN"), (10, 0), TextStyle { color: WHITE.to_backend_color(), font: ("sans-serif", 10).into(), pos: Pos::new(HPos::Right, VPos::Top)});
//     },)).unwrap();

//     chart.draw_series(PointSeries::of_element(vec![(t_spawn_x, t_spawn_y)], 5, &GREEN,                         &|c, s, st| {
//         return EmptyElement::at(c)    // We want to construct a composed element on-the-fly
//         + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
//         + Text::new(format!("{:?}", "T_SPAWN"), (10, 0), TextStyle { color: WHITE.to_backend_color(), font: ("sans-serif", 10).into(), pos: Pos::new(HPos::Right, VPos::Top)});
//     },)).unwrap();

//     // chart.draw_series(PointSeries::of_element(vec![(262.50575474330356, -576.1659109933036)], 5, &BLUE,                         &|c, s, st| {
//     //     return EmptyElement::at(c)    // We want to construct a composed element on-the-fly
//     //     + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
//     //     + Text::new(format!("{:?}", "EDGE"), (10, 0), TextStyle { color: WHITE.to_backend_color(), font: ("sans-serif", 10).into(), pos: Pos::new(HPos::Right, VPos::Top)});
//     // },)).unwrap();

//     chart.draw_series(PointSeries::of_element(vec![(255.64212472098214, -574.796613420759)], 5, &BLUE,                         &|c, s, st| {
//         return EmptyElement::at(c)    // We want to construct a composed element on-the-fly
//         + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
//         + Text::new(format!("{:?}", "EDGE"), (10, 0), TextStyle { color: WHITE.to_backend_color(), font: ("sans-serif", 10).into(), pos: Pos::new(HPos::Right, VPos::Top)});
//     },)).unwrap();

//     // let ct_best_spawn = PLAYERS_BY_ENTITY_ID.lock().await[&current_round.ct_team_data.best_spawn_entity_id]

//     chart.draw_series(PointSeries::of_element(vec![(100., -100.0)], 5, &YELLOW,                         &|c, s, st| {
//         return EmptyElement::at(c)    // We want to construct a composed element on-the-fly
//         + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
//         + Text::new(format!("CT BEST SPAWN: {:?}", current_round.ct_team_data.best_spawn_name), (10, 0), TextStyle { color: WHITE.to_backend_color(), font: ("sans-serif", 10).into(), pos: Pos::new(HPos::Right, VPos::Top)});
//     },)).unwrap();

//     chart.draw_series(PointSeries::of_element(vec![(100., -150.0)], 5, &YELLOW,                         &|c, s, st| {
//         return EmptyElement::at(c)    // We want to construct a composed element on-the-fly
//         + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
//         + Text::new(format!("T BEST SPAWN: {:?}", current_round.t_team_data.best_spawn_name), (10, 0), TextStyle { color: WHITE.to_backend_color(), font: ("sans-serif", 10).into(), pos: Pos::new(HPos::Right, VPos::Top)});
//     },)).unwrap();

//     root.present().expect("THIS HERE");
//     // println!("FINISHED BUILDING SPAWNS");
//     // std::process::exit(1);
//     println!("SPAWNS BUILT FOR ROUND {:?}", current_round.round_num);
//     std::process::exit(1);
// }

async fn build_spawns(current_round: &mut GameRound) {
    // let check = current_round.round_num == 21;
    // let tick = INGAME_TICK.lock().await;
    let entities = ENTITIES.lock().await;
    // if check {
    //     println!("1");
    // }
    let players_by_entity_id = PLAYERS_BY_ENTITY_ID.lock().await;
    // if check {
    //     println!("2");
    // }
    // println!("BEFORE PROJ");
    // let grenade_projectiles = GRENADE_PROJECTILES.lock().await;
    // if check {
    //     println!("3");
    // }
    // println!("AFTER PROJ");
    // let thrown_grenades = THROWN_GRENADES.lock().await;
    // if check {
    //     println!("4");
    // }
    let map_info: MapInfoParsed = todo!();
    // let test_pos = TEST_POS.lock().await;
    // if check {
    //     println!("5");
    // }
    let filename = format!("test_round_{}.png", current_round.round_num);

    let root = BitMapBackend::new(&filename, (1024, 1024)).into_drawing_area();

    root.fill(&WHITE).unwrap();
    let mut chart = ChartBuilder::on(&root)
        .build_cartesian_2d(0.0..1024.0, -1024.0..0.0)
        .unwrap();
    chart.configure_mesh().disable_mesh().draw().unwrap();

    let image = image::load(
        BufReader::new(
            File::open("map.png").map_err(|e| {
                eprintln!("Unable to open file plotters-doc-data.png, please make sure you have clone this repo with --recursive");
                e
            }).unwrap()),
        ImageFormat::Png,
    ).unwrap();

    let elem = BitMapElement::from(((0.0, 0.0), image));
    chart.draw_series(std::iter::once(elem)).unwrap();

    println!(
        "SPAWNS FOR ROUND {:?} | start_tick: {:?} end_tick: {:?}",
        current_round.round_num, current_round.start_tick, current_round.end_tick,
    );

    for (entity_id, player) in players_by_entity_id.iter() {
        if let Some(entity) = entities.get(entity_id) {
            let pos = entity.position_history[&current_round.freeze_time_end_tick].clone();
            let x_pos = (pos.x.into_owned() - map_info.pos_x) / map_info.scale;
            let y_pos = (pos.y.into_owned() - map_info.pos_y) / map_info.scale;
            chart.draw_series(PointSeries::of_element(
                vec![(x_pos.to_owned(), y_pos.to_owned())],
                3,
                &RED,
                &|c, s, st| {
                    return EmptyElement::at(c)    // We want to construct a composed element on-the-fly
                    + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
                    + Text::new(format!("{:?}", player.name), (10, 0), TextStyle { color: WHITE.to_backend_color(), font: ("sans-serif", 10).into(), pos: Pos::new(HPos::Right, VPos::Top)});
                },
            )).unwrap();
            // entity.debug_props(None);
            // std::process::exit(1);
        }
    }
    // println!();

    let ct_spawn_x = 1024.0 * map_info.ct_spawn_x;
    let ct_spawn_y = -1024.0 * map_info.ct_spawn_y;

    let t_spawn_x = 1024.0 * map_info.t_spawn_x;
    let t_spawn_y = -1024.0 * map_info.t_spawn_y;

    chart.draw_series(PointSeries::of_element(vec![(ct_spawn_x, ct_spawn_y)], 5, &GREEN,                         &|c, s, st| {
        return EmptyElement::at(c)    // We want to construct a composed element on-the-fly
        + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
        + Text::new(format!("{:?}", "CT_SPAWN"), (10, 0), TextStyle { color: WHITE.to_backend_color(), font: ("sans-serif", 10).into(), pos: Pos::new(HPos::Right, VPos::Top)});
    },)).unwrap();

    chart.draw_series(PointSeries::of_element(vec![(t_spawn_x, t_spawn_y)], 5, &GREEN,                         &|c, s, st| {
        return EmptyElement::at(c)    // We want to construct a composed element on-the-fly
        + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
        + Text::new(format!("{:?}", "T_SPAWN"), (10, 0), TextStyle { color: WHITE.to_backend_color(), font: ("sans-serif", 10).into(), pos: Pos::new(HPos::Right, VPos::Top)});
    },)).unwrap();

    // chart.draw_series(PointSeries::of_element(vec![(262.50575474330356, -576.1659109933036)], 5, &BLUE,                         &|c, s, st| {
    //     return EmptyElement::at(c)    // We want to construct a composed element on-the-fly
    //     + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
    //     + Text::new(format!("{:?}", "EDGE"), (10, 0), TextStyle { color: WHITE.to_backend_color(), font: ("sans-serif", 10).into(), pos: Pos::new(HPos::Right, VPos::Top)});
    // },)).unwrap();

    // chart.draw_series(PointSeries::of_element(vec![(255.64212472098214, -574.796613420759)], 5, &BLUE,                         &|c, s, st| {
    //     return EmptyElement::at(c)    // We want to construct a composed element on-the-fly
    //     + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
    //     + Text::new(format!("{:?}", "EDGE"), (10, 0), TextStyle { color: WHITE.to_backend_color(), font: ("sans-serif", 10).into(), pos: Pos::new(HPos::Right, VPos::Top)});
    // },)).unwrap();

    // let ct_best_spawn = PLAYERS_BY_ENTITY_ID.lock().await[&current_round.ct_team_data.best_spawn_entity_id]

    if let Some(entity_id) = current_round.ct_team_data.best_spawn_entity_id {
        if let Some(player) = players_by_entity_id.get(&entity_id) {
            draw_point(
                &mut chart,
                100.,
                -100.,
                5,
                YELLOW,
                format!("CT BEST SPAWN: {:?}", player.name),
                TextStyle {
                    color: WHITE.to_backend_color(),
                    font: ("sans-serif", 10).into(),
                    pos: Pos::new(HPos::Right, VPos::Top),
                },
            );
        }
    }

    if let Some(entity_id) = current_round.t_team_data.best_spawn_entity_id {
        if let Some(player) = players_by_entity_id.get(&entity_id) {
            draw_point(
                &mut chart,
                100.,
                -150.,
                5,
                YELLOW,
                format!("T BEST SPAWN: {:?}", player.name),
                TextStyle {
                    color: WHITE.to_backend_color(),
                    font: ("sans-serif", 10).into(),
                    pos: Pos::new(HPos::Right, VPos::Top),
                },
            );
        }
    }

    for (_entity_id, trajectory) in current_round.trajectories.iter() {
        for (_i, position) in trajectory.iter().enumerate() {
            let x_orig = position.x.clone().into_owned();
            let y_orig = position.y.clone().into_owned();
            let x_pos = (x_orig - map_info.pos_x) / map_info.scale;
            let y_pos = (y_orig - map_info.pos_y) / map_info.scale;
            // println!(
            //     "({:.2}, {:.2}) -> ({:?}, {:?})",
            //     x_pos, y_pos, x_orig, y_orig
            // );
            // panic!();
            draw_point(
                &mut chart,
                x_pos,
                y_pos,
                2,
                MAGENTA,
                "".to_owned(),
                TextStyle {
                    color: WHITE.to_backend_color(),
                    font: ("sans-serif", 10).into(),
                    pos: Pos::new(HPos::Right, VPos::Top),
                },
            );
        }
    }

    // for (entity_id, proj) in grenade_projectiles.iter() {
    //     println!("{:?} {:?}", entity_id, proj.trajectory.len());
    //     for (i, position) in proj.trajectory.iter().enumerate() {
    //         let x_pos = (position.x.clone().into_owned() - map_info.pos_x) / map_info.scale;
    //         let y_pos = (position.y.clone().into_owned() - map_info.pos_y) / map_info.scale;
    //         draw_point(
    //             &mut chart,
    //             x_pos,
    //             y_pos,
    //             2,
    //             MAGENTA,
    //             format!("{i}"),
    //             TextStyle {
    //                 color: WHITE.to_backend_color(),
    //                 font: ("sans-serif", 10).into(),
    //                 pos: Pos::new(HPos::Right, VPos::Top),
    //             },
    //         );
    //     }
    // }

    // if current_round.round_num == 2 {
    //     // println!("BEFORE");
    //     for (entity_id, proj) in grenade_projectiles.iter() {
    //         // println!(
    //         //     "{:?} -> {:?} {:?} {:?}",
    //         //     entity_id,
    //         //     proj.trajectory.len(),
    //         //     proj.wep_type,
    //         //     proj.weapon_instance.original_string,
    //         // );
    //         if !proj.trajectory.is_empty() {
    //             if let Some(entity) = entities.get(&entity_id) {
    //                 //     if entity.server_class.name != "CFlashbang" || entity_id == &210 {
    //                 //         continue;
    //                 //     }
    //                 println!(
    //                     "\t{:?} {:?} {:?}-> {:?} | {:?} {:?} {:?}",
    //                     entity_id,
    //                     // proj.thrower.as_ref().unwrap().name,
    //                     format!(
    //                         "({:?} {:?} {:?} {:?})",
    //                         proj.owner.is_some(),
    //                         proj.thrower.is_some(),
    //                         proj.owner_info.is_some(),
    //                         proj.thrower_info.is_some()
    //                     ),
    //                     proj.wep_type,
    //                     proj.trajectory.len(),
    //                     entity.server_class.name,
    //                     entity._is_grenade(),
    //                     entity.server_class.created_handlers,
    //                 );
    //                 for (i, position) in proj.trajectory.iter().enumerate() {
    //                     let x_pos =
    //                         (position.x.clone().into_owned() - map_info.pos_x) / map_info.scale;
    //                     let y_pos =
    //                         (position.y.clone().into_owned() - map_info.pos_y) / map_info.scale;
    //                     draw_point(
    //                         &mut chart,
    //                         x_pos,
    //                         y_pos,
    //                         2,
    //                         MAGENTA,
    //                         format!("{i}"),
    //                         TextStyle {
    //                             color: WHITE.to_backend_color(),
    //                             font: ("sans-serif", 10).into(),
    //                             pos: Pos::new(HPos::Right, VPos::Top),
    //                         },
    //                     );
    //                 }
    //                 // root.present().expect("THIS HERE");
    //                 // std::process::exit(1);
    //             }
    //         }
    //     }
    //     // println!("HERE: {:?}",thrown_grenades.len());
    //     // for (entity_id, thrown_grenade) in thrown_grenades.iter() {

    //     // }
    //     root.present().expect("THIS HERE");
    //     // std::process::exit(1);
    // }

    // println!("{:?}", test_pos);
    // let test_x = (test_pos.0 - map_info.pos_x) / map_info.scale;
    // let test_y = (test_pos.1 - map_info.pos_y) / map_info.scale;
    // println!("ROUND FLASHES: {:?}", current_round.flashes.len());

    for (player_name, flashes) in current_round.flashes.iter_mut() {
        for flash_data in flashes.iter_mut() {
            println!(
                "FLASH THROWN BY: {:?} at {:?} ({:?})",
                player_name, flash_data.clock_time, flash_data.tick,
            );
            // println!("LOOK AT ME: {:?}", current_round.trajectories.len());
            if let Some(trajectory) = current_round.trajectories.get(&flash_data.entity_id) {
                let flash_pos = &trajectory[trajectory.len() - 1];
                let flash_x_orig = flash_pos.x.clone().into_owned();
                let flash_y_orig = flash_pos.y.clone().into_owned();
                let flash_z_orig = flash_pos.z.clone().into_owned();
                let flash_x = (flash_x_orig - map_info.pos_x) / map_info.scale;
                let flash_y = (flash_y_orig - map_info.pos_y) / map_info.scale;
                let flash_z = flash_z_orig / map_info.scale;
                // println!("{:?}", (flash_x, flash_y, flash_z_orig));
                let last_flash_pos = &trajectory[trajectory.len() - 2];
                let last_flash_x_orig = last_flash_pos.x.clone().into_owned();
                let last_flash_y_orig = last_flash_pos.y.clone().into_owned();
                let last_flash_z_orig = flash_pos.z.clone().into_owned();
                let last_flash_x = (last_flash_x_orig - map_info.pos_x) / map_info.scale;
                let last_flash_y = (last_flash_y_orig - map_info.pos_y) / map_info.scale;
                let last_flash_z = last_flash_z_orig / map_info.scale;

                let flash_slope = calculate_slope((flash_x, flash_y), (last_flash_x, last_flash_y));
                // dbg!(flash_slope);

                let flash_slope_angle = (flash_slope.0 / flash_slope.1).atan().to_degrees();
                // dbg!(flash_slope_angle);

                draw_point(
                    &mut chart,
                    flash_x,
                    flash_y,
                    5,
                    PURPLE,
                    "".to_owned(),
                    TextStyle {
                        color: WHITE.to_backend_color(),
                        font: ("sans-serif", 10).into(),
                        pos: Pos::new(HPos::Right, VPos::Top),
                    },
                );
                for flashed_player in flash_data.enemy_flashes.iter_mut() {
                    flashed_player.set_aim_angle_away_from_flash(
                        (flash_x, flash_y, flash_z),
                        map_info.pos_x,
                        map_info.pos_y,
                        map_info.scale,
                    );

                    // let player_pos = flashed_player.position;
                    // let player_pos_x = player_pos.x.clone().into_owned();
                    // let player_pos_y = player_pos.y.clone().into_owned();
                    // let player_pos_z = player_pos.z.clone().into_owned();
                    // let x = (player_pos_x - map_info.pos_x) / map_info.scale;
                    // let y = (player_pos_y - map_info.pos_y) / map_info.scale;
                    // let z = player_pos_z / map_info.scale;
                    // flashed_player.aim_angle_away_from_flash = calculate_cone_angle((player));
                    // // if flashed_player.name == "coldzera" {
                    // let player_pos = &flashed_player.position;
                    // let player_pos_x = player_pos.x.clone().into_owned();
                    // let player_pos_y = player_pos.y.clone().into_owned();
                    // let player_pos_z = player_pos.z.clone().into_owned();
                    // let x = (player_pos_x - map_info.pos_x) / map_info.scale;
                    // let y = (player_pos_y - map_info.pos_y) / map_info.scale;
                    // let z = player_pos_z / map_info.scale;
                    // // println!(
                    // //     "\t{:?} {:?}",
                    // //     (x, y, z / map_info.scale),
                    // //     (player_pos_x, player_pos_y, z)
                    // // );
                    // draw_point(
                    //     &mut chart,
                    //     x,
                    //     y,
                    //     5,
                    //     ORANGE,
                    //     "".to_owned(),
                    //     TextStyle {
                    //         color: WHITE.to_backend_color(),
                    //         font: ("sans-serif", 10).into(),
                    //         pos: Pos::new(HPos::Right, VPos::Top),
                    //     },
                    // );
                    // // println!(
                    // //     "\t\t{:?} {:?} -> {:?}",
                    // //     flashed_player.name, flashed_player.duration, flashed_player.view_position
                    // // );

                    // let x_view_degrees = flashed_player.view_position.x.clone().into_owned();
                    // let y_view_degrees = flashed_player.view_position.y.clone().into_owned();
                    // // let x_view_radians = f64::to_radians(x_view_degrees);
                    // // let m = f64::tan(x_view_radians);
                    // // let b = _calculate_intercept(x, y, m);
                    // // let nearest_x = _calculate_nearest_x(flash_y, m, b);
                    // // let nearest_y = _calculate_nearest_y(flash_x, m, b);
                    // // println!(
                    // //     "{:?} {:?}\t\t{:?} {:?} {:?} {:?} | {:?} {:?}",
                    // //     (flash_x, flash_y, flash_z, flash_z_orig),
                    // //     (x, y, z, player_pos_z),
                    // //     m,
                    // //     b,
                    // //     (nearest_x as i32, x as i32),
                    // //     (nearest_y as i32, y as i32),
                    // //     nearest_x as i32 == x as i32,
                    // //     nearest_y as i32 == y as i32
                    // // );
                    // // println!(
                    // //     "{:?} {:?}",
                    // //     (flash_x, flash_y, flash_z, flash_z_orig),
                    // //     (x, y, z, player_pos_z, x_view_degrees, y_view_degrees),
                    // // );
                    // // // _some_math((x,y,z,flashed_player.view_position.x.clone().into_owned(), flashed_player.view_position.y.clone().into_owned()),(flash_x,flash_y,flash_z,flash_slope_angle));
                    // // // _calculate_direction_angle((x,y,z,player_pos_z,flashed_player.view_position.x.clone().into_owned(), flashed_player.view_position.y.clone().into_owned()),(flash_x,flash_y,flash_z,flash_z_orig,flash_slope_angle));
                    // // // std::process::exit(1);
                    // let x_angle =
                    //     _calculate_offset_angle((x, y, x_view_degrees), (flash_x, flash_y));

                    // let y_angle = _calculate_offset_angle(
                    //     (y, player_pos_z, y_view_degrees + 180.),
                    //     (flash_y, flash_z_orig),
                    // );

                    // // println!(
                    // //     "{:?} -> ({:.2}, {:.2}) {:.4}",
                    // //     flashed_player.name, x_angle, y_angle, flashed_player.duration
                    // // );

                    // let angle = calculate_cone_angle(
                    //     (x, y, z, x_view_degrees, y_view_degrees),
                    //     (flash_x, flash_y, flash_z),
                    // );

                    // println!(
                    //     "\t{:?} -> ({:.2}°, {:.2}°, {:.0}°) {:.4}s | {:.2} away",
                    //     flashed_player.name,
                    //     angle,
                    //     // angle / 4.,
                    //     x_angle,
                    //     y_angle,
                    //     flashed_player.duration,
                    //     calculate_3d_distance((x, y, z), (flash_x, flash_y, flash_z)),
                    //     // (
                    //     //     format!(
                    //     //         "({:.2}, {:.2}, {:.2})",
                    //     //         last_flash_x, last_flash_y, last_flash_z
                    //     //     ),
                    //     //     format!("({:.2}, {:.2}, {:.2})", flash_x, flash_y, flash_z),
                    //     //     format!(
                    //     //         "({:.2}, {:.2}, {:.2}, {:.4}°, {:.4}°)",
                    //     //         x, y, z, x_view_degrees, y_view_degrees
                    //     //     ),
                    //     // ) // ((flash_x, flash_y, flash_z_orig), (x, y, player_pos_z))
                    // );
                    // // }

                    println!(
                        "\t{:?} -> {:.2}° {:.4}s",
                        flashed_player.name,
                        flashed_player.aim_angle_away_from_flash,
                        flashed_player.duration
                    );
                }
            }
        }
    }

    // let x = (1319.810791015625 - map_info.pos_x) / map_info.scale;
    // let y = (-408.419921875 - map_info.pos_y) / map_info.scale;
    // draw_point(
    //     &mut chart,
    //     x,
    //     y,
    //     5,
    //     YELLOW,
    //     "".to_owned(),
    //     TextStyle {
    //         color: WHITE.to_backend_color(),
    //         font: ("sans-serif", 10).into(),
    //         pos: Pos::new(HPos::Right, VPos::Top),
    //     },
    // );

    // draw_point(
    //     &mut chart,
    //     test_x,
    //     test_y,
    //     5,
    //     ORANGE,
    //     "".to_owned(),
    //     TextStyle {
    //         color: WHITE.to_backend_color(),
    //         font: ("sans-serif", 10).into(),
    //         pos: Pos::new(HPos::Right, VPos::Top),
    //     },
    // );

    root.present().expect("THIS HERE");
    // println!("FINISHED BUILDING SPAWNS");
    // std::process::exit(1);
    // println!("SPAWNS BUILT FOR ROUND {:?} | start_tick: {:?}, end_tick: {:?}", current_round.round_num, current_round.start_tick, current_round.end_tick);

    // for (id, thrown_grenades) in THROWN_GRENADES.lock().await.iter() {

    // }
    // if current_round.round_num == 2 {
    //     std::process::exit(1);
    // }
    // std::process::exit(1);
    println!();
}

fn calculate_slope((x1, y1): (f64, f64), (x2, y2): (f64, f64)) -> (f64, f64) {
    ((y2 - y1), (x2 - x1))
}

fn _calculate_slope_from_degress(degrees: f64) -> f64 {
    f64::tan(f64::to_radians(degrees))
}

fn _calculate_offset_angle(
    (target_x, target_y, target_deg): (f64, f64, f64),
    (src_x, src_y): (f64, f64),
) -> f64 {
    let distance = _calculate_distance((src_x, src_y), (target_x, target_y));

    let x_cos_deg = f64::cos(target_deg.to_radians());
    let y_sin_deg = f64::sin(target_deg.to_radians());
    let look_point = (
        target_x + (distance * x_cos_deg),
        target_y + (distance * y_sin_deg),
    );

    let look_point_distance = _calculate_distance(look_point, (src_x, src_y));

    ((look_point_distance / (distance * 2.)).asin() * 2.).to_degrees()
}

/*
    Angle from player's aim     Time of full blindness  Total time of blinding effects
    0-53°                       1.88                    4.87
    53-72°                      0.45                    3.40
    72-101°                     0.08                    1.95
    101-180°                    0.08                    0.95
*/
fn calculate_cone_angle(
    (player_x, player_y, player_z, player_x_deg, player_y_deg): (f64, f64, f64, f64, f64),
    (flash_x, flash_y, flash_z): (f64, f64, f64),
) -> f64 {
    let flash_person_distance =
        calculate_3d_distance((player_x, player_y, player_z), (flash_x, flash_y, flash_z));

    let x_cos_deg = f64::cos(player_x_deg.to_radians());
    let x_sin_deg = f64::sin(player_x_deg.to_radians());
    let y_cos_deg = f64::cos(player_y_deg.to_radians() - 90.);

    let look_point = (
        player_x + (flash_person_distance * x_cos_deg),
        player_y + (flash_person_distance * x_sin_deg),
        player_z + (flash_person_distance * y_cos_deg),
    );

    // let flash_look_point_distance = calculate_3d_distance(look_point, (flash_x, flash_y, flash_z));
    let radius_point = (
        (look_point.0 + flash_x) / 2.,
        (look_point.1 + flash_y) / 2.,
        (look_point.2 + flash_z) / 2.,
    );
    let radius = calculate_3d_distance(radius_point, look_point);

    // let radius = flash_look_point_distance / 2.;
    let height = (flash_person_distance.powi(2) - radius.powi(2)).sqrt();

    // println!("radius: {:?}",radius);
    // println!("height: {:?}",height);

    let slant_length = (height.powi(2) + radius.powi(2)).sqrt();

    f64::asin(radius / slant_length).to_degrees() * 2.

    // (radius / height).atan().to_degrees() * 4.

    // let flash_person_distance = _calculate_distance((player_x, player_y), (flash_x, flash_y));
    // let x_cos_deg = f64::cos(player_x_deg.to_radians());
    // let x_sin_deg = f64::sin(player_x_deg.to_radians());
    // let y_cos_deg = f64::cos(player_y_deg.to_radians());
    // let look_point = (
    //     player_x + (flash_person_distance * x_cos_deg),
    //     player_y + (flash_person_distance * x_sin_deg),
    // );

    // let flash_look_point_distance = _calculate_distance(look_point, (flash_x, flash_y));
    // let radius = flash_look_point_distance / 2.;
    // // let height = flash_person_distance.powi(2) - radius.powi(2);

    // let toa = radius / flash_person_distance;
    // let tan = toa.atan();
    // // println!(
    // //     "{:?} {:?} {:?}",
    // //     (toa, tan, tan.to_degrees(), x_cos_deg, y_sin_deg),
    // //     flash_person_distance,
    // //     look_point
    // // );
    // // (radius / height).atan().to_degrees() * 4.
    // tan.to_degrees() * 4.
}

fn _calculate_direction_angle(
    (player_x, player_y, player_z, player_z_orig, player_x_deg, player_y_deg): (
        f64,
        f64,
        f64,
        f64,
        f64,
        f64,
    ),
    (flash_x, flash_y, flash_z, flash_z_orig, flash_x_deg): (f64, f64, f64, f64, f64),
) {
    // let flash_m = _calculate_slope_from_degress(flash_x_deg);
    // let player_m = _calculate_slope_from_degress(player_x_deg);

    // // let flash_b = _calculate_intercept(flash_x, flash_y, flash_m);
    // // let player_b = _calculate_intercept(player_x, player_y, player_m);

    // // dbg!(player_x_deg);
    // let flash_player_distance = _calculate_distance((flash_x, flash_y), (player_x, player_y));
    // dbg!(flash_player_distance);
    // // let flash_player_deg = (flash_y - player_y)/(flash_x - player_x).tan().to_degrees();

    // let x_cos_deg = f64::cos(player_x_deg.to_radians());
    // let y_sin_deg = f64::sin(player_x_deg.to_radians());
    // let player_look_point = (player_x + (flash_player_distance * x_cos_deg), player_y + (flash_player_distance * y_sin_deg));

    // // let flash_look_point = {
    // //     let x_cos_deg = f64::cos(flash_player_deg.to_radians());
    // //     let y_sin_deg = f64::sin(flash_player_deg.to_radians());
    // //     (player_x + (flash_player_distance * x_cos_deg), player_y + (flash_player_distance * y_sin_deg))
    // // };
    // // dbg!(x_cos_deg, y_sin_deg);
    // // println!("{:?}",player_look_point);
    // // println!("{:?}",flash_look_point);

    // let look_point_to_flash = _calculate_distance(player_look_point, (flash_x, flash_y));
    // // dbg!(look_point_to_flash);
    // // dbg!(look_point_to_flash * 0.5 * std::f64::consts::PI);

    // let angle = (look_point_to_flash / (flash_player_distance * 2.)).asin() * 2.;
    // // dbg!(angle);
    // // dbg!(angle.to_degrees());

    // println!("{:?} {:?}",player_y_deg,angle.to_degrees());

    // let flash_player_distance_yz = _calculate_distance((flash_y, flash_z), (player_y, player_z));
    // // dbg!(flash_player_distance_yz);

    let x_angle = _calculate_offset_angle((player_x, player_y, player_x_deg), (flash_x, flash_y));
    dbg!(x_angle);

    let y_angle = _calculate_offset_angle(
        (player_y, player_z_orig, player_y_deg + 180.),
        (flash_y, flash_z_orig),
    );
    dbg!(y_angle);

    dbg!((x_angle + y_angle) / 2.);

    // let flash_player_distance = _calculate_distance((flash_x, flash_y), (player_x, player_y));

    // let player_look_point = (player_x, player_y - flash_player_distance);

    // let player_flash_chord = _calculate_distance((flash_x, flash_y), player_look_point);

    // let player_flash_arc_len = player_flash_chord * std::f64::consts::PI * 0.5;

    // let player_flash_arc_angle = (player_flash_arc_len / (2. * std::f64::consts::PI * flash_player_distance)) * 360.;

    // dbg!(player_flash_arc_angle, player_flash_arc_angle.to_degrees());
    // // let flash_to_player_x = _calculate_distance((flash_x, flash_y), (player_x, flash_y));

    // let flash_player_m = (player_x - flash_x)/(player_y - flash_y);
    // dbg!(flash_player_m);

    let flashbang_vec = DVec3 {
        x: flash_x,
        y: flash_y,
        z: flash_z_orig,
    };
    let player_vec = DVec3 {
        x: player_x,
        y: player_y,
        z: player_z_orig,
    };

    // let flashbang_vec = DVec3 {x: 550., y: -550., z: 10.};
    // let player_vec = DVec3 {x: 600., y: -550., z: 0.};
    // let player_x_deg = 180.0_f64;
    // let player_y_deg = 10.0_f64;
    let dir_from_player_to_flashbang = { flashbang_vec - player_vec }.normalize();
    // dbg!(dir_from_player_to_flashbang);

    let player_y_deg = if player_y_deg < 180. {
        player_y_deg + 180.
    } else {
        player_y_deg
    };

    let look_quat_xyz = DQuat::from_euler(
        glam::EulerRot::XYZ,
        player_x_deg.to_radians(),
        (player_y_deg).to_radians(),
        0.0,
    );
    let look_quat_xzy = DQuat::from_euler(
        glam::EulerRot::XYZ,
        player_x_deg.to_radians(),
        0.0,
        (player_y_deg).to_radians(),
    );

    // When angles are 0,0 what is the direction you're looking in/axis that you're looking down
    let default_look_dir = DVec3 {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    };

    // dbg!((flashbang_vec - player_vec).length());

    // direction player looking in
    let player_look_dir = look_quat_xyz.mul_vec3(default_look_dir);

    let dot = dir_from_player_to_flashbang.dot(player_look_dir);

    // dot is going to be in the range of [-1.0, 1.0]
    // 1.0 means the player is looking directly at the flashbang
    // -1.0 means the player is looking directly away from the flashbang
    // 0.0 means the player is looking directly perpendicular compared to the flashbang
    dbg!(dot);
    dbg!(dot.to_degrees());
    dbg!(dot.acos());
    dbg!(dot.acos().to_degrees());
    println!();
}

fn _some_math(
    (player_x, player_y, player_z, player_x_deg, player_y_deg): (f64, f64, f64, f64, f64),
    (flash_x, flash_y, flash_z, flash_x_deg): (f64, f64, f64, f64),
) {
    println!("player: ({player_x}, {player_y}, {player_z})");
    println!("flash: ({flash_x}, {flash_y}, {flash_z})");

    // let player_to_right_angle = (player_x, flash_y);
    /*
        Player to right angle   -> (player_x, flash-y)
            (player_y - flash-y).abs()

    */
    let player_to_right_angle = (player_y - flash_y).abs();
    // let player_to_flash_angle = (player_y - flash_y).abs();
    let flash_to_right_angle = (player_x - flash_x).abs();
    dbg!(player_to_right_angle);
    dbg!(flash_to_right_angle);

    let flash_to_player = (player_to_right_angle.powi(2) + flash_to_right_angle.powi(2)).sqrt();
    dbg!(flash_to_player);

    let player_angle = (flash_to_right_angle / flash_to_player).asin().to_degrees();
    dbg!(player_angle);

    let flash_angle = (player_to_right_angle / flash_to_right_angle)
        .atan()
        .to_degrees();
    dbg!(flash_angle);

    // 74.48244621259425
    // 74.482446212594277997442755766671

    let total_degrees = flash_angle + player_angle + 90.;
    dbg!(total_degrees);

    let player_m = {
        let view_radians = f64::to_radians(player_x_deg);
        f64::tan(view_radians)
    };
    dbg!(player_m);

    let player_b = _calculate_intercept(player_x, player_y, player_m);
    dbg!(player_b);

    let new_y = (player_y % 50.) - player_y;

    dbg!(new_y);
    // dbg!(new_y - player_y);
    // dbg!(-new_y + player_y);

    let new_x = (new_y - player_b) / player_m;
    // println!("{:?}",new_x);
    dbg!(new_x);

    let flash_m = {
        let view_radians = f64::to_radians(flash_x_deg);
        f64::tan(view_radians)
    };

    let flash_b = _calculate_intercept(flash_x, flash_y, flash_m);
    dbg!(flash_b);
    // let flash_to_player_point = (flash_x, player_y);
    let new_flash_y_from_player_x = _calculate_nearest_y(player_x, flash_m, flash_b);
    dbg!(new_flash_y_from_player_x);
    let new_flash_x_from_player_y = _calculate_nearest_x(player_y, flash_m, flash_b);
    dbg!(new_flash_x_from_player_y);

    let new_flash_points_distance = _calculate_distance(
        (player_x, new_flash_y_from_player_x),
        (new_flash_x_from_player_y, player_y),
    );
    dbg!(new_flash_points_distance);

    // let new_flash_y_player_distance = (new_flash_y_from_player_x - player_y).abs();
    // let new_flash_x_player_distance = (new_flash_x_from_player_y - player_x).abs();

    let new_flash_y_player_y_distance =
        _calculate_distance((player_x, player_y), (player_x, new_flash_y_from_player_x));
    dbg!(new_flash_y_player_y_distance);

    let new_flash_x_player_x_distance =
        _calculate_distance((player_x, player_y), (new_flash_x_from_player_y, player_y));
    dbg!(new_flash_x_player_x_distance);

    // dbg!(new_flash_x_player_distance, new_flash_y_player_distance);

    // let new_flash_points_distance_v2 = (new_flash_x_player_distance.powi(2) + new_flash_y_player_distance.powi(2)).sqrt();
    // dbg!(new_flash_points_distance, new_flash_points_distance_v2);

    let player_x_flash_angle = {
        (new_flash_y_player_y_distance / new_flash_x_player_x_distance)
            .atan()
            .to_degrees()
    };

    let player_y_flash_angle = {
        (new_flash_x_player_x_distance / new_flash_y_player_y_distance)
            .atan()
            .to_degrees()
    };

    dbg!(
        player_x_flash_angle,
        player_y_flash_angle,
        player_x_flash_angle + player_y_flash_angle
    );

    let player_y_to_flash_distance = _calculate_distance((flash_x, player_y), (flash_x, flash_y));
    dbg!(player_y_to_flash_distance);

    let flash_to_player_distance = _calculate_distance((player_x, player_y), (flash_x, flash_y));
    dbg!(flash_to_player_distance);

    let flash_x_to_player_distance = _calculate_distance((flash_x, player_y), (player_x, player_y));
    dbg!(flash_x_to_player_distance);

    println!();

    let flash_player_y_angle = (flash_x_to_player_distance / player_y_to_flash_distance)
        .atan()
        .to_degrees();
    dbg!(flash_player_y_angle);

    let flash_player_x_angle = (player_y_to_flash_distance / flash_x_to_player_distance)
        .atan()
        .to_degrees();
    dbg!(flash_player_x_angle);

    // dbg!(player_y.abs() % 100.);

    let new_player_y = -((50. - (player_y.abs() % 50.)) + player_y.abs());
    // let new_player_x = (new_player_y - player_b) / player_m;
    let new_player_x = _calculate_nearest_x(new_player_y, player_m, player_b);

    dbg!(new_player_x, new_player_y);

    println!();
    let new_player_x_distance =
        _calculate_distance((new_player_x, new_player_y), (player_x, new_player_y));
    dbg!(new_player_x_distance);

    let new_player_y_distance = _calculate_distance((player_x, player_y), (player_x, new_player_y));
    dbg!(new_player_y_distance);

    let new_player_slope_distance =
        _calculate_distance((player_x, player_y), (new_player_x, new_player_y));
    dbg!(new_player_slope_distance);

    let new_player_y_angle = (new_player_x_distance / new_player_y_distance)
        .atan()
        .to_degrees();
    let new_player_x_angle = (new_player_y_distance / new_player_x_distance)
        .atan()
        .to_degrees();
    dbg!(new_player_x_angle, new_player_y_angle);

    let angle = flash_player_x_angle + 90. + new_player_y_angle;
    dbg!(angle);
}

fn _calculate_distance((x1, y1): (f64, f64), (x2, y2): (f64, f64)) -> f64 {
    ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt()
}

fn calculate_3d_distance((x1, y1, z1): (f64, f64, f64), (x2, y2, z2): (f64, f64, f64)) -> f64 {
    ((x2 - x1).powi(2) + (y2 - y1).powi(2) + (z2 - z1).powi(2)).sqrt()
}

fn _calculate_intercept(x: f64, y: f64, n: f64) -> f64 {
    let mx = x * n;
    // println!("mx: {:?}", mx);
    // println!("({x}, {y}, {n})");
    y - mx
}

fn _calculate_nearest_x(y: f64, m: f64, b: f64) -> f64 {
    (y - b) / m
}

fn _calculate_nearest_y(x: f64, m: f64, b: f64) -> f64 {
    (m * x) + b
}

fn draw_point<S: Into<ShapeStyle>>(
    chart: &mut ChartContext<
        '_,
        BitMapBackend<'_, RGBPixel>,
        Cartesian2d<RangedCoordf64, RangedCoordf64>,
    >,
    x_pos: f64,
    y_pos: f64,
    size: i32,
    color: S,
    text: String,
    text_style: TextStyle<'_>,
) {
    chart
        .draw_series(PointSeries::of_element(
            vec![(x_pos, y_pos)],
            size,
            color,
            &|c, s, st| {
                return EmptyElement::at(c)
                    + Circle::new((0, 0), s, st.filled())
                    + Text::new(text.to_owned(), (10, 0), text_style.to_owned());
            },
        ))
        .unwrap();
}

/*
//nolint:funlen
func newGameEventHandler(parser *parser, ignoreBombsiteIndexNotFound bool) gameEventHandler {
    geh := gameEventHandler{
        parser:                      parser,
        userIDToFallDamageFrame:     make(map[int32]int),
        frameToRoundEndReason:       make(map[int]events.RoundEndReason),
        ignoreBombsiteIndexNotFound: ignoreBombsiteIndexNotFound,
    }

    // some events need to be delayed until their data is available
    // some events can't be delayed because the required state is lost by the end of the tick
    // TODO: maybe we're supposed to delay all of them and store the data we need until the end of the tick
    delay := func(f gameEventHandlerFunc) gameEventHandlerFunc {
        return func(data map[string]*msg.CSVCMsg_GameEventKeyT) {
            parser.delayedEventHandlers = append(parser.delayedEventHandlers, func() {
                f(data)
            })
        }
    }

    // some events only need to be delayed at the start of the demo until players are connected
    delayIfNoPlayers := func(f gameEventHandlerFunc) gameEventHandlerFunc {
        return func(data map[string]*msg.CSVCMsg_GameEventKeyT) {
            if len(parser.gameState.playersByUserID) == 0 {
                delay(f)
            } else {
                f(data)
            }
        }
    }

    geh.gameEventNameToHandler = map[string]gameEventHandlerFunc{
        // sorted alphabetically
        "ammo_pickup":                     nil,                                   // Dunno, only in locally recorded (POV) demo
        "announce_phase_end":              nil,                                   // Dunno
        "begin_new_match":                 geh.beginNewMatch,                     // Match started
        "bomb_beep":                       nil,                                   // Bomb beep
        "bomb_begindefuse":                delayIfNoPlayers(geh.bombBeginDefuse), // Defuse started
        "bomb_beginplant":                 delayIfNoPlayers(geh.bombBeginPlant),  // Plant started
        "bomb_defused":                    delayIfNoPlayers(geh.bombDefused),     // Defuse finished
        "bomb_dropped":                    delayIfNoPlayers(geh.bombDropped),     // Bomb dropped
        "bomb_exploded":                   delayIfNoPlayers(geh.bombExploded),    // Bomb exploded
        "bomb_pickup":                     delayIfNoPlayers(geh.bombPickup),      // Bomb picked up
        "bomb_planted":                    delayIfNoPlayers(geh.bombPlanted),     // Plant finished
        "bot_takeover":                    delay(geh.botTakeover),                // Bot got taken over
        "buytime_ended":                   nil,                                   // Not actually end of buy time, seems to only be sent once per game at the start
        "cs_intermission":                 nil,                                   // Dunno, only in locally recorded (POV) demo
        "cs_match_end_restart":            nil,                                   // Yawn
        "cs_pre_restart":                  nil,                                   // Not sure, doesn't seem to be important
        "cs_round_final_beep":             nil,                                   // Final beep
        "cs_round_start_beep":             nil,                                   // Round start beeps
        "cs_win_panel_match":              geh.csWinPanelMatch,                   // Not sure, maybe match end event???
        "cs_win_panel_round":              nil,                                   // Win panel, (==end of match?)
        "decoy_detonate":                  geh.decoyDetonate,                     // Decoy exploded/expired
        "decoy_started":                   delay(geh.decoyStarted),               // Decoy started. Delayed because projectile entity is not yet created
        "endmatch_cmm_start_reveal_items": nil,                                   // Drops
        "entity_visible":                  nil,                                   // Dunno, only in locally recorded (POV) demo
        "enter_bombzone":                  nil,                                   // Dunno, only in locally recorded (POV) demo
        "exit_bombzone":                   nil,                                   // Dunno, only in locally recorded (POV) demo
        "enter_buyzone":                   nil,                                   // Dunno, only in locally recorded (POV) demo
        "exit_buyzone":                    nil,                                   // Dunno, only in locally recorded (POV) demo
        "flashbang_detonate":              geh.flashBangDetonate,                 // Flash exploded
        "hegrenade_detonate":              geh.heGrenadeDetonate,                 // HE exploded
        "hostage_killed":                  geh.hostageKilled,                     // Hostage killed
        "hostage_hurt":                    geh.hostageHurt,                       // Hostage hurt
        "hostage_rescued":                 geh.hostageRescued,                    // Hostage rescued
        "hostage_rescued_all":             geh.HostageRescuedAll,                 // All hostages rescued
        "hltv_chase":                      nil,                                   // Don't care
        "hltv_fixed":                      nil,                                   // Dunno
        "hltv_message":                    nil,                                   // No clue
        "hltv_status":                     nil,                                   // Don't know
        "hostname_changed":                nil,                                   // Only present in locally recorded (POV) demos
        "inferno_expire":                  geh.infernoExpire,                     // Incendiary expired
        "inferno_startburn":               delay(geh.infernoStartBurn),           // Incendiary exploded/started. Delayed because inferno entity is not yet created
        "inspect_weapon":                  nil,                                   // Dunno, only in locally recorded (POV) demos
        "item_equip":                      delay(geh.itemEquip),                  // Equipped / weapon swap, I think. Delayed because of #142 - Bot entity possibly not yet created
        "item_pickup":                     delay(geh.itemPickup),                 // Picked up or bought? Delayed because of #119 - Equipment.UniqueID()
        "item_pickup_slerp":               nil,                                   // Not sure, only in locally recorded (POV) demos
        "item_remove":                     geh.itemRemove,                        // Dropped?
        "jointeam_failed":                 nil,                                   // Dunno, only in locally recorded (POV) demos
        "other_death":                     nil,                                   // Dunno
        "player_blind":                    delay(geh.playerBlind),                // Player got blinded by a flash. Delayed because Player.FlashDuration hasn't been updated yet
        "player_changename":               nil,                                   // Name change
        "player_connect":                  geh.playerConnect,                     // Bot connected or player reconnected, players normally come in via string tables & data tables
        "player_connect_full":             nil,                                   // Connecting finished
        "player_death":                    delayIfNoPlayers(geh.playerDeath),     // Player died
        "player_disconnect":               geh.playerDisconnect,                  // Player disconnected (kicked, quit, timed out etc.)
        "player_falldamage":               geh.playerFallDamage,                  // Falldamage
        "player_footstep":                 delayIfNoPlayers(geh.playerFootstep),  // Footstep sound.- Delayed because otherwise Player might be nil
        "player_hurt":                     geh.playerHurt,                        // Player got hurt
        "player_jump":                     geh.playerJump,                        // Player jumped
        "player_spawn":                    nil,                                   // Player spawn
        "player_spawned":                  nil,                                   // Only present in locally recorded (POV) demos
        "player_given_c4":                 nil,                                   // Dunno, only present in locally recorded (POV) demos

        // Player changed team. Delayed for two reasons
        // - team IDs of other players changing teams in the same tick might not have changed yet
        // - player entities might not have been re-created yet after a reconnect
        "player_team":                    delay(geh.playerTeam),
        "round_announce_final":           geh.roundAnnounceFinal,           // 30th round for normal de_, not necessarily matchpoint
        "round_announce_last_round_half": geh.roundAnnounceLastRoundHalf,   // Last round of the half
        "round_announce_match_point":     nil,                              // Match point announcement
        "round_announce_match_start":     nil,                              // Special match start announcement
        "round_announce_warmup":          nil,                              // Dunno
        "round_end":                      geh.roundEnd,                     // Round ended and the winner was announced
        "round_end_upload_stats":         nil,                              // Dunno, only present in POV demos
        "round_freeze_end":               geh.roundFreezeEnd,               // Round start freeze ended
        "round_mvp":                      geh.roundMVP,                     // Round MVP was announced
        "round_officially_ended":         geh.roundOfficiallyEnded,         // The event after which you get teleported to the spawn (=> You can still walk around between round_end and this event)
        "round_poststart":                nil,                              // Ditto
        "round_prestart":                 nil,                              // Ditto
        "round_start":                    geh.roundStart,                   // Round started
        "round_time_warning":             nil,                              // Round time warning
        "server_cvar":                    nil,                              // Dunno
        "smokegrenade_detonate":          geh.smokeGrenadeDetonate,         // Smoke popped
        "smokegrenade_expired":           geh.smokeGrenadeExpired,          // Smoke expired
        "switch_team":                    nil,                              // Dunno, only present in POV demos
        "tournament_reward":              nil,                              // Dunno
        "weapon_fire":                    delayIfNoPlayers(geh.weaponFire), // Weapon was fired
        "weapon_fire_on_empty":           nil,                              // Sounds boring
        "weapon_reload":                  geh.weaponReload,                 // Weapon reloaded
        "weapon_zoom":                    nil,                              // Zooming in
        "weapon_zoom_rifle":              nil,                              // Dunno, only in locally recorded (POV) demo
    }

    return geh
}

*/
