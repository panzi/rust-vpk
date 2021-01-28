use std::path::{Path};
use std::io::{Write, BufWriter};
use std::fs;

use crc::{crc32, Hasher32};

use crate::vpk::sort::PHYSICAL_ORDER;
use crate::vpk::archive_cache::ArchiveCache;
use crate::vpk::{Package, Result, Filter, Error};
use crate::vpk::util::vpk_path_to_fs;

pub fn unpack(package: &Package, outdir: impl AsRef<Path>, filter: &Filter, verbose: bool, check: bool) -> Result<()> {
    let mut digest = crc32::Digest::new(crc32::IEEE);
    let mut archs = ArchiveCache::for_reading(package.dirpath.to_path_buf(), package.prefix.to_string());

    let files = match filter {
        Filter::None => package.recursive_file_list(&PHYSICAL_ORDER),
        Filter::Paths(paths) => package.recursive_file_list_from(paths, &PHYSICAL_ORDER)?,
    };

    for (path, file) in files {
        let outpath = vpk_path_to_fs(&outdir, &path);
        if verbose {
            println!("writing {:?}", outpath);
        }

        fs::create_dir_all(outpath.parent().unwrap())?;

        let mut writer = BufWriter::new(fs::File::create(&outpath)?);

        if check {
            digest.reset();
            archs.read_file_data(file, |data| {
                writer.write_all(data)?;
                digest.write(data);
                Ok(())
            })?;

            let sum = digest.sum32();
            if sum != file.crc32 {
                return Err(Error::Other(format!("{}: CRC32 sum missmatch, expected: 0x{:08x}, actual: 0x{:08x}", path, file.crc32, sum)));
            }
        } else {
            archs.read_file_data(file, |data| {
                writer.write_all(data)?;
                Ok(())
            })?;
        }
    }
    Ok(())
}
