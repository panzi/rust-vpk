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

use std::collections::HashMap;

pub struct File {
    pub(crate) index: usize,
    pub(crate) crc32: u32,
    pub(crate) inline_size: u16,
    pub(crate) archive_index: u16,
    pub(crate) offset: u32,
    pub(crate) size: u32,
    pub(crate) preload: Vec<u8>,
}

pub struct Dir {
    pub(crate) children: HashMap<String, Entry>,
}

pub enum Entry {
    File(File),
    Dir(Dir),
}

impl File {
    #[inline]
    pub fn index(&self) -> usize {
        self.index
    }

    #[inline]
    pub fn crc32(&self) -> u32 {
        self.crc32
    }

    #[inline]
    pub fn inline_size(&self) -> u16 {
        self.inline_size
    }

    #[inline]
    pub fn archive_index(&self) -> u16 {
        self.archive_index
    }

    #[inline]
    pub fn offset(&self) -> u32 {
        self.offset
    }

    #[inline]
    pub fn size(&self) -> u32 {
        self.size
    }

    #[inline]
    pub fn preload(&self) -> &[u8] {
        &self.preload
    }
}

impl Dir {
    #[inline]
    pub fn children(&self) -> &HashMap<String, Entry> {
        &self.children
    }
}

impl Entry {
    pub fn is_file(&self) -> bool {
        match self {
            Entry::File(_) => true,
            Entry::Dir(_)  => false,
        }
    }

    pub fn is_dir(&self) -> bool {
        match self {
            Entry::File(_) => false,
            Entry::Dir(_)  => true,
        }
    }
}

