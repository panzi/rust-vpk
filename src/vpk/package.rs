use std::path::{Path, PathBuf};
use std::fs;
use std::io::{Read};
use std::collections::HashMap;

use crate::vpk::entry::{Entry, File};
use crate::vpk::{self, Result, Error};
use crate::vpk::sort::{Order, sort};
use crate::vpk::io::*;
use crate::vpk::util::*;

pub struct Package {
    pub(crate) dirpath: PathBuf,
    pub(crate) prefix: String,

    pub(crate) version: u32,
    pub(crate) data_offset: u32,
    pub(crate) footer_offset: u32,
    pub(crate) footer_size: u32,
    pub(crate) entries: HashMap<String, Entry>,
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

pub(crate) fn parse_path(path: impl AsRef<Path>) -> Result<(PathBuf, String)> {
    let path = path.as_ref();
    let dirpath = if let Some(path) = path.parent() {
        path.to_owned()
    } else {
        return Err(Error::Other(format!("could not get parent directory of: {:?}", path)));
    };
    let prefix = if let Some(name) = path.file_name() {
        if let Some(name) = name.to_str() {
            if let Some(name) = name.strip_suffix("_dir.vpk") {
                name.to_owned()
            } else {
                return Err(Error::Other(format!("Filename does not end in \"_dir.vpk\": {:?}", name)));
            }
        } else {
            return Err(Error::Other(format!("Filename contains invalid unicode bytes: {:?}", name)));
        }
    } else {
        return Err(Error::Other(format!("could not get file name of: {:?}", path)));
    };

    Ok((dirpath, prefix))
}

impl Package {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Package> {
        match fs::File::open(&path) {
            Ok(mut file) => match Self::from_file(&mut file, &path) {
                Ok(package) => Ok(package),
                Err(Error::IO(error)) => Err(Error::IOWithPath(error, path.as_ref().to_path_buf())),
                Err(other) => Err(other),
            }
            Err(error) => Err(Error::IOWithPath(error, path.as_ref().to_path_buf())),
        }
    }

    fn from_file(file: &mut fs::File, path: impl AsRef<Path>) -> Result<Package> {
        let (dirpath, prefix) = parse_path(&path)?;

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

        let header_size: usize;
        let (footer_offset, footer_size) = if version < 2 {
            header_size = 4 * 3;
            (0u32, 0u32)
        } else {
            header_size = 4 * 3 + 4 * 4;
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

        Ok(Package {
            dirpath,
            prefix,
            version,
            data_offset,
            footer_offset,
            footer_size,
            entries,
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
    pub fn footer_offset(&self) -> u32 {
        self.footer_offset
    }

    #[inline]
    pub fn footer_size(&self) -> u32 {
        self.footer_size
    }

    #[inline]
    pub fn root(&self) -> &HashMap<String, Entry> {
        &self.entries
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
                    return Err(Error::NoSuchEntry(path.to_owned()));
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
