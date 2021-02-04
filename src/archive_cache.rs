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
use std::fs;
use std::io::{Read, Write, SeekFrom, Seek};
use std::path::{PathBuf};

use crate::consts::{BUFFER_SIZE, DIR_INDEX};
use crate::result::{Result, Error};
use crate::entry::File;
use crate::util::{archive_path};
use crate::io::transfer;

pub struct ArchiveCache {
    dirpath: PathBuf,
    prefix: String,
    dir_open_options: fs::OpenOptions,
    open_options: fs::OpenOptions,
    archives: HashMap<u16, fs::File>,
}

impl ArchiveCache {
    pub fn dirpath(&self) -> &PathBuf {
        &self.dirpath
    }

    pub fn prefix(&self) -> &String {
        &self.prefix
    }

    pub fn dir_open_options(&self) -> &fs::OpenOptions {
        &self.dir_open_options
    }

    pub fn open_options(&self) -> &fs::OpenOptions {
        &self.open_options
    }

    pub fn archives(&self) -> &HashMap<u16, fs::File> {
        &self.archives
    }

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
                self.dir_open_options.open(&path)
            } else {
                self.open_options.open(&path)
            };
            match reader {
                Ok(reader) => {
                    self.archives.insert(index, reader);
                },
                Err(error) => {
                    return Err(Error::io_with_path(error, path));
                }
            }
        }

        Ok(self.archives.get_mut(&index).unwrap())
    }

    #[inline]
    pub fn archive_path(&self, index: u16) -> PathBuf {
        archive_path(&self.dirpath, &self.prefix, index)
    }

    pub fn read_file_data(&mut self, file: &File, mut callback: impl FnMut(&[u8]) -> Result<()>) -> Result<()> {
        callback(&file.preload)?;

        if file.size > 0 {
            let archive_index = file.archive_index;
            let reader = self.get(archive_index)?;

            if let Err(error) = reader.seek(SeekFrom::Start(file.offset as u64)) {
                return Err(Error::io_with_path(error, self.archive_path(archive_index)));
            }

            let mut buf = [0u8; BUFFER_SIZE];
            let mut remain = file.size as usize;
            while remain >= BUFFER_SIZE {
                if let Err(error) = reader.read_exact(&mut buf) {
                    return Err(Error::io_with_path(error, self.archive_path(archive_index)));
                }
                callback(&buf)?;
                remain -= BUFFER_SIZE;
            }

            if remain > 0 {
                let buf = &mut buf[..remain];
                if let Err(error) = reader.read_exact(buf) {
                    return Err(Error::io_with_path(error, self.archive_path(archive_index)));
                }
                callback(&buf)?;
            }
        }

        Ok(())
    }

    pub fn transfer(&mut self, file: &File, writer: &mut fs::File) -> Result<()> {
        writer.write_all(&file.preload)?;
        
        if file.size > 0 {
            let archive_index = file.archive_index;
            let reader = self.get(archive_index)?;

            if let Err(error) = reader.seek(SeekFrom::Start(file.offset as u64)) {
                return Err(Error::io_with_path(error, self.archive_path(archive_index)));
            }

            transfer(reader, writer, file.size as usize)?;
        }

        Ok(())
    }
}
