use std::fs;
use std::collections::HashMap;

use crate::vpk::{self, Result, Entry, DIR_INDEX};
use crate::vpk::util::{format_size, print_headless_table, print_table, Align::*};

pub struct ArchStats {
    file_count: usize,
    file_size: Option<u64>,
    used_size: u64,
    io_error: Option<std::io::Error>,
}

impl ArchStats {
    pub fn new(file_count: usize, used_size: u64) -> Self {
        Self {
            file_count,
            file_size: None,
            used_size,
            io_error: None,
        }
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
}

pub struct Stats<'a> {
    pub file_count: usize,
    pub dir_count:  usize,
    pub max_inline_size: u16,
    pub max_size:        u32,
    pub max_full_size:   u32,
    pub extmap:  HashMap<&'a str, ExtStats>,
    pub archmap: HashMap<u16, ArchStats>,
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
        }
    }

    pub fn scan(package: &'a vpk::Package) -> Self {
        let mut stats = Self::new();
        stats.scan_entries(&package.entries);

        for (archive_index, archstat) in stats.archmap.iter_mut() {
            let path = package.archive_path(*archive_index);

            if *archive_index == DIR_INDEX {
                archstat.used_size += package.data_offset as u64;
            }

            match fs::metadata(&path) {
                Err(error) => {
                    archstat.io_error = Some(error);
                },
                Ok(meta) => {
                    archstat.file_size = Some(meta.len());
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
                    }).is_none() {
                        self.archmap.insert(
                            file.archive_index,
                            ArchStats::new(1, file.size as u64)
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

pub fn stats(package: &vpk::Package, human_readable: bool) -> Result<()> {
    let stats = Stats::scan(package);

    let fmt_size = if human_readable {
        |size: u64| format_size(size)
    } else {
        |size: u64| format!("{}", size)
    };

    print_headless_table(&[
        vec!["Version:",    &format!("{}", package.version)],
        vec!["Index Size:", &fmt_size(package.data_offset as u64)],
        vec![],
        vec!["File Count:",      &format!("{}", stats.file_count)],
        vec!["Directory Count:", &format!("{}", stats.dir_count)],
        vec!["Extension Count:", &format!("{}", stats.extmap.len())],
        vec!["Archive Count:",   &format!("{}", stats.archmap.len())],
        vec![],
        vec!["Max Inline-Size:",     &fmt_size(stats.max_inline_size as u64)],
        vec!["Max Non-Inline-Size:", &fmt_size(stats.max_size as u64)],
        vec!["Max Full-Size:",       &fmt_size(stats.max_full_size as u64)],
    ], &[Left, Right]);

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

    print_table(
        &["Archive", "File Size", "Used Size", "IO Error"],
        &[Right,     Right,       Right,       Left],
        &arch_indices.iter().map(|archive_index| {
            let archstats = stats.archmap.get(archive_index).unwrap();
            vec![
                if *archive_index == DIR_INDEX {
                    "dir".to_owned()
                } else {
                    format!("{:03}", archive_index)
                },
                if let Some(size) = archstats.file_size {
                    fmt_size(size)
                } else {
                    "".to_owned()
                },
                fmt_size(archstats.used_size as u64),
                if let Some(io_error) = &archstats.io_error {
                    format!("{}", io_error)
                } else {
                    "".to_owned()
                }
            ]
        }).collect::<Vec<_>>()
    );

    Ok(())
}
