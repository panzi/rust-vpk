use std::path::Path;
use std::fs;
use std::io;
use std::io::prelude::*;
use std::ffi::OsString;
use std::collections::HashMap;

use crate::vpk::entry::Entry;
use crate::vpk::{Result, Error};
use crate::vpk;
use crate::vpk::io::*;
use crate::vpk::util::*;

pub struct Archive {
    pub path: OsString,
    pub version: u32,
    pub data_offset: u32,
    pub footer_offset: u32,
    pub footer_size: u32,
    pub entries: HashMap<String, Entry>,
}

fn mkpath<'a>(mut entries: &'a mut HashMap<String, Entry>, dirpath: &str) -> Result<&'a mut HashMap<String, Entry>> {
    for (path, item, _) in split_path(dirpath) {
        if !entries.contains_key(item) {
            let dir = Entry::Dir(vpk::entry::Dir {
                children: HashMap::new(),
            });
            entries.insert(item.to_owned(), dir);
        }

        let entry = entries.get_mut(item).unwrap();
        match entry {
            Entry::Dir(dir) => {
                entries = &mut dir.children;
            },
            Entry::File(_) => {
                return Err(Error::EntryNotADir(path.to_owned()));
            },
        }
    }

    return Ok(entries);
}

impl Archive {
    pub fn new(path: impl AsRef<Path>, version: u32) -> Self {
        Archive {
            path: path.as_ref().as_os_str().to_owned(),
            version,
            data_offset: 0,
            footer_offset: 0,
            footer_size: 0,
            entries: HashMap::new(),
        }
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Archive> {
        let mut file = fs::File::open(&path)?;
        Self::from_file(&mut file, path)
    }

    pub fn from_file(file: &mut fs::File, path: impl AsRef<Path>) -> Result<Archive> {
        let mut file = std::io::BufReader::new(file);
        let mut magic = [0; 4];
        file.read_exact(&mut magic)?;

        if magic != vpk::VPK_MAGIC {
            return Err(Error::IllegalMagic(magic));
        }

        let version = read_u32(&mut file)?;

        if version == 0 || version > 2 {
            return Err(Error::UnsupportedVersion(version));
        }

        let index_size = read_u32(&mut file)?;

        let (footer_offset, footer_size) = if version < 2 {
            (0u32, 0u32)
        } else {
            let footer_offset = read_u32(&mut file)?;
            let expect_0      = read_u32(&mut file)?;
            let footer_size   = read_u32(&mut file)?;
            let expect_48     = read_u32(&mut file)?;

            if expect_0 != 0 {
                // unknown, usually always 0
            }

            if expect_48 != 48 {
                // unknown, usually always 48
            }

            (footer_offset, footer_size)
        };

        let header_size = file.seek(io::SeekFrom::Current(0))?;

        if header_size > u32::MAX as u64 {
            // should not happen
            return Err(Error::Other(format!("offset too big: {}", header_size)));
        }
        let data_offset = header_size as u32 + index_size;

        let mut entries = HashMap::new();
        let mut index = 0usize;

        loop {
            let ext = read_str(&mut file)?;

            if ext.is_empty() {
                break;
            }

            loop {
                let dirname = read_str(&mut file)?;

                if dirname.is_empty() {
                    break;
                }

                let children = mkpath(&mut entries, &dirname)?;

                loop {
                    let mut name = read_str(&mut file)?;

                    if name.is_empty() {
                        break;
                    }

                    name.push('.');
                    name.push_str(&ext);

                    let entry = read_file(&mut file, index, data_offset)?;
                    index += 1;

                    if children.contains_key(&name) {
                        writeln!(std::io::stderr(), "*** warning: file occured more than once: {:?}", format!("{}/{}", dirname, name))?;
                    }

                    children.insert(name, Entry::File(entry));
                }
            }
        }

        Ok(Archive {
            path: path.as_ref().as_os_str().to_owned(),
            version,
            data_offset,
            footer_offset,
            footer_size,
            entries,
        })
    }

    pub fn get<'a>(&'a self, path: &str) -> Option<&'a Entry> {
        let mut entries = &self.entries;
        for (_, item, is_last) in split_path(path) {
            if let Some(entry) = entries.get(item) {
                if is_last {
                    return Some(entry);
                }

                if let Entry::Dir(dir) = entry {
                    entries = &dir.children;
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }

        return None;
    }

    pub fn root(&self) -> &HashMap<String, Entry> {
        &self.entries
    }

    pub fn get_mut<'a>(&'a mut self, path: &str) -> Option<&'a mut Entry> {
        let mut entries = &mut self.entries;
        for (_, item, is_last) in split_path(path) {
            if let Some(entry) = entries.get_mut(item) {
                if is_last {
                    return Some(entry);
                }

                if let Entry::Dir(dir) = entry {
                    entries = &mut dir.children;
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }

        return None;
    }
}
