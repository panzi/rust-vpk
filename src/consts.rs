pub const VPK_MAGIC: crate::package::Magic = [0x34, 0x12, 0xAA, 0x55];

pub const DIR_INDEX:  u16 = 0x7FFF;
pub const TERMINATOR: u16 = 0xFFFF;
pub const BUFFER_SIZE: usize = 8 * 1024;
pub const DEFAULT_MAX_INLINE_SIZE: u16 = 8 * 1024;
