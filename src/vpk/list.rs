use crate::vpk::sort::*;
use crate::vpk::util::{format_size, print_table};
use crate::vpk::{self, Result, Filter, DIR_INDEX};

pub fn list(package: &vpk::Package, order: &Order, human_readable: bool, filter: &Filter) -> Result<()> {
    let files = match filter {
        Filter::None => {
            package.recursive_file_list(order)
        },
        Filter::Paths(paths) => {
            package.recursive_file_list_from(&paths, order)?
        }
    };

    let mut table: Vec<Vec<String>> = Vec::new();

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
            if human_readable {
                format_size(file.inline_size as u32)
            } else {
                format!("{}", file.inline_size)
            },
            if human_readable {
                format_size(file.size)
            } else {
                format!("{}", file.size)
            },
            if human_readable {
                format_size(size)
            } else {
                format!("{}", size)
            },
            format!("0x{:08x}", file.crc32),
            path.to_owned(),
        ]);
    }

    let header = vec!["Index", "Archive", "Offset", "Inline-Size", "Archive-Size", "Full-Size", "CRC32", "Filename"];
    let right_align = vec![true, true, true, true, true, true, true, false];
    print_table(&header, &table, &right_align);

    Ok(())
}
