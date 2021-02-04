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

use std::path::{Path, PathBuf};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::collections::HashMap;

use crate::entry;
use crate::entry::{Entry, File};
use crate::result::{Result, Error};
use crate::sort::{Order, sort};
use crate::consts::{VPK_MAGIC, V1_HEADER_SIZE, V2_HEADER_SIZE, ARCHIVE_MD5_SIZE};
use crate::io::*;
use crate::util::*;

pub type Magic = [u8; 4];
pub type Md5 = [u8; 16];

pub struct ArchiveMd5 {
    pub(crate) archive_index: u16,
    pub(crate) offset:        u32,
    pub(crate) size:          u32,
    pub(crate) md5:           Md5,
}

impl ArchiveMd5 {
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
    pub fn md5(&self) -> &Md5 {
        &self.md5
    }
}

pub struct Package {
    pub(crate) dirpath: PathBuf,
    pub(crate) prefix: String,

    pub(crate) version:          u32,
    pub(crate) data_offset:      u32,
    pub(crate) index_size:       u32,
    pub(crate) data_size:        u32,
    pub(crate) archive_md5_size: u32,
    pub(crate) other_md5_size:   u32,
    pub(crate) signature_size:   u32,
    pub(crate) entries: HashMap<String, Entry>,

    // VPK2
    pub(crate) archive_md5s: Vec<ArchiveMd5>,
    pub(crate) index_md5:        Md5,
    pub(crate) archive_md5s_md5: Md5,
    pub(crate) unknown_md5:      Md5,

    pub(crate) public_key: Vec<u8>,
    pub(crate) signature:  Vec<u8>,
}

fn mkpath<'a>(mut entries: &'a mut HashMap<String, Entry>, dirpath: &str) -> Result<&'a mut HashMap<String, Entry>> {
    for (path, item, _) in split_path(dirpath) {
        if !entries.contains_key(item) {
            let dir = Entry::Dir(entry::Dir {
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
                return Err(Error::entry_not_a_dir(path));
            },
        }
    }

    return Ok(entries);
}

pub(crate) fn parse_path(path: impl AsRef<Path>) -> Result<(PathBuf, String)> {
    let path = path.as_ref();
    let dirpath = if let Some(path) = path.parent() {
        path.to_owned()
    } else {
        return Err(Error::other("could not get parent directory").with_path(path));
    };
    let prefix = if let Some(name) = path.file_name() {
        if let Some(name) = name.to_str() {
            if let Some(name) = name.strip_suffix("_dir.vpk") {
                name.to_owned()
            } else {
                return Err(Error::other(format!("Filename does not end in \"_dir.vpk\": {:?}", name)).with_path(path));
            }
        } else {
            return Err(Error::other(format!("Filename contains invalid unicode bytes: {:?}", name)).with_path(path));
        }
    } else {
        return Err(Error::other("could not get file name of path").with_path(path));
    };

    Ok((dirpath, prefix))
}

impl Package {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Package> {
        match fs::File::open(&path) {
            Ok(mut file) => match Self::from_file(&mut file, &path) {
                Ok(package) => Ok(package),
                Err(error) => if error.path.is_none() {
                    Err(error.with_path(path))
                } else {
                    Err(error)
                },
            }
            Err(error) => Err(Error::io_with_path(error, path.as_ref().to_path_buf())),
        }
    }

    fn from_file(file: &mut fs::File, path: impl AsRef<Path>) -> Result<Package> {
        let (dirpath, prefix) = parse_path(&path)?;

        let mut archive_md5s = Vec::new();
        let mut index_md5:        Md5 = [0; 16];
        let mut archive_md5s_md5: Md5 = [0; 16];
        let mut unknown_md5:      Md5 = [0; 16];
        let mut public_key = Vec::new();
        let mut signature  = Vec::new();

        let mut file = std::io::BufReader::new(file);
        let mut magic = [0; 4];
        file.read_exact(&mut magic)?;

        if magic != VPK_MAGIC {
            return Err(Error::illegal_magic(magic).with_path(path));
        }

        let version = read_u32(&mut file)?;

        if version == 0 || version > 2 {
            return Err(Error::unsupported_version(version).with_path(path));
        }

        let index_size = read_u32(&mut file)?;

        let header_size: usize;
        let mut data_size        = 0u32;
        let mut archive_md5_size = 0u32;
        let mut other_md5_size   = 0u32;
        let mut signature_size   = 0u32;

        if version < 2 {
            header_size = V1_HEADER_SIZE;
        } else {
            header_size      = V2_HEADER_SIZE;
            data_size        = read_u32(&mut file)?;
            archive_md5_size = read_u32(&mut file)?;
            other_md5_size   = read_u32(&mut file)?;
            signature_size   = read_u32(&mut file)?;
        };

        let data_offset = header_size as u32 + index_size;

        let mut entries = HashMap::new();
        let mut index   = 0usize;

        // buffer reuse over loops:
        let mut extbuf  = Vec::new();
        let mut dirbuf  = Vec::new();
        let mut namebuf = Vec::new();

        loop {
            let ext = read_str(&mut file, &mut extbuf)?;

            if ext.is_empty() {
                break;
            }

            loop {
                let dirname = read_str(&mut file, &mut dirbuf)?;

                if dirname.is_empty() {
                    break;
                }

                let children = mkpath(&mut entries, &dirname)?;

                loop {
                    let name = read_str(&mut file, &mut namebuf)?;

                    if name.is_empty() {
                        break;
                    }

                    let mut name = name.to_owned();
                    name.push('.');
                    name.push_str(&ext);

                    let entry = read_file(&mut file, index, data_offset)?;
                    index += 1;

                    if children.contains_key(&name) {
                        eprintln!("*** warning: file occured more than once: {:?}",
                            format!("{}/{}.{}", dirname, name, ext));
                    }

                    children.insert(name, Entry::File(entry));
                }
            }
        }

        if version > 1 {
            if let Err(error) = file.seek(SeekFrom::Current(data_size as i64)) {
                return Err(Error::io_with_path(error, path));
            }

            let mut remaining = archive_md5_size as usize;
            while remaining >= ARCHIVE_MD5_SIZE {
                let archive_index = read_u32(&mut file)?;
                let offset        = read_u32(&mut file)?;
                let size          = read_u32(&mut file)?;
                let mut md5 = [0; 16];

                file.read_exact(&mut md5)?;

                remaining -= ARCHIVE_MD5_SIZE;

                if archive_index > std::u16::MAX as u32 {
                    eprintln!("*** warning: archive_index in MD5 section too big: {} > {}",
                        archive_index, std::u16::MAX);
                    continue;
                }

                archive_md5s.push(ArchiveMd5 {
                    archive_index: archive_index as u16,
                    offset,
                    size,
                    md5,
                });
            }

            archive_md5s.sort_by(|a, b| {
                let cmp = a.archive_index.cmp(&b.archive_index);
                if cmp == std::cmp::Ordering::Equal { a.offset.cmp(&b.offset) } else { cmp }
            });

            if remaining > 0 {
                eprintln!("*** warning: {} bytes left after archive MD5 section", remaining);
                file.seek(SeekFrom::Current(remaining as i64))?;
            }

            let mut remaining = other_md5_size;
            if remaining >= 16 {
                file.read_exact(&mut index_md5)?;
                remaining -= 16;

                if remaining >= 16 {
                    file.read_exact(&mut archive_md5s_md5)?;
                    remaining -= 16;

                    if remaining >= 16 {
                        file.read_exact(&mut unknown_md5)?;
                        remaining -= 16;
                    }
                }
            }

            if remaining > 0 {
                eprintln!("*** warning: {} bytes left after the other MD5 section", remaining);
                file.seek(SeekFrom::Current(remaining as i64))?;
            }

            let mut remaining = signature_size;
            if remaining >= 4 {
                let pubkey_size = read_u32(&mut file)?;
                remaining -= 4;
                public_key.resize(pubkey_size as usize, 0);
                file.read_exact(&mut public_key)?;
                remaining -= pubkey_size;

                if remaining >= 4 {
                    let sig_size = read_u32(&mut file)?;
                    remaining -= 4;
                    signature.resize(sig_size as usize, 0);
                    file.read_exact(&mut signature)?;
                    remaining -= sig_size;
                }
            }

            if remaining > 0 {
                eprintln!("*** warning: {} bytes left after the signature section", remaining);
                file.seek(SeekFrom::Current(remaining as i64))?;
            }
        }

        Ok(Package {
            dirpath,
            prefix,
            version,
            data_offset,
            index_size,
            data_size,
            archive_md5_size,
            other_md5_size,
            signature_size,
            entries,
            archive_md5s,
            index_md5,
            archive_md5s_md5,
            unknown_md5,
            public_key,
            signature,
        })
    }

    #[inline]
    pub fn version(&self) -> u32 {
        self.version
    }

    #[inline]
    pub fn data_offset(&self) -> u32 {
        self.data_offset
    }

    #[inline]
    pub fn index_size(&self) -> u32 {
        self.index_size
    }

    #[inline]
    pub fn data_size(&self) -> u32 {
        self.data_size
    }

    #[inline]
    pub fn archive_md5_size(&self) -> u32 {
        self.archive_md5_size
    }

    #[inline]
    pub fn other_md5_size(&self) -> u32 {
        self.other_md5_size
    }

    #[inline]
    pub fn signature_size(&self) -> u32 {
        self.signature_size
    }

    #[inline]
    pub fn root(&self) -> &HashMap<String, Entry> {
        &self.entries
    }

    #[inline]
    pub fn archive_md5s(&self) -> &Vec<ArchiveMd5> {
        &self.archive_md5s
    }

    #[inline]
    pub fn index_md5(&self) -> Option<&Md5> {
        if self.other_md5_size >= ARCHIVE_MD5_SIZE as u32 {
            Some(&self.index_md5)
        } else {
            None
        }
    }

    #[inline]
    pub fn archive_md5s_md5(&self) -> Option<&Md5> {
        if self.other_md5_size >= ARCHIVE_MD5_SIZE as u32 * 2 {
            Some(&self.archive_md5s_md5)
        } else {
            None
        }
    }

    #[inline]
    pub fn unknown_md5(&self) -> Option<&Md5> {
        if self.other_md5_size >= ARCHIVE_MD5_SIZE as u32 * 3 {
            Some(&self.unknown_md5)
        } else {
            None
        }
    }

    #[inline]
    pub fn public_key(&self) -> Option<&Vec<u8>> {
        if self.signature_size >= 4 {
            Some(&self.public_key)
        } else {
            None
        }
    }

    #[inline]
    pub fn signature(&self) -> Option<&Vec<u8>> {
        if self.signature_size >= 4 + self.public_key.len() as u32 + 4 {
            Some(&self.signature)
        } else {
            None
        }
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

    pub fn recursive_file_list(&self, order: &Order) -> Vec<(String, &File)> {
        let mut list = Vec::new();
        let mut pathbuf = String::new();
        recursive_file_list(&self.entries, &mut pathbuf, &mut list);
        sort(&mut list, order);

        list
    }

    pub fn recursive_file_list_from(&self, paths: &[impl AsRef<str>], order: &Order) -> Result<Vec<(String, &File)>> {
        let mut list = Vec::new();
        let mut pathbuf = String::new();

        for path in paths {
            let path = path.as_ref().trim_matches('/');
            let entry = self.get(path);
            match entry {
                None => {
                    return Err(Error::no_such_entry(path));
                },
                Some(Entry::Dir(dir)) => {
                    pathbuf.clear();
                    pathbuf.push_str(path.as_ref());
                    pathbuf.push('/');
                    recursive_file_list(&dir.children, &mut pathbuf, &mut list);
                },
                Some(Entry::File(file)) => {
                    list.push((path.to_owned(), file));
                }
            }
        }

        sort(&mut list, order);

        Ok(list)
    }

    pub fn archive_path(&self, archive_index: u16) -> PathBuf {
        archive_path(&self.dirpath, &self.prefix, archive_index)
    }
}

fn recursive_file_list<'a>(entries: &'a HashMap<String, Entry>, pathbuf: &mut String, list: &mut Vec<(String, &'a File)>) {
    for (name, entry) in entries {
        let len = pathbuf.len();
        pathbuf.push_str(name);
        match entry {
            Entry::Dir(dir) => {
                pathbuf.push('/');
                recursive_file_list(&dir.children, pathbuf, list);
            },
            Entry::File(file) => {
                list.push((pathbuf.clone(), file));
            }
        }
        pathbuf.truncate(len);
    }
}
