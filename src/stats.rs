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

use std::fs;
use std::collections::HashMap;

use crate::package::{Package, Md5};
use crate::result::Result;
use crate::consts::DIR_INDEX;
use crate::entry::Entry;
use crate::util::{format_size, print_headless_table, print_table, Align::*};

pub struct ArchStats {
    file_count: usize,
    file_with_data_count: usize,
    file_size: Option<u64>,
    used_size: u64,
    io_error: Option<std::io::Error>,
}

impl ArchStats {
    pub fn file_count(&self) -> usize {
        self.file_count
    }

    pub fn file_with_data_count(&self) -> usize {
        self.file_with_data_count
    }

    pub fn file_size(&self) -> Option<u64> {
        self.file_size
    }

    pub fn used_size(&self) -> u64 {
        self.used_size
    }

    pub fn io_error(&self) -> &Option<std::io::Error> {
        &self.io_error
    }
}

pub struct ExtStats {
    file_count: usize,
    sum_size: u64,
}

impl ExtStats {
    pub fn new(file_count: usize, sum_size: u64) -> Self {
        Self {
            file_count,
            sum_size,
        }
    }

    pub fn file_count(&self) -> usize {
        self.file_count
    }

    pub fn sum_size(&self) -> u64 {
        self.sum_size
    }
}

pub struct Stats<'a> {
    file_count: usize,
    dir_count:  usize,
    max_inline_size: u16,
    max_size:        u32,
    max_full_size:   u32,
    extmap:  HashMap<&'a str, ExtStats>,
    archmap: HashMap<u16, ArchStats>,
    error_count: usize,
    sum_used_size:    u64,
    sum_archive_size: u64,
}

impl<'a> Stats<'a> {
    pub fn new() -> Self {
        Stats {
            file_count: 0,
            dir_count: 0,
            max_inline_size: 0,
            max_size: 0,
            max_full_size: 0,
            extmap:  HashMap::new(),
            archmap: HashMap::new(),
            error_count: 0,
            sum_used_size: 0,
            sum_archive_size: 0,
        }
    }

    pub fn file_count(&self) -> usize {
        self.file_count
    }

    pub fn dir_count(&self) -> usize {
        self.dir_count
    }

    pub fn max_inline_size(&self) -> u16 {
        self.max_inline_size
    }

    pub fn max_size(&self) -> u32 {
        self.max_size
    }

    pub fn max_full_size(&self) -> u32 {
        self.max_full_size
    }

    pub fn extensions(&self) -> &HashMap<&'a str, ExtStats> {
        &self.extmap
    }

    pub fn archives(&self) -> &HashMap<u16, ArchStats> {
        &self.archmap
    }

    pub fn error_count(&self) -> usize {
        self.error_count
    }

    pub fn sum_used_size(&self) -> u64 {
        self.sum_used_size
    }

    pub fn sum_archive_size(&self) -> u64 {
        self.sum_archive_size
    }

    pub fn scan(package: &'a Package) -> Self {
        let mut stats = Self::new();
        stats.scan_entries(&package.entries);

        for (archive_index, archstat) in stats.archmap.iter_mut() {
            let path = package.archive_path(*archive_index);

            if *archive_index == DIR_INDEX {
                archstat.used_size += package.data_offset as u64;
            }

            stats.sum_used_size += archstat.used_size;

            match fs::metadata(&path) {
                Err(error) => {
                    archstat.io_error = Some(error);
                    stats.error_count += 1;
                },
                Ok(meta) => {
                    let file_size = meta.len();
                    archstat.file_size = Some(file_size);

                    stats.sum_archive_size += file_size;
                }
            }
        }

        stats
    }

    fn scan_entries(&mut self, entries: &'a HashMap<String, Entry>) {
        for (name, entry) in entries {
            match entry {
                Entry::Dir(dir) => {
                    self.dir_count += 1;
                    self.scan_entries(&dir.children);
                },
                Entry::File(file) => {
                    self.file_count += 1;
                    let dot_index = name.rfind('.').unwrap();
                    let ext = &name[dot_index + 1..];

                    if self.extmap.get_mut(ext).map(|stats| {
                        stats.file_count += 1;
                        stats.sum_size += file.inline_size as u64 + file.size as u64;
                    }).is_none() {
                        self.extmap.insert(
                            ext,
                            ExtStats::new(1, file.inline_size as u64 + file.size as u64)
                        );
                    }

                    if self.archmap.get_mut(&file.archive_index).map(|stats| {
                        stats.file_count += 1;
                        stats.used_size += file.size as u64;
                        if file.size > 0 {
                            stats.file_with_data_count += 1;
                        }
                    }).is_none() {
                        self.archmap.insert(
                            file.archive_index,
                            ArchStats {
                                file_count: 1,
                                file_with_data_count: (file.size > 0) as usize,
                                file_size: None,
                                used_size: file.size as u64,
                                io_error: None,
                            }
                        );
                    }

                    if file.inline_size > self.max_inline_size {
                        self.max_inline_size = file.inline_size;
                    }

                    if file.size > self.max_size {
                        self.max_size = file.size;
                    }

                    let full_size = file.inline_size as u32 + file.size;
                    if full_size > self.max_full_size {
                        self.max_full_size = full_size;
                    }
                }
            }
        }
    }
}

fn format_md5(md5: Option<&Md5>) -> String {
    if let Some(md5) = md5 {
        format!(
            "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            md5[0], md5[1], md5[2],  md5[ 3], md5[ 4], md5[ 5], md5[ 6], md5[ 7],
            md5[8], md5[9], md5[10], md5[11], md5[12], md5[13], md5[14], md5[15],
        )
    } else {
        "".to_owned()
    }
}

pub fn stats(package: &Package, human_readable: bool) -> Result<()> {
    let stats = Stats::scan(package);

    let fmt_size = if human_readable {
        |size: u64| format_size(size)
    } else {
        |size: u64| format!("{}", size)
    };

    let wasted = if stats.sum_used_size > stats.sum_archive_size {
        "Error: Used size bigger than file size!".to_owned()
    } else {
        fmt_size(stats.sum_archive_size - stats.sum_used_size)
    };

    print_headless_table(&[
        vec!["VPK Version:", &format!("{}", package.version)],
        vec!["Index Size:",  &fmt_size(package.data_offset as u64)],
        vec![],
        vec!["File Count:",      &format!("{}", stats.file_count)],
        vec!["Directory Count:", &format!("{}", stats.dir_count)],
        vec!["Extension Count:", &format!("{}", stats.extmap.len())],
        vec!["Archive Count:",   &format!("{}", stats.archmap.len())],
        vec!["IO Error Count:",  &format!("{}", stats.error_count)],
        vec![],
        vec!["Max Inline-Size:",       &fmt_size(stats.max_inline_size as u64)],
        vec!["Max Non-Inline-Size:",   &fmt_size(stats.max_size as u64)],
        vec!["Max Full-Size:",         &fmt_size(stats.max_full_size as u64)],
        vec!["Sum Used Size:",         &fmt_size(stats.sum_used_size)],
        vec!["Sum Archive File Size:", &fmt_size(stats.sum_archive_size)],
        vec!["Wasted Size:",           &wasted],
    ], &[Left, Right]);

    let header_size = package.header_size();
    if package.version > 1 {
        println!();

        print_headless_table(&[
            vec!["Archive MD5 Count:", &format!("{}", package.archive_md5s.len())],
            vec!["Index MD5:",         if package.index_md5().is_some()        { "Yes" } else { "No" }, &format_md5(package.index_md5())],
            vec!["Archive MD5s MD5:",  if package.archive_md5s_md5().is_some() { "Yes" } else { "No" }, &format_md5(package.archive_md5s_md5())],
            vec!["Everything MD5:",    if package.everything_md5().is_some()   { "Yes" } else { "No" }, &format_md5(package.everything_md5())],
            vec!["Public Key:",        if package.public_key().is_some()       { "Yes" } else { "No" }],
            vec!["Signature:",         if package.signature().is_some()        { "Yes" } else { "No" }],
        ], &[Left, Right, Left]);

        let archive_md5s_offset = package.data_offset + package.data_size;
        let other_md5s_offset   = archive_md5s_offset + package.other_md5_size;
        let signature_offset    = other_md5s_offset   + package.signature_size;

        println!();

        print_table(
            &["Section", "Offset", "Size"],
            &[Left,      Right,    Right],
            &[
                vec!["Header:",       "0",                                 &fmt_size(header_size as u64)],
                vec!["Index:",        &format!("{}", header_size),         &fmt_size(package.index_size as u64)],
                vec!["Data:",         &format!("{}", package.data_offset), &fmt_size(package.data_size as u64)],
                vec!["Archive MD5s:", &format!("{}", archive_md5s_offset), &fmt_size(package.archive_md5_size as u64)],
                vec!["Other MD5s:",   &format!("{}", other_md5s_offset),   &fmt_size(package.other_md5_size as u64)],
                vec!["Signature:",    &format!("{}", signature_offset),    &fmt_size(package.signature_size as u64)],
            ]
        );
    } else {
        println!();

        print_table(
            &["Section", "Offset", "Size"],
            &[Left,      Right,    Right],
            &[
                vec!["Header:", "0",                                 &fmt_size(header_size as u64)],
                vec!["Index:",  &format!("{}", header_size),         &fmt_size(package.index_size as u64)],
                vec!["Data:",   &format!("{}", package.data_offset), &fmt_size(package.data_size as u64)],
            ]
        );
    }

    println!();

    let mut exts: Vec<&str> = stats.extmap.keys().map(|ext| *ext).collect();
    exts.sort();

    print_table(
        &["Format", "File Count", "Sum Size"],
        &[Left,     Right,        Right],
        &exts.iter().map(|ext| {
            let stats = stats.extmap.get(ext).unwrap();
            vec![
                (*ext).to_owned(),
                format!("{}", stats.file_count),
                fmt_size(stats.sum_size)
            ]
        }).collect::<Vec<_>>()
    );

    println!();

    let mut arch_indices: Vec<u16> = stats.archmap.keys().map(|index| *index).collect();
    arch_indices.sort();

    let body = arch_indices.iter().map(|archive_index| {
        let archstats = stats.archmap.get(archive_index).unwrap();
        let mut row = vec![
            if *archive_index == DIR_INDEX {
                "dir".to_owned()
            } else {
                format!("{:03}", archive_index)
            },
            format!("{}", archstats.file_count),
            format!("{}", archstats.file_with_data_count),
            if let Some(size) = archstats.file_size {
                fmt_size(size)
            } else {
                "".to_owned()
            },
            fmt_size(archstats.used_size),
            if let Some(size) = archstats.file_size {
                fmt_size(size - archstats.used_size)
            } else {
                "".to_owned()
            }
        ];

        if let Some(io_error) = &archstats.io_error {
            row.push(format!("{}", io_error));
        }

        row
    }).collect::<Vec<_>>();

    if stats.error_count > 0 {
        print_table(
            &["Archive", "File Count", "File With Data Count", "File Size", "Used Size", "Wasted Size", "IO Error"],
            &[Right,     Right,        Right,                  Right,       Right,       Right,         Left],
            &body
        );
    } else {
        print_table(
            &["Archive", "File Count", "File With Data Count", "File Size", "Used Size", "Wasted Size"],
            &[Right,     Right,        Right,                  Right,       Right,       Right],
            &body
        );
    }

    Ok(())
}
