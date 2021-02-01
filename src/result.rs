#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    IOWithPath(std::io::Error, std::path::PathBuf),
    StringFromUTF8(std::string::FromUtf8Error),
    StrFromUTF8(std::str::Utf8Error),
    IllegalMagic(crate::package::Magic),
    UnsupportedVersion(u32),
    IllegalTerminator { terminator: u16, offset: u64 },
    EntryNotADir(String),
    NoSuchEntry(String),
    IllegalArgument { name: &'static str, value: String },
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
