## zune-imageprocs
A library for low level image processing routines

They work on raw pixels (`T`) and they are focused on speed and safety.

## Warning
Some filters are in alpha stage, and some are broken, 
don't use a filter with a `Broken` tag


## Usage
Add the crate to your dependencies e.g `cargo add zune-imageprocs`

After that one can use the processing routines since they implement `zune-image` `OperationsTrait`, anywhere that supports them
can call on them

E.g to increase the exposure of an image
```rust

fn main(){
    let img= 
    
}
```

### Benchmarking 

Most routines in the library can be benchmarked, 
but they require a nightly compiler


To test speed of most routines run

```shell
cargo bench --features=benchmarks
```
