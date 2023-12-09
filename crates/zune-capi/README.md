## zune-capi
C API bindings to the  `zune-image` library


API header can be found in `include/zil.h`

## Including it in your project

### Windows
The DLLs were built using `stable-x86_64-pc-windows-gnu` into your project and you are good to go

We provide ready-made DLLS for the windows platform in the `bins/windows` directory
simply copy the dlls and add `-lzil_c` to your linker to link the libraries, after that, you only need
`*.dll` files to run


### API Details

All api callings are prefixed by `zil_` and all structs are prefixed by `Z` so to read
a file you use `zil_imread`

To deal with the main image struct, those are prefixed with `zil_zimg`, 

Most APIs take a `ZStatus` that is used to indicate whether an operation succeded

E.g. to read image headers to extract width one can use `zil_read_headers` in the following way

```c
#include <zil.h>
#include <stdio.h>

int main() {
    // file containing image data
    const char* file = "/image.png"
    // create a new status struct        
    ZStatus* status = zil_status_new();
    // read image metadata
    ZImageMetadata metadata = zil_read_headers(file,&status);
    // check for error
    if (!zil_status_okay(status)){
        // print and bail
        printf("error: %s",zil_status_message(status));
        // free status
        zil_status_free(&status);
        return -1
    } else{
        printf("Image width: %d",metadata.width);
    }
    // free status
    zil_status_free(&status);
    return 0;
}
```

The status field will tell you if something went wrong and give you additional details of what
that was

## Building

To generate the header file, you need `cbindgen`

First `cd` into the directory

Then Run `cargo install cbindgen` and then run `cbindgenn . -o ./include/zil.h`

### Windows
To build the library on windows, it is recommended to use `stable-x86_64-pc-windows-gnu` and not `stable-x86_64-pc-windows-msvc`
due to linker errors that may occur when trying to link the dll

Rust also automatically links to system libraries to provide internal functionalities, you need to know this libraries

One can use `set RUSTFLAGS=--print native-static-libs` to get information on what static libraries are needed

A full script is

```shell
set RUSTFLAGS=--print native-static-libs
cargo build --release
```

During last linking, rustc will print something like

```text
note: Link against the following native artifacts when linking against this static library. The order and any duplication can be significant on some platforms.                            

note: native-static-libs: -lkernel32 -ladvapi32 -lbcrypt -lkernel32 -lntdll -luserenv -lws2_32 -lkernel32 -lws2_32 -lkernel32
```

You are to add the linker options to the linker you will be using for linking the dll.

### Linux

We follow the same steps from windows but now change the linker arguments

1. `cd ` into the directory
2. Run `RUSTFLAGS="--print native-static-libs" cargo build --release`

The current output (as of December 2023) is 

```text
note: Link against the following native artifacts when linking against this static library. The order and any duplication can be significant on some platforms.

note: native-static-libs: -lgcc_s -lutil -lrt -lpthread -lm -ldl -lc
```

Add those to the linker options for the library you are using

## Running 

### Windows.
Windows requires additional `dlls` provided in the `/dll` directory, make sure the loader can find them 
by pasting them in the same directory as the generated dll

