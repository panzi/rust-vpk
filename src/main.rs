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
use crate::pack::{pack_v1, PackOptions};
use crate::package::Package;

use crate::sort::{parse_order, DEFAULT_ORDER};
use crate::consts::DEFAULT_MAX_INLINE_SIZE;
use crate::result::{Error, Result};
use crate::pack::ArchiveStrategy;
use crate::util::parse_size;

#[cfg(feature = "fuse")]
use crate::mount::{mount, MountOptions};

impl From<clap::Error> for crate::result::Error {
    fn from(error: clap::Error) -> Self {
        crate::result::Error::Other(error.message)
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
            .about("List content of a VPK v1/v2 package.")
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
            .arg(arg_human_readable())
            .arg(arg_package())
            .arg(arg_paths()))

        .subcommand(SubCommand::with_name("stats")
            .alias("s")
            .about("Print some statistics of a VPK v1/v2 package.")
            .arg(arg_human_readable())
            .arg(arg_package()))

        .subcommand(SubCommand::with_name("check")
            .alias("c")
            .about("Check CRC32 sums of files in a VPK v1/v2 package.")
            .arg(Arg::with_name("alignment")
                .long("alignment")
                .short("a")
                .takes_value(true)
                .value_name("ALIGNMENT")
                .help("Assume alignment of file data in bytes and print the differentce to the real alignment."))
            .arg(arg_verbose())
            .arg(arg_human_readable())
            .arg(Arg::with_name("stop-on-error")
                .long("stop-on-error")
                .takes_value(false)
                .help("Stop on first error."))
            .arg(arg_package())
            .arg(arg_paths()))

        .subcommand(SubCommand::with_name("unpack")
            .alias("x")
            .about("Extract files from a VPK v1/v2 package.")
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
            .arg(arg_package())
            .arg(arg_paths()))

        .subcommand(SubCommand::with_name("pack")
            .alias("p")
            .about("Create a VPK v1 package.")
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
        .about("Mount a VPK v1/v2 package as read-only filesystem.")
        .long_about(
            "Mount a VPK v1/v2 package as read-only filesystem.\n\
             Use `fusermount -u <MOUNT-POINT>` to unmount again.")
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

            let human_readable = args.is_present("human-readable");
            let path           = args.value_of("package").unwrap();
            let filter         = Filter::new(args);

            let package = Package::from_path(&path)?;

            list(&package, ListOptions {
                order,
                human_readable,
                filter: filter.as_ref()
            })?;
        },
        ("check", Some(args)) => {
            let human_readable = args.is_present("human-readable");
            let verbose        = args.is_present("verbose");
            let stop_on_error  = args.is_present("stop-on-error");
            let path           = args.value_of("package").unwrap();
            let filter         = Filter::new(args);
            let alignment = if let Some(alignment) = args.value_of("alignment") {
                if let Ok(align) = parse_size(alignment) {
                    if align == 0 || align > std::u32::MAX as usize {
                        return Err(Error::IllegalArgument {
                            name: "--alignment",
                            value: alignment.to_owned(),
                        });
                    }
                    Some(align as u32)
                } else {
                    return Err(Error::IllegalArgument {
                        name: "--alignment",
                        value: alignment.to_owned(),
                    });
                }
            } else {
                None
            };

            let package = Package::from_path(&path)?;

            check(&package, CheckOptions {
                verbose,
                stop_on_error,
                human_readable,
                filter: filter.as_ref(),
                alignment,
            })?;

            if verbose {
                println!("everything ok");
            }
        },
        ("unpack", Some(args)) => {
            let outdir               = args.value_of("outdir").unwrap_or(".");
            let verbose              = args.is_present("verbose");
            let check                = args.is_present("check");
            let dirname_from_archive = args.is_present("dirname-from-archive");
            let path                 = args.value_of("package").unwrap();
            let filter               = Filter::new(args);

            let package = Package::from_path(&path)?;

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
            let verbose = args.is_present("verbose");
            let max_inline_size = if let Some(inline_size) = args.value_of("max-inline-size") {
                if let Ok(size) = parse_size(inline_size) {
                    if size > std::u16::MAX as usize {
                        return Err(Error::IllegalArgument {
                            name: "--max-inline-size",
                            value: inline_size.to_owned(),
                        });
                    }
                    size as u16
                } else {
                    return Err(Error::IllegalArgument {
                        name: "--max-inline-size",
                        value: inline_size.to_owned(),
                    });
                }
            } else {
                DEFAULT_MAX_INLINE_SIZE
            };
            let alignment = if let Some(alignment) = args.value_of("alignment") {
                if let Ok(alignment) = parse_size(alignment) {
                    alignment
                } else {
                    return Err(Error::IllegalArgument {
                        name: "--alignment",
                        value: alignment.to_owned(),
                    });
                }
            } else {
                1
            };
            let strategy = if args.is_present("archive-from-dirname") {
                ArchiveStrategy::ArchiveFromDirName
            } else if let Some(max_arch_size) = args.value_of("max-archive-size") {
                if let Ok(size) = parse_size(max_arch_size) {
                    if size == 0 || size > std::u32::MAX as usize {
                        return Err(Error::IllegalArgument {
                            name: "--max-archive-size",
                            value: max_arch_size.to_owned(),
                        });
                    }
                    ArchiveStrategy::MaxArchiveSize(size as u32)
                } else {
                    return Err(Error::IllegalArgument {
                        name: "--max-archive-size",
                        value: max_arch_size.to_owned(),
                    });
                }
            } else {
                ArchiveStrategy::default()
            };

            pack_v1(&path, &indir, PackOptions { strategy, max_inline_size, alignment, verbose })?;
        },
        ("stats", Some(args)) => {
            let human_readable = args.is_present("human-readable");
            let path           = args.value_of("package").unwrap();

            let package = Package::from_path(&path)?;

            stats(&package, human_readable)?;
        },
        #[cfg(feature = "fuse")]
        ("mount", Some(args)) => {
            let debug       = args.is_present("debug");
            let foreground  = args.is_present("foreground");
            let path        = args.value_of("package").unwrap();
            let mount_point = args.value_of("mount-point").unwrap();

            let package = Package::from_path(&path)?;

            mount(package, &mount_point, MountOptions { foreground, debug })?;
        },
        ("", _) => {
            return Err(Error::Other(
                "subcommand required\n\
                 For more information try --help".to_owned()
            ));
        },
        (cmd, _) => {
            return Err(Error::Other(format!(
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
