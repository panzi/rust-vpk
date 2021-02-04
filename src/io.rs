use std::io::{Read, BufRead, Write, SeekFrom, Seek};

use crate::result::{Result, Error};
use crate::consts::{DIR_INDEX, TERMINATOR};
use crate::entry;

#[inline]
pub(crate) fn read_u16(file: &mut impl Read) -> std::io::Result<u16> {
    let mut buffer = [0; 2];
    file.read_exact(&mut buffer)?;

    Ok((buffer[1] as u16) << 8 | buffer[0] as u16)
}

#[inline]
pub(crate) fn read_u32(file: &mut impl Read) -> std::io::Result<u32> {
    let mut buffer = [0; 4];
    file.read_exact(&mut buffer)?;

    Ok((buffer[3] as u32) << 24 | (buffer[2] as u32) << 16 | (buffer[1] as u32) << 8 | buffer[0] as u32)
}

pub(crate) fn read_str<'a>(file: &mut impl BufRead, mut buffer: &'a mut Vec<u8>) -> Result<&'a str> {
    buffer.clear();
    file.read_until(0, &mut buffer)?;

    match buffer.last() {
        Some(0) => { buffer.pop(); }
        _ => { return Err(Error::unexpected_eof()); }
    }

    Ok(std::str::from_utf8(buffer)?)
}

pub(crate) fn read_file<R>(file: &mut R, index: usize, data_offset: u32) -> Result<entry::File>
where R: Read, R: Seek {
    let crc32         = read_u32(file)?;
    let inline_size   = read_u16(file)?;
    let archive_index = read_u16(file)?;
    let mut offset    = read_u32(file)?;
    let size          = read_u32(file)?;
    let terminator    = read_u16(file)?;
    let mut preload   = vec![0; inline_size as usize];

    if archive_index == DIR_INDEX {
        offset += data_offset;
    }

    if terminator != TERMINATOR {
        let offset = file.seek(SeekFrom::Current(0))? - 1;
        return Err(Error::illegal_terminator(terminator, offset));
    }

    file.read_exact(&mut preload)?;

    Ok(entry::File {
        index,
        crc32,
        inline_size,
        archive_index,
        offset,
        size,
        preload,
    })
}

#[inline]
pub(crate) fn write_u16(file: &mut impl Write, value: u16) -> std::io::Result<()> {
    let buffer = [value as u8, (value >> 8) as u8];
    file.write_all(&buffer)?;
    Ok(())
}

#[inline]
pub(crate) fn write_u32(file: &mut impl Write, value: u32) -> std::io::Result<()> {
    let buffer = [value as u8, (value >> 8) as u8, (value >> 16) as u8, (value >> 24) as u8];
    file.write_all(&buffer)?;
    Ok(())
}

#[inline]
pub(crate) fn write_str(file: &mut impl Write, value: &str) -> std::io::Result<()> {
    file.write_all(value.as_bytes())?;
    file.write_all(&[0])?;
    Ok(())
}

pub(crate) fn write_file(file: &mut impl Write, entry: &entry::File, dir_size: u32) -> std::io::Result<()> {
    write_u32(file, entry.crc32)?;
    write_u16(file, entry.inline_size)?;
    write_u16(file, entry.archive_index)?;
    write_u32(file, if entry.archive_index == DIR_INDEX && entry.offset != 0 {
        entry.offset - dir_size
    } else {
        entry.offset
    })?;
    write_u32(file, entry.size)?;
    write_u16(file, TERMINATOR)?;

    file.write_all(&entry.preload)?;

    Ok(())
}

#[cfg(target_os = "linux")]
pub fn transfer(in_file: &mut std::fs::File, out_file: &mut std::fs::File, count: usize) -> std::io::Result<()> {
    use std::os::unix::io::AsRawFd;

    let in_fd  = in_file.as_raw_fd();
    let out_fd = out_file.as_raw_fd();

    let mut remaining = count;
    while remaining > 0 {
        unsafe {
            let result = libc::sendfile(out_fd, in_fd, std::ptr::null_mut(), remaining as libc::size_t);

            if result < 0 {
                return Err(std::io::Error::last_os_error());
            }

            remaining -= result as usize;
        }
    }

    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn transfer(in_file: &mut std::fs::File, out_file: &mut std::fs::File, count: usize) -> std::io::Result<()> {
    use crate::consts::BUFFER_SIZE;

    let mut buf = [0u8; BUFFER_SIZE];

    let mut remaining = count;
    while remaining >= BUFFER_SIZE {
        in_file.read_exact(&mut buf)?;
        out_file.write_all(&buf)?;
        remaining -= BUFFER_SIZE;
    }

    if remaining > 0 {
        let buf = &mut buf[..remaining];
        in_file.read_exact(buf)?;
        out_file.write_all(buf)?;
    }

    Ok(())
}
