## zune-imageprocs
A library for low level image processing routines

They work on raw pixels (`T`) and they are focused on speed and safety.

### Benchmarking 

Most routines in the library can be benchmarked, 
but they require a nightly compiler


To test speed of most routines run

```shell
cargo bench --features=benchmarks
```
