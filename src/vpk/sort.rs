use std::cmp::Ordering;
use std::convert::TryFrom;

use crate::vpk::{Result, Error};
use crate::vpk::entry::{File};

#[derive(Debug)]
pub enum SortKey {
    Name,
    Size,
    CRC32,
    ArchiveIndex,
    Offset,
    Index,
    RevName,
    RevSize,
    RevCRC32,
    RevArchiveIndex,
    RevOffset,
    RevIndex,
}

pub type Order = [SortKey];

pub const DEFAULT_ORDER: [SortKey; 1] = [SortKey::Name];

impl TryFrom<&str> for SortKey {
    type Error = Error;

    fn try_from(value: &str) -> Result<SortKey> {
        if value.eq_ignore_ascii_case("name") {
            Ok(SortKey::Name)
        } else if value.eq_ignore_ascii_case("size") {
            Ok(SortKey::Size)
        } else if value.eq_ignore_ascii_case("offset") {
            Ok(SortKey::Offset)
        } else if value.eq_ignore_ascii_case("crc32") {
            Ok(SortKey::CRC32)
        } else if value.eq_ignore_ascii_case("archive") || value.eq_ignore_ascii_case("archive-index") {
            Ok(SortKey::ArchiveIndex)
        } else if value.eq_ignore_ascii_case("index") {
            Ok(SortKey::Index)
        } else if value.eq_ignore_ascii_case("-name") {
            Ok(SortKey::RevName)
        } else if value.eq_ignore_ascii_case("-size") {
            Ok(SortKey::RevSize)
        } else if value.eq_ignore_ascii_case("-offset") {
            Ok(SortKey::RevOffset)
        } else if value.eq_ignore_ascii_case("-crc32") {
            Ok(SortKey::RevCRC32)
        } else if value.eq_ignore_ascii_case("-archive") || value.eq_ignore_ascii_case("-archive-index") {
            Ok(SortKey::RevArchiveIndex)
        } else if value.eq_ignore_ascii_case("-index") {
            Ok(SortKey::RevIndex)
        } else {
            Err(Error::IllegalArgument {
                name: "--sort".to_owned(),
                value: value.to_owned()
            })
        }
    }
}

type Item<'a> = (String, &'a File);

impl SortKey {
    #[inline]
    pub fn to_cmp(&self) -> impl Fn(&Item, &Item) -> Ordering {
        match self {
            SortKey::Name            => |a: &Item, b: &Item| { a.0.cmp(&b.0) },
            SortKey::Size            => |a: &Item, b: &Item| { (a.1.size as usize + a.1.inline_size as usize).cmp(&(b.1.size as usize + b.1.inline_size as usize)) },
            SortKey::CRC32           => |a: &Item, b: &Item| { a.1.crc32.cmp(&b.1.crc32) },
            SortKey::ArchiveIndex    => |a: &Item, b: &Item| { a.1.archive_index.cmp(&b.1.archive_index) },
            SortKey::Offset          => |a: &Item, b: &Item| { a.1.offset.cmp(&b.1.offset) },
            SortKey::Index           => |a: &Item, b: &Item| { a.1.index.cmp(&b.1.index) },

            SortKey::RevName         => |a: &Item, b: &Item| { b.0.cmp(&a.0) },
            SortKey::RevSize         => |a: &Item, b: &Item| { (b.1.size as usize + b.1.inline_size as usize).cmp(&(a.1.size as usize + a.1.inline_size as usize)) },
            SortKey::RevCRC32        => |a: &Item, b: &Item| { b.1.crc32.cmp(&a.1.crc32) },
            SortKey::RevArchiveIndex => |a: &Item, b: &Item| { b.1.archive_index.cmp(&a.1.archive_index) },
            SortKey::RevOffset       => |a: &Item, b: &Item| { b.1.offset.cmp(&a.1.offset) },
            SortKey::RevIndex        => |a: &Item, b: &Item| { b.1.index.cmp(&a.1.index) },
        }
    }
}

fn chain<'a>(cmp1: Box<dyn Fn(&Item, &Item) -> Ordering>, cmp2: Box<dyn Fn(&Item, &Item) -> Ordering>) -> Box<dyn Fn(&Item, &Item) -> Ordering> {
    Box::new(move |a: &Item, b: &Item| {
        match cmp1(a, b) {
            Ordering::Equal => cmp2(a, b),
            ord => ord,
        }
    })
}

fn make_chain(cmp1: Box<dyn Fn(&Item, &Item) -> Ordering>, mut iter: std::slice::Iter<SortKey>) -> Box<dyn Fn(&Item, &Item) -> Ordering> {
    if let Some(key) = iter.next() {
        make_chain(chain(cmp1, Box::new(key.to_cmp())), iter)
    } else {
        cmp1
    }
}

pub fn sort(list: &mut Vec<(String, &File)>, order: &Order) {
    let mut iter = order.iter();

    if let Some(first_key) = iter.next() {
        let cmp = make_chain(Box::new(first_key.to_cmp()), iter);
        list.sort_by(cmp);
    }
}

pub fn parse_order(value: &str) -> Result<Vec<SortKey>> {
    let mut order = Vec::new();
    for key in value.split(',') {
        order.push(SortKey::try_from(key)?);
    }
    Ok(order)
}
