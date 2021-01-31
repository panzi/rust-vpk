pub(crate) mod io;
pub(crate) mod util;

pub mod list;
pub use self::list::list;

pub mod stats;
pub use self::stats::stats;

pub mod sort;
pub use self::sort::sort;

pub mod check;
pub use self::check::check;

pub mod unpack;
pub use self::unpack::unpack;

pub mod pack;
pub use self::pack::pack_v1;

pub mod package;
pub use self::package::Package;

pub mod entry;
pub use self::entry::Entry;

#[cfg(feature = "fuse")]
pub mod mount;

#[cfg(feature = "fuse")]
pub use self::mount::mount;

pub mod archive_cache;

pub type Magic = [u8; 4];

pub const VPK_MAGIC: Magic = [0x34, 0x12, 0xAA, 0x55];

pub const DIR_INDEX:  u16 = 0x7FFF;
pub const TERMINATOR: u16 = 0xFFFF;
pub const BUFFER_SIZE: usize = 8 * 1024;
pub const DEFAULT_MAX_INLINE_SIZE: u16 = 8 * 1024;

#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    IOWithPath(std::io::Error, std::path::PathBuf),
    StringFromUTF8(std::string::FromUtf8Error),
    StrFromUTF8(std::str::Utf8Error),
    IllegalMagic(Magic),
    UnsupportedVersion(u32),
    IllegalTerminator { terminator: u16, offset: u64 },
    EntryNotADir(String),
    NoSuchEntry(String),
    IllegalArgument { name: String, value: String },
    UnexpectedEOF,
    Other(String),
}

impl Error {
    pub fn message(&self) -> String {
        format!("{}", self)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::IO(err) => err.fmt(f),
            Error::IOWithPath(err, path) => write!(f, "{:?}: {}", path, err),
            Error::StringFromUTF8(err) => err.fmt(f),
            Error::StrFromUTF8(err) => err.fmt(f),
            Error::IllegalMagic(magic) => write!(f, "illegal file magic: {:02X} {:02X} {:02X} {:02X}", magic[0], magic[1], magic[2], magic[3]),
            Error::UnsupportedVersion(version) => write!(f, "unsupported varision: {}", version),
            Error::IllegalTerminator { terminator, offset } => write!(f, "illegal terminator 0x{:02x} at offset {}", terminator, offset),
            Error::EntryNotADir(path) => write!(f, "entry is not a directory: {:?}", path),
            Error::NoSuchEntry(path) => write!(f, "entry not found: {:?}", path),
            Error::IllegalArgument { name, value } => write!(f, "illegal argument for {}: {:?}", name, value),
            Error::UnexpectedEOF => write!(f, "unexpected end of file"),
            Error::Other(msg) => msg.fmt(f),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::IO(error)
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(error: std::string::FromUtf8Error) -> Self {
        Error::StringFromUTF8(error)
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(error: std::str::Utf8Error) -> Self {
        Error::StrFromUTF8(error)
    }
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug)]
pub enum Filter {
    None,
    Paths(Vec<String>)
}
