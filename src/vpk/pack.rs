use std::collections::{HashMap, HashSet};
use std::path::{Path};
use std::fs::{self, read_dir};
use std::io::{Read, Write, Seek, SeekFrom, BufWriter};

use crc::{crc32, Hasher32};

use crate::vpk::{Package, Result, Error, DIR_INDEX, BUFFER_SIZE, VPK_MAGIC};
use crate::vpk::package::{parse_path};
use crate::vpk::entry::{Entry, File, Dir};
use crate::vpk::io::{write_u32, write_str, write_file, transfer};
use crate::vpk::util::{split_path, archive_path};

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

struct Item<'a> {
    path: String,
    dot_index:   usize,
    slash_index: usize,
    file: &'a mut File,
}

impl Item<'_> {
    #[inline]
    fn ext(&self) -> &str {
        &self.path[self.dot_index + 1..]
    }

    #[inline]
    fn name(&self) -> &str {
        &self.path[self.slash_index + 1..self.dot_index]
    }

    #[inline]
    fn dir(&self) -> &str {
        &self.path[..self.slash_index]
    }
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

    fn gather_files(&mut self, entries: &mut HashMap<String, Entry>, archive_index: u16, dirpath: &Path, root: bool, verbose: bool) -> Result<()> {
        let dirents = match read_dir(dirpath) {
            Ok(dirents) => dirents,
            Err(error) => return Err(Error::IOWithPath(error, dirpath.to_path_buf())),
        };
        for dirent in dirents {
            let dirent = match dirent {
                Ok(dirent) => dirent,
                Err(error) => return Err(Error::IOWithPath(error, dirpath.to_path_buf())),
            };
            if verbose {
                println!("reading {:?}", dirent.path());
            }
            let os_name = dirent.file_name();
            if let Some(name) = os_name.to_str() {
                let file_type = match dirent.file_type() {
                    Ok(file_type) => file_type,
                    Err(error) => return Err(Error::IOWithPath(error, dirent.path())),
                };
                if file_type.is_dir() {
                    let mut dir = Dir {
                        children: HashMap::new()
                    };
                    self.gather_files(&mut dir.children, archive_index, &dirent.path(), false, verbose)?;
                    entries.insert(name.to_owned(), Entry::Dir(dir));
                } else if root {
                    return Err(Error::Other(format!("All files must be in sub-directories: {:?}", dirent.path())));
                } else if let Some(dot_index) = name.rfind('.') {
                    if dot_index == 0 || dot_index + 1 == name.len() {
                        return Err(Error::Other(format!("Filenames must be of format \"NAME.EXT\": {:?}", dirent.path())));
                    }

                    let ext = &name[dot_index + 1..];
                    if !self.exts.contains(ext) {
                        self.exts.insert(ext.to_owned());
                    }

                    let mut reader = match fs::File::open(dirent.path()) {
                        Ok(reader) => reader,
                        Err(error) => return Err(Error::IOWithPath(error, dirent.path())),
                    };
                    let meta = match reader.metadata() {
                        Ok(meta) => meta,
                        Err(error) => return Err(Error::IOWithPath(error, dirent.path())),
                    };
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
                        if let Err(error) = reader.read_exact(&mut preload) {
                            return Err(Error::IOWithPath(error, dirent.path()));
                        }
                        self.digest.write(&preload);
                    } else {
                        let mut remain = size as usize;
                        inline_size = 0;
                        while remain >= BUFFER_SIZE {
                            if let Err(error) = reader.read_exact(&mut self.buf) {
                                return Err(Error::IOWithPath(error, dirent.path()));
                            }
                            self.digest.write(&self.buf);
                            remain -= BUFFER_SIZE;
                        }
                        if remain > 0 {
                            let buf = &mut self.buf[..remain];
                            if let Err(error) = reader.read_exact(buf) {
                                return Err(Error::IOWithPath(error, dirent.path()));
                            }
                            self.digest.write(buf);
                        }
                    }
                    let crc32 = self.digest.sum32();

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

        Ok(())
    }
}

fn recursive_file_list<'a>(entries: &'a mut HashMap<String, Entry>, pathbuf: &mut String, list: &mut Vec<Item<'a>>) {
    for (name, entry) in entries.iter_mut() {
        let len = pathbuf.len();
        pathbuf.push_str(name);
        match entry {
            Entry::Dir(dir) => {
                pathbuf.push('/');
                recursive_file_list(&mut dir.children, pathbuf, list);
            },
            Entry::File(file) => {
                let path = pathbuf.to_string();

                // I know that there is a '.' in the file name, I checked above.
                let dot_index = path.rfind('.').unwrap();

                // I know that there is a '/' in the path, because I checked above.
                let slash_index = path[..dot_index].rfind('/').unwrap();

                list.push(Item {
                    path,
                    dot_index,
                    slash_index,
                    file
                });
            }
        }
        pathbuf.truncate(len);
    }
}

fn write_dir(extmap: &HashMap<&str, HashMap<&str, Vec<&Item>>>, dirvpk_path: impl AsRef<Path>, dir_size: u32, index_size: u32) -> std::io::Result<fs::File> {
    let mut dirfile = fs::File::create(dirvpk_path)?;
    let mut dirwriter = BufWriter::new(&mut dirfile);

    let mut exts: Vec<&str> = extmap.keys().map(|s| s.as_ref()).collect();
    exts.sort();

    dirwriter.write_all(&VPK_MAGIC)?;

    write_u32(&mut dirwriter, 1)?;
    write_u32(&mut dirwriter, index_size)?;

    for ext in &exts {
        write_str(&mut dirwriter, ext)?;

        let dirmap = extmap.get(ext).unwrap();
        let mut dirs: Vec<&str> = dirmap.keys().map(|s| s.as_ref()).collect();
        dirs.sort();

        for dir in &dirs {
            write_str(&mut dirwriter, dir)?;

            let filelist = dirmap.get(dir).unwrap();
            for item in filelist {
                let name = item.name();

                write_str(&mut dirwriter, name)?;
                write_file(&mut dirwriter, item.file, dir_size)?;
            }
            dirwriter.write_all(&[0])?;
        }
        dirwriter.write_all(&[0])?;
    }
    dirwriter.write_all(&[0])?;

    drop(dirwriter);

    Ok(dirfile)
}

// TODO: more grouping/file order options?
pub fn pack_v1(dirvpk_path: impl AsRef<Path>, indir: impl AsRef<Path>, arch_opts: ArchiveOptions, max_inline_size: u16, alignment: usize, verbose: bool) -> Result<Package> {
    let (dirpath, prefix) = parse_path(&dirvpk_path)?;

    let mut entries = HashMap::new();
    let mut gather = Gather::new(max_inline_size);

    if verbose {
        println!("scanning {:?}", indir.as_ref());
    }

    match arch_opts {
        ArchiveOptions::ArchiveFromDirName => {
            let dirents = match read_dir(indir.as_ref()) {
                Ok(dirents) => dirents,
                Err(error) => return Err(Error::IOWithPath(error, dirpath.to_path_buf())),
            };
            for dirent in dirents {
                let dirent = match dirent {
                    Ok(dirent) => dirent,
                    Err(error) => return Err(Error::IOWithPath(error, dirpath.to_path_buf())),
                };
                let file_type = match dirent.file_type() {
                    Ok(file_type) => file_type,
                    Err(error) => return Err(Error::IOWithPath(error, dirent.path())),
                };
                if file_type.is_dir() {
                    if let Some(name) = dirent.file_name().to_str() {
                        if name.eq("dir") {
                            gather.gather_files(&mut entries, DIR_INDEX, &dirent.path(), true, verbose)?;
                        } else if name.len() != 3 {
                            eprintln!("WRANING: directory name is neither 3 digit a number nor \"dir\": {}", name);
                        } else if let Ok(archive_index) = name.parse::<u16>() {
                            if archive_index <= 999 {
                                gather.gather_files(&mut entries, archive_index, &dirent.path(), true, verbose)?;
                            } else {
                                eprintln!("WRANING: directory name represents a too large number for an archive index: {}", name);
                            }
                        } else {
                            eprintln!("WRANING: directory name is neither 3 digit a number nor \"dir\": {}", name);
                        }
                    }
                }
            }
        },
        ArchiveOptions::MaxArchiveSize(_) => {
            gather.gather_files(&mut entries, DIR_INDEX, indir.as_ref(), true, verbose)?;
        }
    }

    if verbose {
        print!("calculating index size... ");
        let _ = std::io::stdout().flush();
    }

    let mut index_size = 0usize;

    // group files by extension and dir, for writing the index
    let mut extmap: HashMap<&str, HashMap<&str, Vec<&Item>>> =
        HashMap::with_capacity(gather.exts.len());

    let mut sizemap: HashMap<&str, HashSet<&str>> =
        HashMap::with_capacity(gather.exts.len());

    for ext in &gather.exts {
        extmap.insert(ext, HashMap::new());
        sizemap.insert(ext, HashSet::new());
        index_size += ext.len() + 1 + 1;
    }
    index_size += 1;

    let v1_header_size = 4 + 4 + 4;
    
    let mut pathbuf = String::new();
    let mut list = Vec::new();
    recursive_file_list(&mut entries, &mut pathbuf, &mut list);
    list.sort_by(|a, b| a.path.cmp(&b.path));

    for item in &list {
        let extname  = item.ext();
        let dirname  = item.dir();
        let filename = item.name();

        let dirs = sizemap.get_mut(extname).unwrap();

        if !dirs.contains(dirname) {
            dirs.insert(dirname);
            index_size += dirname.len() + 1 + 1;
        }

        index_size += filename.len() + 1 +
            4 + 2 + 2 + 4 + 4 + 2;
        index_size += item.file.inline_size as usize;
    }

    let index_size = index_size;
    if verbose {
        println!("{}", index_size);
    }

    if index_size > std::i32::MAX as usize {
        return Err(Error::Other(format!("index too large: {} > {}", index_size, std::i32::MAX)));
    }

    let dir_size = v1_header_size + index_size;

    if verbose {
        println!("distributing files to archives...");
    }
    match arch_opts {
        ArchiveOptions::MaxArchiveSize(max_size) => {
            // distribute files to archives

            let mut archive_index = DIR_INDEX;
            let mut archive_size = dir_size;

            for item in list.iter_mut() {
                if item.file.size > 0 {
                    let remainder = archive_size % alignment;
                    if remainder != 0 {
                        archive_size += alignment - remainder;
                    }

                    let new_archive_size = archive_size + item.file.size as usize;
                    if new_archive_size > max_size as usize {
                        if archive_index == DIR_INDEX {
                            archive_index = 0;
                        } else if archive_index == 999 {
                            return Err(Error::Other(format!("too many archives")));
                        } else {
                            archive_index += 1;
                        }
                        archive_size = item.file.size as usize;
                        item.file.offset = 0;
                    } else {
                        item.file.offset = archive_size as u32;
                        archive_size = new_archive_size;
                    }
                    item.file.archive_index = archive_index;
                }
            }
        },
        ArchiveOptions::ArchiveFromDirName => {
            let mut archmap = HashMap::new();
            archmap.insert(DIR_INDEX, dir_size);

            for item in list.iter_mut() {
                if item.file.size > 0 {
                    if !archmap.contains_key(&item.file.archive_index) {
                        archmap.insert(item.file.archive_index, 0);
                    }
                    let archive_size = archmap.get_mut(&item.file.archive_index).unwrap();
                    let remainder = *archive_size % alignment;
                    if remainder != 0 {
                        *archive_size += alignment - remainder;
                    }
                    item.file.offset = *archive_size as u32;
                    *archive_size += item.file.size as usize;
                }
            }
        }
    }

    // group all of the above also per archive, for writing the data
    let mut archmap: HashMap<u16, Vec<(&str, &File)>> =
        HashMap::new();

    for item in &list {
        let extname = item.ext();
        let dirname = item.dir();

        let dirmap = extmap.get_mut(extname).unwrap();

        if !dirmap.contains_key(dirname) {
            dirmap.insert(dirname, Vec::new());
        }

        let filelist = dirmap.get_mut(dirname).unwrap();
        filelist.push(item);

        // group archives
        if !archmap.contains_key(&item.file.archive_index) {
            archmap.insert(item.file.archive_index, Vec::new());
        }
        let sublist = archmap.get_mut(&item.file.archive_index).unwrap();
        sublist.push((&item.path, item.file));
    }

    if verbose {
        println!("writing index to file: {:?}", dirvpk_path.as_ref());
    }

    let mut dirwriter = match write_dir(&extmap, dirvpk_path.as_ref(), dir_size as u32, index_size as u32) {
        Ok(dirwriter) => dirwriter,
        Err(error) => return Err(Error::IOWithPath(error, dirvpk_path.as_ref().to_path_buf())),
    };

    let actual_dir_size = match dirwriter.seek(SeekFrom::Current(0)) {
        Ok(offset) => offset,
        Err(error) => return Err(Error::IOWithPath(error, dirvpk_path.as_ref().to_path_buf())),
    };

    if actual_dir_size != dir_size as u64 {
        return Err(Error::Other(format!("actual_dir_size {} != dir_size {}", actual_dir_size, dir_size)));
    }

    drop(dirwriter);

    for (archive_index, files) in &archmap {
        let archive_index = *archive_index;
        let archpath = archive_path(&dirpath, &prefix, archive_index);

        if verbose {
            println!("writing archive: {:?}", archpath);
        }

        // TODO: somehow re-use dirwriter from above if archive_index == DIR_INDEX
        let writer = if archive_index == DIR_INDEX {
            fs::OpenOptions::new().create(true).write(true).truncate(false).open(&archpath)
        } else {
            fs::File::create(&archpath)
        };

        let mut writer = match writer {
            Ok(writer) => writer,
            Err(error) => return Err(Error::IOWithPath(error, archpath)),
        };

        for (vpk_path, file) in files {
            if verbose {
                println!("writing {} bytes at offset {}: {:?}", file.size, file.offset, vpk_path);
            }

            if let Err(error) = writer.seek(SeekFrom::Start(file.offset as u64)) {
                return Err(Error::IOWithPath(error, archpath));
            }

            let mut fs_path = indir.as_ref().to_path_buf();

            if let ArchiveOptions::ArchiveFromDirName = arch_opts {
                if archive_index == DIR_INDEX {
                    fs_path.push("dir");
                } else {
                    fs_path.push(format!("{:03}", archive_index));
                }
            }

            for (_, item, _) in split_path(vpk_path) {
                fs_path.push(item);
            }

            if file.size > 0 {
                match fs::File::open(&fs_path) {
                    Ok(mut reader) => {
                        if file.inline_size > 0 {
                            if let Err(error) = reader.seek(SeekFrom::Start(file.inline_size as u64)) {
                                return Err(Error::IOWithPath(error, fs_path));
                            }
                        }

                        if let Err(error) = transfer(&mut reader, &mut writer, file.size as usize) {
                            return Err(Error::IOWithPath(error, fs_path));
                        }
                    },
                    Err(error) => {
                        return Err(Error::IOWithPath(error, fs_path));
                    }
                }
            }
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
