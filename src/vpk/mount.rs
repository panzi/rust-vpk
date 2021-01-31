use std::path::{Path, PathBuf};
use std::ffi::OsStr;
use std::fs;
use std::collections::HashMap;
use std::os::linux::fs::MetadataExt;
use std::time::SystemTime;

use cntr_fuse as fuse;
use fuse::{Filesystem, FileType, Request, ReplyEntry, FileAttr, ReplyAttr, ReplyXattr, ReplyEmpty, ReplyOpen, ReplyDirectory, ReplyStatfs, ReplyRead, FUSE_ROOT_ID};
use daemonize::{Daemonize, DaemonizeError};
use libc::{c_int, ENOSYS, ENOENT, EISDIR, EACCES, ENOTDIR, ENOATTR};

use crate::vpk::{self, Error, Package, Entry, entry::File, DIR_INDEX};
use crate::vpk::util::{archive_path};

struct Dir {
    children: HashMap<String, u64>,
}

enum INodeData {
    File(File),
    Dir(Dir),
}

struct INode {
    parent: u64,
    data: INodeData,
    attrs: FileAttr,
}

impl INode {
    fn is_dir(&self) -> bool {
        match self.data {
            INodeData::Dir(_)  => true,
            INodeData::File(_) => false,
        }
    }

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
}

impl VPKFS {
    pub fn new(package: &Package) -> vpk::Result<Self> {
        let path = package.archive_path(DIR_INDEX);
        let meta = match fs::metadata(&path) {
            Err(error) => return Err(Error::IOWithPath(error, path)),
            Ok(meta) => meta,
        };

        let mut vpkfs = Self {
            dirpath: package.dirpath.to_owned(),
            prefix: package.prefix.to_owned(),
            archives: HashMap::new(),
            inodes: HashMap::new(),
            next_inode: FUSE_ROOT_ID + 1,
            
            atime:  meta.st_atime().into(), // TODO
            mtime:  meta.st_mtime().into(),
            ctime:  meta.st_ctime().into(),
            crtime: meta.created()?,

            uid:    meta.st_uid(),
            gid:    meta.st_gid(),

            blksize: meta.st_blksize(),
        };

        let mut fsdir = Dir {
            children: HashMap::new()
        };

        vpkfs.init(&package.entries, FUSE_ROOT_ID, &mut fsdir.children)?;

        let mut attrs = FileAttr {
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
            attrs.size += name.len() as u64 + 1;
            if child.is_dir() {
                attrs.nlink += 1;
            }
        }

        attrs.blocks = if attrs.size != 0 { 1 + ((attrs.size - 1) / vpkfs.blksize) } else { 0 };

        vpkfs.inodes.insert(FUSE_ROOT_ID, INode {
            parent: FUSE_ROOT_ID,
            data: INodeData::Dir(fsdir),
            attrs,
        });

        Ok(vpkfs)
    }

    fn init(&mut self, entries: &HashMap<String, Entry>, parent_inode: u64, parent_entries: &mut HashMap<String, u64>) -> vpk::Result<()> {
        for (name, entry) in entries {
            let inode = self.next_inode;
            self.next_inode += 1;
            parent_entries.insert(name.to_owned(), inode);

            match entry {
                Entry::Dir(dir) => {
                    let mut fsdir = Dir {
                        children: HashMap::new()
                    };
                    self.init(&dir.children, inode, &mut fsdir.children)?;

                    let mut attrs = FileAttr {
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
                        attrs.size += name.len() as u64 + 1;
                        if child.is_dir() {
                            attrs.nlink += 1;
                        }
                    }

                    attrs.blocks = if attrs.size != 0 { 1 + ((attrs.size - 1) / self.blksize) } else { 0 };

                    self.inodes.insert(inode, INode {
                        parent: parent_inode,
                        data: INodeData::Dir(fsdir),
                        attrs,
                    });
                },
                Entry::File(file) => {
                    let mut attrs = FileAttr {
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

                    attrs.blocks = if attrs.size != 0 { 1 + ((attrs.size - 1) / self.blksize) } else { 0 };

                    self.inodes.insert(inode, INode {
                        parent: parent_inode,
                        data: INodeData::File(file.clone()),
                        attrs,
                    });
                    let archive_index = file.archive_index;
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

impl<'a> Filesystem for VPKFS {
    // TODO
    fn lookup(&mut self, _req: &Request, _parent: u64, _name: &OsStr, reply: ReplyEntry) {
        reply.error(ENOSYS);
    }

    fn getattr(&mut self, _req: &Request, _ino: u64, reply: ReplyAttr) {
        reply.error(ENOSYS);
    }

    fn getxattr(&mut self, _req: &Request, _ino: u64, _name: &OsStr, _size: u32, reply: ReplyXattr) {
        reply.error(ENOSYS);
    }

    fn listxattr(&mut self, _req: &Request, _ino: u64, _size: u32, reply: ReplyXattr) {
        reply.error(ENOSYS);
    }

    fn access(&mut self, _req: &Request, _ino: u64, _mask: u32, reply: ReplyEmpty) {
        reply.error(ENOSYS);
    }

    fn opendir(&mut self, _req: &Request, _ino: u64, _flags: u32, reply: ReplyOpen) {
        reply.opened(0, 0);
    }

    fn readdir(&mut self, _req: &Request, _ino: u64, _fh: u64, _offset: i64, reply: ReplyDirectory) {
        reply.error(ENOSYS);
    }

    fn statfs(&mut self, _req: &Request, _ino: u64, reply: ReplyStatfs) {
        reply.statfs(0, 0, 0, 0, 0, 512, 255, 0);
    }

    fn open(&mut self, _req: &Request, _ino: u64, _flags: u32, reply: ReplyOpen) {
        reply.opened(0, 0);
    }

    fn read(&mut self, _req: &Request, _ino: u64, _fh: u64, _offset: i64, _size: u32, reply: ReplyRead) {
        reply.error(ENOSYS);
    }
}

impl std::convert::From<DaemonizeError> for Error {
    fn from(error: DaemonizeError) -> Self {
        Error::Other(format!("{}", error))
    }
}

pub fn mount(package: &Package, mount_point: impl AsRef<Path>, mut foreground: bool, debug: bool) -> vpk::Result<()> {
    let mut options = vec![
        OsStr::new("fsname=vpkfs"),
        OsStr::new("subtype=vpkfs"),
        OsStr::new("ro")
    ];

    if debug {
        foreground = true;
        options.push(OsStr::new("debug"));
    }

    if !foreground {
        let daemonize = Daemonize::new()
            .working_directory("/")
            .umask(0);
        
        daemonize.start()?;
    }

    let fs = VPKFS::new(package)?;
    fuse::mount(fs, mount_point.as_ref(), &options)?;

    Ok(())
}
