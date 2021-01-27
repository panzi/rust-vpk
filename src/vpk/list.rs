use std::io::Write;

use crate::vpk;
use crate::vpk::sort::*;
use crate::vpk::util::format_size;
use crate::vpk::{Result, Filter, DIR_INDEX};

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
    // TODO: maybe count graphemes? needs extra lib. haven't seen non-ASCII filenames anyway
    let mut lens: Vec<usize> = header.iter().map(|x| x.chars().count()).collect();
    for row in table.iter() {
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

    for row in table.iter() {
        print_row(row, &lens, &right_align);
    }

    Ok(())
}
