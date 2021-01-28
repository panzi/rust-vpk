use std::collections::HashMap;
use std::fs;
use std::io::{Read, SeekFrom, Seek};
use std::path::{PathBuf};

use crate::vpk::{Result, BUFFER_SIZE, DIR_INDEX};
use crate::vpk::entry::File;
use crate::vpk::util::{archive_path};

pub struct ArchiveCache {
    dirpath: PathBuf,
    prefix: String,
    dir_open_options: fs::OpenOptions,
    open_options: fs::OpenOptions,
    archives: HashMap<u16, fs::File>,
}

impl ArchiveCache {
    pub fn new(dirpath: PathBuf, prefix: String, dir_open_options: fs::OpenOptions, open_options: fs::OpenOptions) -> ArchiveCache {
        ArchiveCache {
            dirpath,
            prefix,
            dir_open_options,
            open_options,
            archives: HashMap::new(),
        }
    }

    pub fn for_reading(dirpath: PathBuf, prefix: String) -> Self {
        let mut dir_opts = fs::OpenOptions::new();
        dir_opts.read(true);

        let mut opts = fs::OpenOptions::new();
        opts.read(true);

        ArchiveCache::new(dirpath, prefix, dir_opts, opts)
    }

    /// Assumes that the index in *_dir.vpk is already written.
    pub fn for_writing(dirpath: PathBuf, prefix: String) -> Self {
        let mut dir_opts = fs::OpenOptions::new();
        dir_opts.write(true).create_new(false).truncate(false);

        let mut opts = fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);

        ArchiveCache::new(dirpath, prefix, dir_opts, opts)
    }

    pub fn get(&mut self, index: u16) -> Result<&mut fs::File> {
        if !self.archives.contains_key(&index) {
            let path = archive_path(&self.dirpath, &self.prefix, index);
            let reader = if index == DIR_INDEX {
                self.dir_open_options.open(path)?
            } else {
                self.open_options.open(path)?
            };
            self.archives.insert(index, reader);
        }

        Ok(self.archives.get_mut(&index).unwrap())
    }

    pub fn read_file_data(&mut self, file: &File, mut callback: impl FnMut(&[u8]) -> Result<()>) -> Result<()> {
        callback(&file.preload)?;

        if file.size > 0 {
            let reader = self.get(file.archive_index)?;

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
        }

        Ok(())
    }
}
