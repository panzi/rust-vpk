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

Build
-----

Linux:

    cargo build --release

Other operating systems:

    cargo build --no-default-features --release

Cross compile to Windows:

    cargo build --target x86_64-pc-windows-gnu --no-default-features --release

`--no-default-features` is needed to not try to build FUSE support. I don't
know how to make default features target specific. I think it's not yet
possible.

TODO
----

* [x] list
* [x] check
* [x] unpack
* [x] pack
* [x] stats
* [x] more stats for v2 packages
* [x] read-only fuse filesystem
* [x] read and evaluate md5 from VPK v2 files
* [x] BUG: random missing files when mounting!
* [ ] find out if the archive MD5 sum offsets for the `_dir.vpk` archive need
      to be adjusted like they do for file entries. Currently I assumed they
      don't, but I don't have a game that uses VPK v2 and embeds any data in the
      `_dir.vpk` file (outside the data directly inlined in the index).
* [x] maybe support "version 0"? The version without any header.
* [ ] read and evaluate signature from VPK v2 files? algorithm used is unknown
* [x] better help message
* [x] code cleanups/refactorings
* [x] find out what the last remaining MD5 sum does
* [ ] more code cleanups/refactorings (mainly pack and check)
* [x] choose license (probably GPLv3)

GPLv3 License
-------------

rust-vpk is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

rust-vpk is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with rust-vpk.  If not, see <https://www.gnu.org/licenses/>.
