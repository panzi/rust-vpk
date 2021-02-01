use std::path::{Path, PathBuf};
use std::ffi::OsStr;
use std::fs;
use std::collections::HashMap;
use std::os::unix::fs::FileExt;
use std::os::linux::fs::MetadataExt;
use std::time::{SystemTime, UNIX_EPOCH, Duration};

use cntr_fuse as fuse;
use fuse::{Filesystem, FileType, Request, ReplyEntry, FileAttr, ReplyAttr, ReplyXattr, ReplyEmpty, ReplyOpen, ReplyDirectory, ReplyStatfs, ReplyRead, FUSE_ROOT_ID};
use daemonize::{Daemonize, DaemonizeError};
use libc::{ENOENT, EISDIR, EACCES, ENOTDIR, ENODATA, ERANGE, EINVAL, EIO, O_RDONLY};

use crate::entry::{Entry, File};
use crate::consts::DIR_INDEX;
use crate::package::Package;
use crate::result::{Result, Error};
use crate::util::{archive_path};

struct Dir {
    children: HashMap<String, u64>,
}

enum INodeData {
    File(File),
    Dir(Dir),
}

struct INode {
    parent: u64,
    inode: u64,
    data: INodeData,
    stat: FileAttr,
}

impl INode {
    fn is_dir(&self) -> bool {
        match self.data {
            INodeData::Dir(_)  => true,
            INodeData::File(_) => false,
        }
    }

    #[allow(unused)]
    fn is_file(&self) -> bool {
        match self.data {
            INodeData::Dir(_)  => false,
            INodeData::File(_) => true,
        }
    }
}

pub struct VPKFS {
    dirpath: PathBuf,
    prefix: String,

    archives: HashMap<u16, fs::File>,
    inodes: HashMap<u64, INode>,
    next_inode: u64,

    atime:  SystemTime,
    mtime:  SystemTime,
    ctime:  SystemTime,
    crtime: SystemTime,

    uid: u32,
    gid: u32,

    blksize: u64,
    blocks:  u64,
}

fn make_time(mut time: i64, mut nsec: i64) -> SystemTime {
    if time <= 0 {
        time = -time;
        if nsec < 0 {
            nsec = -nsec;
        } else {
            time += 1;
            nsec = 1_000_000_000 - nsec;
        }

        return UNIX_EPOCH - Duration::new(time as u64, nsec as u32);
    } else {
        if nsec < 0 {
            time -= 1;
            nsec += 1_000_000_000;
        }
        return UNIX_EPOCH + Duration::new(time as u64, nsec as u32);
    }
}

impl VPKFS {
    pub fn new(package: Package) -> Result<Self> {
        let path = package.archive_path(DIR_INDEX);
        let meta = match fs::metadata(&path) {
            Err(error) => return Err(Error::IOWithPath(error, path)),
            Ok(meta) => meta,
        };

        let mut vpkfs = Self {
            dirpath:  package.dirpath.to_owned(),
            prefix:   package.prefix.to_owned(),
            archives: HashMap::new(),
            inodes:   HashMap::new(),
            next_inode: FUSE_ROOT_ID + 1,

            atime:  make_time(meta.st_atime(), meta.st_atime_nsec()),
            mtime:  make_time(meta.st_mtime(), meta.st_mtime_nsec()),
            ctime:  make_time(meta.st_ctime(), meta.st_ctime_nsec()),
            crtime: meta.created().unwrap_or(UNIX_EPOCH),

            uid:    meta.st_uid(),
            gid:    meta.st_gid(),

            blksize: meta.st_blksize(),
            blocks:  0,
        };

        let mut fsdir = Dir {
            children: HashMap::new()
        };

        vpkfs.init(package.entries, FUSE_ROOT_ID, &mut fsdir.children)?;

        let mut sum_size = 0u64;
        for (archive_index, file) in &vpkfs.archives {
            let meta = match file.metadata() {
                Err(error) => return Err(Error::IOWithPath(error, archive_path(&vpkfs.dirpath, &vpkfs.prefix, *archive_index))),
                Ok(meta) => meta,
            };
            sum_size += meta.len();
        }
        if sum_size != 0 {
            vpkfs.blocks = 1 + ((sum_size - 1) / vpkfs.blksize);
        }

        let mut stat = FileAttr {
            ino:    FUSE_ROOT_ID,
            size:   5,
            blocks: 0,
            atime:  vpkfs.atime,
            mtime:  vpkfs.mtime,
            ctime:  vpkfs.ctime,
            crtime: vpkfs.crtime,
            kind:   FileType::Directory,
            perm:   0o555,
            nlink:  1,
            uid:    vpkfs.uid,
            gid:    vpkfs.gid,
            rdev:   0,
            flags:  0,
        };

        for (name, inode) in &fsdir.children {
            let child = vpkfs.inodes.get(inode).unwrap();
            stat.size += name.len() as u64 + 1;
            if child.is_dir() {
                stat.nlink += 1;
            }
        }

        stat.blocks = if stat.size != 0 { 1 + ((stat.size - 1) / vpkfs.blksize) } else { 0 };

        vpkfs.inodes.insert(FUSE_ROOT_ID, INode {
            inode:  FUSE_ROOT_ID,
            parent: FUSE_ROOT_ID,
            data: INodeData::Dir(fsdir),
            stat,
        });

        Ok(vpkfs)
    }

    fn init(&mut self, entries: HashMap<String, Entry>, parent_inode: u64, parent_entries: &mut HashMap<String, u64>) -> Result<()> {
        for (name, entry) in entries {
            let inode = self.next_inode;
            self.next_inode += 1;
            parent_entries.insert(name.to_owned(), inode);

            match entry {
                Entry::Dir(dir) => {
                    let mut fsdir = Dir {
                        children: HashMap::new()
                    };
                    self.init(dir.children, inode, &mut fsdir.children)?;

                    let mut stat = FileAttr {
                        ino:    inode,
                        size:   5,
                        blocks: 0,
                        atime:  self.atime,
                        mtime:  self.mtime,
                        ctime:  self.ctime,
                        crtime: self.crtime,
                        kind:   FileType::Directory,
                        perm:   0o555,
                        nlink:  1,
                        uid:    self.uid,
                        gid:    self.gid,
                        rdev:   0,
                        flags:  0,
                    };

                    for (name, inode) in &fsdir.children {
                        let child = self.inodes.get(inode).unwrap();
                        stat.size += name.len() as u64 + 1;
                        if child.is_dir() {
                            stat.nlink += 1;
                        }
                    }

                    stat.blocks = if stat.size != 0 { 1 + ((stat.size - 1) / self.blksize) } else { 0 };

                    self.inodes.insert(inode, INode {
                        inode,
                        parent: parent_inode,
                        data: INodeData::Dir(fsdir),
                        stat,
                    });
                },
                Entry::File(file) => {
                    let mut stat = FileAttr {
                        ino:    inode,
                        size:   file.inline_size as u64 + file.size as u64,
                        blocks: 0,
                        atime:  self.atime,
                        mtime:  self.mtime,
                        ctime:  self.ctime,
                        crtime: self.crtime,
                        kind:   FileType::RegularFile,
                        perm:   0o444,
                        nlink:  1,
                        uid:    self.uid,
                        gid:    self.gid,
                        rdev:   0,
                        flags:  0,
                    };

                    stat.blocks = if stat.size != 0 { 1 + ((stat.size - 1) / self.blksize) } else { 0 };

                    let archive_index = file.archive_index;
                    self.inodes.insert(inode, INode {
                        inode,
                        parent: parent_inode,
                        data: INodeData::File(file),
                        stat,
                    });
                    if !self.archives.contains_key(&archive_index) {
                        let archive = fs::File::open(archive_path(&self.dirpath, &self.prefix, archive_index))?;
                        self.archives.insert(archive_index, archive);
                    }
                },
            }
        }
        Ok(())
    }
}

const TTL: Duration = Duration::from_secs(std::u64::MAX);
const DIR_XATTRS:  &[u8] = b"user.vpkfs.dir_path\0";
const FILE_XATTRS: &[u8] =
    b"user.vpkfs.dir_path\0\
      user.vpkfs.crc32\0\
      user.vpkfs.archive_path\0\
      user.vpkfs.inline_size\0\
      user.vpkfs.archive_index\0\
      user.vpkfs.offset\0";

impl<'a> Filesystem for VPKFS {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        if let Some(mut inode_data) = self.inodes.get(&parent) {
            if name == OsStr::new(".") {
                // done
            } else if name == OsStr::new("..") {
                inode_data = if let Some(inode_data) = self.inodes.get(&inode_data.parent) {
                    inode_data
                } else {
                    return reply.error(ENOENT);
                };
            } else if let INodeData::Dir(dir) = &inode_data.data {
                if let Some(name) = name.to_str() {
                    if let Some(inode) = dir.children.get(name) {
                        inode_data = if let Some(inode_data) = self.inodes.get(&inode) {
                            inode_data
                        } else {
                            return reply.error(ENOENT);
                        };
                    } else {
                        return reply.error(ENOENT);
                    }
                } else {
                    return reply.error(ENOENT);
                }
            } else {
                return reply.error(ENOTDIR);
            }

            return reply.entry(&TTL, &inode_data.stat, 0);
        } else {
            return reply.error(ENOENT);
        }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        if let Some(inode_data) = self.inodes.get(&ino) {
            return reply.attr(&TTL, &inode_data.stat);
        } else {
            return reply.error(ENOENT);
        }
    }

    fn getxattr(&mut self, _req: &Request, ino: u64, name: &OsStr, size: u32, reply: ReplyXattr) {
        if let Some(inode_data) = self.inodes.get(&ino) {
            // assumes UTF-8 as OS encoding (which should be true on POSIX)
            let data = if name == OsStr::new("user.vpkfs.dir_path") {
                let mut path: String = archive_path(&self.dirpath, &self.prefix, DIR_INDEX)
                    .to_string_lossy().as_ref().to_owned();
                path.push('\0');
                path
            } else if let INodeData::File(file) = &inode_data.data {
                if name == OsStr::new("user.vpkfs.crc32") {
                    format!("0x{:08x}\0", file.crc32)
                } else if name == OsStr::new("user.vpkfs.archive_path") {
                    let mut path: String = archive_path(&self.dirpath, &self.prefix, file.archive_index)
                        .to_string_lossy().as_ref().to_owned();
                    path.push('\0');
                    path
                } else if name == OsStr::new("user.vpkfs.inline_size") {
                    format!("{}\0", file.inline_size)
                } else if name == OsStr::new("user.vpkfs.archive_index") {
                    format!("{}\0", file.archive_index)
                } else if name == OsStr::new("user.vpkfs.offset") {
                    format!("{}\0", file.offset)
                } else {
                    return reply.error(ENODATA);
                }
            } else {
                return reply.error(ENODATA);
            }.into_bytes();

            if size == 0 {
                return reply.size(data.len() as u32);
            } else if data.len() > size as usize {
                return reply.error(ERANGE);
            }
            return reply.data(&data);
        } else {
            return reply.error(ENOENT);
        }
    }

    fn listxattr(&mut self, _req: &Request, ino: u64, size: u32, reply: ReplyXattr) {
        if let Some(inode_data) = self.inodes.get(&ino) {
            let data = match inode_data.data {
                INodeData::Dir(_)  => DIR_XATTRS,
                INodeData::File(_) => FILE_XATTRS,
            };
            if size == 0 {
                return reply.size(data.len() as u32);
            }
            if data.len() > size as usize {
                return reply.error(ERANGE);
            }
            return reply.data(&data);
        } else {
            return reply.error(ENOENT);
        }
    }

    fn access(&mut self, _req: &Request, ino: u64, mask: u32, reply: ReplyEmpty) {
        if let Some(inode_data) = self.inodes.get(&ino) {
            if mask & inode_data.stat.perm as u32 != mask {
                return reply.error(EACCES);
            }
            return reply.ok();
        } else {
            return reply.error(ENOENT);
        }
    }

    fn opendir(&mut self, _req: &Request, ino: u64, _flags: u32, reply: ReplyOpen) {
        if let Some(inode_data) = self.inodes.get(&ino) {
            if !inode_data.is_dir() {
                return reply.error(ENOTDIR);
            }
            return reply.opened(ino, 0);
        } else {
            return reply.error(ENOENT);
        }
    }

    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
        if let Some(inode_data) = self.inodes.get(&ino) {
            if let INodeData::Dir(dir) = &inode_data.data {
                if offset == 0 {
                    let mut offset = 0i64;
                    reply.add(ino,               offset, FileType::Directory, ".");
                    offset += 1;
                    reply.add(inode_data.parent, offset, FileType::Directory, "..");
                    offset += 1;
                    for (name, child_inode) in &dir.children {
                        let child = self.inodes.get(child_inode).unwrap();
                        reply.add(child.inode, offset, if child.is_dir() {
                            FileType::Directory
                        } else {
                            FileType::RegularFile
                        }, name);
                        offset += 1;
                    }
                }
                return reply.ok();
            } else {
                return reply.error(ENOTDIR);
            }
        } else {
            return reply.error(ENOENT);
        }
    }

    fn statfs(&mut self, _req: &Request, _ino: u64, reply: ReplyStatfs) {
        reply.statfs(
            /* blocks  */ 0,
            /* bfree   */ 0,
            /* bavail  */ 0,
            /* files   */ self.inodes.len() as u64,
            /* ffree   */ 0,
            /* bsize   */ self.blksize as u32,
            /* namelen */ std::u32::MAX,
            /* frsize  */ 0);
    }

    fn open(&mut self, _req: &Request, ino: u64, flags: u32, reply: ReplyOpen) {
        if let Some(inode_data) = self.inodes.get(&ino) {
            if inode_data.is_dir() {
                return reply.error(EISDIR);
            } else if flags & 3 != O_RDONLY as u32 {
                return reply.error(EACCES);
            }
            return reply.opened(ino, 0);
        } else {
            return reply.error(ENOENT);
        }
    }

    fn read(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, size: u32, reply: ReplyRead) {
        if let Some(inode_data) = self.inodes.get(&ino) {
            if let INodeData::File(file) = &inode_data.data {
                if offset < 0 {
                    return reply.error(EINVAL);
                } else if offset as u64 > std::usize::MAX as u64 {
                    return reply.data(&[]);
                }
                let offset = offset as u64;
                let inline_size = file.inline_size as u64;

                if offset < inline_size {
                    let end_offset = offset + size as u64;
                    if end_offset <= inline_size {
                        return reply.data(&file.preload[offset as usize..end_offset as usize]);
                    } else if file.size == 0 {
                        return reply.data(&file.preload[offset as usize..]);
                    } else {
                        let mut body_size = end_offset - inline_size;
                        if body_size > file.size as u64 {
                            body_size = file.size as u64;
                        }
                        let actual_size = inline_size - offset + body_size;

                        let mut buffer = Vec::with_capacity(actual_size as usize);
                        buffer.extend_from_slice(&file.preload[offset as usize..]);
                        let index = buffer.len();
                        buffer.resize(actual_size as usize, 0);

                        let archive = self.archives.get_mut(&file.archive_index).unwrap();
                        if let Err(error) = archive.read_exact_at(&mut buffer[index..], file.offset as u64 + offset - inline_size) {
                            return reply.error(error.raw_os_error().unwrap_or(EIO));
                        }

                        return reply.data(&buffer);
                    }
                } else {
                    let offset = offset - inline_size;
                    if offset >= file.size as u64 {
                        return reply.data(&[]);
                    }
                    let end_offset = offset + size as u64;
                    let actual_size = if end_offset > file.size as u64 {
                        file.size as u64 - offset
                    } else {
                        size as u64
                    };

                    let mut buffer = Vec::with_capacity(actual_size as usize);
                    buffer.resize(actual_size as usize, 0);

                    let archive = self.archives.get_mut(&file.archive_index).unwrap();
                    if let Err(error) = archive.read_exact_at(&mut buffer, file.offset as u64 + offset) {
                        return reply.error(error.raw_os_error().unwrap_or(EIO));
                    }

                    return reply.data(&buffer);
                }
            } else {
                return reply.error(EISDIR);
            }
        } else {
            return reply.error(ENOENT);
        }
    }
}

impl std::convert::From<DaemonizeError> for Error {
    fn from(error: DaemonizeError) -> Self {
        Error::Other(format!("{}", error))
    }
}

pub struct MountOptions {
    pub foreground: bool,
    pub debug: bool,
}

impl MountOptions {
    #[inline]
    pub fn new() -> Self {
        MountOptions::default()
    }
}

impl Default for MountOptions {
    #[inline]
    fn default() -> Self {
        Self {
            foreground: false,
            debug: false,
        }
    }
}

pub fn mount(package: Package, mount_point: impl AsRef<Path>, options: MountOptions) -> Result<()> {
    let mut fuse_options = vec![
        OsStr::new("fsname=vpkfs"),
        OsStr::new("subtype=vpkfs"),
        OsStr::new("ro")
    ];

    let foreground;
    if options.debug {
        foreground = true;
        fuse_options.push(OsStr::new("debug"));
    } else {
        foreground = options.foreground;
    }

    if !foreground {
        let daemonize = Daemonize::new()
            .working_directory("/")
            .umask(0);
        
        daemonize.start()?;
    }

    let fs = VPKFS::new(package)?;
    fuse::mount(fs, mount_point.as_ref(), &fuse_options)?;

    Ok(())
}
