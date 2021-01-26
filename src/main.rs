use clap::{Arg, App, SubCommand};
use std::io::{self, Write};
use vpk::list::{Filter};
use vpk::sort::{parse_order, DEFAULT_ORDER};

pub mod vpk;

impl From<clap::Error> for vpk::Error {
    fn from(error: clap::Error) -> Self {
        vpk::Error::Other(error.message)
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
            .arg(Arg::with_name("archive").index(1).required(true))
            .arg(Arg::with_name("paths").index(2).multiple(true)))
        .subcommand(SubCommand::with_name("check")
            .alias("c")
            .arg(Arg::with_name("archive").index(1).required(true))
            .arg(Arg::with_name("paths").index(2).multiple(true)))
        .subcommand(SubCommand::with_name("unpack")
            .alias("x")
            .arg(Arg::with_name("output").long("output").short("o"))
            .arg(Arg::with_name("archive").index(1).required(true))
            .arg(Arg::with_name("paths").index(2).multiple(true)))
        .subcommand(SubCommand::with_name("pack")
            .alias("p")
            // TODO: how to distribute files over archives? group them in archive?
            .arg(Arg::with_name("archive").index(1).required(true))
            .arg(Arg::with_name("inline-size").long("inline-size").short("i").takes_value(true))
            .arg(Arg::with_name("paths").index(2).multiple(true).required(true)))
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
            let path = args.value_of("archive").unwrap();
            let filter = if let Some(filter) = args.values_of("paths") {
                let paths: Vec<String> = filter.map(|name| name.to_owned()).collect();
                if paths.is_empty() {
                    Filter::None
                } else {
                    Filter::Paths(paths)
                }
            } else {
                Filter::None
            };

            let archive = vpk::Archive::from_path(&path)?;

            vpk::list(&archive, order, human_readable, &filter)?;
        },
        ("check", Some(args)) => {
            println!("unpack: {:?}", args);
        },
        ("unpack", Some(args)) => {
            println!("unpack: {:?}", args);
        },
        ("pack", Some(args)) => {
            println!("pack: {:?}", args);
        },
        ("", _) => {
            writeln!(io::stderr(), "subcommand required")?;
            writeln!(io::stderr(), "For more information try --help")?;
        },
        (cmd, _) => {
            writeln!(io::stderr(), "unknown subcommand: {}", cmd)?;
            writeln!(io::stderr(), "For more information try --help")?;
        }
    }

    Ok(())
}

fn main() {
    match run() {
        Err(error) => {
            let _ = writeln!(std::io::stderr(), "{}", error);
        },
        Ok(()) => {}
    }
}
