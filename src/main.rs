use clap::{Arg, App, SubCommand};

use std::io::{Write};

use vpk::sort::{parse_order, DEFAULT_ORDER};
use vpk::{Package, Filter, DEFAULT_MAX_INLINE_SIZE, Error};
use vpk::pack::ArchiveOptions;

pub mod vpk;

impl From<clap::Error> for vpk::Error {
    fn from(error: clap::Error) -> Self {
        vpk::Error::Other(error.message)
    }
}

fn get_filter(args: &clap::ArgMatches) -> Filter {
    if let Some(filter) = args.values_of("paths") {
        let paths: Vec<String> = filter.map(|name| name.to_owned()).collect();
        if paths.is_empty() {
            Filter::None
        } else {
            Filter::Paths(paths)
        }
    } else {
        Filter::None
    }
}

fn run() -> vpk::Result<()> {
    let matches = App::new("VPK Valve Packages")
        .version("1.0")
        .author("Mathias Panzenböck <grosser.meister.morti@gmx.net>")

        .subcommand(SubCommand::with_name("list")
            .alias("l")
            .arg(Arg::with_name("sort").long("sort").short("s").takes_value(true))
            .arg(Arg::with_name("human-readable").long("human-readable").short("h").takes_value(false))
            .arg(Arg::with_name("package").index(1).required(true))
            .arg(Arg::with_name("paths").index(2).multiple(true)))

        .subcommand(SubCommand::with_name("check")
            .alias("c")
            .arg(Arg::with_name("verbose").long("verbose").short("v").takes_value(false))
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
            // TODO: how to distribute files over archives? group them in archive?
            .arg(Arg::with_name("indir").long("indir").short("i").takes_value(true))
            .arg(Arg::with_name("archive-from-dirname").long("archive-from-dirname").short("n").takes_value(false).conflicts_with("max-archive-size"))
            .arg(Arg::with_name("max-archive-size").long("max-archive-size").short("a").takes_value(true))
            .arg(Arg::with_name("max-inline-size").long("max-inline-size").short("x").takes_value(true))
            .arg(Arg::with_name("package").index(1).required(true)))

        .get_matches();

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
            let path = args.value_of("package").unwrap();
            let filter = get_filter(args);

            let package = Package::from_path(&path)?;

            vpk::list(&package, order, human_readable, &filter)?;
        },
        ("check", Some(args)) => {
            let verbose       = args.is_present("verbose");
            let stop_on_error = args.is_present("stop-on-error");
            let path          = args.value_of("package").unwrap();

            let package = Package::from_path(&path)?;

            vpk::check(&package, verbose, stop_on_error)?;

            if verbose {
                println!("everything ok");
            }
        },
        ("unpack", Some(args)) => {
            let outdir  = args.value_of("outdir").unwrap_or(".");
            let verbose = args.is_present("verbose");
            let check   = args.is_present("check");
            let path    = args.value_of("package").unwrap();
            let filter  = get_filter(args);

            let package = Package::from_path(&path)?;

            vpk::unpack(&package, outdir, &filter, verbose, check)?;
        },
        ("pack", Some(args)) => {
            let indir   = args.value_of("indir").unwrap_or(".");
            let path    = args.value_of("package").unwrap();
            let verbose = args.is_present("verbose");
            let max_inline_size = if let Some(inline_size) = args.value_of("max-inline-size") {
                if let Ok(inline_size) = inline_size.parse::<u16>() {
                    inline_size
                } else {
                    return Err(Error::IllegalArgument {
                        name: "--max-inline-size".to_owned(),
                        value: inline_size.to_owned(),
                    });
                }
            } else {
                DEFAULT_MAX_INLINE_SIZE
            };
            let arch_opts = if args.is_present("archive-from-dirname") {
                ArchiveOptions::ArchiveFromDirName
            } else if let Some(max_arch_size) = args.value_of("max-archive-size") {
                if let Ok(max_arch_size) = max_arch_size.parse::<u32>() {
                    ArchiveOptions::MaxArchiveSize(max_arch_size)
                } else {
                    return Err(Error::IllegalArgument {
                        name: "--max-archive-size".to_owned(),
                        value: max_arch_size.to_owned(),
                    });
                }
            } else {
                ArchiveOptions::MaxArchiveSize(std::i32::MAX as u32)
            };

            vpk::pack(&path, &indir, arch_opts, max_inline_size, verbose)?;
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
        let _ = writeln!(std::io::stderr(), "{}", error);
        std::process::exit(1);
    }
}
