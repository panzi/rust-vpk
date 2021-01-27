use std::collections::HashMap;
use std::path::{Path};
use std::fs::{read_dir};

use crate::vpk::{Package, Result};
use crate::vpk::package::parse_path;

pub enum ArchiveOptions {
    ArchiveFromDirName,
    MaxArchiveSize(u32),
}

pub fn pack(package: impl AsRef<Path>, indir: impl AsRef<Path>, arch_opts: ArchiveOptions, max_inline_size: u16, verbose: bool) -> Result<Package> {
    let (dirpath, prefix) = parse_path(package)?;
    // TODO

    let entries = HashMap::new();

    match arch_opts {
        ArchiveOptions::ArchiveFromDirName => {
            for dirent in read_dir(indir)? {
                let dirent = dirent?;
                if dirent.file_type()?.is_dir() {
                    if let Some(name) = dirent.file_name().to_str() {
                        if name.eq("dir") {
                            // TODO
                        } else if let Ok(archive_index) = name.parse::<u16>() {
                            if archive_index < 0x7FFF {
                                // TODO
                            } else {
                                // archive_index too big
                            }
                        } else {
                            // unhandeled name
                        }
                    }
                }
            }
        },
        ArchiveOptions::MaxArchiveSize(max_size) => {
            // TODO
        }
    }

    Ok(Package {
        dirpath,
        prefix,
        version: 1,
        data_offset: 0,
        footer_offset: 0,
        footer_size: 0,
        entries,
    })
}
