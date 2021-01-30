use std::collections::{HashMap, HashSet};
use std::path::{Path};
use std::fs::{self, read_dir};
use std::io::{Read, Write, Seek, SeekFrom, BufWriter};

use crc::{crc32, Hasher32};

use crate::vpk::{Package, Result, Error, DIR_INDEX, BUFFER_SIZE, VPK_MAGIC};
use crate::vpk::package::{parse_path};
use crate::vpk::entry::{Entry, File, Dir};
use crate::vpk::io::{write_u32, write_str, write_file, transfer};
use crate::vpk::archive_cache::ArchiveCache;
use crate::vpk::util::{split_path};

pub enum ArchiveOptions {
    ArchiveFromDirName,
    MaxArchiveSize(u32),
}

struct Gather {
    digest: crc32::Digest,
    max_inline_size: u16,
    buf: [u8; BUFFER_SIZE],
    exts: HashSet<String>,
}

impl Gather {
    #[inline]
    fn new(max_inline_size: u16) -> Self {
        Gather {
            digest: crc32::Digest::new(crc32::IEEE),
            max_inline_size,
            buf: [0; BUFFER_SIZE],
            exts: HashSet::new(),
        }
    }

    fn gather_files(&mut self, entries: &mut HashMap<String, Entry>, archive_index: u16, dirpath: &Path, root: bool) -> Result<usize> {
        let mut index_size = 0;
        for dirent in read_dir(dirpath)? {
            let dirent = dirent?;
            let os_name = dirent.file_name();
            if let Some(name) = os_name.to_str() {
                if dirent.file_type()?.is_dir() {
                    let mut dir = Dir {
                        children: HashMap::new()
                    };
                    if !root {
                        index_size += 1; // for '/'
                    }
                    index_size += name.len() + 1;
                    index_size += self.gather_files(&mut dir.children, archive_index, &dirent.path(), false)?;
                    index_size += 1; // terminating NIL
                    entries.insert(name.to_owned(), Entry::Dir(dir));
                } else if root {
                    return Err(Error::Other(format!("All files must be in sub-directories: {:?}", dirent.path())));
                } else if let Some(dot_index) = name.rfind('.') {
                    if dot_index == 0 || dot_index + 1 == name.len() {
                        return Err(Error::Other(format!("Filenames must be of format \"NAME.EXT\": {:?}", dirent.path())));
                    }

                    let ext = &name[dot_index + 1..];
                    if !self.exts.contains(ext) {
                        index_size += ext.len() + 1 + 1;
                        self.exts.insert(ext.to_owned());
                    }

                    let mut reader = fs::File::open(dirent.path())?;
                    let meta = reader.metadata()?;
                    let size = meta.len();

                    if size > std::i32::MAX as u64 {
                        return Err(Error::Other(format!("File too big {} > {}: {:?}", size, std::i32::MAX, dirent.path())));
                    }

                    let mut size = size as u32;
                    let mut preload = Vec::new();
                    let inline_size: u16;

                    self.digest.reset();
                    if size <= self.max_inline_size as u32 {
                        inline_size = size as u16;
                        size = 0;
                        preload.resize(inline_size as usize, 0);
                        reader.read_exact(&mut preload)?;
                        self.digest.write(&preload);
                    } else {
                        let mut remain = size as usize;
                        inline_size = 0;
                        while remain >= BUFFER_SIZE {
                            reader.read_exact(&mut self.buf)?;
                            self.digest.write(&mut self.buf);
                            remain -= BUFFER_SIZE;
                        }
                        if remain > 0 {
                            let buf = &mut self.buf[..remain];
                            reader.read_exact(buf)?;
                            self.digest.write(buf);
                        }
                    }
                    let crc32 = self.digest.sum32();

                    index_size += dot_index + 1 + 4 + 2 + 2 + 4 + 4 + 2 + inline_size as usize;
                    let file = File {
                        index: 0, // not used when writing
                        crc32,
                        inline_size,
                        archive_index,
                        offset: 0, // to be determined
                        size,
                        preload,
                    };
                    entries.insert(name.to_owned(), Entry::File(file));
                } else {
                    return Err(Error::Other(format!("Filenames must be of format \"NAME.EXT\": {:?}", dirent.path())));
                }
            } else {
                return Err(Error::Other(format!("Cannot handle filename: {:?}", dirent.path())));
            }
        }

        Ok(index_size)
    }
}

fn recursive_file_list<'a>(entries: &'a mut HashMap<String, Entry>, pathbuf: &mut String, list: &mut Vec<(String, &'a mut File)>) {
    for (name, entry) in entries.iter_mut() {
        let len = pathbuf.len();
        pathbuf.push_str(name);
        match entry {
            Entry::Dir(dir) => {
                pathbuf.push('/');
                recursive_file_list(&mut dir.children, pathbuf, list);
            },
            Entry::File(file) => {
                list.push((pathbuf.clone(), file));
            }
        }
        pathbuf.truncate(len);
    }
}

// TODO: more grouping/file order options?
pub fn pack_v1(dirvpk_path: impl AsRef<Path>, indir: impl AsRef<Path>, arch_opts: ArchiveOptions, max_inline_size: u16, alignment: usize, verbose: bool) -> Result<Package> {
    let (dirpath, prefix) = parse_path(&dirvpk_path)?;

    let mut entries = HashMap::new();
    let mut gather = Gather::new(max_inline_size);
    let mut index_size = 0;
    let mut list = Vec::new();

    match arch_opts {
        ArchiveOptions::ArchiveFromDirName => {
            for dirent in read_dir(indir.as_ref())? {
                let dirent = dirent?;
                if dirent.file_type()?.is_dir() {
                    if let Some(name) = dirent.file_name().to_str() {
                        if name.eq("dir") {
                            index_size += gather.gather_files(&mut entries, DIR_INDEX, &dirent.path(), true)?;
                        } else if name.len() != 3 {
                            eprintln!("WRANING: directory name is neither 3 digit a number nor \"dir\": {}", name);
                        } else if let Ok(archive_index) = name.parse::<u16>() {
                            if archive_index < 0x7FFF {
                                index_size += gather.gather_files(&mut entries, archive_index, &dirent.path(), true)?;
                            } else {
                                eprintln!("WRANING: directory name represents a too larg number for an archive index: {}", name);
                            }
                        } else {
                            eprintln!("WRANING: directory name is neither 3 digit a number nor \"dir\": {}", name);
                        }
                    }
                }
            }
        },
        ArchiveOptions::MaxArchiveSize(_) => {
            index_size += gather.gather_files(&mut entries, DIR_INDEX, indir.as_ref(), true)?;
        }
    }

    index_size += 1; // ext terminator
    let v1_header_size = 4 + 4 + 4;

    if index_size > std::i32::MAX as usize {
        return Err(Error::Other(format!("index too large: {} > {}", index_size, std::i32::MAX)));
    }

    let dir_size = v1_header_size + index_size;

    let mut pathbuf = String::new();
    recursive_file_list(&mut entries, &mut pathbuf, &mut list);
    list.sort_by(|a, b| a.0.cmp(&b.0));

    if let ArchiveOptions::MaxArchiveSize(max_size) = arch_opts {
        // distribute files to archives

        let mut archive_index = DIR_INDEX;
        let mut archive_size = dir_size;

        for (_, file) in list.iter_mut() {
            let remainder = archive_size % alignment;
            if remainder != 0 {
                archive_size += alignment - remainder;
            }

            let new_archive_size = archive_size + file.size as usize;
            file.offset = archive_size as u32;
            if new_archive_size > max_size as usize {
                if archive_index == DIR_INDEX {
                    archive_index = 0;
                } else if archive_index == 999 {
                    return Err(Error::Other(format!("too many archives")));
                } else {
                    archive_index += 1;
                }
                archive_size = file.size as usize;
            } else {
                archive_size = new_archive_size;
            }
            file.archive_index = archive_index;
        }
    }

    // group files by extension and dir
    let mut extmap: HashMap<&str, HashMap<&str, Vec<(&str, &File)>>> =
        HashMap::with_capacity(gather.exts.len());

    for ext in &gather.exts {
        extmap.insert(ext, HashMap::new());
    }

    for (path, file) in &list {
        // I know that there is a '.' in the file name, I checked above.
        let dot_index = path.rfind('.').unwrap();

        // I know that there is a '/' in the path, because I checked above.
        let slash_index = path[..dot_index].rfind('/').unwrap();

        let extname  = &path[dot_index + 1..];
        let dirname  = &path[..slash_index];
        let filename = &path[slash_index + 1..];

        let dirmap = extmap.get_mut(extname).unwrap();

        if !dirmap.contains_key(dirname) {
            dirmap.insert(dirname, Vec::new());
        }

        let filelist = dirmap.get_mut(dirname).unwrap();
        filelist.push((filename, file));
    }

    if verbose {
        println!("writing index: {:?}", dirvpk_path.as_ref());
    }

    let mut dirwriter = BufWriter::new(fs::File::create(dirvpk_path)?);

    dirwriter.write_all(&VPK_MAGIC)?;
    
    write_u32(&mut dirwriter, 1)?;
    write_u32(&mut dirwriter, index_size as u32)?;

    let mut exts: Vec<&str> = extmap.keys().map(|s| s.as_ref()).collect();
    exts.sort();

    for ext in &exts {
        write_str(&mut dirwriter, ext)?;

        let dirmap = extmap.get(ext).unwrap();
        let mut dirs: Vec<&str> = dirmap.keys().map(|s| s.as_ref()).collect();
        dirs.sort();

        for dir in &dirs {
            write_str(&mut dirwriter, dir)?;

            let filelist = dirmap.get(dir).unwrap();
            for (full_name, file) in filelist {
                let dot_index = full_name.rfind('.').unwrap();
                let name = &full_name[..dot_index];

                write_str(&mut dirwriter, name)?;
                write_file(&mut dirwriter, file)?;
            }
        }
        dirwriter.write_all(&[0])?;
    }
    dirwriter.write_all(&[0])?;

    drop(dirwriter);

    let mut archs = ArchiveCache::for_writing(dirpath.to_path_buf(), prefix.to_string());

    for (vpk_path, file) in &list {
        if verbose {
            println!("writing data: {:?}", vpk_path);
        }
    
        let writer = archs.get(file.archive_index)?;

        writer.seek(SeekFrom::Start(file.offset as u64))?;

        let mut fs_path = indir.as_ref().to_path_buf();

        if let ArchiveOptions::MaxArchiveSize(_) = arch_opts {
            if file.archive_index == DIR_INDEX {
                fs_path.push("dir");
            } else {
                fs_path.push(format!("{:03}", file.archive_index));
            }
        }

        for (_, item, _) in split_path(vpk_path) {
            fs_path.push(item);
        }

        if file.size > 0 {
            let mut reader = fs::File::open(fs_path)?;

            if file.inline_size > 0 {
                reader.seek(SeekFrom::Start(file.inline_size as u64))?;
            }

            transfer(&mut reader, writer, file.size as usize)?;
        }
    }

    if verbose {
        println!("done");
    }

    Ok(Package {
        dirpath,
        prefix,
        version: 1,
        data_offset: dir_size as u32,
        footer_offset: 0,
        footer_size: 0,
        entries,
    })
}
