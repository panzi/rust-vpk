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

use std::path::{Path};
use std::io::{Write};
use std::fs;

use crc::{crc32, Hasher32};

use crate::sort::PHYSICAL_ORDER;
use crate::archive_cache::ArchiveCache;
use crate::package::Package;
use crate::result::{Result, Error};
use crate::util::split_path;
use crate::consts::DIR_INDEX;

pub struct UnpackOptions<'a> {
    pub filter:               Option<&'a [&'a str]>,
    pub verbose:              bool,
    pub check:                bool,
    pub dirname_from_archive: bool,
}

impl UnpackOptions<'_> {
    #[inline]
    pub fn new() -> Self {
        UnpackOptions::default()
    }
}

impl Default for UnpackOptions<'_> {
    #[inline]
    fn default() -> Self {
        Self {
            filter:               None,
            verbose:              false,
            check:                false,
            dirname_from_archive: false,
        }
    }
}

pub fn unpack(package: &Package, outdir: impl AsRef<Path>, options: UnpackOptions) -> Result<()> {
    let mut digest = crc32::Digest::new(crc32::IEEE);
    let mut archs = ArchiveCache::for_reading(package.dirpath.to_path_buf(), package.prefix.to_string());

    let files = match options.filter {
        None => package.recursive_file_list(&PHYSICAL_ORDER),
        Some(paths) => package.recursive_file_list_from(paths, &PHYSICAL_ORDER)?,
    };

    for (path, file) in files {
        let mut outpath = outdir.as_ref().to_path_buf();

        if options.dirname_from_archive {
            if file.archive_index == DIR_INDEX {
                outpath.push("dir");
            } else {
                outpath.push(format!("{:03}", file.archive_index));
            }
        }

        for (_, item, _) in split_path(&path) {
            outpath.push(item);
        }

        if options.verbose {
            println!("writing {:?}", outpath);
        }

        if let Err(error) = fs::create_dir_all(outpath.parent().unwrap()) {
            return Err(Error::io_with_path(error, outpath));
        }

        match fs::File::create(&outpath) {
            Ok(mut writer) => {
                if options.check {
                    digest.reset();
                    archs.read_file_data(file, |data| {
                        if let Err(error) = writer.write_all(data) {
                            return Err(Error::io_with_path(error, outpath.to_path_buf()));
                        }
                        digest.write(data);
                        Ok(())
                    })?;

                    let sum = digest.sum32();
                    if sum != file.crc32 {
                        return Err(Error::other(format!(
                            "{}: CRC32 sum missmatch, expected: 0x{:08x}, actual: 0x{:08x}",
                            path, file.crc32, sum)));
                    }
                } else {
                    match archs.transfer(file, &mut writer) {
                        Err(error) => return if error.path.is_none() {
                            Err(error.with_path(outpath))
                        } else {
                            Err(error)
                        },
                        Ok(()) => {}
                    }
                }
            },
            Err(error) => {
                return Err(Error::io_with_path(error, outpath));
            }
        }
    }
    Ok(())
}
