use std::{
    borrow::Cow,
    io::{Cursor, Read, Seek},
    pin::Pin,
};

use ahash::AHashMap as HashMap;
use custom_bitreader::BitReader;

use crate::{
    common::Vector64,
    entity::{Entity, Property},
    sendtable::SendTableProperty,
    INGAME_TICK,
};

type CreateHandler = fn(i32) -> Pin<Box<(dyn futures::Future<Output = ()> + Send + Sync)>>;

#[derive(Clone)]
pub struct ServerClass {
    pub id: i32,
    pub name: String,
    pub dt_id: i32,
    pub dt_name: String,
    pub base_classes_by_name: Option<HashMap<String, ServerClass>>,
    pub flattened_props: Option<Vec<FlattenedPropEntry>>,
    pub prop_name_to_idx: Option<HashMap<String, i32>>,

    pub instance_baseline: Option<Vec<u8>>,
    pub preprocessed_baseline: Option<HashMap<i32, PropertyValueEnum>>,

    pub created_handlers: Option<Vec<CreateHandler>>,
    pub index: i32,
}

impl ServerClass {
    #[inline]
    pub fn _base_class_exists(&self, name: &str) -> bool {
        matches!(&self.base_classes_by_name, Some(base_classes_by_name) if base_classes_by_name.contains_key(name))
    }

    #[inline]
    pub fn new<T: Read + Seek + Send>(i: usize, r: &mut BitReader<T>, max: usize) -> Self {
        let class_id = r.read_int(16);
        if class_id > max {
            panic!("Invalid class index");
        }

        Self {
            id: class_id as i32,
            name: r.read_string(),
            dt_id: -1,
            dt_name: r.read_string(),
            base_classes_by_name: None,
            flattened_props: None,
            prop_name_to_idx: None,

            instance_baseline: None,
            preprocessed_baseline: None,

            created_handlers: None,

            index: i as i32,
        }
    }

    pub async fn new_entity<T: Read + Seek + Send>(
        &mut self,
        r: &mut BitReader<T>,
        id: i32,
        prop_indices_vec: &mut Vec<u32>,
    ) -> Entity {
        let mut entity = Entity {
            server_class: self.to_owned(),
            id,
            props: if let Some(fp_len) = self.get_flattened_props_len() {
                Vec::with_capacity(fp_len)
            } else {
                Vec::new()
            },
            position: |_| -> Vector64 { Vector64::default() },
            last_position: Vector64::default(),
            inventory: None,
            wep_prefix: Cow::Borrowed(""),
            weapon_cache: None,
            position_history: HashMap::new(),
            created_on_tick: *INGAME_TICK.lock().await,
            is_in_buyzone: false,
            buyzone_leave_pos: Vector64::default(),
            team: 0,
            ..Default::default()
        };

        if let Some(flattened_props) = &self.flattened_props {
            for fp in flattened_props.iter() {
                entity.props.push(Property {
                    entry: fp.to_owned(),
                    value: PropertyValueEnum::None,
                    update_handlers: None,
                });
            }
        }

        Entity::initialize(&mut entity);

        if let Some(false) = self.preprocessed_baseline_is_empty() {
            entity.apply_baseline();
        } else if let Some(instance_baseline) = &mut self.instance_baseline {
            let mut _r = BitReader::new_small_bit_reader(Cursor::new(instance_baseline));
            entity.apply_update(&mut _r, prop_indices_vec).await;

            let mut ppb: HashMap<i32, PropertyValueEnum> =
                HashMap::with_capacity(entity.props.len());
            ppb.extend(
                entity
                    .props
                    .iter()
                    .enumerate()
                    .map(|(i, prop)| (i as i32, prop.value.to_owned())),
            );
            self.preprocessed_baseline = Some(ppb);
        } else {
            self.clear_preprocessed_baseline();
        }

        entity.apply_update(r, prop_indices_vec).await;

        if let Some(created_handlers) = &self.created_handlers {
            for h in created_handlers {
                h(entity.id).await;
            }
        }

        entity
    }

    fn clear_preprocessed_baseline(&mut self) {
        if let Some(preprocessed_baseline) = &mut self.preprocessed_baseline {
            preprocessed_baseline.clear();
        }
    }

    fn preprocessed_baseline_is_empty(&self) -> Option<bool> {
        match &self.preprocessed_baseline {
            Some(preprocessed_baseline) if preprocessed_baseline.is_empty() => Some(true),
            Some(preprocessed_baseline) if !preprocessed_baseline.is_empty() => Some(false),
            _ => None,
        }
    }

    fn get_flattened_props_len(&self) -> Option<usize> {
        self.flattened_props
            .as_ref()
            .map(|flattened_props| flattened_props.len())
    }
}

#[derive(Debug, Clone)]
pub struct FlattenedPropEntry {
    pub prop: SendTableProperty,
    pub array_elem_prop: Option<SendTableProperty>,
    pub name: String,
    pub index: i32,
}

impl Default for FlattenedPropEntry {
    fn default() -> Self {
        Self {
            prop: SendTableProperty {
                flags: -1,
                name: "".to_string(),
                dt_name: "".to_string(),
                low_value: -1.0,
                high_value: -1.0,
                num_bits: -1,
                num_elems: -1,
                priority: -1,
                raw_type: -1,
            },
            array_elem_prop: None,
            name: "".to_string(),
            index: -1,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PropertyValueEnum {
    Array(Vec<PropertyValueEnum>),
    Vector(Cow<'static, Vector64>),
    Integer(Cow<'static, i32>),
    String(Cow<'static, str>),
    Float(Cow<'static, f64>),
    None,
}

impl PropertyValueEnum {
    pub fn as_integer(&self) -> i32 {
        if let PropertyValueEnum::Integer(Cow::Owned(val)) = self {
            return val.to_owned();
        }
        -1
    }

    pub fn is_match(value: &PropertyValueEnum, to_match: &PropertyValueEnum) -> bool {
        match value {
            PropertyValueEnum::Array(_) => matches!(to_match, PropertyValueEnum::Array(_)),
            PropertyValueEnum::Vector(_) => matches!(to_match, PropertyValueEnum::Vector(_)),
            PropertyValueEnum::Integer(_) => matches!(to_match, PropertyValueEnum::Integer(_)),
            PropertyValueEnum::String(_) => matches!(to_match, PropertyValueEnum::String(_)),
            PropertyValueEnum::Float(_) => matches!(to_match, PropertyValueEnum::Float(_)),
            _ => false,
        }
    }
}
