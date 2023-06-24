use std::borrow::Cow;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vector64 {
    pub x: Cow<'static, f64>,
    pub y: Cow<'static, f64>,
    pub z: Cow<'static, f64>,
}

impl Default for Vector64 {
    fn default() -> Self {
        Self {
            x: Cow::Owned(0.0),
            y: Cow::Owned(0.0),
            z: Cow::Owned(0.0),
        }
    }
}
