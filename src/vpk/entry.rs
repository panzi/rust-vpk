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

