use std::io::Write;

use crc::{crc32, Hasher32};

use crate::sort::PHYSICAL_ORDER;
use crate::archive_cache::ArchiveCache;
use crate::package::Package;
use crate::result::{Result, Error};
use crate::consts::DIR_INDEX;
use crate::util::format_size;

pub struct CheckOptions<'a> {
    pub verbose: bool,
    pub stop_on_error: bool,
    pub human_readable: bool,
    pub filter: Option<&'a [&'a str]>,
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
            verbose: false,
            stop_on_error: false,
            human_readable: false,
            filter: None,
        }
    }
}

pub fn check(package: &Package, options: CheckOptions) -> Result<()> {
    let mut digest = crc32::Digest::new(crc32::IEEE);
    let mut archs = ArchiveCache::for_reading(package.dirpath.to_path_buf(), package.prefix.to_string());
    let mut stdout = std::io::stdout();
    let mut faild_count = 0usize;

    let fmt_size = if options.human_readable {
        |size: u64| format_size(size)
    } else {
        |size: u64| format!("{}", size)
    };

    if options.verbose {
        println!("Archive      Offset  Inline-Size  Archive-Size       CRC32  Filename");
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
        if options.verbose {
            if file.archive_index == DIR_INDEX {
                print!("    dir");
            } else {
                print!("    {:03}", file.archive_index);
            }
            print!("  {:>10}  {:>11}  {:>12}  0x{:08x}  {}... ",
                file.offset, fmt_size(file.inline_size as u64), fmt_size(file.size as u64), file.crc32,
                path);
            let _ = stdout.flush();
        }
        digest.reset();
        if let Err(error) = archs.read_file_data(file, |data| {
            digest.write(data);
            Ok(())
        }) {
            faild_count += 1;
            if options.verbose {
                println!("FAILED, {}", error);
            } else {
                eprintln!("{}: {}", path, error);
            }
        } else {
            let sum = digest.sum32();

            if options.verbose {
                if sum == file.crc32 {
                    println!("OK");
                } else {
                    faild_count += 1;
                    println!("FAILED, CRC32 sum missmatch, expected: 0x{:08x}, actual: 0x{:08x}", file.crc32, sum);
                }
            } else if sum != file.crc32 {
                faild_count += 1;
                eprintln!("{}: CRC32 sum missmatch, expected: 0x{:08x}, actual: 0x{:08x}",
                    path, file.crc32, sum);
            }
        }

        if options.stop_on_error && faild_count > 0 {
            return Err(Error::Other("package check failed".to_owned()));
        }
    }

    if faild_count == 0 {
        Ok(())
    } else {
        Err(Error::Other(format!("check failed for {} files", faild_count)))
    }
}
