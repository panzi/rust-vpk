use std::str::FromStr;
use std::path::{Path, PathBuf};

pub struct PathSplitter<'a> {
    path: &'a str,
    index: usize,
    char_iter: std::str::CharIndices<'a>,
}

impl<'a> Iterator for PathSplitter<'a> {
    type Item = (&'a str, &'a str, bool);

    fn next(&mut self) -> Option<(&'a str, &'a str, bool)> {
        let start_index = loop {
            if let Some((index, ch)) = self.char_iter.next() {
                if ch != '/' {
                    break index;
                }
            } else {
                return None;
            }
        };
        let end_index = loop {
            if let Some((index, ch)) = self.char_iter.next() {
                if ch == '/' {
                    self.index = index + 1;
                    break index;
                }
            } else {
                self.index = self.path.len();
                break self.index;
            }
        };

        if start_index == end_index {
            return None;
        }

        Some((&self.path[..end_index], &self.path[start_index..end_index], self.index == end_index))
    }
}

pub fn split_path<'a>(path: &'a str) -> PathSplitter<'a> {
    let path = path.trim_matches('/');

    PathSplitter {
        path,
        index: 0,
        char_iter: path.char_indices(),
    }
}

pub fn format_size(size: u32) -> String {
    if size >= 1024 * 1024 * 1024 {
        format!("{} G", size / (1024 * 1024 * 1024))
    } else if size >= 1024 * 1024 {
        format!("{} M", size / (1024 * 1024))
    } else if size >= 1024 {
        format!("{} K", size / 1024)
    } else {
        format!("{} B", size)
    }
}

pub fn vpk_path_to_fs(prefix: impl AsRef<Path>, path: &str) -> PathBuf {
    let mut buf = prefix.as_ref().to_path_buf();
    
    for (_, item, _) in split_path(path) {
        buf.push(item);
    }

    buf
}

pub fn archive_path(dirpath: impl AsRef<Path>, prefix: &str, archive_index: u16) -> PathBuf {
    let mut path = dirpath.as_ref().to_path_buf();
    
    if archive_index == crate::vpk::DIR_INDEX {
        path.push(format!("{}_dir.vpk", prefix));
    } else {
        path.push(format!("{}_{:03}.vpk", prefix, archive_index));
    }

    path
}

pub fn print_row(row: &Vec<impl AsRef<str>>, lens: &Vec<usize>, right_align: &Vec<bool>) {
    let mut first = true;
    for ((cell, len), right_align) in row.iter().zip(lens.iter()).zip(right_align.iter()) {
        if first {
            first = false;
        } else {
            print!("  "); // cell spacing
        }

        if *right_align {
            print!("{:>1$}", cell.as_ref(), *len);
        } else {
            print!("{:<1$}", cell.as_ref(), *len);
        }
    }

    println!();
}

pub fn print_table(header: &Vec<impl AsRef<str>>, body: &Vec<Vec<impl AsRef<str>>>, right_align: &Vec<bool>) {
    // TODO: maybe count graphemes? needs extra lib. haven't seen non-ASCII filenames anyway
    let mut lens: Vec<usize> = header.iter().map(|x| x.as_ref().chars().count()).collect();
    for row in body {
        for (cell, max_len) in row.iter().zip(lens.iter_mut()) {
            let len = cell.as_ref().chars().count();
            if len > *max_len {
                *max_len = len;
            }
        }
    }

    print_row(&header, &lens, &right_align);
    let mut first = true;
    for len in lens.iter() {
        let mut len = *len;
        if first {
            first = false;
        } else {
            len += 2; // cell spacing
        }

        while len > 0 {
            print!("-");
            len -= 1;
        }
    }
    println!();

    for row in body {
        print_row(row, &lens, &right_align);
    }
}

pub fn parse_size(value: &str) -> std::result::Result<usize, <usize as FromStr>::Err> {
    let mut value = value.trim();

    if value.ends_with("B") {
        value = &value[..value.len() - 1];
    }

    if value.ends_with("K") {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024)
    } else if value.ends_with("M") {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024 * 1024)
    } else if value.ends_with("G") {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024 * 1024 * 1024)
    } else if value.ends_with("T") {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024 * 1024 * 1024 * 1024)
    } else if value.ends_with("P") {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024 * 1024 * 1024 * 1024 * 1024)
    } else if value.ends_with("E") {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024 * 1024 * 1024 * 1024 * 1024 * 1024)
    } else if value.ends_with("Z") {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024 * 1024 * 1024 * 1024 * 1024 * 1024 * 1024)
    } else if value.ends_with("Y") {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024 * 1024 * 1024 * 1024 * 1024 * 1024 * 1024 * 1024)
    } else {
        value.parse()
    }
}
