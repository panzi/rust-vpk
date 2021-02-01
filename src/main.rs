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

fn run() -> Result<()> {
    let app = App::new("VPK Valve Packages")
        .version("1.0")
        .author("Mathias Panzenböck <grosser.meister.morti@gmx.net>")

        .subcommand(SubCommand::with_name("list")
            .alias("l")
            .arg(Arg::with_name("sort").long("sort").short("s").takes_value(true))
            .arg(Arg::with_name("human-readable").long("human-readable").short("h").takes_value(false))
            .arg(Arg::with_name("package").index(1).required(true))
            .arg(Arg::with_name("paths").index(2).multiple(true)))

        .subcommand(SubCommand::with_name("stats")
            .alias("s")
            .arg(Arg::with_name("human-readable").long("human-readable").short("h").takes_value(false))
            .arg(Arg::with_name("package").index(1).required(true)))

        .subcommand(SubCommand::with_name("check")
            .alias("c")
            .arg(Arg::with_name("verbose").long("verbose").short("v").takes_value(false))
            .arg(Arg::with_name("human-readable").long("human-readable").short("h").takes_value(false))
            .arg(Arg::with_name("stop-on-error").long("stop-on-error").takes_value(false))
            .arg(Arg::with_name("package").index(1).required(true))
            .arg(Arg::with_name("paths").index(2).multiple(true)))

        .subcommand(SubCommand::with_name("unpack")
            .alias("x")
            .arg(Arg::with_name("outdir").long("outdir").short("o").takes_value(true))
            .arg(Arg::with_name("verbose").long("verbose").short("v").takes_value(false))
            .arg(Arg::with_name("check").long("check").short("c").takes_value(false))
            .arg(Arg::with_name("package").index(1).required(true))
            .arg(Arg::with_name("paths").index(2).multiple(true)))

        .subcommand(SubCommand::with_name("pack")
            .alias("p")
            .arg(Arg::with_name("alignment").long("alignment").short("a").takes_value(true))
            .arg(Arg::with_name("archive-from-dirname").long("archive-from-dirname").short("n").takes_value(false).conflicts_with("max-archive-size"))
            .arg(Arg::with_name("max-archive-size").long("max-archive-size").short("s").takes_value(true))
            .arg(Arg::with_name("max-inline-size").long("max-inline-size").short("x").takes_value(true))
            .arg(Arg::with_name("verbose").long("verbose").short("v").takes_value(false))
            .arg(Arg::with_name("package").index(1).required(true))
            .arg(Arg::with_name("indir").index(2).required(true)));

    #[cfg(feature = "fuse")]
    let app = app.subcommand(SubCommand::with_name("mount")
        .alias("m")
        .arg(Arg::with_name("foreground").long("foreground").short("f").takes_value(false))
        .arg(Arg::with_name("debug").long("debug").short("d").takes_value(false))
        .arg(Arg::with_name("package").index(1).required(true))
        .arg(Arg::with_name("mount-point").index(2).required(true)));
    
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

            let package = Package::from_path(&path)?;

            check(&package, CheckOptions {
                verbose,
                stop_on_error,
                human_readable,
                filter: filter.as_ref()
            })?;

            if verbose {
                println!("everything ok");
            }
        },
        ("unpack", Some(args)) => {
            let outdir  = args.value_of("outdir").unwrap_or(".");
            let verbose = args.is_present("verbose");
            let check   = args.is_present("check");
            let path    = args.value_of("package").unwrap();
            let filter  = Filter::new(args);

            let package = Package::from_path(&path)?;

            unpack(&package, outdir, UnpackOptions {
                filter: filter.as_ref(),
                verbose,
                check
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
                    if size > std::u32::MAX as usize {
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
