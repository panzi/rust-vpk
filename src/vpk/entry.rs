use std::collections::HashMap;

pub struct File {
    pub index: usize,
    pub crc32: u32,
    pub inline_size: u16,
    pub archive_index: u16,
    pub offset: u32,
    pub size: u32,
    pub preload: Vec<u8>,
}

pub struct Dir {
    pub children: HashMap<String, Entry>,
}

pub enum EntryType {
    File,
    Dir,
}

pub enum Entry {
    File(File),
    Dir(Dir),
}

impl Entry {
    fn entry_type(&self) -> EntryType {
        match self {
            Entry::File(_) => EntryType::File,
            Entry::Dir(_)  => EntryType::Dir,
        }
    }
}

