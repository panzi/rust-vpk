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

pub(crate) mod io;
pub(crate) mod util;

pub mod list;
pub mod stats;
pub mod sort;
pub mod check;
pub mod unpack;
pub mod pack;
pub mod package;
pub mod entry;
pub mod archive_cache;
pub mod result;
pub mod consts;

#[cfg(feature = "fuse")]
pub mod mount;

use clap::{Arg, App, SubCommand};

use crate::list::{list, ListOptions};
use crate::stats::stats;
use crate::check::{check, CheckOptions};
use crate::unpack::{unpack, UnpackOptions};
use crate::pack::{pack, PackOptions};
use crate::package::Package;

use crate::sort::{parse_order, DEFAULT_ORDER};
use crate::consts::{DEFAULT_MAX_INLINE_SIZE, DEFAULT_MD5_CHUNK_SIZE};
use crate::result::{Error, Result};
use crate::pack::ArchiveStrategy;
use crate::util::parse_size;

#[cfg(feature = "fuse")]
use crate::mount::{mount, MountOptions};

impl From<clap::Error> for crate::result::Error {
    fn from(error: clap::Error) -> Self {
        crate::result::Error::other(error.message)
    }
}

pub enum Filter<'a> {
    None,
    Paths(Vec<&'a str>),
}

impl<'a> Filter<'a> {
    pub fn new(args: &'a clap::ArgMatches) -> Self {
        if let Some(paths) = args.values_of("paths") {
            if paths.len() == 0 {
                Filter::None
            } else {
                Filter::Paths(paths.collect())
            }
        } else {
            Filter::None
        }
    }

    pub fn as_ref(&self) -> Option<&[&'a str]> {
        match self {
            Filter::None => None,
            Filter::Paths(paths) => Some(&paths[..])
        }
    }
}

fn arg_human_readable<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("human-readable")
        .long("human-readable")
        .short("h")
        .takes_value(false)
        .help("Print sizes like 1.0 K, 2.2 M, 4.1 G etc.")
}

fn arg_package<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("package")
        .index(1)
        .required(true)
        .value_name("PACKAGE")
        .help("A file ending in _dir.vpk (e.g. pak01_dir.vpk)")
}

fn arg_paths<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("paths")
        .index(2)
        .multiple(true)
        .value_name("PATH")
        .help("If given, only consider these files from the package.")
}

fn arg_verbose<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("verbose")
        .long("verbose")
        .short("v")
        .takes_value(false)
        .help("Print verbose output.")
}

fn arg_allow_v0<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("allow-v0")
        .long("allow-v0")
        .short("0")
        .takes_value(false)
        .help("Allow version 0 packages. (Packages without a header.)")
}

fn run() -> Result<()> {
    let default_max_inline_size_str = format!("{}", DEFAULT_MAX_INLINE_SIZE);

    let app = App::new("VPK - Valve Packages")
        .version("1.0")
        .author("Mathias Panzenböck <grosser.meister.morti@gmx.net>");

    #[cfg(feature = "fuse")]
    let app = app
        .about("pack, list, unpack, check, mount VPK Valve packages");

    #[cfg(not(feature = "fuse"))]
    let app = app
        .about("pack, list, unpack, check VPK Valve packages");

    let app = app
        .subcommand(SubCommand::with_name("list")
            .alias("l")
            .about("List content of a VPK package.")
            .arg(Arg::with_name("sort")
                .long("sort")
                .short("s")
                .takes_value(true)
                .value_name("ORDER")
                .help(
                    "Sort order of list as comma separated keys:\n\
                     \n\
                     * path         - path of the file inside the package\n\
                     * inline-size  - size of the data embedded in the index\n\
                     * archive-size - size of the data in the actual archive\n\
                     * full-size    - sum of the other two sizes\n\
                     * offset       - offset inside of the archive\n\
                     * archive      - archive where the file is stored\n\
                     * index        - index of the file in the package index\n\
                     \n\
                     If you prepend the order with - you invert the sort order for that key. E.g.:\n\
                     \n\
                     vpk list --sort=-full-size,name")
            )
            .arg(arg_allow_v0())
            .arg(arg_human_readable())
            .arg(arg_package())
            .arg(arg_paths()))

        .subcommand(SubCommand::with_name("stats")
            .alias("s")
            .about("Print some statistics of a VPK package.")
            .arg(arg_allow_v0())
            .arg(arg_human_readable())
            .arg(arg_package()))

        .subcommand(SubCommand::with_name("check")
            .alias("c")
            .about("Check CRC32 and MD5 sums of files in a VPK package.")
            .arg(Arg::with_name("alignment")
                .long("alignment")
                .short("a")
                .takes_value(true)
                .value_name("ALIGNMENT")
                .help("Assume alignment of file data in bytes and print the differentce to the real alignment."))
            .arg(arg_verbose())
            .arg(arg_allow_v0())
            .arg(arg_human_readable())
            .arg(Arg::with_name("stop-on-error")
                .long("stop-on-error")
                .takes_value(false)
                .help("Stop on first error."))
            .arg(arg_package())
            .arg(arg_paths()))

        .subcommand(SubCommand::with_name("unpack")
            .alias("x")
            .about("Extract files from a VPK package.")
            .arg(arg_verbose())
            .arg(Arg::with_name("outdir")
                .long("outdir")
                .short("o")
                .value_name("OUTDIR")
                .takes_value(true)
                .help("Write files to OUTDIR instead of current directory."))
            .arg(Arg::with_name("dirname-from-archive")
                .long("dirname-from-archive")
                .short("a")
                .help(
                    "Take directory names from the archives of the files.\n\
                     Meaning the first level of generated directory names will be named \"000\", \"001\", \"002\", \"003\", ... and \"dir\"."))
            .arg(Arg::with_name("check")
                .long("check")
                .short("c")
                .takes_value(false)
                .help("Check CRC32 sums while unpacking."))
            .arg(arg_allow_v0())
            .arg(arg_package())
            .arg(arg_paths()))

        .subcommand(SubCommand::with_name("pack")
            .alias("p")
            .about("Create a VPK package.")
            .arg(Arg::with_name("version")
                .long("version")
                .short("V")
                .takes_value(true)
                .value_name("VERSION")
                .default_value("1")
                .help("VPK version. Only 1 and 2 (without signing) are supported."))
            .arg(Arg::with_name("md5-chunk-size")
                .long("md5-chunk-size")
                .short("c")
                .takes_value(true)
                .value_name("SIZE")
                .help("Size of chunks with MD5 checksums (VPK v2 only). [default: 1 M]"))
            .arg(Arg::with_name("alignment")
                .long("alignment")
                .short("a")
                .takes_value(true)
                .value_name("ALIGNMENT")
                .help("Ensure that data in archives is aligned at given number of bytes."))
            .arg(Arg::with_name("archive-from-dirname")
                .long("archive-from-dirname")
                .short("d")
                .takes_value(false)
                .conflicts_with("max-archive-size")
                .help(
                    "Take archive distribution from directory names.\n\
                     Meaning the first level of directory names have to be named \"000\", \"001\", \"002\", \"003\", ... and \"dir\".\n\
                     Conficts with: --max-archive-size"))
            .arg(Arg::with_name("max-archive-size")
                .long("max-archive-size")
                .short("s")
                .takes_value(true)
                .value_name("SIZE")
                .help(
                    "Distribute files to archives by ensuring no archive is bigger than the given size.\n\
                     Conflicts with: --archive-from-dirname"
                ))
            .arg(Arg::with_name("max-inline-size")
                .long("max-inline-size")
                .short("i")
                .takes_value(true)
                .value_name("SIZE")
                .default_value(&default_max_inline_size_str)
                .help("Maximum size of files that will be embedded in the index."))
            .arg(arg_verbose())
            .arg(arg_package())
            .arg(Arg::with_name("indir")
                .index(2)
                .required(true)
                .value_name("INDIR")
                .help("Read files from this directory.")));

    #[cfg(feature = "fuse")]
    let app = app.subcommand(SubCommand::with_name("mount")
        .alias("m")
        .about("Mount a VPK package as read-only filesystem.")
        .long_about(
            "Mount a VPK package as read-only filesystem.\n\
             Use `fusermount -u <MOUNT-POINT>` to unmount again.")
        .arg(arg_allow_v0())
        .arg(Arg::with_name("foreground")
            .long("foreground")
            .short("f")
            .takes_value(false)
            .help("Keep process in foreground (i.e. don't daemonize)."))
        .arg(Arg::with_name("debug")
            .long("debug")
            .short("d")
            .takes_value(false)
            .help("Add \"debug\" to FUSE options. Implies: --foreground"))
        .arg(arg_package())
        .arg(Arg::with_name("mount-point")
            .index(2)
            .required(true)
            .value_name("MOUNT-POINT")
            .help("Directory where filesystem will be mounted.")));
    
    let matches = app.get_matches();

    match matches.subcommand() {
        ("list", Some(args)) => {
            let order = if let Some(order) = args.value_of("sort") {
                Some(parse_order(order)?)
            } else {
                None
            };
            let order = match &order {
                Some(order) => &order[..],
                None => &DEFAULT_ORDER[..],
            };

            let allow_v0       = args.is_present("allow-v0");
            let human_readable = args.is_present("human-readable");
            let path           = args.value_of("package").unwrap();
            let filter         = Filter::new(args);

            let package = Package::from_path(path, allow_v0)?;

            list(&package, ListOptions {
                order,
                human_readable,
                filter: filter.as_ref()
            })?;
        },
        ("check", Some(args)) => {
            let allow_v0       = args.is_present("allow-v0");
            let human_readable = args.is_present("human-readable");
            let verbose        = args.is_present("verbose");
            let stop_on_error  = args.is_present("stop-on-error");
            let path           = args.value_of("package").unwrap();
            let filter         = Filter::new(args);
            let alignment = if let Some(alignment) = args.value_of("alignment") {
                if let Ok(align) = parse_size(alignment) {
                    if align == 0 || align > std::u32::MAX as usize {
                        return Err(Error::illegal_argument(
                            "--alignment",
                            alignment
                        ));
                    }
                    Some(align as u32)
                } else {
                    return Err(Error::illegal_argument(
                        "--alignment",
                        alignment
                    ));
                }
            } else {
                None
            };

            let package = Package::from_path(path, allow_v0)?;

            check(&package, CheckOptions {
                verbose,
                stop_on_error,
                human_readable,
                filter: filter.as_ref(),
                alignment,
            })?;

            if verbose {
                println!("everything is ok");
            }
        },
        ("unpack", Some(args)) => {
            let allow_v0             = args.is_present("allow-v0");
            let outdir               = args.value_of("outdir").unwrap_or(".");
            let verbose              = args.is_present("verbose");
            let check                = args.is_present("check");
            let dirname_from_archive = args.is_present("dirname-from-archive");
            let path                 = args.value_of("package").unwrap();
            let filter               = Filter::new(args);

            let package = Package::from_path(path, allow_v0)?;

            unpack(&package, outdir, UnpackOptions {
                filter: filter.as_ref(),
                verbose,
                check,
                dirname_from_archive,
            })?;
        },
        ("pack", Some(args)) => {
            let indir   = args.value_of("indir").unwrap_or(".");
            let path    = args.value_of("package").unwrap();
            let version = if let Some(version) = args.value_of("version") {
                if let Ok(value) = version.parse::<u32>() {
                    if value > 2 {
                        return Err(Error::illegal_argument(
                            "--version",
                            version
                        ));
                    }

                    value
                } else {
                    return Err(Error::illegal_argument(
                        "--version",
                        version
                    ));
                }
            } else {
                1u32
            };
            let md5_chunk_size = if let Some(md5_chunk_size) = args.value_of("md5-chunk-size") {
                if version < 2 {
                    return Err(Error::other("--md5-chunk-size requires --version to be 2"));
                }
                if let Ok(size) = parse_size(md5_chunk_size) {
                    if size == 0 || size > std::u32::MAX as usize {
                        return Err(Error::illegal_argument(
                            "--md5-chunk-size",
                            md5_chunk_size
                        ));
                    }
                    size as u32
                } else {
                    return Err(Error::illegal_argument(
                        "--md5-chunk-size",
                        md5_chunk_size
                    ));
                }
            } else {
                DEFAULT_MD5_CHUNK_SIZE
            };
            let verbose = args.is_present("verbose");
            let max_inline_size = if let Some(inline_size) = args.value_of("max-inline-size") {
                if let Ok(size) = parse_size(inline_size) {
                    if size > std::u16::MAX as usize {
                        return Err(Error::illegal_argument(
                            "--max-inline-size",
                            inline_size
                        ));
                    }
                    size as u16
                } else {
                    return Err(Error::illegal_argument(
                        "--max-inline-size",
                        inline_size
                    ));
                }
            } else {
                DEFAULT_MAX_INLINE_SIZE
            };
            let alignment = if let Some(alignment) = args.value_of("alignment") {
                if let Ok(alignment) = parse_size(alignment) {
                    alignment
                } else {
                    return Err(Error::illegal_argument(
                        "--alignment",
                        alignment
                    ));
                }
            } else {
                1
            };
            let strategy = if args.is_present("archive-from-dirname") {
                ArchiveStrategy::ArchiveFromDirName
            } else if let Some(max_arch_size) = args.value_of("max-archive-size") {
                if let Ok(size) = parse_size(max_arch_size) {
                    if size == 0 || size > std::u32::MAX as usize {
                        return Err(Error::illegal_argument(
                            "--max-archive-size",
                            max_arch_size
                        ));
                    }
                    ArchiveStrategy::MaxArchiveSize(size as u32)
                } else {
                    return Err(Error::illegal_argument(
                        "--max-archive-size",
                        max_arch_size
                    ));
                }
            } else {
                ArchiveStrategy::default()
            };

            pack(path, indir, PackOptions {
                version,
                md5_chunk_size,
                strategy,
                max_inline_size,
                alignment,
                verbose
            })?;
        },
        ("stats", Some(args)) => {
            let allow_v0       = args.is_present("allow-v0");
            let human_readable = args.is_present("human-readable");
            let path           = args.value_of("package").unwrap();

            let package = Package::from_path(path, allow_v0)?;

            stats(&package, human_readable)?;
        },
        #[cfg(feature = "fuse")]
        ("mount", Some(args)) => {
            let allow_v0    = args.is_present("allow-v0");
            let debug       = args.is_present("debug");
            let foreground  = args.is_present("foreground");
            let path        = args.value_of("package").unwrap();
            let mount_point = args.value_of("mount-point").unwrap();

            let package = Package::from_path(path, allow_v0)?;

            mount(package, &mount_point, MountOptions { foreground, debug })?;
        },
        ("", _) => {
            return Err(Error::other(
                "subcommand required\n\
                 For more information try --help".to_owned()
            ));
        },
        (cmd, _) => {
            return Err(Error::other(format!(
                "unknown subcommand: {}\n\
                 For more information try --help",
                 cmd
            )));
        }
    }

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{}", error);
        std::process::exit(1);
    }
}
