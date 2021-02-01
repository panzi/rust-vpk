Rust-VPK
========

A tool to list, and unpack VPKv1 and v2 files and to create VPKv1 files. (WIP)

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
* [x] read-only fuse filesystem
* [ ] maybe update command?
* [x] better help message
* [ ] code cleanups/refactorings
* [ ] choose license (probably GPLv3)
