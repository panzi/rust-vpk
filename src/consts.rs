use crate::package::{Magic};

pub const VPK_MAGIC: Magic = [0x34, 0x12, 0xAA, 0x55];

pub const DIR_INDEX:  u16 = 0x7FFF;
pub const TERMINATOR: u16 = 0xFFFF;
pub const BUFFER_SIZE: usize = 8 * 1024;
pub const DEFAULT_MAX_INLINE_SIZE: u16 = 8 * 1024;

pub const V1_HEADER_SIZE: usize = 4 * 3;
pub const V2_HEADER_SIZE: usize = 4 * 3 + 4 * 4;

pub const ARCHIVE_MD5_SIZE: usize = 4 * 3 + 16;
