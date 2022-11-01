## zune-imageprocs
A library for low level image processing routines

They work on raw pixels (`u8`) and they are focused on speed and safety.

### Specifications
All huge allocations should be kept away from this
library, the routine is allowed to accept
temporary buffers from the caller

This allows for buffer reuse and smaller
memory usage where possible


E.g for guassian blur
```rust
pub fn gaussian_blur(
    in_out_image: &mut [u8], scratch_space: &mut [u8], width: usize, height: usize, sigma: f32,
)
```
The `scratch_space` is only used for internal gaussian blur shenanigans but because we pass it
, as a mutable reference, we can reuse it for multiple channels

```rust
let mut channels = vec![vec![0;2000];3];

let mut scratch_space = vec![0;2000];

for channel in channels{
    gaussian_blur(&mut channel,&mut scratch_space)
}
```

### Benchmarking 

Most routines in the library can be benchmarked, 
but they require a nightly compiler


To test speed of most routines run

```shell
cargo bench --features=benchmarks
```
