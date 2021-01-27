pub(super) struct PathSplitter<'a> {
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

pub(super) fn split_path<'a>(path: &'a str) -> PathSplitter<'a> {
    let path = path.trim_matches('/');

    PathSplitter {
        path,
        index: 0,
        char_iter: path.char_indices(),
    }
}

pub(super) fn format_size(size: u32) -> String {
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
