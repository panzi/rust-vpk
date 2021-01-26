use std::collections::{HashSet, HashMap};
use std::io::Write;

use crate::vpk;
use crate::vpk::{Result, Error};
use crate::vpk::entry::Entry;

#[derive(Debug)]
pub enum Sort {
    Name,
    Size,
    ArchiveAndOffset,
    Index,
}

impl std::convert::TryFrom<&str> for Sort {
    type Error = vpk::Error;

    fn try_from(value: &str) -> Result<Sort> {
        if value.eq_ignore_ascii_case("name") {
            Ok(Sort::Name)
        } else if value.eq_ignore_ascii_case("size") {
            Ok(Sort::Size)
        } else if value.eq_ignore_ascii_case("offset") || value.eq_ignore_ascii_case("archive-and-offset") {
            Ok(Sort::ArchiveAndOffset)
        } else if value.eq_ignore_ascii_case("index") {
            Ok(Sort::Index)
        } else {
            Err(vpk::Error::IllegalArgument {
                name: "--sort".to_owned(),
                value: value.to_owned()
            })
        }
    }
}

impl Default for Sort {
    fn default() -> Self { Sort::Name }
}

#[derive(Debug)]
pub enum Filter {
    None,
    Paths(HashSet<String>)
}

fn fill_table<'a>(table: &mut Vec<(&'a vpk::entry::File, Vec<String>)>, prefix: &str, entries: &'a HashMap<String, Entry>, human_readable: bool) {
    for (name, ref entry) in entries {
        let path = format!("{}/{}", prefix, name); // inefficient
        match entry {
            Entry::Dir(dir) => {
                fill_table(table, &path, &dir.children, human_readable);
            },
            Entry::File(file) => {
                insert_file(table, &path, file, human_readable);
            }
        }
    }
}

fn format_size(size: u32) -> String {
    if size >= 1024 * 1024 * 1024 {
        format!("{} G", size / (1024 * 1024 * 1024))
    } else if size >= 1024 * 1024 {
        format!("{} M", size / (1024 * 1024))
    } else if size >= 1024 {
        format!("{} K", size / 1024)
    } else {
        format!("{}", size)
    }
}

fn insert_file<'a>(table: &mut Vec<(&'a vpk::entry::File, Vec<String>)>, path: &str, file: &'a vpk::entry::File, human_readable: bool) {
    let size = file.inline_size as u32 + file.size;
    table.push((
        file,
        vec![
            format!("{}", file.index),
            format!("{}", file.archive_index),
            format!("{}", file.offset),
            if human_readable { format_size(size) } else { format!("{}", size) },
            format!("0x{:04x}", file.crc32),
            path.to_owned(),
        ]
    ));
}

fn print_row(row: &Vec<impl AsRef<str>>, lens: &Vec<usize>, right_align: &Vec<bool>) {
    let mut first = true;
    for ((cell, len), right_align) in row.iter().zip(lens.iter()).zip(right_align.iter()) {
        if first {
            first = false;
        } else {
            print!("  ");
        }

        if *right_align {
            print!("{:>1$}", cell.as_ref(), *len);
        } else {
            print!("{:<1$}", cell.as_ref(), *len);
        }
    }

    println!();
}

pub fn list(archive: &vpk::Archive, sort: Sort, human_readable: bool, filter: &Filter) -> Result<()> {
    let mut table: Vec<(&vpk::entry::File, Vec<String>)> = Vec::new();

    match filter {
        Filter::None => {
            for (name, entry) in archive.root() {
                match entry {
                    Entry::Dir(dir) => {
                        fill_table(&mut table, name, &dir.children, human_readable);
                    },
                    Entry::File(file) => {
                        insert_file(&mut table, name, file, human_readable);
                    }
                }
            }
        },
        Filter::Paths(paths) => {
            for path in paths {
                let entry = archive.get(path);
                match entry {
                    None => {
                        return Err(Error::NoSuchEntry(path.to_owned()));
                    },
                    Some(Entry::Dir(dir)) => {
                        fill_table(&mut table, path, &dir.children, human_readable);
                    },
                    Some(Entry::File(file)) => {
                        insert_file(&mut table, path, file, human_readable);
                    }
                }
            }
        }
    }

    match sort {
        Sort::Name => {
            table.sort_by(|lhs, rhs| lhs.1[4].cmp(&rhs.1[4]));
        },
        Sort::Size => {
            table.sort_by_key(|rec| rec.0.inline_size as u32 + rec.0.size);
        },
        Sort::ArchiveAndOffset => {
            table.sort_by(|lhs, rhs| {
                let cmp = lhs.0.archive_index.cmp(&rhs.0.archive_index);
                match cmp {
                    std::cmp::Ordering::Equal => {
                        lhs.0.offset.cmp(&rhs.0.offset)
                    },
                    _ => cmp
                }
            });
        },
        Sort::Index => {
            table.sort_by_key(|rec| rec.0.index);
        },
    }

    let header = vec!["Index", "Archive", "Offset", "Size", "CRC32", "Filename"];
    let right_align = vec![true, true, true, true, true, false];
    // TODO: maybe count graphemes? needs extra lib. haven't seen non-ASCII filenames anyway
    let mut lens: Vec<usize> = header.iter().map(|x| x.chars().count()).collect();
    for (_, row) in table.iter() {
        for (cell, max_len) in row.iter().zip(lens.iter_mut()) {
            let len = cell.chars().count();
            if len > *max_len {
                *max_len = len;
            }
        }
    }

    print_row(&header, &lens, &right_align);
    let mut first = true;
    let mut stdout = std::io::stdout();
    for len in lens.iter() {
        let mut len = *len;
        if first {
            first = false;
        } else {
            len += 2;
        }

        while len > 0 {
            stdout.write(&['-' as u8])?;
            len -= 1;
        }
    }
    println!();

    for (_, row) in table.iter() {
        print_row(row, &lens, &right_align);
    }

    Ok(())
}
