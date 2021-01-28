use std::path::Path;

use crate::vpk::{self, Result};

pub fn stats(package: &vpk::Package, path: impl AsRef<Path>, human_readable: bool) -> Result<()> {
    // TODO
    let mut file_count = 0;
    let mut dir_count = 0;
    let mut ext_count = 0;

    println!("TODO");

    Ok(())
}
