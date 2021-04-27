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

use std::path::{PathBuf, Path};

#[derive(Debug)]
pub enum ErrorType {
    IO(std::io::Error),
    StringFromUTF8(std::string::FromUtf8Error),
    StrFromUTF8(std::str::Utf8Error),
    IllegalMagic(crate::package::Magic),
    UnsupportedVersion(u32),
    IllegalTerminator { terminator: u16, offset: u64 },
    EntryNotADir(String),
    NoSuchEntry(String),
    IllegalArgument { name: &'static str, value: String },
    UnexpectedEOF,
    SanityCheckFaild(String),
    Other(String),
}

#[derive(Debug)]
pub struct Error {
    pub(crate) error_type: ErrorType,
    pub(crate) path:       Option<PathBuf>,
}

impl Error {
    #[inline]
    pub fn new(error_type: ErrorType, path: Option<PathBuf>) -> Self {
        Error {
            path,
            error_type,
        }
    }

    #[inline]
    pub fn error_type(&self) -> &ErrorType {
        &self.error_type
    }

    #[inline]
    pub fn path(&self) -> &Option<PathBuf> {
        &self.path
    }

    #[inline]
    pub fn with_path(self, path: impl AsRef<Path>) -> Self {
        Error {
            path:       Some(path.as_ref().to_path_buf()),
            error_type: self.error_type,
        }
    }

    #[inline]
    pub fn io_with_path(error: std::io::Error, path: impl AsRef<Path>) -> Self {
        Error {
            path:       Some(path.as_ref().to_path_buf()),
            error_type: ErrorType::IO(error),
        }
    }

    #[inline]
    pub fn io(error: std::io::Error) -> Self {
        Error {
            path:       None,
            error_type: ErrorType::IO(error),
        }
    }

    #[inline]
    pub fn other(message: impl AsRef<str>) -> Self {
        Error {
            path:       None,
            error_type: ErrorType::Other(message.as_ref().to_owned()),
        }
    }

    #[inline]
    pub fn entry_not_a_dir(path: impl AsRef<str>) -> Self {
        Error {
            path:       None,
            error_type: ErrorType::EntryNotADir(path.as_ref().to_owned()),
        }
    }

    #[inline]
    pub fn no_such_entry(path: impl AsRef<str>) -> Self {
        Error {
            path:       None,
            error_type: ErrorType::NoSuchEntry(path.as_ref().to_owned()),
        }
    }

    #[inline]
    pub fn illegal_magic(magic: crate::package::Magic) -> Self {
        Error {
            path:       None,
            error_type: ErrorType::IllegalMagic(magic),
        }
    }

    #[inline]
    pub fn unsupported_version(version: u32) -> Self {
        Error {
            path:       None,
            error_type: ErrorType::UnsupportedVersion(version),
        }
    }

    #[inline]
    pub fn unexpected_eof() -> Self {
        Error {
            path:       None,
            error_type: ErrorType::UnexpectedEOF,
        }
    }

    #[inline]
    pub fn illegal_terminator(terminator: u16, offset: u64) -> Self {
        Error {
            path:       None,
            error_type: ErrorType::IllegalTerminator {
                terminator,
                offset,
            },
        }
    }

    #[inline]
    pub fn illegal_argument(name: &'static str, value: impl AsRef<str>) -> Self {
        Error {
            path:       None,
            error_type: ErrorType::IllegalArgument {
                name,
                value: value.as_ref().to_owned(),
            },
        }
    }

    #[inline]
    pub fn sanity_check_failed(message: impl AsRef<str>) -> Self {
        Error {
            path:       None,
            error_type: ErrorType::SanityCheckFaild(
                message.as_ref().to_owned(),
            ),
        }
    }
}

impl std::fmt::Display for ErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorType::IO(err)                         => err.fmt(f),
            ErrorType::StringFromUTF8(err)             => err.fmt(f),
            ErrorType::StrFromUTF8(err)                => err.fmt(f),
            ErrorType::IllegalMagic(magic)             => write!(f, "illegal file magic: {:02X} {:02X} {:02X} {:02X}", magic[0], magic[1], magic[2], magic[3]),
            ErrorType::UnsupportedVersion(version)     => write!(f, "version {} is not supported", version),
            ErrorType::IllegalTerminator { terminator, offset } => write!(f, "illegal terminator 0x{:02x} at offset {}", terminator, offset),
            ErrorType::EntryNotADir(path)              => write!(f, "entry is not a directory: {:?}", path),
            ErrorType::NoSuchEntry(path)               => write!(f, "entry not found: {:?}", path),
            ErrorType::IllegalArgument { name, value } => write!(f, "illegal argument for {}: {:?}", name, value),
            ErrorType::UnexpectedEOF                   => write!(f, "unexpected end of file"),
            ErrorType::SanityCheckFaild(msg)           => msg.fmt(f),
            ErrorType::Other(msg)                      => msg.fmt(f),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(path) = &self.path {
            write!(f, "{:?}: {}", path, self.error_type)
        } else {
            self.error_type.fmt(f)
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error {
            error_type: ErrorType::IO(error),
            path: None,
        }
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(error: std::string::FromUtf8Error) -> Self {
        Error {
            error_type: ErrorType::StringFromUTF8(error),
            path: None,
        }
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(error: std::str::Utf8Error) -> Self {
        Error {
            error_type: ErrorType::StrFromUTF8(error),
            path: None,
        }
    }
}

impl From<clap::Error> for crate::result::Error {
    fn from(error: clap::Error) -> Self {
        crate::result::Error::other(error.message)
    }
}

pub type Result<T> = core::result::Result<T, Error>;
