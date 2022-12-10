## Image decoding in WASM

Now with first class support.

And it works, on everyone's machine.

## Compiling

You need to have `rustc`, `cargo` and `wasm-pack` installed to compile

To compile this, follow the following instructions

```shell
wasm-pack build
``` 

will create a new directory called `pkg` which will contain the generated web assembly and
Javascript and typescript bindings.

To get more speed, one can leverage WASM [fixed width simd](https://github.com/webassembly/simd/)
which allows the compiler's autovectorizer to write some functions in SIMD

This isn't supported in Safari.

To get SIMD run

```shell
RUSTFLAGS="-C target-feature=+simd128" wasm-pack build 
```

One can investigate that SIMD is compiled by loading the WASM into the browser, browser logs
will contain details about SIMD(and other extra things)