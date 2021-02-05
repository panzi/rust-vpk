VPK File Format Specification
=============================

My guess is that VPK stands for "Valve Package". It is used by games based on
Valves Source engine.

VPK stores files into several archives. The directory information is stored
in a file that ends with "`_dir.vpk`". All other archives related to this
package are located in the same directory and have the same prefix but instead
of "`_dir.vpk`" they end in "`_###.vpk`", where "`###`" is a zero padded number
starting with `000`.

Example listing of a game folder:

```plain
pak01_000.vpk
pak01_001.vpk
pak01_002.vpk
pak01_dir.vpk
```

The Directory File
------------------

### Overview

| Section          | V0  | V1  | V2  |
| ---------------- | :-: | :-: | :-: |
| Header           |     |  X  |  X  |
| Index            |  X  |  X  |  X  |
| Data             |  X  |  X  |  X  |
| Archive MD5 Sums |     |     |  X  |
| Other MD5 Sums   |     |     |  X  |
| Signature        |     |     |  X  |

### Basic Types

Note that all numbers in VPK are stored in *little endian* byte order.

```plain
Bytes  Type    Description
    1  Byte    a raw data byte
    2  U16     unsinged 16 bit integer
    4  U32     unsinged 32 bit integer
   >1  AsciiZ  Zero terminated ASCII string.
               Each character is one byte in size.
```

### Header

Note that the header seems to be optional/was introduced in version 1 of the
format. Check for the file magic. If the file starts with the binary string
0x34 0x12 0xAA 0x55 (or the U32 value 0x55AA1234) it contains a header. An
accidental collision with the older format is improbable, because these values
would be very odd ASCII characters for a pathname.

#### Version 1

```plain
 Offset  Count  Type    Description
 0x0000      1  U32     File magic: 0x55AA1234
 0x0004      1  U32     VPK version: 1
 0x0008      1  U32     Index size.
```

#### Version 2

Not much is known about version 2. Currently only Counter Strike: Global
Offensive is known to use this format.

```plain
 Offset  Count  Type    Description
 0x0000      1  U32     File magic: 0x55AA1234
 0x0004      1  U32     VPK version: 2
 0x0008      1  U32     Index size.
 0x000C      1  U32     Data size. The data inside the `_dir.vpk` file, not
                        the data embedded in the index.
 0x0010      1  U32     Archive MD5 section size.
 0x0014      1  U32     Other MD5 sums section size.
 0x0018      1  U32     Signature section size.
```

### Index

```plain
 Offset  Count  Type    Description
 0x0000      *  Type    A list of file types. Files are grouped by their
                        types. The list is terminated by a 0-byte. This can
                        also be interpreted as a zero-length type name.
```

#### Type

```plain
 Offset  Count  Type    Description
 0x0000      1  AsciiZ  Type name. This is just the file name extension.
                        E.g.: "mdl", "vcd", "vtx" or "wav"
      ?      *  Dir     A list of directories. The list is terminated by
                        a 0-byte. This can also be interpreted as a
                        zero-length directory path.
```

#### Dir

```plain
 Offset  Count  Type    Description
 0x0000      1  AsciiZ  Directory path. Subdirectories are separated
                        with "/". E.g.: "sound/music"
      ?      *  File    A list of files. The list is terminated by a 0-byte.
                        This can also be interpreted as a zero-length
                        directory path.
```

#### File

The first part of a file may be inlined in the directory file. The rest is
stored in the separate archive files referenced by an archive index. In any
case the file is stored as plain consecutive data without any further
encoding or compression.

```plain
 Offset  Count  Type    Description
 0x0000      1  AsciiZ  File name (without extension). E.g.: "ding_on"
      ?      1  U32     CRC32 checksum
+0x0004      1  U16     Inlined file size (IFS).
+0x0006      1  U16     Archive index. This is the "###" part of the
                        archive file names. If the index is 0x7fff (max signed
                        16bit int) then the file is embedded in the directory
                        file instead of a separate archive.
+0x0008      1  U32     Offset within the archive where the file starts.
                        If the file is embedded in the direcory file (index =
                        0x7fff) this is relative to the end of the file index
                        (i.e. (0 or 12) + index_size).
+0x000C      1  U32     File size. A file size of 0 indicates that the file
                        data is inlined in the directory file.
+0x0010      1  U16     Terminator: 0xFFFF
+0x0012    IFS  Byte    The inlined file data of the size defined above.
```

### Archive MD5 Sum

```plain
 Offset  Count  Type    Description
 0x0000      1  U32     Archive index.
 0x0004      1  U32     Offset in archive. NOTE: I don't know if this offset
                        needs to be adjusted for the _dir.vpk file (archive
                        index = 0x7fff) like for the file entries. I assumed
                        no, but that might be wrong.
 0x0008      1  U32     Size of the slice for which the MD5 sum is calculated.
 0x000C     16  Byte    MD5 sum.
```

### Other MD5 Sums

```plain
 Offset  Count  Type    Description
 0x0000     16  Byte    MD5 sum of index section.
 0x0010     16  Byte    MD5 sum of archive MD5 sum section.
 0x0020     16  Byte    MD5 sum of everything in the _dir.vpk file up to this
                        point.
```

### Signature

The signature algorithm and what exactly is signed is unknown.

```plain
 Offset  Count  Type    Description
 0x0000      1  U32     Public key size (PKS).
 0x0004    PKS  Byte    Public key.
      ?      1  U32     Signature size (SS).
+0x0004     SS  Byte    Signature.
```

### Pseudo C structs

All of the above again as pseudo C structs.

```CPP
struct Vpk {
    struct Header      header[0...1];
    struct Index       index;
    uint8_t            data[*];
    struct  ArchiveMd5 archive_md5s[*];
    struct  OtherMd5s  other_md5s[0...1];
    struct  Signature  signature[0...1];
};

struct Header {
    // VPK1:
    uint32_t magic;
    uint32_t version;
    uint32_t index_size;

    // VPK2:
    uint32_t data_size;
    uint32_t archive_md5_size;
    uint32_t other_md5_size;
    uint32_t signature_size;
};

struct Index {
    struct Type types[*];
    uint8_t     nil;
}

struct ArchiveMd5 {
    uint32_t archive_index;
    uint32_t offset;
    uint32_t size;
    uint8_t  md5[16];
}

struct OtherMd5s {
    uint8_t index_md5[16];
    uint8_t archive_md5s_md5[16];
    uint8_t everything_md5[16];
}

struct Signature {
    uint32_t pubkey_size;
    uint8_t  pubkey[pubkey_size];
    uint32_t signature_size;
    uint8_t  signature[pubkey_size];
}

struct Type {
    char       name[*];
    struct Dir dirs[*];
    uint8_t    nil;
};

struct Dir {
    char        path[*];
    struct File files[*];
    uint8_t     nil;
};

struct File {
    char     name[*];
    uint32_t crc32;
    utin16_t inlined_size;
    uint16_t archive_index;
    uint32_t offset;
    uint32_t size;
    uint16_t terminator;
    uint8_t  inlined_data[inlined_size];
};
```

### Extracted File Paths

File paths for archive extraction can then be reconstructed as:

```CPP
pathname + "/" + filename + "." + filetype
```

The archive files seem not to contain any data besides the files.

Oddities
--------

There are sometimes areas in the archive files that are not referenced by any
file. These areas seem to contain valid files (I've seen WAV, VTX and VCD at a
glance). Still, Steam does not report any error for this either.

References
----------

* [VPK File Format - Valve Developer Community](http://developer.valvesoftware.com/wiki/VPK_File_Format)
* [jvpklib](https://github.com/ata4/jvpklib), in particular [this part](https://github.com/ata4/jvpklib/blob/9b2dc1a7727a23b9303bc237ad58452ecf91e9ee/src/info/ata4/vpk/VPKArchive.java#L79)
