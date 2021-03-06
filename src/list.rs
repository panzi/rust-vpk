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

use std::io::Write;

use crate::sort::{Order, DEFAULT_ORDER};
use crate::util::{format_size, print_table, Align::*};
use crate::result::Result;
use crate::package::Package;
use crate::consts::DIR_INDEX;

#[derive(Debug, PartialEq)]
pub enum ListStyle {
    Table { human_readable: bool },
    OnlyNames { null_separated: bool },
}

pub struct ListOptions<'a> {
    pub order: &'a Order,
    pub style: ListStyle,
    pub filter: Option<&'a [&'a str]>,
}

impl ListOptions<'_> {
    #[inline]
    pub fn new() -> Self {
        ListOptions::default()
    }
}

impl Default for ListStyle {
    #[inline]
    fn default() -> Self {
        ListStyle::Table { human_readable: false }
    }
}

impl Default for ListOptions<'_> {
    #[inline]
    fn default() -> Self {
        Self {
            order: &DEFAULT_ORDER,
            style: ListStyle::default(),
            filter: None,
        }
    }
}

pub fn list(package: &Package, options: ListOptions) -> Result<()> {
    let files = match options.filter {
        None => {
            package.recursive_file_list(options.order)
        },
        Some(paths) => {
            package.recursive_file_list_from(&paths, options.order)?
        }
    };

    match options.style {
        ListStyle::Table { human_readable } => {
            let mut table: Vec<Vec<String>> = Vec::new();

            let fmt_size = if human_readable {
                |size: u64| format_size(size)
            } else {
                |size: u64| format!("{}", size)
            };

            for (path, file) in files {
                let size = file.inline_size as u32 + file.size;
                table.push(vec![
                    format!("{}", file.index),
                    if file.archive_index == DIR_INDEX {
                        "dir".to_owned()
                    } else {
                        format!("{}", file.archive_index)
                    },
                    format!("{}", file.offset),
                    fmt_size(file.inline_size as u64),
                    fmt_size(file.size as u64),
                    fmt_size(size as u64),
                    format!("0x{:08x}", file.crc32),
                    path.to_owned(),
                ]);
            }

            print_table(
                &["Index", "Archive", "Offset", "Inline-Size", "Archive-Size", "Full-Size", "CRC32", "Filename"],
                &[Right,   Right,     Right,    Right,         Right,          Right,       Right,   Left],
                &table);
        }
        ListStyle::OnlyNames { null_separated } => {
            let sep = [if null_separated { 0 } else { '\n' as u8 }];
            let mut stdout = std::io::stdout();
            for (path, _) in files {
                stdout.write_all(path.as_bytes())?;
                stdout.write_all(&sep)?;
            }
        }
    }

    Ok(())
}
