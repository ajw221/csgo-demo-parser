use std::io::{Read, Seek, SeekFrom};

const SMALL_BUFFER: usize = 512;
const LARGE_BUFFER: usize = 1024 * 128;

const SLED: usize = 8;
const SLED_MASK: usize = SLED - 1;
const SLED_BITS: usize = SLED << 3;

#[derive(Clone)]
pub struct Stack(pub Vec<usize>);

impl Default for Stack {
    fn default() -> Self {
        Self(Vec::with_capacity(2))
    }
}

impl Stack {
    fn push(&mut self, v: usize) {
        self.0.push(v);
    }

    fn pop(&mut self) -> (Vec<usize>, usize) {
        let l = self.0.len();
        (self.0[..l - 1].to_vec(), self.0[l - 1])
    }

    fn top(&self) -> usize {
        self.0[self.0.len() - 1]
    }
}

const MIN_STRING_BUFFER_LENGTH: usize = 256;
const VALVE_MAX_STRING_LENGTH: usize = 4096;

pub struct BitReader<T>
where
    T: Read + Seek + Send,
{
    pub underlying: T,
    pub buffer: Vec<u8>,
    pub offset: usize,
    pub bits_in_buffer: usize,
    pub lazy_position: usize,
    pub chunk_targets: Stack,
    pub end_reached: bool,
}

impl<T> BitReader<T>
where
    T: Read + Seek + Send,
{
    pub fn new_small_bit_reader(underlying: T) -> Self {
        Self::new_bit_reader(underlying, vec![0; SMALL_BUFFER])
    }

    pub fn new_large_bit_reader(underlying: T) -> Self {
        Self::new_bit_reader(underlying, vec![0; LARGE_BUFFER])
    }

    pub fn new_bit_reader(underlying: T, buffer: Vec<u8>) -> Self {
        Self::open_with_buffer(underlying, buffer)
    }

    pub fn open_with_buffer(mut underlying: T, mut buffer: Vec<u8>) -> Self {
        underlying.read_exact(&mut buffer).unwrap();
        let bits_in_buffer: usize = if ((buffer.len() as isize) << 3) - (SLED_BITS as isize) < 0 {
            (((buffer.len() as isize) << 3) + (SLED_BITS as isize)) as usize
        } else {
            (buffer.len() << 3) - SLED_BITS
        };
        Self {
            underlying,
            bits_in_buffer,
            buffer,
            offset: 0,
            lazy_position: 0,
            chunk_targets: Stack(Vec::with_capacity(2)),
            end_reached: false,
        }
    }

    pub fn lazy_position(&self) -> usize {
        self.lazy_position
    }

    pub fn actual_position(&self) -> usize {
        self.lazy_position + self.offset
    }

    fn advance(&mut self, bits: usize) {
        self.offset += bits;
        while self.offset > self.bits_in_buffer {
            self.refill_buffer();
        }
    }

    pub fn read_bit(&mut self) -> bool {
        let res = (self.buffer[self.offset >> 3] & (1 << (self.offset & 7))) != 0;
        self.advance(1);
        res
    }

    pub fn read_single_byte(&mut self) -> u8 {
        self.read_byte_internal(self.offset & 7 != 0)
    }

    fn read_byte_internal(&mut self, bit_level: bool) -> u8 {
        if !bit_level {
            let res = self.buffer[self.offset >> 3];
            self.advance(8);
            return res;
        }
        self.read_bits_to_bytes(8)
    }

    pub fn read_bits_to_bytes(&mut self, n: usize) -> u8 {
        self.read_int(n) as u8
    }

    pub fn read_int(&mut self, n: usize) -> usize {
        let val = uint64(&self.buffer[(self.offset >> 3) & !3..]);
        let res = (val << (64 - (self.offset & 31) - n) >> (64 - n)) as usize;
        self.advance(n);
        res
    }

    pub fn read_bytes(&mut self, n: usize) -> Vec<u8> {
        let mut res = vec![0; n];
        self.read_bytes_into(&mut res, n);
        res
    }

    pub fn read_bytes_into(&mut self, out: &mut [u8], n: usize) {
        let bit_level = self.offset & 7 != 0;
        if !bit_level && self.offset + (n << 3) <= self.bits_in_buffer {
            out[0..n].copy_from_slice(&self.buffer[self.offset >> 3..(self.offset >> 3) + n]);
            self.advance(n << 3);
        } else {
            for item in out.iter_mut().take(n) {
                *item = self.read_byte_internal(bit_level);
            }
        }
    }

    pub fn read_cstring(&mut self, n: usize) -> String {
        let b = self.read_bytes(n);
        let end = match b.iter().position(|v| *v == 0) {
            Some(v) => v,
            None => n,
        };
        std::str::from_utf8(&b[..end])
            .expect("should have converted bytes to &str")
            .to_string()
    }

    pub fn read_signed_int(&mut self, n: usize) -> isize {
        let val = int64(&self.buffer[self.offset >> 3 & !3..]);
        let res = (val << (64 - (self.offset & 31) - n) >> (64 - n)) as isize;
        self.advance(n);
        res
    }

    pub fn begin_chunk(&mut self, n: usize) {
        self.chunk_targets.push(self.actual_position() + n);
    }

    pub fn end_chunk(&mut self) {
        let (new_stack, target) = self.chunk_targets.pop();
        self.chunk_targets.0 = new_stack;
        let delta = target as isize - self.actual_position() as isize;

        match delta.cmp(&0) {
            std::cmp::Ordering::Greater => {
                self.skip(delta as usize);
            }
            std::cmp::Ordering::Less => panic!("Someone read beyond a chunk boundary, what a dick"),
            std::cmp::Ordering::Equal => {}
        }

        if target != self.actual_position() {
            panic!(
                "Skipping data failed, expected position {} got {}",
                target,
                self.actual_position()
            );
        }
    }

    pub fn chunk_finished(&mut self) -> bool {
        self.chunk_targets.top() <= self.actual_position()
    }

    pub fn skip(&mut self, n: usize) {
        let buffer_bits = self.bits_in_buffer as isize - self.offset as isize;
        if n as isize > buffer_bits + SLED_BITS as isize {
            let unbuffered_skip_bits = n - buffer_bits as usize;
            let global_offset = self.underlying.stream_position().unwrap()
                + ((unbuffered_skip_bits >> 3) - SLED) as u64;

            self.lazy_position = (global_offset << 3) as usize;

            self.underlying
                .seek(SeekFrom::Start(global_offset))
                .unwrap();

            let bytes = self.underlying.read(&mut self.buffer).unwrap();

            self.offset = unbuffered_skip_bits & SLED_MASK;

            self.bits_in_buffer = (bytes << 3) - SLED_BITS;
            if bytes <= SLED {
                self.bits_in_buffer += SLED_BITS;
            }
        } else {
            self.advance(n);
        }
    }

    fn refill_buffer(&mut self) {
        let src =
            &self.buffer[self.bits_in_buffer >> 3..(self.bits_in_buffer >> 3) + SLED].to_vec();

        self.buffer[0..SLED].copy_from_slice(src);

        self.offset -= self.bits_in_buffer;
        self.lazy_position += self.bits_in_buffer;

        let bytes = self.underlying.read(&mut self.buffer[SLED..]).unwrap();
        self.bits_in_buffer = bytes << 3;

        if self.bits_in_buffer == 0 {
            self.bits_in_buffer += SLED_BITS;
            self.end_reached = true;
        }
    }

    pub fn read_string(&mut self) -> String {
        self.read_string_limited(VALVE_MAX_STRING_LENGTH, false)
    }

    fn read_string_limited(&mut self, limit: usize, end_on_new_line: bool) -> String {
        let mut result = Vec::with_capacity(MIN_STRING_BUFFER_LENGTH);
        for _ in 0..limit {
            let b = self.read_single_byte();
            if b == 0 || (end_on_new_line && b as char == '\n') {
                break;
            }
            result.push(b);
        }
        String::from_utf8(result).unwrap()
    }

    pub fn read_float(&mut self) -> f32 {
        f32::from_bits(self.read_int(32) as u32)
    }

    pub fn read_varint32(&mut self) -> u32 {
        let mut result = 0_u32;
        for i in 0..5 {
            let b = self.read_single_byte() as u32;
            result |= (b & 0x7F) << (7 * i);
            if (b & 0x80) == 0 || (i == 4) {
                break;
            }
        }
        result
    }

    pub fn read_signed_varint32(&mut self) -> i32 {
        let res = self.read_varint32() as i32;
        (res >> 1) ^ -(res & 1)
    }

    pub fn read_ubitint(&mut self) -> usize {
        let res = self.read_int(6);
        match res & (16 | 32) {
            16 => (res & 15) | (self.read_int(4) << 4),
            32 => (res & 15) | (self.read_int(8) << 4),
            48 => (res & 15) | (self.read_int(28) << 4),
            _ => res,
        }
    }

    pub fn read_field_index(&mut self, last_idx: isize, new_way: bool) -> isize {
        if new_way && self.read_bit() {
            return last_idx + 1;
        }

        let mut ret: usize;
        if new_way && self.read_bit() {
            ret = self.read_int(3);
        } else {
            ret = self.read_int(7);
            match ret & (32 | 64) {
                32 => {
                    ret = (ret & !96) | (self.read_int(2) << 5);
                }
                64 => {
                    ret = (ret & !96) | (self.read_int(4) << 5);
                }
                96 => {
                    ret = (ret & !96) | (self.read_int(7) << 5);
                }
                _ => {}
            }
        }

        if ret == 0xfff {
            return -1;
        }

        last_idx + 1 + ret as isize
    }

    pub fn read_bitcoord(&mut self) -> f32 {
        let mut is_neg = false;
        let mut res = 0.0_f32;

        let mut int_val = self.read_int(1);
        let mut fract_val = self.read_int(1);

        if int_val | fract_val != 0 {
            is_neg = self.read_bit();

            if int_val == 1 {
                int_val = self.read_int(14) + 1;
            }

            if fract_val == 1 {
                fract_val = self.read_int(5);
            }

            res = int_val as f32 + (fract_val as f32 * (1.0 / (1 << 5) as f32));
        }

        match is_neg {
            true => -res,
            false => res,
        }
    }

    pub fn read_bitcoordmp(&mut self, is_integ: bool, is_lp: bool) -> f32 {
        let mut res = 0.0_f32;
        let mut is_neg = false;

        let in_bounds = self.read_bit();

        if is_integ {
            if self.read_bit() {
                is_neg = self.read_bit();

                match in_bounds {
                    true => res = (self.read_int(11) + 1) as f32,
                    false => res = (self.read_int(14) + 1) as f32,
                }
            }
        } else {
            let read_int_val = self.read_bit();
            is_neg = self.read_bit();

            let mut int_val = 0_isize;
            if read_int_val {
                match in_bounds {
                    true => int_val = (self.read_int(11) + 1) as isize,
                    false => int_val = (self.read_int(14) + 1) as isize,
                }
            }

            res = int_val as f32
                + match is_lp {
                    true => (self.read_int(3) as f32) * 0.125,
                    false => (self.read_int(5) as f32) * 0.03125,
                };
        }

        match is_neg {
            true => -res,
            false => res,
        }
    }

    pub fn read_bitnormal(&mut self) -> f32 {
        let is_neg = self.read_bit();

        let fract_val = self.read_int(11);

        let res = fract_val as f32 * 0.000976562;

        match is_neg {
            true => -res,
            false => res,
        }
    }

    pub fn read_bitcellcoord(&mut self, bits: usize, is_integ: bool, is_lp: bool) -> f32 {
        match is_integ {
            true => self.read_int(bits) as f32,
            false => match is_lp {
                true => self.read_int(bits) as f32 + (self.read_int(3) as f32 * 0.125),
                false => self.read_int(bits) as f32 + (self.read_int(5) as f32 * 0.03125),
            },
        }
    }
}

fn uint64(b: &[u8]) -> u64 {
    u64::from_le_bytes(b[..8].try_into().unwrap())
}

fn int64(b: &[u8]) -> i64 {
    i64::from_le_bytes(b[..8].try_into().unwrap())
}
