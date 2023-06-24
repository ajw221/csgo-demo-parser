use std::io::{Read, Seek};

use custom_bitreader::BitReader;

const MAX_OS_PATH: usize = 260;

#[derive(Debug, PartialEq, Clone)]
pub struct Header {
    pub demo_type: String,
    pub version: u32,
    pub protocol: u32,
    pub server: String,
    pub nick: String,
    pub map: String,
    pub game: String,
    pub duration: f32,
    pub ticks: u32,
    pub frames: u32,
    pub signon: u32,
}

impl Header {
    pub fn _parse<T: Read + Seek + Send>(r: &mut BitReader<T>) -> Self {
        Self {
            demo_type: r.read_cstring(8),
            version: r.read_signed_int(32) as u32,
            protocol: r.read_signed_int(32) as u32,
            server: r.read_cstring(MAX_OS_PATH),
            nick: r.read_cstring(MAX_OS_PATH),
            map: r.read_cstring(MAX_OS_PATH),
            game: r.read_cstring(MAX_OS_PATH),
            duration: r.read_float(),
            ticks: r.read_signed_int(32) as u32,
            frames: r.read_signed_int(32) as u32,
            signon: r.read_signed_int(32) as u32,
        }
    }
}

impl Default for Header {
    fn default() -> Self {
        Self {
            demo_type: "".to_string(),
            version: 0,
            protocol: 0,
            server: "".to_string(),
            nick: "".to_string(),
            map: "".to_string(),
            game: "".to_string(),
            duration: 0.0,
            ticks: 0,
            frames: 0,
            signon: 0,
        }
    }
}
