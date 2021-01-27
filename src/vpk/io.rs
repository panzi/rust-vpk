use std::io::{Read, BufRead, Write, SeekFrom, Seek};

use crate::vpk;
use crate::vpk::{Result, Error, DIR_INDEX, TERMINATOR};

#[inline]
pub(super) fn read_u16(file: &mut impl Read) -> Result<u16> {
    let mut buffer = [0; 2];
    file.read_exact(&mut buffer)?;

    Ok((buffer[1] as u16) << 8 | buffer[0] as u16)
}

#[inline]
pub(super) fn read_u32(file: &mut impl Read) -> Result<u32> {
    let mut buffer = [0; 4];
    file.read_exact(&mut buffer)?;

    Ok((buffer[3] as u32) << 24 | (buffer[2] as u32) << 16 | (buffer[1] as u32) << 8 | buffer[0] as u32)
}

pub(super) fn read_str<'a>(file: &mut impl BufRead, mut buffer: &'a mut Vec<u8>) -> Result<&'a str> {
    buffer.clear();
    file.read_until(0, &mut buffer)?;

    match buffer.last() {
        Some(0) => { buffer.pop(); }
        _ => { return Err(Error::UnexpectedEOF); }
    }

    Ok(std::str::from_utf8(buffer)?)
}

pub(super) fn read_file<R>(file: &mut R, index: usize, data_offset: u32) -> Result<vpk::entry::File>
where R: Read, R: Seek {
    let crc32         = read_u32(file)?;
    let inline_size   = read_u16(file)?;
    let archive_index = read_u16(file)?;
    let mut offset    = read_u32(file)?;
    let size          = read_u32(file)?;
    let mut preload   = vec![0; inline_size as usize];
    let terminator    = read_u16(file)?;

    if archive_index == DIR_INDEX {
        offset += data_offset;
    }

    if terminator != TERMINATOR {
        let offset = file.seek(SeekFrom::Current(0))? - 1;
        return Err(Error::IllegalTerminator { terminator, offset });
    }

    file.read_exact(&mut preload)?;

    Ok(vpk::entry::File {
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
pub(super) fn write_u16(file: &mut impl Write, value: u16) -> Result<()> {
    let buffer = [value as u8, (value >> 8) as u8];
    file.write_all(&buffer)?;
    Ok(())
}

#[inline]
pub(super) fn write_u32(file: &mut impl Write, value: u32) -> Result<()> {
    let buffer = [value as u8, (value >> 8) as u8, (value >> 16) as u8, (value >> 24) as u8];
    file.write_all(&buffer)?;
    Ok(())
}

#[inline]
pub(super) fn write_str(file: &mut impl Write, value: &str) -> Result<()> {
    file.write_all(value.as_bytes())?;
    file.write_all(&[0])?;
    Ok(())
}

pub(super) fn write_file(file: &mut impl Write, entry: &vpk::entry::File) -> Result<()> {
    write_u32(file, entry.crc32)?;
    write_u16(file, entry.inline_size)?;
    write_u16(file, entry.archive_index)?;
    write_u32(file, entry.offset)?;
    write_u32(file, entry.size)?;
    write_u16(file, 0xFFFF)?;

    file.write_all(&entry.preload)?;

    Ok(())
}
