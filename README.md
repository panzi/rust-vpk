Rust-VPK
========

A tool to create, list, check, and unpack VPK files files.

Under Linux this tool can also be used to mount VPK packages as read-only FUSE
filesystem.

This is similar to [another tool](https://github.com/panzi/unvpk) I wrote, but
this time in Rust instead of C++ (for the fun of it!).

Limitations
-----------

Checking and generating of cryptographic signatures is not supported, since
there's no information out there on how to do that.

I don't know if the offsets in the archive MD5 sum entries need to be adjusted
for the data embedded in the `_dir.vpk` file, like it has to be for the offsets
of the file entries. Currently I assume they don't. If that is wrong I generate
wrong VPK v2 packages and make mistakes checking VPK v2 packages, but only if
there is data inside the `_dir.vpk` file in the data section (i.e. not inlined
directly in the index). I don't have a game that does that, so I can't check.

Usage
-----

```plain
USAGE:
    vpk [SUBCOMMAND]

FLAGS:
    -h, --help
            Prints help information

    -V, --version
            Prints version information


SUBCOMMANDS:
    check     Check CRC32 and MD5 sums of files in a VPK package.
    help      Prints this message or the help of the given subcommand(s)
    list      List content of a VPK package.
    mount     Mount a VPK package as read-only filesystem.
    pack      Create a VPK package.
    stats     Print some statistics of a VPK package.
    unpack    Extract files from a VPK package.
```

For usage information about a sub-command type `vpk help $SUBCOMMAND`, e.g.
`vpk help list`.

Build
-----

Linux:

```bash
cargo build --release
```

Other operating systems:

```bash
cargo build --no-default-features --release
```

Cross compile to Windows:

```bash
cargo build --target x86_64-pc-windows-gnu --no-default-features --release
```

`--no-default-features` is needed to not try to build FUSE support. I don't
know how to make default features target specific. I think it's not yet
possible.

TODO
----

* [ ] Find out if the archive MD5 sum offsets for the `_dir.vpk` archive need
      to be adjusted like they do for file entries. Currently I assumed they
      don't, but I don't have a game that uses VPK v2 and embeds any data in the
      `_dir.vpk` file (outside the data directly inlined in the index) to
      verify. Don't know how I would find that out.
* [ ] Read and evaluate signature from VPK v2 files? Algorithm used is unknown.
      Don't know how I would find that out.

GPLv3 License
-------------

[rust-vpk](https://github.com/panzi/rust-vpk) is free software: you can
redistribute it and/or modify it under the terms of the GNU General Public
License as published by the Free Software Foundation, either version 3 of the
License, or (at your option) any later version.

rust-vpk is distributed in the hope that it will be useful, but WITHOUT ANY
WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
PARTICULAR PURPOSE.  See the GNU General Public License for more details.

You should have received a copy of the GNU General Public License along with
rust-vpk.  If not, see <https://www.gnu.org/licenses/>.
