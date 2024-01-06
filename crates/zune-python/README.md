## Zune-Python: Python Bindings to zune-image

### Features
- Decoding from multiple image formats antagonistically while preserving image information (no implicit conversions)
- Support for some basic image processing functionalities (transformations, sobel, e.t.c)
- Support for conversion between image modes (RGB->Grayscale)
- Support for image filters (gaussian blur, sharpening)
- Support for image transparency
- Multiple depths and bit types(`f32`,`u16`, `u8`)
- Support for `numpy` arrays (outputting image to numpy,creating an image from numpy array)

### Performance
- The image library is performant with some processes taking advantage of multiple threads (e.g sobel uses multiple threads per channel)
- The routines are written in a safe and perfomant manner with care being given to ensure optimal assembly is generated for performance sensitive functions
- The library contains various benchmarks to compare it with other libraries (opencv, vips, image-rs), and internal benchmarks to keep track of operations.


### Safety
- 
### Building

- To build the library, you need the following

### Prerequisites

- `cargo/rust`: See https://www.rust-lang.org/tools/install for install instructions
- `python/pip`: See https://www.python.org/downloads/ for download and install instructions

# Steps

1. Clone the repo

```shell
git clone https://github.com/etemesi254/zune-image
```

2. cd into the repo and into the zune-python repository

```
shell cd ./zune-image/zune-python
```

3. Create a virtual environment for the repo

```shell
python -m venv .env
source .env/bin/activate 
```

4. Install [maturin](https://github.com/PyO3/maturin)

```shell
pip install maturin
```

5. Call `maturin build --release` This will build the project with optimizations turned

```shell
maturin build --release
```

Wait until you see

```text
📦 Built wheel for CPython 3.11 to ./target/wheels/zune_image-0.4.0-cp311-cp311-manylinux_2_34_x86_64.whl
```

6. Navigate to `{CRATE_DIR}/target/wheels/`
7. Call pip install with the local built package

```shell
 pip install --force-reinstall ./zune_image-0.4.0-cp311-cp311-manylinux_2_34_x86_64.whl

```

8. call `python` or `ipython` to get an interactive shell.
9. Import `zil` from there and decode an image

```python
# Import the package
import zil
IMAGE_FILE = "image.png"
# Returns the image pixels as numpy
numpy_pix = zil.imread(IMAGE_FILE);
# or manipulate the image in Rust
im_rust = zil.Image.open(IMAGE_FILE);
# eg carry out sobel
im_rust.sobel()
```
