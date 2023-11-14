# zune-image

This directory contains the crates for libraries distributed within `zune-image`

| crate         | reason                                                             |
|---------------|--------------------------------------------------------------------|
| zune-bin      | Binary for the crate                                               |
| zune-bmp      | BMP decoder                                                        |
| zune-core     | Core routines shared amongst image codecs(io,colorspace info etc)  |
| zune-farbfeld | Farbfeld image decoder and encoder                                 |
| zune-hdr      | HDR image decoder and encoder                                      |
| zune-image    | Main image library, ties together most crates inside here          |
| zune-inflate  | Deflate decoding and encoding                                      |
| zune-jpeg     | JPEG decoding                                                      |
| zune-jpegxl   | JPEG-XL encoding                                                   |
| zune-opencl   | Experimental OpenCL bindings for certain image processing routines |
| zune-png      | PNG decoding and experimental encoding                             |
| zune-ppm      | PPM decoding and encoding , including PFM support                  |
| zune-psd      | Simple Photoshop decoding                                          |
| zune-python   | Python bindings to the zune-image crate                            |
| zune-qoi      | QOI decoding and encoding support                                  |
| zune-wasm     | Experimental Webassembly support                                   |