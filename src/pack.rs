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

// TODO: make nicer

use std::collections::{HashMap, HashSet};
use std::path::{Path};
use std::fs::{self, read_dir};
use std::io::{Read, Write, Seek, SeekFrom, BufWriter};
//use std::fmt::Write;

use crc::{crc32, Hasher32};

use crate::result::{Result, Error};
use crate::consts::{DIR_INDEX, BUFFER_SIZE, VPK_MAGIC, DEFAULT_MAX_INLINE_SIZE, V1_HEADER_SIZE, V2_HEADER_SIZE, DEFAULT_MD5_CHUNK_SIZE, ARCHIVE_MD5_SIZE};
use crate::package::{Package, ArchiveMd5, Md5, parse_path};
use crate::entry::{Entry, File, Dir};
use crate::io::{write_u32, write_str, write_file, transfer};
use crate::util::{split_path, archive_path};

pub enum ArchiveStrategy {
    ArchiveFromDirName,
    MaxArchiveSize(u32),
}

impl Default for ArchiveStrategy {
    #[inline]
    fn default() -> Self {
        ArchiveStrategy::MaxArchiveSize(std::i32::MAX as u32)
    }
}

pub struct PackOptions {
    pub version: u32,
    pub md5_chunk_size: u32,
    pub strategy: ArchiveStrategy,
    pub max_inline_size: u16,
    pub alignment: usize,
    pub verbose: bool,
}

impl PackOptions {
    #[inline]
    pub fn new() -> Self {
        PackOptions::default()
    }
}

impl Default for PackOptions {
    #[inline]
    fn default() -> Self {
        Self {
            version: 1,
            md5_chunk_size: DEFAULT_MD5_CHUNK_SIZE,
            strategy: ArchiveStrategy::default(),
            max_inline_size: DEFAULT_MAX_INLINE_SIZE,
            alignment: 1,
            verbose: false,
        }
    }
}

struct Gather {
    digest: crc32::Digest,
    max_inline_size: u16,
    buf: [u8; BUFFER_SIZE],
    exts: HashSet<String>,
    verbose: bool,
    inline: bool,
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
    fn new(max_inline_size: u16, verbose: bool) -> Self {
        Gather {
            digest: crc32::Digest::new(crc32::IEEE),
            max_inline_size,
            buf: [0; BUFFER_SIZE],
            exts: HashSet::new(),
            verbose,
            inline: false,
        }
    }

    fn gather_files(&mut self, entries: &mut HashMap<String, Entry>, archive_index: u16, dirpath: &Path, root: bool) -> Result<()> {
        let dirents = match read_dir(dirpath) {
            Ok(dirents) => dirents,
            Err(error) => return Err(Error::io_with_path(error, dirpath)),
        };
        for dirent in dirents {
            let dirent = match dirent {
                Ok(dirent) => dirent,
                Err(error) => return Err(Error::io_with_path(error, dirpath)),
            };
            if self.verbose {
                println!("scanning {:?}", dirent.path());
            }
            let os_name = dirent.file_name();
            if let Some(name) = os_name.to_str() {
                let file_type = match dirent.file_type() {
                    Ok(file_type) => file_type,
                    Err(error) => return Err(Error::io_with_path(error, dirent.path())),
                };
                if file_type.is_dir() {
                    if let Some(entry) = entries.get_mut(name) {
                        match entry {
                            Entry::Dir(dir) => {
                                self.gather_files(&mut dir.children, archive_index, &dirent.path(), false)?;
                            },
                            Entry::File(_) => {
                                // TODO: parameter to entry_not_a_dir() should be path in the package
                                return Err(Error::entry_not_a_dir(name).with_path(dirent.path()));
                            }
                        }
                    } else {
                        let mut dir = Dir {
                            children: HashMap::new()
                        };
                        self.gather_files(&mut dir.children, archive_index, &dirent.path(), false)?;
                        entries.insert(name.to_owned(), Entry::Dir(dir));
                    }
                } else if root {
                    return Err(Error::other("all files must be in sub-directories").with_path(dirent.path()));
                } else if let Some(dot_index) = name.rfind('.') {
                    if dot_index == 0 || dot_index + 1 == name.len() {
                        return Err(Error::other("filenames must be of format \"NAME.EXT\"").with_path(dirent.path()));
                    }

                    let ext = &name[dot_index + 1..];
                    if !self.exts.contains(ext) {
                        self.exts.insert(ext.to_owned());
                    }

                    let mut reader = match fs::File::open(dirent.path()) {
                        Ok(reader) => reader,
                        Err(error) => return Err(Error::io_with_path(error, dirent.path())),
                    };
                    let meta = match reader.metadata() {
                        Ok(meta) => meta,
                        Err(error) => return Err(Error::io_with_path(error, dirent.path())),
                    };
                    let size = meta.len();

                    if size > std::i32::MAX as u64 {
                        return Err(Error::other(format!("file too big {} > {}", size, std::i32::MAX))
                            .with_path(dirent.path()));
                    }

                    let mut size = size as u32;
                    let mut preload = Vec::new();
                    let inline_size: u16;

                    self.digest.reset();
                    if self.inline || size <= self.max_inline_size as u32 {
                        if size > std::u16::MAX as u32 {
                            return Err(Error::other(format!(
                                "file is meant to be inlined into the index, but is too big: {} > {}",
                                size, std::u16::MAX))
                                .with_path(dirent.path()));
                        }
                        inline_size = size as u16;
                        size = 0;
                        preload.resize(inline_size as usize, 0);
                        if let Err(error) = reader.read_exact(&mut preload) {
                            return Err(Error::io_with_path(error, dirent.path()));
                        }
                        self.digest.write(&preload);
                    } else {
                        let mut remain = size as usize;
                        inline_size = 0;
                        while remain >= BUFFER_SIZE {
                            if let Err(error) = reader.read_exact(&mut self.buf) {
                                return Err(Error::io_with_path(error, dirent.path()));
                            }
                            self.digest.write(&self.buf);
                            remain -= BUFFER_SIZE;
                        }
                        if remain > 0 {
                            let buf = &mut self.buf[..remain];
                            if let Err(error) = reader.read_exact(buf) {
                                return Err(Error::io_with_path(error, dirent.path()));
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
                    if let Some(old) = entries.insert(name.to_owned(), Entry::File(file)) {
                        use std::fmt::Write;

                        let mut msg = String::new();
                        write!(&mut msg, "file \"{}\" occured twice, once ", name).unwrap();
                        if inline_size > 0 && size == 0 {
                            msg.push_str("inlined in index");
                        } else if archive_index == DIR_INDEX {
                            msg.push_str("from \"dir\"");
                        } else {
                            write!(&mut msg, "from archive \"{:03}\"", archive_index).unwrap();
                        }
                        msg.push_str(", and once ");
                        match old {
                            Entry::Dir(_) => {
                                msg.push_str("it's a directory");
                            },
                            Entry::File(file) => {
                                if file.inline_size > 0 && file.size == 0 {
                                    msg.push_str("inlined in index");
                                } else if file.archive_index == DIR_INDEX {
                                    msg.push_str("from \"dir\"");
                                } else {
                                    write!(&mut msg, "from archive \"{:03}\"", file.archive_index).unwrap();
                                }
                            }
                        }

                        return Err(Error::other(msg).with_path(dirent.path()));
                    }
                } else {
                    return Err(Error::other("filenames must be of format \"NAME.EXT\"").with_path(dirent.path()));
                }
            } else {
                return Err(Error::other("cannot handle filename").with_path(dirent.path()));
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

fn write_dir(
        extmap: &HashMap<&str, HashMap<&str, Vec<&Item>>>,
        dirvpk_path: impl AsRef<Path>,
        version:    u32,
        dir_size:   u32,
        index_size: u32) -> std::io::Result<fs::File> {
    let mut dirfile = fs::File::create(dirvpk_path)?;
    let mut dirwriter = BufWriter::new(&mut dirfile);

    let mut exts: Vec<&str> = extmap.keys().map(|s| s.as_ref()).collect();
    exts.sort();

    if version > 0 {
        dirwriter.write_all(&VPK_MAGIC)?;

        write_u32(&mut dirwriter, version)?;
        write_u32(&mut dirwriter, index_size)?;

        if version > 1 {
            // write placeholder
            dirwriter.write_all(&[
                0, 0, 0, 0, // data size
                0, 0, 0, 0, // archive MD5 size
                0, 0, 0, 0, // other MD5 size
                0, 0, 0, 0, // signature size
            ])?;
        }
    }

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

fn write_sizes<W>(dirwriter: &mut W, data_size: u32, archive_md5_size: u32, other_md5_size: u32, signature_size: u32) -> std::io::Result<()>
where W: Write, W: Seek {
    dirwriter.seek(SeekFrom::Start(V1_HEADER_SIZE as u64))?;

    write_u32(dirwriter, data_size)?;
    write_u32(dirwriter, archive_md5_size)?;
    write_u32(dirwriter, other_md5_size)?;
    write_u32(dirwriter, signature_size)?;

    Ok(())
}

fn write_archive_md5s(dirwriter: &mut impl Write, archive_md5s: &Vec<ArchiveMd5>) -> std::io::Result<()> {
    for item in archive_md5s {
        write_u32(dirwriter, item.archive_index as u32)?;
        write_u32(dirwriter, item.offset)?;
        write_u32(dirwriter, item.size)?;
        dirwriter.write_all(&item.md5)?;
    }
    dirwriter.flush()
}

fn calculate_md5<R>(reader: &mut R, offset: u64, size: u64) -> std::io::Result<Md5>
where R: Read, R: Seek {

    reader.seek(SeekFrom::Start(offset))?;

    let mut buf = [0u8; BUFFER_SIZE];
    let mut remaining = size as usize;
    let mut hasher = md5::Context::new();

    while remaining >= BUFFER_SIZE {
        reader.read_exact(&mut buf)?;
        hasher.consume(&buf);
        remaining -= BUFFER_SIZE;
    }

    if remaining > 0 {
        let buf = &mut buf[..remaining];
        reader.read_exact(buf)?;
        hasher.consume(buf);
    }

    Ok(*hasher.compute())
}

// TODO: more grouping/file order options?
pub fn pack(dirvpk_path: impl AsRef<Path>, indir: impl AsRef<Path>, options: PackOptions) -> Result<Package> {
    let header_size = match options.version {
        0 => 0,
        1 => V1_HEADER_SIZE,
        2 => V2_HEADER_SIZE,
        _ => return Err(Error::unsupported_version(options.version)),
    };

    let (dirpath, prefix) = parse_path(dirvpk_path.as_ref())?;

    let mut entries = HashMap::new();
    let mut gather = Gather::new(options.max_inline_size, options.verbose);

    if options.verbose {
        println!("scanning {:?}", indir.as_ref());
    }

    match options.strategy {
        ArchiveStrategy::ArchiveFromDirName => {
            let dirents = match read_dir(indir.as_ref()) {
                Ok(dirents) => dirents,
                Err(error) => return Err(Error::io_with_path(error, dirpath)),
            };
            for dirent in dirents {
                let dirent = match dirent {
                    Ok(dirent) => dirent,
                    Err(error) => return Err(Error::io_with_path(error, dirpath)),
                };
                let file_type = match dirent.file_type() {
                    Ok(file_type) => file_type,
                    Err(error) => return Err(Error::io_with_path(error, dirent.path())),
                };
                if file_type.is_dir() {
                    if let Some(name) = dirent.file_name().to_str() {
                        if name == "dir" {
                            gather.inline = false;
                            gather.gather_files(&mut entries, DIR_INDEX, &dirent.path(), true)?;
                        } else if name == "inline" {
                            gather.inline = true;
                            gather.gather_files(&mut entries, DIR_INDEX, &dirent.path(), true)?;
                        } else if name.len() != 3 {
                            eprintln!("WARNING: directory name is neither a 3 digit number, \"dir\", nor \"inline\": {:?}", dirent.path());
                        } else if let Ok(archive_index) = name.parse::<u16>() {
                            if archive_index <= 999 {
                                gather.inline = false;
                                gather.gather_files(&mut entries, archive_index, &dirent.path(), true)?;
                            } else {
                                eprintln!("WARNING: directory name represents a too large number for an archive index: {:?}", dirent.path());
                            }
                        } else {
                            eprintln!("WARNING: directory name is neither a 3 digit number, \"dir\", nor \"inline\": {:?}", dirent.path());
                        }
                    }
                }
            }
        },
        ArchiveStrategy::MaxArchiveSize(_) => {
            gather.gather_files(&mut entries, DIR_INDEX, indir.as_ref(), true)?;
        }
    }

    if options.verbose {
        println!("calculating index size... ");
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

    if index_size > std::i32::MAX as usize {
        return Err(Error::other(format!(
                "index too large: {} > {}",
                index_size, std::i32::MAX)).
            with_path(dirvpk_path));
    }

    let dir_size = header_size + index_size;
    let index_size = index_size as u32;

    if options.verbose {
        println!("distributing files to archives...");
    }
    let mut data_end_offset = dir_size as u64;
    match options.strategy {
        ArchiveStrategy::MaxArchiveSize(max_size) => {
            // distribute files to archives

            let mut archive_index = DIR_INDEX;
            let mut archive_size = dir_size;

            for item in list.iter_mut() {
                if item.file.size > 0 {
                    let remainder = archive_size % options.alignment;
                    if remainder != 0 {
                        archive_size += options.alignment - remainder;
                    }

                    let new_archive_size = archive_size + item.file.size as usize;
                    if new_archive_size > max_size as usize {
                        if archive_index == DIR_INDEX {
                            data_end_offset = archive_size as u64;
                            archive_index = 0;
                        } else if archive_index == 999 {
                            return Err(Error::other(format!("too many archives")));
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

            if archive_index == DIR_INDEX {
                data_end_offset = archive_size as u64;
            }
        },
        ArchiveStrategy::ArchiveFromDirName => {
            let mut archmap = HashMap::new();
            archmap.insert(DIR_INDEX, dir_size);

            for item in list.iter_mut() {
                if item.file.size > 0 {
                    if !archmap.contains_key(&item.file.archive_index) {
                        archmap.insert(item.file.archive_index, 0);
                    }
                    let archive_size = archmap.get_mut(&item.file.archive_index).unwrap();
                    let remainder = *archive_size % options.alignment;
                    if remainder != 0 {
                        *archive_size += options.alignment - remainder;
                    }
                    item.file.offset = *archive_size as u32;
                    *archive_size += item.file.size as usize;
                }
            }

            data_end_offset = *archmap.get(&DIR_INDEX).unwrap() as u64;
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

    if options.verbose {
        println!("writing index to file: {:?}", dirvpk_path.as_ref());
    }

    let mut dirwriter = match write_dir(
            &extmap,
            dirvpk_path.as_ref(),
            options.version,
            dir_size as u32,
            index_size) {
        Ok(dirwriter) => dirwriter,
        Err(error) => return Err(Error::io_with_path(error, dirvpk_path)),
    };

    let actual_dir_size = match dirwriter.seek(SeekFrom::Current(0)) {
        Ok(offset) => offset,
        Err(error) => return Err(Error::io_with_path(error, dirvpk_path)),
    };

    if actual_dir_size != dir_size as u64 {
        return Err(Error::other(format!(
                "internal error: actual_dir_size {} != dir_size {}",
                actual_dir_size, dir_size)).
            with_path(dirvpk_path));
    }

    enum SelectFile<'a> {
        Referenced(&'a mut fs::File),
        Contained(fs::File),
    }

    impl SelectFile<'_> {
        #[inline]
        fn get(&mut self) -> &mut fs::File {
            match self {
                SelectFile::Referenced(writer) => writer,
                SelectFile::Contained(writer)  => writer,
            }
        }
    }

    for (archive_index, files) in &archmap {
        let archive_index = *archive_index;
        let archpath = archive_path(&dirpath, &prefix, archive_index);

        if options.verbose {
            println!("writing archive: {:?}", archpath);
        }

        // TODO: is there a better way to do this?
        let mut writer = if archive_index == DIR_INDEX {
            SelectFile::Referenced(&mut dirwriter)
        } else {
            SelectFile::Contained(fs::File::create(&archpath)?)
        };
        let writer = writer.get();

        for (vpk_path, file) in files {
            if options.verbose {
                if archive_index == DIR_INDEX {
                    println!("writing {:>10} bytes at offset {:>10} to {}_dir.vpk: {:?}",
                        file.size, file.offset, prefix, vpk_path);
                } else {
                    println!("writing {:>10} bytes at offset {:>10} to {}_{:03}.vpk: {:?}",
                        file.size, file.offset, file.archive_index, prefix, vpk_path);
                }
            }

            if file.size > 0 {
                let mut fs_path = indir.as_ref().to_path_buf();

                if let ArchiveStrategy::ArchiveFromDirName = options.strategy {
                    if archive_index == DIR_INDEX {
                        fs_path.push("dir");
                    } else {
                        fs_path.push(format!("{:03}", archive_index));
                    }
                }

                for (_, item, _) in split_path(vpk_path) {
                    fs_path.push(item);
                }

                if let Err(error) = writer.seek(SeekFrom::Start(file.offset as u64)) {
                    return Err(Error::io_with_path(error, archpath));
                }

                match fs::File::open(&fs_path) {
                    Ok(mut reader) => {
                        if file.inline_size > 0 {
                            if let Err(error) = reader.seek(SeekFrom::Start(file.inline_size as u64)) {
                                return Err(Error::io_with_path(error, fs_path));
                            }
                        }

                        if let Err(error) = transfer(&mut reader, writer, file.size as usize) {
                            return Err(Error::io_with_path(error, fs_path));
                        }
                    },
                    Err(error) => {
                        return Err(Error::io_with_path(error, fs_path));
                    }
                }
            }
        }
    }

    let data_offset = dir_size as u32;
    let data_size   = (data_end_offset - data_offset as u64) as u32;
    let signature_size = 0;

    // VPK 2 support
    let mut archive_md5s = Vec::new();
    let archive_md5_size;
    let other_md5_size;
    let index_md5;
    let archive_md5s_md5;
    let everything_md5;

    if options.version < 2 {
        archive_md5_size = 0;
        other_md5_size   = 0;
        index_md5        = [0; 16];
        archive_md5s_md5 = [0; 16];
        everything_md5   = [0; 16];
    } else {
        other_md5_size   = 16 * 3;
        let mut buf = Vec::with_capacity(options.md5_chunk_size as usize);
        buf.resize(options.md5_chunk_size as usize, 0);

        let mut dirreader = match fs::File::open(&dirvpk_path) {
            Ok(file) => file,
            Err(error) => return Err(Error::io_with_path(error, dirvpk_path)),
        };

        for archive_index in archmap.keys() {
            let archive_index = *archive_index;
            let archpath = archive_path(&dirpath, &prefix, archive_index);

            if options.verbose {
                println!("calculation MD5 sums of: {:?}", archpath);
            }

            let mut file = if archive_index == DIR_INDEX {
                SelectFile::Referenced(&mut dirreader)
            } else {
                SelectFile::Contained(fs::File::open(&archpath)?)
            };
            let file = file.get();

            let mut offset;
            let mut remaining;

            if archive_index == DIR_INDEX {
                offset    = data_offset;
                remaining = data_size;
            } else {
                let meta = match file.metadata() {
                    Ok(meta) => meta,
                    Err(error) => return Err(Error::io_with_path(error, archpath)),
                };
                if meta.len() > std::u32::MAX as u64 {
                    return Err(Error::other(
                            format!("file too big: {} > {}", meta.len(), std::u32::MAX))
                        .with_path(archpath));
                }
                offset    = 0;
                remaining = meta.len() as u32;
            }

            if let Err(error) = file.seek(SeekFrom::Start(offset as u64)) {
                return Err(Error::io_with_path(error, archpath));
            }

            while remaining >= options.md5_chunk_size {
                if let Err(error) = file.read_exact(&mut buf) {
                    return Err(Error::io_with_path(error, archpath));
                }

                let md5 = *md5::compute(&buf);
                archive_md5s.push(ArchiveMd5 {
                    archive_index,
                    offset,
                    size: options.md5_chunk_size,
                    md5,
                });

                offset    += options.md5_chunk_size;
                remaining -= options.md5_chunk_size;
            }

            if remaining > 0 {
                let buf = &mut buf[..remaining as usize];
                if let Err(error) = file.read_exact(buf) {
                    return Err(Error::io_with_path(error, archpath));
                }

                let md5 = *md5::compute(buf);
                archive_md5s.push(ArchiveMd5 {
                    archive_index,
                    offset,
                    size: remaining,
                    md5,
                });
            }
        }

        let size = ARCHIVE_MD5_SIZE * archive_md5s.len();
        if size > std::u32::MAX as usize {
            return Err(Error::other(format!(
                    "MD5 section is too big: {} > {}",
                    size, std::u32::MAX))
                .with_path(dirvpk_path));
        }
        archive_md5_size = size as u32;
        if options.verbose {
            println!("writing archive MD5 sums...");
        }

        let mut writer = BufWriter::new(&mut dirwriter);

        if let Err(error) = writer.seek(SeekFrom::Start(data_end_offset)) {
            return Err(Error::io_with_path(error, dirvpk_path));
        }

        if let Err(error) = write_archive_md5s(&mut writer, &archive_md5s) {
            return Err(Error::io_with_path(error, dirvpk_path));
        }

        if options.verbose {
            println!("calculating index MD5 sum...");
        }

        index_md5 = match calculate_md5(&mut dirreader, V2_HEADER_SIZE as u64, index_size as u64) {
            Ok(md5) => md5,
            Err(error) => return Err(Error::io_with_path(error, dirvpk_path)),
        };

        if options.verbose {
            println!("calculating MD5 sum section MD5 sum...");
        }

        archive_md5s_md5 = match calculate_md5(&mut dirreader, data_end_offset, archive_md5_size as u64) {
            Ok(md5) => md5,
            Err(error) => return Err(Error::io_with_path(error, dirvpk_path)),
        };

        if options.verbose {
            println!("writing these two MD5 sums...");
        }

        if let Err(error) = writer.write_all(&index_md5) {
            return Err(Error::io_with_path(error, dirvpk_path));
        }

        if let Err(error) = writer.write_all(&archive_md5s_md5) {
            return Err(Error::io_with_path(error, dirvpk_path));
        }

        if options.verbose {
            println!("writing missing sizes to head...");
        }

        if let Err(error) = write_sizes(&mut writer, data_size, archive_md5_size, other_md5_size, signature_size) {
            return Err(Error::io_with_path(error, dirvpk_path));
        }

        if let Err(error) = writer.flush() {
            return Err(Error::io_with_path(error, dirvpk_path));
        }

        if options.verbose {
            println!("calculating MD5 sum of everything above...");
        }

        everything_md5 = match calculate_md5(&mut dirreader, 0, data_end_offset + archive_md5_size as u64 + 16 * 2) {
            Ok(md5) => md5,
            Err(error) => return Err(Error::io_with_path(error, dirvpk_path)),
        };

        if options.verbose {
            println!("writing this last MD5 sum...");
        }

        let everything_md5_offset = data_end_offset + archive_md5_size as u64 + 16 * 2;
        if let Err(error) = writer.seek(SeekFrom::Start(everything_md5_offset)) {
            return Err(Error::io_with_path(error, dirvpk_path));
        }

        if let Err(error) = writer.write_all(&everything_md5) {
            return Err(Error::io_with_path(error, dirvpk_path));
        }
    }

    if options.verbose {
        println!("done");
    }

    Ok(Package {
        dirpath,
        prefix,
        version: options.version,
        data_offset,
        index_size,
        data_size,
        archive_md5_size,
        other_md5_size,
        signature_size,
        entries,

        // VPK 2
        archive_md5s,
        index_md5,
        archive_md5s_md5,
        everything_md5,
        public_key: Vec::new(),
        signature:  Vec::new(),
    })
}
