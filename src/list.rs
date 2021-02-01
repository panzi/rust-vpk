use crate::sort::{Order, DEFAULT_ORDER};
use crate::util::{format_size, print_table, Align::*};
use crate::result::Result;
use crate::package::Package;
use crate::consts::DIR_INDEX;

pub struct ListOptions<'a> {
    pub order: &'a Order,
    pub human_readable: bool,
    pub filter: Option<&'a [&'a str]>,
}

impl ListOptions<'_> {
    #[inline]
    pub fn new() -> Self {
        ListOptions::default()
    }
}

impl Default for ListOptions<'_> {
    #[inline]
    fn default() -> Self {
        Self {
            order: &DEFAULT_ORDER,
            human_readable: false,
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

    let mut table: Vec<Vec<String>> = Vec::new();

    let fmt_size = if options.human_readable {
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

    Ok(())
}
