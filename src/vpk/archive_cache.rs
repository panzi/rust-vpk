use std::collections::HashMap;
use std::fs;
use std::io::{Read, SeekFrom, Seek};

use crate::vpk::{Package, Result, BUFFER_SIZE};
use crate::vpk::entry::File;

pub struct ArchiveCache<'a> {
    package: &'a Package,
    archives: HashMap<u16, fs::File>,
}

impl<'a> ArchiveCache<'a> {
    pub fn new(package: &'a Package) -> ArchiveCache<'a> {
        ArchiveCache {
            package,
            archives: HashMap::new(),
        }
    }

    pub fn get(&mut self, index: u16) -> Result<&mut fs::File> {
        if !self.archives.contains_key(&index) {
            let path = self.package.archive_path(index);
            let reader = fs::File::open(path)?;
            self.archives.insert(index, reader);
        }

        Ok(self.archives.get_mut(&index).unwrap())
    }

    pub fn read_file_data(&mut self, file: &File, mut callback: impl FnMut(&[u8]) -> Result<()>) -> Result<()> {
        let reader = self.get(file.archive_index)?;
        callback(&file.preload)?;

        reader.seek(SeekFrom::Start(file.offset as u64))?;

        let mut buf = [0u8; BUFFER_SIZE];
        let mut remain = file.size as usize;
        while remain >= BUFFER_SIZE {
            reader.read_exact(&mut buf)?;
            callback(&buf)?;
            remain -= BUFFER_SIZE;
        }

        if remain > 0 {
            let buf = &mut buf[..remain];
            reader.read_exact(buf)?;
            callback(&buf)?;
        }

        Ok(())
    }
}
