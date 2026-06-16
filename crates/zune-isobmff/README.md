# Zune ISO Base Media File Format (ISOBMFF)

Shared ISOMBFF "box" header parsing.

The concept of a Box is originally from Apple Quicktime, and has been adopted by ISO as part of ISO/IEC 14496-12.
That is used in various image formats, including HEIF (HEIC, AVIF), JPEG 2000, and JPEG 360.

A Box can be considered as a Box Header and a Box Payload. The Box Header serves to provide a common way to identify
the Box, and to support parsing of the specific Box Payload contents.

This crate only deals with the Box Header. Payload processing is format specific.

## Box Header

ISO/IEC 14496-12 Section 4.2.1 provides the following syntax description:

```
aligned(8) class BoxHeader (unsigned int(32) boxtype, optional unsigned int(8)[16] extended_type) {
  unsigned int(32) size;
  unsigned int(32) type = boxtype;
  if (size==1) {
    unsigned int(64) largesize;
  } else if (size==0) {
    // box extends to end of file
  }
  if (boxtype=='uuid') {
    unsigned int(8)[16] usertype = extended_type;
  }
}
```

So every box header (and hence box, since the header is at the start) starts off with a four byte big endian
size value. There are three options here:
 - that size is 0, in which case the box extends to the end of the file.
 - that size is 1, in which case the real size is given by the 8 byte `largesize` value later in the header
 - that size is >= 8, in which case it describes the size of box, including the header

The next four bytes are the `boxtype` which ISO describes as `unsigned int(32)` but should really be treated
as `[u8; 4]`, and are almost always ASCII printable characters. Its often called the "FourCC" or "4CC", where
the "CC" part is for "character code". 

There is a special case for `boxtype` of `uuid` (0x75756964), which allows the real box type to be vendor defined by
use of a 16-byte UUID. That is rarely used, although it is supported by this crate.

