
use std::io::Write;

use crc::{crc32, Hasher32};

use crate::sort::PHYSICAL_ORDER;
use crate::archive_cache::ArchiveCache;
use crate::package::Package;
use crate::result::{Result, Error};

pub fn check(package: &Package, verbose: bool, stop_on_error: bool) -> Result<()> {
    let mut digest = crc32::Digest::new(crc32::IEEE);
    let mut archs = ArchiveCache::for_reading(package.dirpath.to_path_buf(), package.prefix.to_string());
    let mut stdout = std::io::stdout();
    let mut ok = true;

    for (path, file) in package.recursive_file_list(&PHYSICAL_ORDER) {
        if verbose {
            print!("checking archive={} offset={} inline={} size={} path={}... ",
                file.archive_index, file.offset, file.inline_size, file.size,
                path);
            let _ = stdout.flush();
        }
        digest.reset();
        if let Err(error) = archs.read_file_data(file, |data| {
            digest.write(data);
            Ok(())
        }) {
            ok = false;
            if verbose {
                println!("FAILED, {}", error);
            } else {
                eprintln!("{}: {}", path, error);
            }
        } else {
            let sum = digest.sum32();

            if verbose {
                if sum == file.crc32 {
                    println!("OK");
                } else {
                    ok = false;
                    println!("FAILED, CRC32 sum missmatch, expected: 0x{:08x}, actual: 0x{:08x}", file.crc32, sum);
                }
            } else if sum != file.crc32 {
                ok = false;
                eprintln!("{}: CRC32 sum missmatch, expected: 0x{:08x}, actual: 0x{:08x}",
                    path, file.crc32, sum);
            }
        }

        if stop_on_error && !ok {
            return Err(Error::Other("package check failed".to_owned()));
        }
    }

    if ok {
        Ok(())
    } else {
        Err(Error::Other("package check failed".to_owned()))
    }
}
