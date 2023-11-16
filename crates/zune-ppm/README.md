## zune-ppm
A Portable Pixel Format (PPM) and Portable FloatMap Format (PFM) decoder
and encoder

This crate contains a decoder and encoder that understands the [ppm specification](https://netpbm.sourceforge.net/doc/ppm.html) and
hence can parse those formats.


| Format | Decoder | Encoder |
|--------|---------|---------|
| P1-P3  | No      | No      |
| P5     | Yes     | Yes     |
| P6     | Yes     | Yes     |
| P7     | Yes     | Yes     |
| [PFM]  | Yes     | No      |

## Usage
A simple decoding looks like
```rust
 use zune_ppm::PPMDecoder;
 use zune_ppm::PPMDecodeErrors;
 use zune_core::result::DecodingResult;

 fn main()->Result<(),PPMDecodeErrors>{
    let mut decoder = PPMDecoder::new(&[]);
    let pix = decoder.decode()?;
    match pix {  
        DecodingResult::U8(_) => {
            // deal with 8 bit images
        }
        DecodingResult::U16(_) => {
            // deal with 16 bit images
        }
        DecodingResult::F32(_) => {
            // deal with 32 bit images (PFM)
        },
        _=>unreachable!()
    };
    Ok(())
 }
```
Note that all routes have to be handled since PPMs come in many flavours. 

## Speed
PPM isn't really a format where speed matters, hence benchmarks have been skipped. Nonetheless 
the library is still as efficient as they come


## Security

The crate is continuously fuzzed in CI to ensure that untrusted input does not cause panics

The library also has `#!forbid[(unsafe_code)]` to help prevent any future unsafe creep.

[PFM]: https://www.pauldebevec.com/Research/HDR/PFM/