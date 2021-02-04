Rust-VPK
========

A tool to create, list, check, and unpack VPK files files.

Under Linux this tool can also be used to mount VPK packages as read-only FUSE
filesystem.

Checking and generating of cryptographic signatures is not supported, since
there's no information out there on how to do that.

Similar to [another tool](https://github.com/panzi/unvpk) I wrote, but this time
in Rust instead of C++ (for the fun of it!).

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
* [ ] more stats for v2 packages
* [x] read-only fuse filesystem
* [x] read and evaluate md5 from VPK v2 files
* [ ] read and evaluate signature from VPK v2 files? algorithm used is unknown
* [x] better help message
* [x] code cleanups/refactorings
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
