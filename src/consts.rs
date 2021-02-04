// This file is part of rust-vpk.
//
// rust-vpk is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// rust-vpk is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with rust-vpk.  If not, see <https://www.gnu.org/licenses/>.

use crate::package::{Magic};

pub const VPK_MAGIC: Magic = [0x34, 0x12, 0xAA, 0x55];

pub const DIR_INDEX:  u16 = 0x7FFF;
pub const TERMINATOR: u16 = 0xFFFF;
pub const BUFFER_SIZE: usize = 1024 * 1024;
pub const DEFAULT_MAX_INLINE_SIZE: u16 = 8 * 1024;
pub const DEFAULT_MD5_CHUNK_SIZE: u32 = 1024 * 1024;

pub const V1_HEADER_SIZE: usize = 4 * 3;
pub const V2_HEADER_SIZE: usize = 4 * 3 + 4 * 4;

pub const ARCHIVE_MD5_SIZE: usize = 4 * 3 + 16;
