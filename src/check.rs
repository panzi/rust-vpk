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

use std::io::{Write, Read, Seek, SeekFrom};

use crc::{crc32, Hasher32};

use crate::sort::PHYSICAL_ORDER;
use crate::archive_cache::ArchiveCache;
use crate::package::{Package, Md5};
use crate::result::{Result, Error};
use crate::consts::{DIR_INDEX, BUFFER_SIZE, V2_HEADER_SIZE};
use crate::util::format_size;

pub struct CheckOptions<'a> {
    pub verbose:        bool,
    pub stop_on_error:  bool,
    pub human_readable: bool,
    pub filter:    Option<&'a [&'a str]>,
    pub alignment: Option<u32>,
}

impl CheckOptions<'_> {
    #[inline]
    pub fn new() -> Self {
        CheckOptions::default()
    }
}

impl Default for CheckOptions<'_> {
    #[inline]
    fn default() -> Self {
        Self {
            verbose:        false,
            stop_on_error:  false,
            human_readable: false,
            filter:    None,
            alignment: None,
        }
    }
}

fn check_range(arch: &mut std::fs::File, buf: &mut [u8], offset: u64, size: u64, expected: &Md5, what: &str, verbose: bool) -> std::io::Result<bool> {
    if verbose {
        print!("checking MD5 sum of {}... ", what);
        let _ = std::io::stdout().flush();
    }

    if let Err(error) = arch.seek(SeekFrom::Start(offset)) {
        if verbose {
            println!("FAILED");
        }
        return Err(error);
    }

    let mut hasher = md5::Context::new();
    let mut remaining = size;
    while remaining >= buf.len() as u64 {
        if let Err(error) = arch.read_exact(buf) {
            if verbose {
                println!("FAILED");
            }
            return Err(error);
        }
        remaining -= buf.len() as u64;
        hasher.consume(&buf);
    }

    if remaining > 0 {
        let buf = &mut buf[..remaining as usize];
        if let Err(error) = arch.read_exact(buf) {
            if verbose {
                println!("FAILED");
            }
            return Err(error);
        }
        hasher.consume(buf);
    }

    let sum = *hasher.compute();
    if expected != &sum {
        if verbose {
            println!("FAILED");
        } else {
            eprintln!("checking MD5 of {} failed", what);
        }
        return Ok(false);
    } else if verbose {
        println!("OK");
    }

    Ok(true)
}

pub fn check(package: &Package, options: CheckOptions) -> Result<()> {
    let mut digest = crc32::Digest::new(crc32::IEEE);
    let mut archs  = ArchiveCache::for_reading(package.dirpath.to_path_buf(), package.prefix.to_string());
    let mut stdout = std::io::stdout();
    let mut faild_files_count = 0usize;
    let alignment = options.alignment.unwrap_or(0);

    let fmt_size = if options.human_readable {
        |size: u64| format_size(size)
    } else {
        |size: u64| format!("{}", size)
    };

    if options.verbose {
        if alignment > 0 {
            println!("Archive      Offset   Unaligned  Inline-Size  Archive-Size       CRC32  Filename");
        } else {
            println!("Archive      Offset  Inline-Size  Archive-Size       CRC32  Filename");
        }
    }

    let files = match options.filter {
        None => {
            package.recursive_file_list(&PHYSICAL_ORDER)
        },
        Some(paths) => {
            package.recursive_file_list_from(&paths, &PHYSICAL_ORDER)?
        }
    };

    for (path, file) in files {
        let mut ok = true;
        let reminder = if alignment > 0 { file.offset % alignment } else { 0 };
        if options.verbose {
            if file.archive_index == DIR_INDEX {
                print!("    dir");
            } else {
                print!("    {:03}", file.archive_index);
            }
            print!("  {:>10}", file.offset);
            if alignment > 0 {
                print!("  {:>10}", reminder);
            }
            print!("  {:>11}  {:>12}  0x{:08x}  {}... ",
                fmt_size(file.inline_size as u64), fmt_size(file.size as u64), file.crc32,
                path);
            let _ = stdout.flush();
        }
        digest.reset();
        if let Err(error) = archs.read_file_data(file, |data| {
            digest.write(data);
            Ok(())
        }) {
            ok = false;
            if options.verbose {
                print!("FAILED, {}", error);
            } else {
                eprint!("{}: {}", path, error);
            }
        } else {
            let sum = digest.sum32();

            if options.verbose {
                if sum != file.crc32 {
                    ok = false;
                    print!("FAILED, CRC32 sum missmatch, expected: 0x{:08x}, actual: 0x{:08x}",
                        file.crc32, sum);
                }
            } else if sum != file.crc32 {
                ok = false;
                eprint!("{}: CRC32 sum missmatch, expected: 0x{:08x}, actual: 0x{:08x}",
                    path, file.crc32, sum);
            }
        }

        if reminder != 0 {
            if options.verbose {
                if ok {
                    print!("FAILED");
                }
                print!(", not aligned");
            } else {
                if ok {
                    eprint!("{}: ", path);
                } else {
                    eprint!(", ");
                }
                eprint!("not aligned, remainder: {}", reminder);
            }
            ok = false;
        }

        if ok {
            if options.verbose {
                println!("OK");
            }
        } else {
            if options.verbose {
                println!();
            } else {
                eprintln!();
            }
            if options.stop_on_error {
                return Err(Error::other("package check failed"));
            }
            faild_files_count += 1;
        }
    }

    let mut failed_md5_count = 0usize;

    if package.version > 1 {
        #[cfg(not(target_os = "windows"))]
        let mut buf = [0; BUFFER_SIZE];

        #[cfg(target_os = "windows")]
        let mut buf = Vec::with_capacity(BUFFER_SIZE);
        #[cfg(target_os = "windows")]
        buf.resize(BUFFER_SIZE, 0);

        let arch = archs.get(DIR_INDEX)?;

        if let Some(md5) = package.index_md5() {
            if options.verbose {
                println!();
            }

            match check_range(arch, &mut buf,
                    V2_HEADER_SIZE as u64,
                    package.index_size as u64,
                    md5,
                    "directory index",
                    options.verbose) {
                Ok(valid) => {
                    if !valid {
                        if options.stop_on_error {
                            return Err(Error::other("package check failed"));
                        }
                        failed_md5_count += 1;
                    }
                },
                Err(error) => {
                    return Err(Error::io_with_path(error, archs.archive_path(DIR_INDEX)));
                }
            }
        }

        if let Some(md5) = package.archive_md5s_md5() {
            match check_range(arch, &mut buf,
                    package.data_offset as u64 + package.data_size as u64,
                    package.archive_md5_size as u64,
                    md5,
                    "MD5 sum list",
                    options.verbose) {
                Ok(valid) => {
                    if !valid {
                        if options.stop_on_error {
                            return Err(Error::other("package check failed"));
                        }
                        failed_md5_count += 1;
                    }
                },
                Err(error) => {
                    return Err(Error::io_with_path(error, archs.archive_path(DIR_INDEX)));
                }
            }
        }

        if let Some(md5) = package.everything_md5() {
            // TODO: instead of reading everything twice do this check alongside?
            match check_range(arch, &mut buf,
                    0,
                    package.data_offset as u64 +
                    package.data_size as u64 +
                    package.archive_md5_size as u64 +
                    16 * 2,
                    md5,
                    "everything",
                    options.verbose) {
                Ok(valid) => {
                    if !valid {
                        if options.stop_on_error {
                            return Err(Error::other("package check failed"));
                        }
                        failed_md5_count += 1;
                    }
                },
                Err(error) => {
                    return Err(Error::io_with_path(error, archs.archive_path(DIR_INDEX)));
                }
            }
        }

        if !package.archive_md5s.is_empty() {
            if options.verbose {
                println!();
                println!("Archive      Offset        Size  MD5 Sum");
            }

            for item in &package.archive_md5s {
                let arch = archs.get(item.archive_index)?;
                let mut remaining = item.size;
                let mut hasher = md5::Context::new();

                if options.verbose {
                    if item.archive_index == DIR_INDEX {
                        print!("    dir");
                    } else {
                        print!("    {:03}", item.archive_index);
                    }

                    print!("  {:>10}  {:>10}  {:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}...  ",
                        item.offset, fmt_size(item.size as u64),
                        item.md5[0], item.md5[1], item.md5[2], item.md5[3], item.md5[4], item.md5[5], item.md5[6], item.md5[7], item.md5[8],
                        item.md5[9], item.md5[10], item.md5[11], item.md5[12], item.md5[13], item.md5[14], item.md5[15], 
                    );
                    let _ = stdout.flush();
                }

                if let Err(error) = arch.seek(SeekFrom::Start(item.offset as u64)) {
                    if options.verbose {
                        println!("FAILED");
                    }
                    return Err(Error::io_with_path(error, archs.archive_path(DIR_INDEX)));
                }

                while remaining >= BUFFER_SIZE as u32 {
                    if let Err(error) = arch.read_exact(&mut buf) {
                        if options.verbose {
                            println!("FAILED");
                        }
                        return Err(Error::io_with_path(error, archs.archive_path(item.archive_index)));
                    }
                    remaining -= BUFFER_SIZE as u32;
                    hasher.consume(&buf);
                }

                if remaining > 0 {
                    let buf = &mut buf[..remaining as usize];
                    if let Err(error) = arch.read_exact(buf) {
                        if options.verbose {
                            println!("FAILED");
                        }
                        return Err(Error::io_with_path(error, archs.archive_path(item.archive_index)));
                    }
                    hasher.consume(buf);
                }

                let sum = *hasher.compute();
                if sum != item.md5 {
                    if options.verbose {
                        println!("FAILED");
                    } else if item.archive_index == DIR_INDEX {
                        eprintln!("archive dir at offset {} with size {}: MD5 sum missmatch",
                            item.offset, item.size);
                    } else {
                        eprintln!("archive {:03} at offset {} with size {}: MD5 sum missmatch",
                        item.archive_index, item.offset, item.size);
                    }

                    failed_md5_count += 1;
                    if options.stop_on_error {
                        return Err(Error::other("package check failed"));
                    }
                } else if options.verbose {
                    println!("OK");
                }
            }
        }
    }

    if faild_files_count == 0 && failed_md5_count == 0 {
        Ok(())
    } else {
        Err(Error::other(format!("CRC32 check failed for {} file(s) and MD5 check failed for {} section(s)",
            faild_files_count, failed_md5_count)))
    }
}
