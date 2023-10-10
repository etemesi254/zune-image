## Zune-Python: Python Bindings to zune-image

## Warnings

- There is an expensive cost to convert Rust's `Vec<u8>` to a python `List`, this impacts
  decoding speed by a significant portion when using `decode_{format}`(eg `decode_jpeg`,`decode_png`)
  E.g decoding a 7680 by 4320 jpeg image:

```text
In [20]: %timeit zune_python.decode_jpeg(data)
    705 ms ± 5.89 ms per loop (mean ± std. dev. of 7 runs, 1 loop each)
    
In [21]: %timeit zune_python.decode_image(data)
    129 ms ± 2.84 ms per loop (mean ± std. dev. of 7 runs, 1 loop each)
      
```

- The former converts to Python's list while the latter creates a Rust struct that can be called from Python (hence no
  conversions)

### Building

- To build the library, you need the following

### Prerequisites

- `cargo/rust`: See https://www.rust-lang.org/tools/install for install instructions
- `python/pip`: See https://www.python.org/downloads/ for download and install instructions

# Steps

1. Clone the repo `git clone https://github.com/etemesi254/zune-image`
2. cd into the repo and into the zune-python repository `cd ./zune-image/zune-python`
3. Create a virtual environment for the repo

```shell
python -m venv .env
source .env/bin/activate 
```

4. Install [maturin](https://github.com/PyO3/maturin)

```shell
pip install maturin
```

5. Call `maturin develop --release` This will build the project with optimizations turned

```shell
maturin develop
```

Wait until you see

```text
🛠 Installed zune-python-0.1.0
```

6. call `python` or `ipython` to get an interactive shell.
7. Import `zune_python` from there and decode an image

```python
from zune_python import decode_image
data = open("a_file.jpg",mode="rb").read()
image = decode_image(data)
print(image.dimensions())
```