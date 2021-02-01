use crate::sort::Order;
use crate::util::{format_size, print_table, Align::*};
use crate::result::Result;
use crate::package::Package;
use crate::consts::DIR_INDEX;

pub fn list(package: &Package, order: &Order, human_readable: bool, filter: Option<&[&str]>) -> Result<()> {
    let files = match filter {
        None => {
            package.recursive_file_list(order)
        },
        Some(paths) => {
            package.recursive_file_list_from(&paths, order)?
        }
    };

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

    Ok(())
}
